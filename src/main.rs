mod app;
mod cli;
mod config;
mod diagnostics;
mod dns;
mod domain;
mod pac;
mod proxy;
mod tls;

use app::{AppError, Runtime};
use clap::Parser;
use cli::{Cli, Command};
use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(error) = run().await {
        tracing::error!(outcome = %error.outcome(), error = %error, "SplitHello stopped");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start(args) => {
            let config = Config::load(&args.config)?;
            let mut runtime = Runtime::start(config).await?;
            runtime.ready().emit();
            let run_result = runtime.run_until_signal().await;
            let shutdown_result = runtime.shutdown().await;
            run_result.and(shutdown_result)
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
