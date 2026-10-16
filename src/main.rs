mod app;
mod cli;
mod config;
mod control;
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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app::AppError;
use clap::Parser;
use cli::{Cli, Command, ConfigArgs};
use config::{Config, ConfigError};
use lifecycle::CommandStatus;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::init(&cli.log_options());
    let starting = matches!(cli.command, Command::Start(_));
    match run(cli).await {
        Ok(status) => status.into(),
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
    match cli.command {
        Command::Start(args) => lifecycle::start(&args).await,
        Command::Status => lifecycle::status().await,
        Command::Stop(args) => lifecycle::stop(&args).await,
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
