use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "splithello", about = "Local loopback CONNECT proxy")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Start the local proxy.
    Start(StartArgs),
}

#[derive(Debug, Args)]
pub(crate) struct StartArgs {
    /// Path to the TOML configuration file.
    #[arg(long, value_name = "PATH")]
    pub(crate) config: PathBuf,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn start_requires_config_path() {
        let error = Cli::try_parse_from(["splithello", "start"]).unwrap_err();
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn start_accepts_exact_config_shape() {
        let cli = Cli::try_parse_from(["splithello", "start", "--config", "config.toml"])
            .expect("valid CLI");

        let Command::Start(args) = cli.command;
        assert_eq!(args.config, std::path::PathBuf::from("config.toml"));
    }
}
