//! `start -d`, `logs`, and `restart` against the real binary, on every OS.

mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use tempfile::TempDir;
use tokio::net::TcpStream;

use support::{
    DohResponse, TEST_TIMEOUT, TestConfig, TestDoh, TestOrigin, connect_via_proxy,
    isolated_command, parse_ready_socket,
};

const BINARY: &str = env!("CARGO_BIN_EXE_splithello");

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A state directory for one test. Dropping it force-stops whatever runs there, so a failed
/// test leaves no background SplitHello behind (on Windows one would lock the binary).
struct Background {
    directory: TempDir,
}

impl Background {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        isolated_command(Path::new(BINARY), self.directory.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("run splithello")
    }

    fn config(&self, doh: &TestDoh, origin_port: u16) -> String {
        let text = TestConfig {
            doh,
            origin_port,
            target_host: "selected.test",
            with_pac: false,
            max_connections: 4,
            shutdown_seconds: 1,
            injected_secret: None,
        }
        .render(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), None);
        self.write_config("config.toml", &text)
    }

    fn write_config(&self, name: &str, text: &str) -> String {
        let path = self.directory.path().join(name);
        fs::write(&path, text).unwrap();
        path.display().to_string()
    }

    /// `start -d` that must succeed; returns the proxy address from its ready line.
    fn start(&self, config: &str, flags: &[&str]) -> SocketAddr {
        let mut args = flags.to_vec();
        args.extend(["--log-format", "logfmt", "start", "-d", "--config", config]);
        let output = self.run(&args);
        let printed = text(&output);
        assert!(output.status.success(), "{printed}");
        assert!(
            printed.contains("running in the background · pid"),
            "{printed}"
        );
        parse_ready_socket(&printed, "proxy_addr=").expect("ready line names the proxy")
    }

    fn state(&self) -> PathBuf {
        self.directory.path().join("splithello")
    }

    fn pid(&self) -> u32 {
        let record = fs::read_to_string(self.state().join("run.toml")).expect("run.toml");
        record
            .lines()
            .find_map(|line| line.strip_prefix("pid = "))
            .and_then(|pid| pid.parse().ok())
            .expect("run.toml has a pid")
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.run(&["stop", "--force"]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_background_instance_serves_until_stopped() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let background = Background::new();
    let config = background.config(&doh, origin.addr.port());
    let proxy = background.start(&config, &[]);

    let status = background.run(&["status"]);
    assert!(status.status.success(), "{}", text(&status));
    assert!(text(&status).contains(&format!("pid {}", background.pid())));
    assert!(
        text(&status).contains("splithello.log"),
        "{}",
        text(&status)
    );

    let tunnel = connect_via_proxy(proxy, &format!("localhost:{}", origin.addr.port())).await;
    assert!(text(&background.run(&["status"])).contains("tunnels  1 open"));
    drop(tunnel);

    let logs = text(&background.run(&["logs", "-n", "20"]));
    assert!(logs.contains("outcome=ready"), "{logs}");

    let stop = background.run(&["stop"]);
    assert!(stop.status.success(), "{}", text(&stop));
    assert!(TcpStream::connect(proxy).await.is_err());
    assert_eq!(background.run(&["status"]).status.code(), Some(3));
    assert!(!background.state().join("run.toml").exists());
    let logs = text(&background.run(&["logs"]));
    assert!(
        logs.contains("stop requested"),
        "the last run's log stays: {logs}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn only_one_background_instance_starts() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    background.start(&config, &[]);
    let pid = background.pid();

    let second = background.run(&["start", "-d", "--config", &config]);
    let printed = text(&second);
    assert_eq!(second.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains(&format!("already running (pid {pid})")),
        "{printed}"
    );
    assert_eq!(background.pid(), pid);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_starts_leave_exactly_one_running() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    let outputs = std::thread::scope(|scope| {
        let starts =
            [(); 3].map(|()| scope.spawn(|| background.run(&["start", "-d", "--config", &config])));
        starts.map(|start| start.join().unwrap())
    });
    let started = outputs
        .iter()
        .filter(|output| output.status.success())
        .count();
    let printed = outputs.iter().map(text).collect::<Vec<_>>().join("\n---\n");
    assert_eq!(started, 1, "{printed}");
    for output in outputs.iter().filter(|output| !output.status.success()) {
        assert!(text(output).contains("already running"), "{printed}");
    }
    assert!(background.run(&["status"]).status.success());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_start_shows_why_and_leaves_nothing_running() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let busy = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let background = Background::new();
    let text_config = TestConfig {
        doh: &doh,
        origin_port: 443,
        target_host: "selected.test",
        with_pac: false,
        max_connections: 4,
        shutdown_seconds: 1,
        injected_secret: None,
    }
    .render(busy.local_addr().unwrap(), None);
    let config = background.write_config("busy.toml", &text_config);

    let start = background.run(&["start", "-d", "--config", &config]);
    let printed = text(&start);
    assert_eq!(start.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains("could not bind proxy listener"),
        "{printed}"
    );
    assert!(printed.contains("--- last lines of"), "{printed}");
    assert!(printed.contains("exited while starting"), "{printed}");
    assert_eq!(background.run(&["status"]).status.code(), Some(3));
}

#[test]
fn a_config_without_targets_stops_before_anything_starts() {
    let background = Background::new();
    let config = background.write_config("empty.toml", "targets = []\n");
    let start = background.run(&["start", "-d", "--config", &config]);
    let printed = text(&start);
    assert_eq!(start.status.code(), Some(1), "{printed}");
    assert!(printed.contains("no targets in"), "{printed}");
    assert!(!background.state().join("splithello.log").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_keeps_the_config_and_log_flags_and_rotates_the_log() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    background.start(&config, &["-v"]);
    let first = background.pid();

    let restart = background.run(&["restart"]);
    let printed = text(&restart);
    assert!(restart.status.success(), "{printed}");
    assert!(printed.contains("stopped"), "{printed}");
    let second = background.pid();
    assert_ne!(first, second);
    let record = fs::read_to_string(background.state().join("run.toml")).unwrap();
    assert!(record.contains("\"-v\""), "{record}");
    assert!(record.contains("config.toml"), "{record}");
    assert!(background.state().join("splithello.log.1").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_starts_a_stopped_instance() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    let restart = background.run(&["restart", "--config", &config]);
    let printed = text(&restart);
    assert!(restart.status.success(), "{printed}");
    assert!(
        printed.contains("was not running; starting it"),
        "{printed}"
    );
    assert!(background.run(&["status"]).status.success());
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_follow_prints_new_lines() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    background.start(&config, &[]);

    let mut follower = isolated_command(Path::new(BINARY), background.directory.path())
        .args(["logs", "-f", "-n", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(follower.stdout.take().unwrap()).lines();
    lines.next().expect("the last line first").unwrap();

    // Stopping writes one more line, which the follower must print.
    assert!(background.run(&["stop"]).status.success());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in lines.map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
    let deadline = std::time::Instant::now() + TEST_TIMEOUT;
    let mut seen = Vec::new();
    while std::time::Instant::now() < deadline {
        if let Ok(line) = receiver.recv_timeout(Duration::from_millis(100)) {
            if line.contains("stop requested") {
                follower.kill().unwrap();
                let _ = follower.wait();
                return;
            }
            seen.push(line);
        }
    }
    follower.kill().unwrap();
    let _ = follower.wait();
    panic!("logs -f did not print the new line; saw {seen:?}");
}

#[test]
fn logs_without_a_background_run_say_so() {
    let background = Background::new();
    let logs = background.run(&["logs"]);
    let printed = text(&logs);
    assert_eq!(logs.status.code(), Some(1), "{printed}");
    assert!(printed.contains("no log at"), "{printed}");
    assert!(printed.contains("splithello start -d"), "{printed}");
}

/// What `brew services` starts: `stop` and `restart` point to Homebrew instead of fighting it.
#[tokio::test(flavor = "multi_thread")]
async fn a_homebrew_service_is_left_to_homebrew() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    let start = isolated_command(Path::new(BINARY), background.directory.path())
        .args(["start", "-d", "--config", &config])
        .env("SPLITHELLO_MANAGED_BY", "homebrew")
        .output()
        .unwrap();
    assert!(start.status.success(), "{}", text(&start));
    let pid = background.pid();

    let status = text(&background.run(&["status"]));
    assert!(
        status.contains("managed  Homebrew (brew services)"),
        "{status}"
    );

    let stop = background.run(&["stop"]);
    let printed = text(&stop);
    assert_eq!(stop.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains("brew services stop splithello"),
        "{printed}"
    );
    let restart = background.run(&["restart"]);
    assert!(text(&restart).contains("brew services restart splithello"));
    let again = text(&background.run(&["start", "-d", "--config", &config]));
    assert!(
        again.contains("started by Homebrew (brew services)"),
        "{again}"
    );
    assert_eq!(background.pid(), pid, "still the same instance");

    let force = background.run(&["stop", "--force"]);
    let printed = text(&force);
    assert!(force.status.success(), "{printed}");
    assert!(printed.contains("may start SplitHello again"), "{printed}");
}

/// A session of its own has no controlling terminal, so closing the terminal sends nothing.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn the_background_instance_leads_its_own_session() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let background = Background::new();
    let config = background.config(&doh, 443);
    background.start(&config, &[]);
    let pid = i32::try_from(background.pid()).unwrap();
    let pid = rustix::process::Pid::from_raw(pid).unwrap();
    assert_eq!(rustix::process::getsid(Some(pid)).unwrap(), pid);
}
