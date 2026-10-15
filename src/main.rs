mod app;
mod cli;
mod config;
mod diagnostics;
mod dns;
mod domain;
mod pac;
mod proxy;
mod setup;
mod tls;

use std::path::PathBuf;

use app::{AppError, Runtime};
use clap::Parser;
use cli::{Cli, Command, ConfigArgs};
use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();
    match run().await {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            tracing::error!(outcome = %error.outcome(), error = %error, "SplitHello stopped");
            std::process::exit(1);
        }
    }
}

/// Returns `Ok(false)` when a command finished but reported failures (`check`).
async fn run() -> Result<bool, AppError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start(args) => {
            let config = Config::load(&config_path(&args)?)?;
            let mut runtime = Runtime::start(config).await?;
            runtime.ready().emit();
            let run_result = runtime.run_until_signal().await;
            let shutdown_result = runtime.shutdown().await;
            run_result.and(shutdown_result).map(|()| true)
        }
        Command::Init(args) => setup::init(&config_path(&args.config)?, args.force).map(|()| true),
        Command::Check(args) => {
            let path = config_path(&args)?;
            let config = Config::load(&path)?;
            setup::check(&path, &config).await
        }
    }
}

fn config_path(args: &ConfigArgs) -> Result<PathBuf, AppError> {
    args.path().ok_or(AppError::NoConfigPath)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
