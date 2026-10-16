//! Starter-config creation and `splithello check`: one-shot steps that never start services.

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::app::AppError;
use crate::cli::default_cache_path;
use crate::config::Config;
use crate::dns::{DohResolver, TargetResolver};
use crate::instance::{self, Liveness, RunRecord, StatePaths};
use crate::update::{self, InstallMethod, ReleaseSource};

/// The repository example doubles as the starter file, so the two cannot drift.
const TEMPLATE: &str = include_str!("../config.example.toml");

/// Writes the starter configuration to `path` unless a file is already there, and returns
/// whether it did. `create_new` makes the existence check and the write one step, so an
/// existing file is never overwritten.
pub(crate) fn ensure_config(path: &Path) -> Result<bool, AppError> {
    let write = |path: &Path| -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?
            .write_all(TEMPLATE.as_bytes())
    };
    match write(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(AppError::ConfigWrite {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Prints the effective configuration and probes what `start` depends on.
/// Returns whether every probe passed.
pub(crate) async fn check(path: &Path, config: &Config) -> Result<bool, AppError> {
    let mut failures = 0_usize;
    println!("config   {}", path.display());

    // Names the owner of a busy port when it is SplitHello itself.
    let running = StatePaths::from_env()
        .ok()
        .and_then(|paths| instance::probe(&paths).ok())
        .and_then(|liveness| match liveness {
            Liveness::Running(record) => Some(record),
            Liveness::NotRunning { .. } | Liveness::Starting => None,
        });
    let proxy = config.proxy();
    let owner = running
        .as_ref()
        .filter(|record| record.proxy == proxy.listen());
    failures += usize::from(!report_listener("proxy", proxy.listen(), None, owner));
    match proxy.pac_listen() {
        Some(address) => {
            let url = format!("http://{address}/proxy.pac");
            let owner = running
                .as_ref()
                .filter(|record| record.pac.as_deref() == Some(url.as_str()));
            failures += usize::from(!report_listener("pac", address, Some(&url), owner));
        }
        None => println!("pac      disabled"),
    }
    let ports = proxy
        .allowed_ports()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    println!("ports    {ports}");

    let dns = config.dns();
    println!(
        "dns      {} (timeout {}, cache {} entries ≤ {})",
        dns.upstream(),
        humantime::format_duration(dns.timeout()),
        dns.cache_capacity(),
        humantime::format_duration(dns.cache_max_ttl()),
    );

    let resolver = Arc::new(DohResolver::new(dns).map_err(AppError::Resolver)?);
    println!("targets  {}", config.targets().len());
    for rule in config.targets() {
        let scope = if rule.includes_subdomains() {
            " + subdomains"
        } else {
            ""
        };
        let started = Instant::now();
        match resolver.resolve_ipv4(rule.host()).await {
            Ok(addresses) => {
                let addresses = addresses
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "  ✓ {}{scope}  →  {addresses}  ({} ms)",
                    rule.host(),
                    started.elapsed().as_millis()
                );
            }
            Err(error) => {
                failures += 1;
                println!(
                    "  ✗ {}{scope}  →  {}: {error}",
                    rule.host(),
                    error.outcome()
                );
            }
        }
    }
    // Informational only: being offline or behind GitHub's rate limit is not a failure.
    println!(
        "version  {}  {}",
        update::CURRENT_VERSION,
        release_status(config, resolver).await
    );

    if failures == 0 {
        println!("ok: ready to `splithello start`");
    } else {
        println!("{failures} check(s) failed");
    }
    Ok(failures == 0)
}

async fn release_status(config: &Config, resolver: Arc<DohResolver>) -> String {
    if !update::check_enabled(config.update().check()) {
        update::forget_cached();
        return "· update check off".to_owned();
    }
    let source = match ReleaseSource::github(resolver) {
        Ok(source) => source,
        Err(error) => return format!("· update check failed: {error}"),
    };
    let cache = default_cache_path(update::CACHE_FILE);
    match update::latest_version_cached(&source, cache.as_deref()).await {
        Ok(latest) if update::is_newer(&latest) => format!(
            "↑ {latest} available → {}",
            InstallMethod::current().upgrade_command()
        ),
        Ok(_) => "✓ latest".to_owned(),
        Err(update::UpdateError::NoRelease) => "· no stable release yet".to_owned(),
        Err(error) => format!("· update check failed: {error}"),
    }
}

fn report_listener(
    label: &str,
    address: SocketAddr,
    display: Option<&str>,
    owner: Option<&RunRecord>,
) -> bool {
    let shown = display.map_or_else(|| address.to_string(), str::to_owned);
    match std::net::TcpListener::bind(address) {
        Ok(_) => {
            println!("{label:<8} {shown}  ✓ available");
            true
        }
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            match owner {
                Some(record) => println!(
                    "{label:<8} {shown}  ✗ in use by SplitHello (pid {}) → `splithello stop` first",
                    record.pid
                ),
                None => println!("{label:<8} {shown}  ✗ in use by another program"),
            }
            false
        }
        Err(error) => {
            println!("{label:<8} {shown}  ✗ {error}");
            false
        }
    }
}
