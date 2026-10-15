mod support;

use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::{Command, Output};

use support::{DohResponse, TestConfig, TestDoh};

fn splithello(args: &[&str], xdg_config_home: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_splithello"))
        .args(args)
        .env("XDG_CONFIG_HOME", xdg_config_home)
        .env("RUST_LOG", "info")
        .output()
        .expect("run splithello")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const EXAMPLE: &str = include_str!("../config.example.toml");

/// `start` on a missing file writes the starter there, then stops because it has no targets.
fn assert_creates_starter_and_stops(output: &Output, path: &std::path::Path) {
    let printed = text(output);
    assert!(!output.status.success(), "{printed}");
    assert_eq!(fs::read_to_string(path).unwrap(), EXAMPLE);
    let shown = path.display().to_string();
    assert!(
        printed.contains(&format!("created starter configuration {shown}")),
        "{printed}"
    );
    assert!(
        printed.contains(&format!("no targets in {shown}")),
        "{printed}"
    );
    assert!(!printed.contains("SplitHello is ready"), "{printed}");
}

#[test]
fn start_creates_the_default_config_and_stops_without_targets() {
    let home = tempfile::tempdir().unwrap();
    let output = splithello(&["start"], home.path());
    assert_creates_starter_and_stops(&output, &home.path().join("splithello/config.toml"));
}

#[test]
fn start_creates_an_explicit_config_path_and_stops_without_targets() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("nested/dir/custom.toml");
    let output = splithello(
        &["start", "--config", &path.display().to_string()],
        home.path(),
    );
    assert_creates_starter_and_stops(&output, &path);
    assert!(!home.path().join("splithello/config.toml").exists());
}

#[test]
fn an_existing_config_is_never_overwritten() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("splithello/config.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "targets = []\n# edited\n").unwrap();

    let output = splithello(&["check"], home.path());
    let printed = text(&output);
    assert!(!output.status.success(), "{printed}");
    assert!(
        !printed.contains("created starter configuration"),
        "{printed}"
    );
    assert!(printed.contains("no targets in"), "{printed}");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "targets = []\n# edited\n"
    );
}

#[test]
fn init_is_no_longer_a_command() {
    let home = tempfile::tempdir().unwrap();
    let output = splithello(&["init"], home.path());
    assert!(!output.status.success());
    assert!(!home.path().join("splithello/config.toml").exists());
}

async fn check_against(response: DohResponse) -> (Output, tempfile::TempDir) {
    let doh = TestDoh::start(response).await;
    let directory = tempfile::tempdir().unwrap();
    let config = TestConfig {
        doh: &doh,
        origin_port: 443,
        target_host: "selected.test",
        with_pac: false,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    }
    .render(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), None);
    let path = directory.path().join("config.toml");
    fs::write(&path, config).unwrap();
    let path = path.display().to_string();
    let home = directory.path().to_path_buf();
    let output =
        tokio::task::spawn_blocking(move || splithello(&["check", "--config", &path], &home))
            .await
            .unwrap();
    (output, directory)
}

#[tokio::test]
async fn check_resolves_every_target_and_exits_zero() {
    let (output, _directory) =
        check_against(DohResponse::Addresses(vec![Ipv4Addr::new(192, 0, 2, 7)])).await;
    let printed = text(&output);
    assert!(output.status.success(), "{printed}");
    assert!(
        printed.contains("✓ selected.test + subdomains  →  192.0.2.7"),
        "{printed}"
    );
    assert!(printed.contains("pac      disabled"), "{printed}");
    assert!(
        printed.contains("ok: ready to `splithello start`"),
        "{printed}"
    );
}

#[tokio::test]
async fn check_reports_dns_failures_and_exits_non_zero() {
    let (output, _directory) = check_against(DohResponse::HttpStatus(503)).await;
    let printed = text(&output);
    assert!(!output.status.success(), "{printed}");
    assert!(
        printed.contains("✗ selected.test + subdomains  →  dns-error"),
        "{printed}"
    );
    assert!(printed.contains("1 check(s) failed"), "{printed}");
}

#[test]
fn check_reports_an_occupied_listener() {
    let occupied = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = occupied.local_addr().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        format!(
            "targets = [\"selected.test\"]\n[proxy]\nlisten = \"{address}\"\npac = false\n[dns]\nupstream = \"https://127.0.0.1:9/dns-query\"\ntimeout = \"200ms\"\n"
        ),
    )
    .unwrap();
    let output = splithello(
        &["check", "--config", &path.display().to_string()],
        directory.path(),
    );
    let printed = text(&output);
    assert!(!output.status.success(), "{printed}");
    assert!(
        printed.contains(&format!("proxy    {address}  ✗ in use")),
        "{printed}"
    );
}
