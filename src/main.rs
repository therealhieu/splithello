mod app;
mod cli;
mod config;
mod control;
mod daemon;
mod diagnostics;
mod dns;
mod domain;
mod instance;
mod lifecycle;
mod logging;
mod pac;
mod proxy;
mod setup;
mod tls;
mod update;

use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app::AppError;
use clap::Parser;
use cli::{Cli, Command, ConfigArgs};
use config::{Config, ConfigError};
use lifecycle::CommandStatus;
use logging::LogTarget;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    // A log file is opened by `start` itself, once it holds the instance lock.
    let logs_to_file =
        matches!(&cli.command, Command::Start(args) if args.log_file.is_some() && !args.detach);
    if !logs_to_file {
        logging::init(&cli.log_options(), LogTarget::Stdout);
    }
    let starting = matches!(cli.command, Command::Start(_));
    let update_notice = !cli.quiet && ends_with_update_notice(&cli.command);
    match run(cli).await {
        Ok(status) => {
            if update_notice {
                print_update_notice();
            }
            status.into()
        }
        Err(error) if !tracing::dispatcher::has_been_set() => {
            // Failed before its log file was open: stderr is all there is (for `start -d`,
            // the parent shows it).
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "error: {}", diagnostics::error_chain(&error));
            if let Some(hint) = error.hint() {
                let _ = writeln!(stderr, "  → {hint}");
            }
            CommandStatus::Failed.into()
        }
        Err(error) => {
            if starting {
                tracing::error!(
                    outcome = %error.outcome(),
                    error = %error,
                    hint = error.hint(),
                    "SplitHello stopped"
                );
            } else {
                tracing::error!(
                    outcome = %error.outcome(),
                    error = %error,
                    hint = error.hint(),
                    "command failed"
                );
            }
            CommandStatus::Failed.into()
        }
    }
}

/// `Failed` when a command finished but reported failures (`check`) or stopped before doing
/// anything because no targets are configured.
async fn run(cli: Cli) -> Result<CommandStatus, AppError> {
    let options = cli.log_options();
    match cli.command {
        Command::Start(args) => lifecycle::start(&args, &options).await,
        Command::Status => lifecycle::status().await,
        Command::Stop(args) => lifecycle::stop(&args).await,
        Command::Restart(args) => lifecycle::restart(&args, &options).await,
        Command::Logs(args) => daemon::logs(&args).await,
        Command::Check(args) => {
            let path = config_path(&args)?;
            let Some(config) = load_config(&path)? else {
                return Ok(CommandStatus::Failed);
            };
            setup::check(&path, &config).await.map(CommandStatus::from)
        }
        Command::Update(args) => update::run_command(&args).await.map(CommandStatus::from),
    }
}

/// Short commands that do not report releases themselves; `status`, `check`, `update`, and a
/// running `start`'s log already do.
fn ends_with_update_notice(command: &Command) -> bool {
    match command {
        Command::Stop(_) | Command::Restart(_) => true,
        Command::Start(args) => args.detach,
        Command::Status | Command::Logs(_) | Command::Check(_) | Command::Update(_) => false,
    }
}

/// On stderr and only on a terminal, so scripts and pipes never see it.
fn print_update_notice() {
    let mut stderr = std::io::stderr();
    if !stderr.is_terminal() {
        return;
    }
    if let Some(latest) = update::pending_update() {
        let _ = writeln!(
            stderr,
            "\n{}",
            update::notice(&latest, update::InstallMethod::current())
        );
    }
}

fn config_path(args: &ConfigArgs) -> Result<PathBuf, AppError> {
    args.path().ok_or(AppError::NoConfigPath)
}

/// Writes the starter file if `path` is missing, then loads it. Returns `None` after a
/// warning when no targets are configured, which is the starter file's state.
fn load_config(path: &Path) -> Result<Option<Config>, AppError> {
    if setup::ensure_config(path)? {
        tracing::info!("created starter configuration {}", path.display());
    }
    match Config::load(path) {
        Ok(config) => Ok(Some(config)),
        Err(ConfigError::EmptyTargets) => {
            tracing::warn!(
                "no targets in {}; add sites to `targets`, for example \
                 targets = [\"reddit.com\", \"medium.com\"], then run this command again",
                path.display()
            );
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
