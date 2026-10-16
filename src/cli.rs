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
    /// Replace this binary with the newest release; Homebrew and Cargo installs print their
    /// upgrade command instead.
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    /// Its [dns] settings resolve GitHub; defaults apply when the file is missing or invalid.
    #[command(flatten)]
    pub(crate) config: ConfigArgs,
    /// Only report whether a newer release exists.
    #[arg(long, conflicts_with_all = ["yes", "version"])]
    pub(crate) check: bool,
    /// Install without asking.
    #[arg(short, long)]
    pub(crate) yes: bool,
    /// Install this release instead of the newest, e.g. v0.2.0; it may be older or a prerelease.
    #[arg(long, value_name = "TAG")]
    pub(crate) version: Option<String>,
}

impl UpdateArgs {
    /// The `--version` release tag, with the `v` prefix releases are tagged with.
    pub(crate) fn tag(&self) -> Option<String> {
        self.version.as_deref().map(|version| {
            if version.starts_with('v') {
                version.to_owned()
            } else {
                format!("v{version}")
            }
        })
    }
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

/// `$XDG_CONFIG_HOME/splithello/config.toml`, else `~/.config/splithello/config.toml`.
pub(crate) fn default_config_path() -> Option<PathBuf> {
    xdg_path(
        std::env::var_os("XDG_CONFIG_HOME"),
        home_dir(),
        ".config",
        "config.toml",
    )
}

/// `$XDG_CACHE_HOME/splithello/<file>`, else `~/.cache/splithello/<file>`.
pub(crate) fn default_cache_path(file: &str) -> Option<PathBuf> {
    xdg_path(
        std::env::var_os("XDG_CACHE_HOME"),
        home_dir(),
        ".cache",
        file,
    )
}

/// `HOME` on Unix, `USERPROFILE` on Windows.
fn home_dir() -> Option<OsString> {
    std::env::home_dir().map(PathBuf::into_os_string)
}

/// XDG base-directory rule: a relative or empty `$XDG_*_HOME` is ignored.
fn xdg_path(
    xdg_home: Option<OsString>,
    home: Option<OsString>,
    home_fallback: &str,
    file: &str,
) -> Option<PathBuf> {
    xdg_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(home_fallback))
        })
        .map(|base| base.join("splithello").join(file))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, xdg_path};
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
    fn update_takes_check_yes_and_a_release_tag() {
        Cli::command().debug_assert();

        let cli = Cli::try_parse_from(["splithello", "update", "--version", "0.2.0", "-y"])
            .expect("valid CLI");
        let Command::Update(args) = cli.command else {
            panic!("expected update");
        };
        assert!(args.yes);
        assert_eq!(args.tag().as_deref(), Some("v0.2.0"));

        let cli = Cli::try_parse_from(["splithello", "update", "--version", "v0.1.0-rc.3"])
            .expect("valid CLI");
        let Command::Update(args) = cli.command else {
            panic!("expected update");
        };
        assert_eq!(args.tag().as_deref(), Some("v0.1.0-rc.3"));

        let error = Cli::try_parse_from(["splithello", "update", "--check", "--yes"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        assert!(Cli::try_parse_from(["splithello", "--version"]).is_err());
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
        // Absolute on every OS; a literal "/xdg" is not absolute on Windows.
        let root = std::env::temp_dir();
        let (xdg, home) = (root.join("xdg"), root.join("home"));
        let os = |path: &PathBuf| Some(path.clone().into_os_string());
        let config = |xdg, home| xdg_path(xdg, home, ".config", "config.toml");
        assert_eq!(
            config(os(&xdg), os(&home)),
            Some(xdg.join("splithello").join("config.toml"))
        );
        assert_eq!(
            config(Some(OsString::from("relative")), os(&home)),
            Some(home.join(".config").join("splithello").join("config.toml"))
        );
        assert_eq!(
            config(None, os(&home)),
            Some(home.join(".config").join("splithello").join("config.toml"))
        );
        assert_eq!(config(None, Some(OsString::from("relative"))), None);
        assert_eq!(config(None, None), None);
        assert_eq!(
            xdg_path(None, os(&home), ".cache", "update-check.toml"),
            Some(
                home.join(".cache")
                    .join("splithello")
                    .join("update-check.toml")
            )
        );
    }
}
