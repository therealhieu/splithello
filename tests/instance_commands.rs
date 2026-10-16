//! `status` and `stop` against a real SplitHello process, on every OS.

mod support;

use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use support::{
    DohResponse, RunningSplitHello, TEST_TIMEOUT, TestConfig, TestDoh, TestOrigin,
    connect_via_proxy, isolated_command,
};

const BINARY: &str = env!("CARGO_BIN_EXE_splithello");

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn start(doh: &TestDoh, origin_port: u16) -> RunningSplitHello {
    RunningSplitHello::start(TestConfig {
        doh,
        origin_port,
        target_host: "selected.test",
        with_pac: true,
        max_connections: 4,
        shutdown_seconds: 1,
        injected_secret: None,
    })
}

/// `splithello <args>` with its state in `directory` and nothing running there.
fn run_in(directory: &Path, args: &[&str]) -> Output {
    isolated_command(Path::new(BINARY), directory)
        .args(args)
        .output()
        .expect("run splithello")
}

#[tokio::test(flavor = "multi_thread")]
async fn status_finds_a_foreground_instance_and_stop_drains_it() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let mut process = start(&doh, origin.addr.port());

    let status = process.cli(&["status"]);
    let printed = text(&status);
    assert!(status.status.success(), "{printed}");
    assert!(
        printed.contains(&format!("running · pid {}", process.pid())),
        "{printed}"
    );
    assert!(
        printed.contains(&format!("proxy    {}", process.proxy_addr)),
        "{printed}"
    );
    assert!(printed.contains("tunnels  0 open"), "{printed}");

    let mut active = connect_via_proxy(
        process.proxy_addr,
        &format!("localhost:{}", origin.addr.port()),
    )
    .await;
    let status = text(&process.cli(&["status"]));
    assert!(status.contains("tunnels  1 open"), "{status}");

    let stop = process.cli(&["stop"]);
    let printed = text(&stop);
    assert!(stop.status.success(), "{printed}");
    assert!(printed.contains("stopping SplitHello"), "{printed}");
    assert!(printed.contains("stopped\n"), "{printed}");
    assert!(
        printed.contains(&format!("a browser still set to {}", process.proxy_addr)),
        "{printed}"
    );
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
    let mut rest = Vec::new();
    timeout(TEST_TIMEOUT, active.read_to_end(&mut rest))
        .await
        .expect("the open tunnel closes")
        .expect("read the closed tunnel");
    assert!(TcpStream::connect(process.proxy_addr).await.is_err());
    assert!(!process.state_dir().join("run.toml").exists());

    let status = process.cli(&["status"]);
    assert_eq!(status.status.code(), Some(3), "{}", text(&status));
    assert!(text(&status).contains("SplitHello is not running"));
    let again = process.cli(&["stop"]);
    assert!(again.status.success());
    assert!(text(&again).contains("SplitHello is not running"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_start_is_refused_while_one_runs() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let mut process = start(&doh, 443);
    let config = process.config_path().display().to_string();

    let second = process.cli(&["start", "--config", &config]);
    let printed = text(&second);
    assert_eq!(second.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains(&format!("already running (pid {})", process.pid())),
        "{printed}"
    );
    assert!(printed.contains("splithello status"), "{printed}");

    let check = process.cli(&["check", "--config", &config]);
    assert!(
        text(&check).contains("✓ available"),
        "port 0 is always free"
    );

    process.interrupt();
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test(flavor = "multi_thread")]
async fn check_names_the_instance_holding_the_port() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let mut process = start(&doh, 443);
    let busy = TestConfig {
        doh: &doh,
        origin_port: 443,
        target_host: "selected.test",
        with_pac: false,
        max_connections: 4,
        shutdown_seconds: 1,
        injected_secret: None,
    }
    .render(process.proxy_addr, None);
    let path = process.directory_path().join("busy.toml");
    fs::write(&path, busy).unwrap();

    let check = process.cli(&["check", "--config", &path.display().to_string()]);
    let printed = text(&check);
    assert!(!check.status.success(), "{printed}");
    assert!(
        printed.contains(&format!(
            "proxy    {}  ✗ in use by SplitHello (pid {})",
            process.proxy_addr,
            process.pid()
        )),
        "{printed}"
    );

    process.interrupt();
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_token_is_refused_and_never_printed() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let mut process = start(&doh, 443);
    let run = process.state_dir().join("run.toml");
    let original = fs::read_to_string(&run).unwrap();
    let token = original
        .lines()
        .find_map(|line| line.strip_prefix("token = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("run.toml has a token")
        .to_owned();
    assert_eq!(token.len(), 64);

    fs::write(&run, original.replace(&token, &"0".repeat(64))).unwrap();
    let stop = process.cli(&["stop"]);
    let printed = text(&stop);
    assert_eq!(stop.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains("refused the request: unauthorized"),
        "{printed}"
    );
    let status = process.cli(&["status"]);
    assert!(status.status.success(), "still running: {}", text(&status));
    assert!(text(&status).contains("not answering"), "{}", text(&status));

    fs::write(&run, &original).unwrap();
    let status = text(&process.cli(&["status"]));
    assert!(status.contains("tunnels  0 open"), "{status}");
    process.interrupt();
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
    for output in [printed, status, process.output()] {
        assert!(!output.contains(&token), "the token leaked: {output}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_force_kills_and_clears_the_record() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let mut process = start(&doh, 443);

    let stop = process.cli(&["stop", "--force"]);
    let printed = text(&stop);
    assert!(stop.status.success(), "{printed}");
    assert!(
        printed.contains(&format!("killed SplitHello (pid {})", process.pid())),
        "{printed}"
    );
    assert!(!process.wait_for_exit(TEST_TIMEOUT).success());
    assert!(!process.state_dir().join("run.toml").exists());
    assert_eq!(process.cli(&["status"]).status.code(), Some(3));
}

#[test]
fn a_crashed_instance_is_reported_then_cleared() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("splithello");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("splithello.lock"), "").unwrap();
    fs::write(
        state.join("run.toml"),
        format!(
            "format = 1\npid = 999999\nversion = \"0.0.1\"\nstarted_at = 0\nconfig = \"config.toml\"\n\
             proxy = \"127.0.0.1:9\"\ncontrol = \"127.0.0.1:9\"\ntoken = \"{}\"\n",
            "a".repeat(64)
        ),
    )
    .unwrap();

    let status = run_in(directory.path(), &["status"]);
    let printed = text(&status);
    assert_eq!(status.status.code(), Some(3), "{printed}");
    assert!(
        printed.contains("last run  pid 999999 did not shut down cleanly"),
        "{printed}"
    );

    let stop = run_in(directory.path(), &["stop"]);
    let printed = text(&stop);
    assert!(stop.status.success(), "{printed}");
    assert!(
        printed.contains("cleared the record of one that crashed"),
        "{printed}"
    );
    assert!(!state.join("run.toml").exists());
    assert!(
        state.join("splithello.lock").exists(),
        "the lock file stays"
    );
}

#[test]
fn an_instance_that_never_publishes_is_reported_as_starting() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("splithello");
    fs::create_dir_all(&state).unwrap();
    let lock = fs::File::create(state.join("splithello.lock")).unwrap();
    lock.try_lock().unwrap();

    let status = run_in(directory.path(), &["status"]);
    assert!(status.status.success(), "{}", text(&status));
    assert!(text(&status).contains("SplitHello is starting"));

    let started = std::time::Instant::now();
    let stop = run_in(directory.path(), &["stop"]);
    let printed = text(&stop);
    assert_eq!(stop.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains("has not said how to reach it"),
        "{printed}"
    );
    assert!(
        started.elapsed() >= Duration::from_secs(4),
        "stop waits for it first"
    );
    drop(lock);
}
