mod app;
mod cli;
mod config;
mod diagnostics;
mod dns;
mod domain;
mod logging;
mod pac;
mod proxy;
mod setup;
mod tls;
mod update;

use std::path::{Path, PathBuf};

use app::{AppError, Runtime};
use clap::Parser;
use cli::{Cli, Command, ConfigArgs};
use config::{Config, ConfigError};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    logging::init(&cli.log_options());
    match run(cli).await {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            tracing::error!(
                outcome = %error.outcome(),
                error = %error,
                hint = error.hint(),
                "SplitHello stopped"
            );
            std::process::exit(1);
        }
    }
}

/// Returns `Ok(false)` when a command finished but reported failures (`check`) or stopped
/// before doing anything because no targets are configured.
async fn run(cli: Cli) -> Result<bool, AppError> {
    match cli.command {
        Command::Start(args) => {
            let Some(config) = load_config(&config_path(&args)?)? else {
                return Ok(false);
            };
            let check_updates = update::check_enabled(config.update().check());
            let mut runtime = Runtime::start(config).await?;
            runtime.ready().emit();
            if check_updates {
                runtime.spawn_update_notifier();
            }
            let run_result = runtime.run_until_signal().await;
            let shutdown_result = runtime.shutdown().await;
            run_result.and(shutdown_result).map(|()| true)
        }
        Command::Check(args) => {
            let path = config_path(&args)?;
            let Some(config) = load_config(&path)? else {
                return Ok(false);
            };
            setup::check(&path, &config).await
        }
        Command::Update(args) => update::run_command(&args).await,
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
