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

#[test]
fn init_writes_the_example_to_the_default_path_and_refuses_to_overwrite() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("splithello/config.toml");

    let first = splithello(&["init"], home.path());
    assert!(first.status.success(), "{}", text(&first));
    assert!(text(&first).contains(&path.display().to_string()));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/config.example.toml")).unwrap()
    );

    fs::write(&path, "targets = [\"edited.example\"]\n").unwrap();
    let second = splithello(&["init"], home.path());
    assert!(!second.status.success());
    assert!(text(&second).contains("already exists; pass --force"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "targets = [\"edited.example\"]\n"
    );

    let forced = splithello(&["init", "--force"], home.path());
    assert!(forced.status.success(), "{}", text(&forced));
    assert!(fs::read_to_string(&path).unwrap().contains("reddit.com"));
}

#[test]
fn start_without_a_config_points_at_init() {
    let home = tempfile::tempdir().unwrap();
    let output = splithello(&["start"], home.path());
    assert!(!output.status.success());
    let output = text(&output);
    assert!(output.contains("outcome=config-error"), "{output}");
    assert!(output.contains("run `splithello init`"), "{output}");
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
