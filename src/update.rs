//! Release lookups against GitHub: the daily "update available" notice and `splithello update`.
//!
//! Requests resolve GitHub's hosts over the configured DoH upstream, like target traffic, so
//! a check adds no plain-DNS lookup. Prereleases are never offered; `--version` installs one.

use std::ffi::OsString;
use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use self_update::backends::github;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::app::AppError;
use crate::cli::UpdateArgs;
use crate::config::{Config, DnsConfig};
use crate::dns::{DohReqwestResolver, DohResolver};

pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Set to anything but empty or `0` to turn the automatic check off.
pub(crate) const OPT_OUT_ENV: &str = "SPLITHELLO_NO_UPDATE_CHECK";
pub(crate) const CACHE_FILE: &str = "update-check.toml";
const GITHUB_API: &str = "https://api.github.com";
const REPO_OWNER: &str = "therealhieu";
const REPO_NAME: &str = "splithello";
const BIN_NAME: &str = "splithello";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Lets the ready banner and the first tunnels go first.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(5);
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub(crate) enum UpdateError {
    #[error("could not build the update HTTP client")]
    Client(#[source] reqwest::Error),
    #[error("release lookup timed out after {}", humantime::format_duration(*.0))]
    Timeout(Duration),
    #[error("no stable release is published yet")]
    NoRelease,
    #[error(transparent)]
    Release(#[from] self_update::errors::Error),
}

/// Whether `start` and `check` should look for a newer release.
pub(crate) fn check_enabled(config_check: bool) -> bool {
    config_check && !opted_out(std::env::var_os(OPT_OUT_ENV))
}

fn opted_out(value: Option<OsString>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

/// Whether `latest` is a newer version than the running binary.
pub(crate) fn is_newer(latest: &str) -> bool {
    self_update::version::bump_is_greater(CURRENT_VERSION, latest).unwrap_or(false)
}

/// How the running binary was installed, which decides how it should be upgraded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallMethod {
    Homebrew,
    Cargo,
    /// A release archive, or anything else `splithello update` may replace in place.
    Archive,
}

impl InstallMethod {
    pub(crate) fn current() -> Self {
        let Ok(exe) = std::env::current_exe().and_then(std::fs::canonicalize) else {
            return Self::Archive;
        };
        let cargo_bin = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".cargo")))
            .map(|cargo_home| cargo_home.join("bin"))
            .map(|bin| std::fs::canonicalize(&bin).unwrap_or(bin));
        Self::detect(&exe, cargo_bin.as_deref())
    }

    fn detect(exe: &Path, cargo_bin: Option<&Path>) -> Self {
        if exe.components().any(|part| part.as_os_str() == "Cellar") {
            Self::Homebrew
        } else if cargo_bin.is_some_and(|bin| exe.parent() == Some(bin)) {
            Self::Cargo
        } else {
            Self::Archive
        }
    }

    pub(crate) const fn upgrade_command(self) -> &'static str {
        match self {
            Self::Homebrew => "brew upgrade splithello",
            Self::Cargo => "cargo install splithello --locked",
            Self::Archive => "splithello update",
        }
    }
}

/// Where releases come from: GitHub's API, reached through DoH.
#[derive(Clone)]
pub(crate) struct ReleaseSource {
    client: reqwest::Client,
    api_base: String,
}

impl ReleaseSource {
    pub(crate) fn github(resolver: Arc<DohResolver>) -> Result<Self, UpdateError> {
        // No total timeout: a download may take longer than a lookup; lookups get their own.
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .https_only(true)
            .no_proxy()
            .dns_resolver(DohReqwestResolver::new(resolver))
            .user_agent(concat!("splithello/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(UpdateError::Client)?;
        Ok(Self {
            client,
            api_base: GITHUB_API.to_owned(),
        })
    }

    /// A plain-HTTP source for a local fake of GitHub's API.
    #[cfg(test)]
    fn local(api_base: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("test client"),
            api_base,
        }
    }

    fn updater(&self) -> github::UpdateBuilder {
        let mut builder = github::Update::configure();
        builder
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .current_version(CURRENT_VERSION)
            .api_base_url(self.api_base.clone())
            .reqwest_async_client(self.client.clone())
            .show_output(false)
            .show_download_progress(false)
            .no_confirm(true);
        builder
    }

    /// The newest stable release's version, without a leading `v`.
    pub(crate) async fn latest_version(&self) -> Result<String, UpdateError> {
        let updater = self.updater().build_async()?;
        let releases = tokio::time::timeout(CHECK_TIMEOUT, updater.get_latest_release_async())
            .await
            .map_err(|_| UpdateError::Timeout(CHECK_TIMEOUT))?
            // GitHub answers 404 while only prereleases exist.
            .map_err(|error| match error {
                self_update::errors::Error::NotFound { .. } => UpdateError::NoRelease,
                error => UpdateError::Release(error),
            })?;
        let release = releases.latest().ok_or(UpdateError::NoRelease)?;
        Ok(release.version().to_owned())
    }

    /// Installs the newest stable release, or exactly `tag` when given (even an older one).
    /// Returns the installed version, or `None` when already up to date.
    pub(crate) async fn install(
        &self,
        tag: Option<&str>,
        install_path: Option<&Path>,
    ) -> Result<Option<String>, UpdateError> {
        let mut builder = self.updater();
        if let Some(tag) = tag {
            builder.release_tag(tag);
        }
        if let Some(path) = install_path {
            builder.bin_install_path(path);
        }
        builder.verify_binary(verify_new_binary);
        let status = builder.build_async()?.update_extended_async().await?;
        Ok(status.version().map(str::to_owned))
    }
}

/// Runs the downloaded binary once before it replaces the installed one.
fn verify_new_binary(new_exe: &Path) -> self_update::errors::Result<()> {
    let output = std::process::Command::new(new_exe)
        .arg("--version")
        .output()
        .map_err(|error| {
            self_update::errors::Error::verification_rejected(format!(
                "could not run the downloaded binary: {error}"
            ))
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && stdout.starts_with("splithello ") {
        Ok(())
    } else {
        Err(self_update::errors::Error::verification_rejected(format!(
            "the downloaded binary printed {:?} for --version",
            stdout.trim()
        )))
    }
}

/// The last lookup, so a restart within a day neither repeats the request nor forgets
/// that a newer release exists.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CacheEntry {
    /// Unix seconds.
    checked_at: u64,
    latest: String,
}

impl CacheEntry {
    fn is_fresh(&self, now: u64) -> bool {
        // A timestamp from the future (clock change) counts as stale.
        now.checked_sub(self.checked_at)
            .is_some_and(|age| age < CHECK_INTERVAL.as_secs())
    }
}

fn read_cache(path: &Path) -> Option<CacheEntry> {
    toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Writes through a temporary file, so a concurrent reader never sees half a file.
fn write_cache(path: &Path, entry: &CacheEntry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string(entry).map_err(std::io::Error::other)?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The newest release version: from `cache` while it is under a day old, else from GitHub
/// (then cached). A failed lookup falls back to a stale cached answer.
pub(crate) async fn latest_version_cached(
    source: &ReleaseSource,
    cache: Option<&Path>,
) -> Result<String, UpdateError> {
    let cached = cache.and_then(read_cache);
    if let Some(entry) = &cached
        && entry.is_fresh(unix_now())
    {
        return Ok(entry.latest.clone());
    }
    match source.latest_version().await {
        Ok(latest) => {
            if let Some(path) = cache {
                let entry = CacheEntry {
                    checked_at: unix_now(),
                    latest: latest.clone(),
                };
                if let Err(error) = write_cache(path, &entry) {
                    tracing::debug!(path = %path.display(), error = %error, "update cache not written");
                }
            }
            Ok(latest)
        }
        Err(error) => cached.map(|entry| entry.latest).ok_or(error),
    }
}

/// Logs once per newer release, a few seconds after startup and then daily.
pub(crate) async fn notify(
    source: ReleaseSource,
    cache: Option<PathBuf>,
    cancel: CancellationToken,
) {
    let mut wait = FIRST_CHECK_DELAY;
    let mut announced: Option<String> = None;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
        }
        wait = CHECK_INTERVAL;
        let lookup = tokio::select! {
            _ = cancel.cancelled() => return,
            lookup = latest_version_cached(&source, cache.as_deref()) => lookup,
        };
        match lookup {
            Ok(latest) if is_newer(&latest) && announced.as_ref() != Some(&latest) => {
                announce(&latest, InstallMethod::current());
                announced = Some(latest);
            }
            Ok(_) => {}
            Err(error) => tracing::debug!(error = %error, "update check failed"),
        }
    }
}

/// `splithello update`. Returns `Ok(false)` when the user declines the install.
pub(crate) async fn run_command(args: &UpdateArgs) -> Result<bool, AppError> {
    // Resolve GitHub with the user's DoH settings when there are usable ones.
    let config = args.config.path().and_then(|path| Config::load(&path).ok());
    let dns = config.as_ref().map(Config::dns);
    let resolver = match dns {
        Some(dns) => DohResolver::new(dns),
        None => DohResolver::new(&DnsConfig::default()),
    }
    .map_err(AppError::Resolver)?;
    let source = ReleaseSource::github(Arc::new(resolver))?;
    let method = InstallMethod::current();

    let tag = args.tag();
    let target = match &tag {
        Some(tag) => tag.trim_start_matches('v').to_owned(),
        None => {
            let latest = match source.latest_version().await {
                Err(UpdateError::NoRelease) => {
                    println!(
                        "no stable release is published yet; `--version <TAG>` installs a prerelease"
                    );
                    return Ok(true);
                }
                latest => latest?,
            };
            if !is_newer(&latest) {
                println!("splithello {CURRENT_VERSION} is up to date");
                return Ok(true);
            }
            latest
        }
    };
    if args.check {
        println!(
            "splithello {target} is available (you have {CURRENT_VERSION}) → {}",
            method.upgrade_command()
        );
        return Ok(true);
    }
    if method != InstallMethod::Archive {
        println!(
            "splithello {target} is available; this copy is managed by {}, so run `{}`",
            match method {
                InstallMethod::Homebrew => "Homebrew",
                _ => "Cargo",
            },
            method.upgrade_command()
        );
        return Ok(true);
    }
    if !args.yes && std::io::stdin().is_terminal() && !confirm(&target)? {
        println!("not updated");
        return Ok(false);
    }

    println!("downloading splithello {target} …");
    match source.install(tag.as_deref(), None).await? {
        Some(installed) => println!(
            "updated {CURRENT_VERSION} → {installed}; restart `splithello start` to use it"
        ),
        None => println!("splithello {CURRENT_VERSION} is up to date"),
    }
    Ok(true)
}

fn confirm(version: &str) -> Result<bool, AppError> {
    let exe = std::env::current_exe().map_err(AppError::Prompt)?;
    print!(
        "replace {} ({CURRENT_VERSION}) with splithello {version}? [y/N] ",
        exe.display()
    );
    std::io::stdout().flush().map_err(AppError::Prompt)?;
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(AppError::Prompt)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

fn announce(latest: &str, method: InstallMethod) {
    tracing::info!(
        latest_version = latest,
        current_version = CURRENT_VERSION,
        hint = method.upgrade_command(),
        "update available"
    );
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{
        CHECK_INTERVAL, CacheEntry, InstallMethod, is_newer, opted_out, read_cache, write_cache,
    };

    #[test]
    fn opt_out_needs_a_non_empty_non_zero_value() {
        assert!(!opted_out(None));
        assert!(!opted_out(Some(OsString::new())));
        assert!(!opted_out(Some(OsString::from("0"))));
        assert!(opted_out(Some(OsString::from("1"))));
        assert!(opted_out(Some(OsString::from("true"))));
    }

    #[test]
    fn only_strictly_newer_versions_count() {
        assert!(is_newer("999.0.0"));
        assert!(!is_newer(super::CURRENT_VERSION));
        assert!(!is_newer("0.0.1"));
        assert!(!is_newer("not a version"));
    }

    #[test]
    fn install_method_follows_the_binary_location() {
        let cargo_bin = Path::new("/home/me/.cargo/bin");
        let detect = |exe: &str| InstallMethod::detect(Path::new(exe), Some(cargo_bin));
        assert_eq!(
            detect("/opt/homebrew/Cellar/splithello/0.1.0/bin/splithello"),
            InstallMethod::Homebrew
        );
        assert_eq!(
            detect("/home/linuxbrew/.linuxbrew/Cellar/splithello/0.1.0/bin/splithello"),
            InstallMethod::Homebrew
        );
        assert_eq!(
            detect("/home/me/.cargo/bin/splithello"),
            InstallMethod::Cargo
        );
        assert_eq!(detect("/usr/local/bin/splithello"), InstallMethod::Archive);
        assert_eq!(
            InstallMethod::detect(Path::new("/home/me/.cargo/bin/splithello"), None),
            InstallMethod::Archive
        );
        assert_eq!(
            InstallMethod::Homebrew.upgrade_command(),
            "brew upgrade splithello"
        );
    }

    #[test]
    fn cache_is_fresh_for_a_day_and_survives_a_round_trip() {
        let day = CHECK_INTERVAL.as_secs();
        let entry = CacheEntry {
            checked_at: 1_000_000,
            latest: "0.2.0".to_owned(),
        };
        assert!(entry.is_fresh(1_000_000));
        assert!(entry.is_fresh(1_000_000 + day - 1));
        assert!(!entry.is_fresh(1_000_000 + day));
        assert!(!entry.is_fresh(999_999), "a future timestamp is stale");

        let directory = tempdir().unwrap();
        let path = directory.path().join("nested").join("update-check.toml");
        assert_eq!(read_cache(&path), None, "a missing cache reads as none");
        write_cache(&path, &entry).unwrap();
        assert_eq!(read_cache(&path), Some(entry));

        std::fs::write(&path, "checked_at = \"yesterday\"").unwrap();
        assert_eq!(read_cache(&path), None, "a corrupt cache reads as none");
    }

    /// Serves GitHub's release endpoints for one fake `v9.9.9` release whose archive holds a
    /// script standing in for the binary, or, given an empty archive, a repository with no
    /// stable release (every request is 404). Returns the API base URL.
    #[cfg(unix)]
    async fn fake_github(archive: Vec<u8>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let asset = format!("splithello-v9.9.9-{}.tar.gz", self_update::get_target());
        let release = format!(
            r#"{{"tag_name":"v9.9.9","name":"v9.9.9","created_at":"2026-01-01T00:00:00Z",
                "assets":[{{"name":"{asset}","url":"{base}/asset/1"}}]}}"#
        );
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let (status, content_type, body) = match path {
                    _ if archive.is_empty() => {
                        ("404 Not Found", "application/json", b"{}".to_vec())
                    }
                    "/repos/therealhieu/splithello/releases/latest"
                    | "/repos/therealhieu/splithello/releases/tags/v9.9.9" => {
                        ("200 OK", "application/json", release.clone().into_bytes())
                    }
                    "/asset/1" => ("200 OK", "application/octet-stream", archive.clone()),
                    _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
            }
        });
        base
    }

    /// A release archive whose `splithello` is a script that prints `splithello 9.9.9`.
    #[cfg(unix)]
    fn fake_archive(directory: &Path) -> Vec<u8> {
        use std::os::unix::fs::PermissionsExt;

        let stage = directory.join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        let script = stage.join("splithello");
        std::fs::write(&script, "#!/bin/sh\necho 'splithello 9.9.9'\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let archive = directory.join("release.tar.gz");
        let status = std::process::Command::new("tar")
            .arg("-C")
            .arg(&stage)
            .arg("-czf")
            .arg(&archive)
            .arg("splithello")
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::read(archive).unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn looks_up_caches_and_installs_a_release() {
        let directory = tempdir().unwrap();
        let source = super::ReleaseSource::local(fake_github(fake_archive(directory.path())).await);

        assert_eq!(source.latest_version().await.unwrap(), "9.9.9");

        let cache = directory.path().join("cache").join("update-check.toml");
        assert_eq!(
            super::latest_version_cached(&source, Some(&cache))
                .await
                .unwrap(),
            "9.9.9"
        );
        assert_eq!(read_cache(&cache).unwrap().latest, "9.9.9");
        // A fresh cache answers without the network.
        let offline = super::ReleaseSource::local("http://127.0.0.1:9".to_owned());
        assert_eq!(
            super::latest_version_cached(&offline, Some(&cache))
                .await
                .unwrap(),
            "9.9.9"
        );

        let installed = directory.path().join("bin").join("splithello");
        std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
        std::fs::write(&installed, "old").unwrap();
        let version = source
            .install(Some("v9.9.9"), Some(&installed))
            .await
            .unwrap();
        assert_eq!(version.as_deref(), Some("9.9.9"));
        let output = std::process::Command::new(&installed)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "splithello 9.9.9\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_repository_without_a_stable_release_has_no_latest_version() {
        let source = super::ReleaseSource::local(fake_github(Vec::new()).await);
        assert!(matches!(
            source.latest_version().await,
            Err(super::UpdateError::NoRelease)
        ));
    }
}
