use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

use crate::logging::{LogFormat, LogOptions};

#[derive(Debug, Parser)]
#[command(
    name = "splithello",
    version,
    about = "Local loopback CONNECT proxy that splits the TLS ClientHello for selected sites"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
    /// Show more: -v adds every direct tunnel, -vv adds trace output.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub(crate) verbose: u8,
    /// Show only warnings and errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub(crate) quiet: bool,
    /// Log format; `auto` is human on a terminal and logfmt otherwise. RUST_LOG overrides levels.
    #[arg(
        long,
        value_enum,
        default_value_t,
        global = true,
        value_name = "FORMAT"
    )]
    pub(crate) log_format: LogFormat,
}

impl Cli {
    pub(crate) fn log_options(&self) -> LogOptions {
        LogOptions {
            format: self.log_format,
            verbose: self.verbose,
            quiet: self.quiet,
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Start the local proxy; writes a starter configuration first if none exists.
    Start(ConfigArgs),
    /// Validate the configuration, check listeners, and resolve every target over DoH.
    Check(ConfigArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ConfigArgs {
    /// Configuration file [default: $XDG_CONFIG_HOME/splithello/config.toml,
    /// else ~/.config/splithello/config.toml]
    #[arg(long, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,
}

impl ConfigArgs {
    /// The explicit `--config` path, else the per-user default.
    pub(crate) fn path(&self) -> Option<PathBuf> {
        self.config.clone().or_else(default_config_path)
    }
}

pub(crate) fn default_config_path() -> Option<PathBuf> {
    config_path_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

/// XDG base-directory rule: a relative or empty `XDG_CONFIG_HOME` is ignored.
fn config_path_from(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    xdg_config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".config"))
        })
        .map(|base| base.join("splithello").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use clap::Parser;

    use super::{Cli, Command, config_path_from};
    use crate::logging::LogFormat;

    #[test]
    fn start_config_is_optional() {
        let cli = Cli::try_parse_from(["splithello", "start"]).expect("valid CLI");
        let Command::Start(args) = cli.command else {
            panic!("expected start");
        };
        assert_eq!(args.config, None);
    }

    #[test]
    fn start_accepts_explicit_config() {
        let cli = Cli::try_parse_from(["splithello", "start", "--config", "config.toml"])
            .expect("valid CLI");
        let Command::Start(args) = cli.command else {
            panic!("expected start");
        };
        assert_eq!(args.path(), Some(PathBuf::from("config.toml")));
    }

    #[test]
    fn check_shares_the_config_flag_and_init_is_gone() {
        let cli =
            Cli::try_parse_from(["splithello", "check", "--config", "a.toml"]).expect("valid CLI");
        let Command::Check(args) = cli.command else {
            panic!("expected check");
        };
        assert_eq!(args.config, Some(PathBuf::from("a.toml")));

        let error = Cli::try_parse_from(["splithello", "init"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn log_flags_are_global_and_exclusive() {
        let cli = Cli::try_parse_from(["splithello", "start", "-vv", "--log-format", "logfmt"])
            .expect("valid CLI");
        assert_eq!(cli.verbose, 2);
        assert_eq!(cli.log_format, LogFormat::Logfmt);

        let cli = Cli::try_parse_from(["splithello", "-q", "start"]).expect("valid CLI");
        assert!(cli.quiet);
        assert_eq!(cli.log_format, LogFormat::Auto);

        let error = Cli::try_parse_from(["splithello", "start", "-q", "-v"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn default_path_follows_xdg_then_home() {
        let os = |value: &str| Some(OsString::from(value));
        assert_eq!(
            config_path_from(os("/xdg"), os("/home/me")),
            Some(PathBuf::from("/xdg/splithello/config.toml"))
        );
        assert_eq!(
            config_path_from(os("relative"), os("/home/me")),
            Some(PathBuf::from("/home/me/.config/splithello/config.toml"))
        );
        assert_eq!(
            config_path_from(None, os("/home/me")),
            Some(PathBuf::from("/home/me/.config/splithello/config.toml"))
        );
        assert_eq!(config_path_from(None, None), None);
    }
}
