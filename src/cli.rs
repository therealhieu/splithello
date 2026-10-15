use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "splithello",
    version,
    about = "Local loopback CONNECT proxy that splits the TLS ClientHello for selected sites"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Start the local proxy.
    Start(ConfigArgs),
    /// Write a starter configuration file.
    Init(InitArgs),
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

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    #[command(flatten)]
    pub(crate) config: ConfigArgs,
    /// Overwrite an existing configuration file.
    #[arg(long)]
    pub(crate) force: bool,
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
    fn init_and_check_share_the_config_flag() {
        let cli = Cli::try_parse_from(["splithello", "init", "--config", "a.toml", "--force"])
            .expect("valid CLI");
        let Command::Init(args) = cli.command else {
            panic!("expected init");
        };
        assert!(args.force);
        assert_eq!(args.config.config, Some(PathBuf::from("a.toml")));

        let cli = Cli::try_parse_from(["splithello", "check"]).expect("valid CLI");
        assert!(matches!(cli.command, Command::Check(_)));
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
