//! End-to-end SplitHello benchmarks.
//!
//! Each scenario drives a compiled `splithello` binary over loopback:
//!
//! ```text
//! bench client ──CONNECT──► splithello ──DoH (simulated RTT)──► TestDoh (keep-alive HTTPS)
//!                               └──TCP + split ClientHello──► TLS origin / stream origin
//! ```
//!
//! Two modes share the scenarios:
//!
//! - **criterion** (default): one binary, statistics and history under `target/criterion`.
//! - **paired** (`SPLITHELLO_BENCH_BASE_BIN=<binary>`): base and HEAD binaries measured in
//!   alternating rounds (ABBA…) inside one process, so machine drift hits both equally.
//!   The verdict uses the per-round HEAD/base ratio. This is the PR gate.
//!
//! Every exchange is closed by the connecting side first (client → proxy, then proxy →
//! origin), so TIME_WAIT lands on ephemeral ports the kernel knows to skip. If a listener
//! closed first instead, its TIME_WAIT entry would be invisible to the connecting side's
//! port allocator and later SYNs reusing that 4-tuple would stall for seconds.
//! Timing stops at the event being measured; the close handshake and port-budget pacing
//! ([`reserve_tunnels`]) happen outside the measured span.

#[path = "../tests/support/mod.rs"]
mod support;

use std::collections::VecDeque;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use criterion::{Criterion, SamplingMode, Throughput};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use support::{DohOptions, DohResponse, RunningSplitHello, TestDoh, tls_material};

/// Simulated client ↔ DoH round trip; roughly a nearby public resolver.
const DOH_RTT: Duration = Duration::from_millis(20);
const TARGET_DOMAIN: &str = "bench.test";
const BURST: u64 = 20;
const THROUGHPUT_DOWN: u64 = 64 * 1024 * 1024;
const THROUGHPUT_BIDIR: u64 = 32 * 1024 * 1024;
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
const BASE_BIN_ENV: &str = "SPLITHELLO_BENCH_BASE_BIN";
/// When set to `1`, the paired gate also fails unless at least one scenario is faster:
/// the merge rule for PRs is "no regression, and measurably faster than the base".
const REQUIRE_FASTER_ENV: &str = "SPLITHELLO_BENCH_REQUIRE_FASTER";
/// A regression fails the gate only when the whole 95% CI of the median change is above it.
const REGRESSION_GATE: f64 = 0.05;
const NOISE: f64 = 0.01;

/// A closed loopback connection stays in TIME_WAIT for 2×MSL (30s on macOS) and pins the
/// ephemeral port on its far side, so each tunnel (client→proxy, proxy→origin) holds two of
/// the ~16k ephemeral ports for that long. Fast scenarios open ~2k tunnels/s; pacing keeps
/// them inside the budget instead of failing with connect timeouts.
const PORT_WINDOW: Duration = Duration::from_secs(32);
const MAX_TUNNELS_PER_WINDOW: u64 = 6_000;
static TUNNEL_WINDOW: Mutex<VecDeque<(Instant, u64)>> = Mutex::new(VecDeque::new());

/// Waits (unmeasured) until `tunnels` more connections fit in the TIME_WAIT budget.
async fn reserve_tunnels(tunnels: u64) {
    loop {
        let wait = {
            let mut window = TUNNEL_WINDOW.lock().expect("tunnel window lock");
            let now = Instant::now();
            while window
                .front()
                .is_some_and(|(opened, _)| now.duration_since(*opened) >= PORT_WINDOW)
            {
                window.pop_front();
            }
            let used = window.iter().map(|(_, count)| count).sum::<u64>();
            match window.front() {
                Some((oldest, _)) if used + tunnels > MAX_TUNNELS_PER_WINDOW => {
                    PORT_WINDOW - now.duration_since(*oldest)
                }
                _ => {
                    window.push_back((now, tunnels));
                    return;
                }
            }
        };
        tokio::time::sleep(wait).await;
    }
}

/// Waits until every earlier tunnel has left TIME_WAIT, so the next scenario starts with
/// the full budget and never pauses while criterion plans iteration counts.
async fn drain_tunnel_budget() {
    let newest = TUNNEL_WINDOW
        .lock()
        .expect("tunnel window lock")
        .back()
        .map(|(opened, _)| *opened);
    if let Some(newest) = newest {
        tokio::time::sleep((newest + PORT_WINDOW).saturating_duration_since(Instant::now())).await;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    TargetColdTtfb,
    TargetWarmTtfb,
    DirectTtfb,
    BurstCold20,
    FirstRequestAfterStart,
    ThroughputDown64m,
    ThroughputBidir32m,
}

impl Scenario {
    const ALL: [Self; 7] = [
        Self::TargetColdTtfb,
        Self::TargetWarmTtfb,
        Self::DirectTtfb,
        Self::BurstCold20,
        Self::ThroughputDown64m,
        Self::ThroughputBidir32m,
        Self::FirstRequestAfterStart,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::TargetColdTtfb => "target_cold_ttfb",
            Self::TargetWarmTtfb => "target_warm_ttfb",
            Self::DirectTtfb => "direct_ttfb",
            Self::BurstCold20 => "burst_cold_20",
            Self::FirstRequestAfterStart => "first_request_after_start",
            Self::ThroughputDown64m => "throughput_down_64m",
            Self::ThroughputBidir32m => "throughput_bidir_32m",
        }
    }

    const fn throughput_bytes(self) -> Option<u64> {
        match self {
            Self::ThroughputDown64m => Some(THROUGHPUT_DOWN),
            Self::ThroughputBidir32m => Some(THROUGHPUT_BIDIR * 2),
            _ => None,
        }
    }

    /// Paired mode: (rounds, iterations per sample). Sized to stay under the tunnel budget
    /// for both binaries together while giving each sample a stable mean.
    const fn paired_shape(self) -> (usize, u64) {
        match self {
            Self::TargetColdTtfb => (40, 2),
            Self::TargetWarmTtfb | Self::DirectTtfb => (80, 20),
            Self::BurstCold20 => (40, 1),
            Self::FirstRequestAfterStart => (16, 1),
            Self::ThroughputDown64m | Self::ThroughputBidir32m => (40, 3),
        }
    }

    const fn needs_shared_proxy(self) -> bool {
        !matches!(self, Self::FirstRequestAfterStart)
    }
}

/// Which configuration schema a binary understands. v2 (`splithello check` exists) has
/// top-level `targets`; v1 used `[[targets]]` tables and `*_seconds` keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigSchema {
    V1,
    V2,
}

impl ConfigSchema {
    fn detect(binary: &Path) -> Self {
        let help = Command::new(binary)
            .arg("--help")
            .output()
            .expect("run splithello --help");
        if String::from_utf8_lossy(&help.stdout).contains("check") {
            Self::V2
        } else {
            Self::V1
        }
    }
}

/// One binary under test with its own DoH server, origins and (optionally) proxy process.
struct Variant {
    binary: PathBuf,
    schema: ConfigSchema,
    lab: Arc<Lab>,
    proxy: Option<RunningSplitHello>,
    doh_queries_start: usize,
}

impl Variant {
    async fn start(binary: &Path, scenario: Scenario) -> Self {
        let schema = ConfigSchema::detect(binary);
        let lab = Arc::new(Lab::start().await);
        let mut variant = Self {
            binary: binary.to_path_buf(),
            schema,
            lab,
            proxy: None,
            doh_queries_start: 0,
        };
        if scenario.needs_shared_proxy() {
            variant.proxy = Some(variant.spawn_proxy().await);
        }
        if scenario == Scenario::TargetWarmTtfb {
            variant.target_ttfb(&warm_host()).await;
        }
        variant.doh_queries_start = variant.lab.doh.queries().len();
        variant
    }

    async fn spawn_proxy(&self) -> RunningSplitHello {
        let binary = self.binary.clone();
        let config = self.lab.config(self.schema);
        tokio::task::spawn_blocking(move || {
            RunningSplitHello::start_binary(&binary, &config, false)
        })
        .await
        .expect("spawn proxy")
    }

    fn proxy_addr(&self) -> SocketAddr {
        self.proxy.as_ref().expect("shared proxy").proxy_addr
    }

    fn doh_queries(&self) -> usize {
        self.lab.doh.queries().len() - self.doh_queries_start
    }

    async fn target_ttfb(&self, host: &str) -> Duration {
        reserve_tunnels(1).await;
        let started = Instant::now();
        self.lab.target_request(self.proxy_addr(), host).await - started
    }

    /// One iteration of `scenario`; only the measured span is returned.
    async fn iteration(&self, scenario: Scenario, counter: &AtomicU64) -> Duration {
        match scenario {
            Scenario::TargetColdTtfb => self.target_ttfb(&unique_host(counter, "cold")).await,
            Scenario::TargetWarmTtfb => self.target_ttfb(&warm_host()).await,
            Scenario::DirectTtfb => {
                reserve_tunnels(1).await;
                self.lab.direct_transfer(self.proxy_addr(), 1, 0).await.0
            }
            Scenario::BurstCold20 => {
                let host = unique_host(counter, "burst");
                reserve_tunnels(BURST).await;
                let started = Instant::now();
                let proxy = self.proxy_addr();
                let tasks = (0..BURST)
                    .map(|_| {
                        let (lab, host) = (Arc::clone(&self.lab), host.clone());
                        tokio::spawn(async move { lab.target_request(proxy, &host).await })
                    })
                    .collect::<Vec<_>>();
                let mut last_first_byte = started;
                for task in tasks {
                    last_first_byte = last_first_byte.max(task.await.expect("burst tunnel"));
                }
                last_first_byte - started
            }
            Scenario::FirstRequestAfterStart => {
                let process = self.spawn_proxy().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
                reserve_tunnels(1).await;
                let started = Instant::now();
                let first_byte = self
                    .lab
                    .target_request(process.proxy_addr, TARGET_DOMAIN)
                    .await;
                drop(process);
                first_byte - started
            }
            Scenario::ThroughputDown64m => {
                reserve_tunnels(1).await;
                self.lab
                    .direct_transfer(self.proxy_addr(), THROUGHPUT_DOWN, 0)
                    .await
                    .1
            }
            Scenario::ThroughputBidir32m => {
                reserve_tunnels(1).await;
                self.lab
                    .direct_transfer(self.proxy_addr(), THROUGHPUT_BIDIR, THROUGHPUT_BIDIR)
                    .await
                    .1
            }
        }
    }
}

struct Lab {
    doh: TestDoh,
    tls_origin: TlsOrigin,
    stream_origin: StreamOrigin,
    _material_dir: tempfile::TempDir,
}

impl Lab {
    async fn start() -> Self {
        let doh = TestDoh::start_with(DohOptions {
            response: DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST]),
            keep_alive: true,
            rtt: DOH_RTT,
            ttl: 300,
        })
        .await;
        let material_dir = tempfile::tempdir().expect("bench TLS directory");
        let tls_origin = TlsOrigin::start(&material_dir).await;
        let stream_origin = StreamOrigin::start().await;
        Self {
            doh,
            tls_origin,
            stream_origin,
            _material_dir: material_dir,
        }
    }

    /// Configuration that mirrors a user install (default limits and cache size)
    /// with the proxy on a kernel-assigned loopback port and PAC disabled.
    fn config(&self, schema: ConfigSchema) -> String {
        let (tls, stream) = (self.tls_origin.addr.port(), self.stream_origin.addr.port());
        let (upstream, ca) = (self.doh.url(), self.doh.ca_pem.display());
        match schema {
            ConfigSchema::V1 => format!(
                "[proxy]\nlisten = \"127.0.0.1:0\"\nallowed_ports = [{tls}, {stream}]\n\n[dns]\nupstream = \"{upstream}\"\nca_certificate = \"{ca}\"\n\n[[targets]]\nhost = \"{TARGET_DOMAIN}\"\ninclude_subdomains = true\nstrategy = \"tls-record-split\"\n"
            ),
            ConfigSchema::V2 => format!(
                "targets = [\"{TARGET_DOMAIN}\"]\n\n[proxy]\nlisten = \"127.0.0.1:0\"\npac = false\nports = [{tls}, {stream}]\n\n[dns]\nupstream = \"{upstream}\"\nca_certificate = \"{ca}\"\n"
            ),
        }
    }

    /// CONNECT → TLS handshake (split by the proxy) → GET → first response byte.
    /// Returns the instant the first response byte arrived, then closes client-first.
    async fn target_request(&self, proxy: SocketAddr, host: &str) -> Instant {
        let tunnel = connect(proxy, &format!("{host}:{}", self.tls_origin.addr.port())).await;
        let name = ServerName::try_from(host.to_owned()).expect("bench server name");
        let mut tls = self
            .tls_origin
            .connector
            .connect(name, tunnel)
            .await
            .expect("TLS handshake through proxy");
        tls.write_all(format!("GET / HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes())
            .await
            .expect("write request");
        let mut response = vec![0_u8; RESPONSE.len()];
        tls.read_exact(&mut response[..1])
            .await
            .expect("first response byte");
        let first_byte = Instant::now();
        tls.read_exact(&mut response[1..])
            .await
            .expect("rest of response");
        let _ = tls.shutdown().await;
        drain_to_eof(tls.get_mut().0).await;
        first_byte
    }

    /// Plain direct tunnel; the origin sends `down` bytes while the client sends `up`.
    /// Returns (first downstream byte, all bytes exchanged) relative to the start.
    async fn direct_transfer(&self, proxy: SocketAddr, down: u64, up: u64) -> (Duration, Duration) {
        let started = Instant::now();
        let tunnel = connect(
            proxy,
            &format!("localhost:{}", self.stream_origin.addr.port()),
        )
        .await;
        let (first_byte, done) = transfer(tunnel, down, up).await;
        (first_byte - started, done - started)
    }
}

struct TlsOrigin {
    addr: SocketAddr,
    connector: TlsConnector,
    task: JoinHandle<()>,
}

impl TlsOrigin {
    async fn start(directory: &tempfile::TempDir) -> Self {
        let wildcard = format!("*.{TARGET_DOMAIN}");
        let material = tls_material(directory.path(), &[TARGET_DOMAIN, wildcard.as_str()]);
        let connector = client_connector(material.ca_der.clone());
        let acceptor = TlsAcceptor::from(Arc::new(material.server));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind TLS origin");
        let addr = listener.local_addr().expect("TLS origin address");
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let _ = stream.set_nodelay(true);
                    let Ok(mut tls) = acceptor.accept(stream).await else {
                        return;
                    };
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    while !request.ends_with(b"\r\n\r\n") {
                        match tls.read(&mut buffer).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => request.extend_from_slice(&buffer[..read]),
                        }
                    }
                    let _ = tls.write_all(RESPONSE).await;
                    // Close only after the proxy's TCP FIN (not just close_notify),
                    // so the proxy is the active closer of this connection.
                    drain_to_eof(&mut tls).await;
                    let (tcp, _) = tls.get_mut();
                    drain_to_eof(tcp).await;
                    let _ = tcp.shutdown().await;
                });
            }
        });
        Self {
            addr,
            connector,
            task,
        }
    }
}

impl Drop for TlsOrigin {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Plain TCP origin speaking a tiny protocol: the client sends `down: u64, up: u64`
/// (big endian), then both sides stream their byte counts concurrently. The origin
/// closes only after the client's EOF arrives.
struct StreamOrigin {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl StreamOrigin {
    async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind stream origin");
        let addr = listener.local_addr().expect("stream origin address");
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let _ = stream.set_nodelay(true);
                    let mut header = [0_u8; 16];
                    if stream.read_exact(&mut header).await.is_err() {
                        return;
                    }
                    let down = u64::from_be_bytes(header[..8].try_into().expect("down"));
                    let (mut reader, mut writer) = stream.into_split();
                    tokio::join!(write_zeros(&mut writer, down), drain_to_eof(&mut reader));
                    let _ = writer.shutdown().await;
                });
            }
        });
        Self { addr, task }
    }
}

impl Drop for StreamOrigin {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn client_connector(ca: CertificateDer<'static>) -> TlsConnector {
    let mut roots = RootCertStore::empty();
    roots.add(ca).expect("trust bench CA");
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

async fn connect(proxy: SocketAddr, authority: &str) -> TcpStream {
    let mut stream = TcpStream::connect(proxy).await.expect("connect to proxy");
    stream.set_nodelay(true).expect("client nodelay");
    stream
        .write_all(format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
        .await
        .expect("write CONNECT");
    let mut response = Vec::with_capacity(64);
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .await
            .expect("read CONNECT response");
        response.push(byte[0]);
    }
    assert!(
        response.starts_with(b"HTTP/1.1 200"),
        "CONNECT {authority} failed: {}",
        String::from_utf8_lossy(&response)
    );
    stream
}

/// Client half of the stream protocol: exchange the byte counts, then close first.
async fn transfer(stream: TcpStream, down: u64, up: u64) -> (Instant, Instant) {
    let (mut reader, mut writer) = stream.into_split();
    let mut header = [0_u8; 16];
    header[..8].copy_from_slice(&down.to_be_bytes());
    header[8..].copy_from_slice(&up.to_be_bytes());
    writer
        .write_all(&header)
        .await
        .expect("write transfer header");
    let (_, (received, first_byte)) = tokio::join!(write_zeros(&mut writer, up), async {
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut received = 0_u64;
        let mut first_byte = None;
        while received < down {
            let wanted = (down - received).min(buffer.len() as u64) as usize;
            match reader.read(&mut buffer[..wanted]).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    first_byte.get_or_insert_with(Instant::now);
                    received += read as u64;
                }
            }
        }
        (received, first_byte)
    });
    let done = Instant::now();
    assert_eq!(received, down, "tunnel delivered every byte");
    let _ = writer.shutdown().await;
    drain_to_eof(&mut reader).await;
    (first_byte.unwrap_or(done), done)
}

async fn write_zeros<W: AsyncWriteExt + Unpin>(writer: &mut W, mut remaining: u64) {
    static ZEROS: [u8; 64 * 1024] = [0; 64 * 1024];
    while remaining > 0 {
        let chunk = remaining.min(ZEROS.len() as u64) as usize;
        if writer.write_all(&ZEROS[..chunk]).await.is_err() {
            return;
        }
        remaining -= chunk as u64;
    }
}

async fn drain_to_eof<R: AsyncReadExt + Unpin>(reader: &mut R) {
    let mut buffer = vec![0_u8; 64 * 1024];
    while let Ok(read) = reader.read(&mut buffer).await {
        if read == 0 {
            break;
        }
    }
}

/// Hosts are unique across the whole process so "cold" stays a cache miss.
fn unique_host(counter: &AtomicU64, prefix: &str) -> String {
    let n = counter.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}{n}.{TARGET_DOMAIN}")
}

fn warm_host() -> String {
    format!("www.{TARGET_DOMAIN}")
}

fn head_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_splithello"))
}

fn runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("bench runtime")
}

/// The positional scenario filter (substring), skipping criterion flags and their values,
/// so filtered-out scenarios skip their lab setup too.
fn cli_filter() -> String {
    const VALUE_FLAGS: &[&str] = &[
        "--save-baseline",
        "--baseline",
        "--baseline-lenient",
        "--load-baseline",
        "--sample-size",
        "--warm-up-time",
        "--measurement-time",
        "--nresamples",
        "--noise-threshold",
        "--confidence-level",
        "--significance-level",
        "--plotting-backend",
        "--output-format",
        "--color",
        "--format",
        "--profile-time",
    ];
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if VALUE_FLAGS.contains(&arg.as_str()) {
            args.next();
        } else if !arg.starts_with('-') {
            return arg;
        }
    }
    String::new()
}

fn main() {
    match std::env::var_os(BASE_BIN_ENV) {
        Some(base) => paired_compare(&PathBuf::from(base)),
        None => {
            let mut criterion = Criterion::default().configure_from_args();
            criterion_benches(&mut criterion);
            criterion.final_summary();
        }
    }
}

fn criterion_benches(c: &mut Criterion) {
    let runtime = runtime();
    let counter = AtomicU64::new(0);
    let binary = head_binary();
    let mut burst_queries = None;

    let filter = cli_filter();
    for scenario in Scenario::ALL {
        if !scenario.name().contains(&filter) {
            continue;
        }
        runtime.block_on(drain_tunnel_budget());
        let variant = runtime.block_on(Variant::start(&binary, scenario));
        let mut group = c.benchmark_group("proxy");
        match scenario {
            Scenario::ThroughputDown64m | Scenario::ThroughputBidir32m => {
                group.sample_size(40);
                group.measurement_time(Duration::from_secs(15));
            }
            Scenario::FirstRequestAfterStart => {
                group.sample_size(10);
                group.sampling_mode(SamplingMode::Flat);
                group.measurement_time(Duration::from_secs(5));
            }
            _ => {
                group.sample_size(50);
                group.warm_up_time(Duration::from_secs(1));
                group.measurement_time(Duration::from_secs(3));
            }
        }
        if let Some(bytes) = scenario.throughput_bytes() {
            group.throughput(Throughput::Bytes(bytes));
        }
        let bursts = AtomicU64::new(0);
        group.bench_function(scenario.name(), |b| {
            b.to_async(&runtime).iter_custom(|iterations| {
                let (variant, counter, bursts) = (&variant, &counter, &bursts);
                async move {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += variant.iteration(scenario, counter).await;
                    }
                    bursts.fetch_add(iterations, Ordering::Relaxed);
                    total
                }
            });
        });
        group.finish();
        if scenario == Scenario::BurstCold20 {
            let per_burst =
                variant.doh_queries() as f64 / bursts.load(Ordering::Relaxed).max(1) as f64;
            println!("burst_cold_20: {per_burst:.2} DoH queries per burst of {BURST}");
            burst_queries = Some(per_burst);
        }
        drop(variant);
    }

    if let Some(per_burst) = burst_queries {
        write_extras("burst_cold_20", "doh_queries_per_burst", per_burst);
    }
}

/// Saves a scalar side metric next to criterion's output, keyed like criterion baselines:
/// `<criterion home>/extras/<baseline>.json` (`new` unless `--save-baseline` was given).
fn write_extras(scenario: &str, metric: &str, value: f64) {
    let args = std::env::args().collect::<Vec<_>>();
    let baseline = args
        .windows(2)
        .find(|pair| pair[0] == "--save-baseline")
        .map_or_else(|| "new".to_owned(), |pair| pair[1].clone());
    let directory = criterion_home().join("extras");
    fs::create_dir_all(&directory).expect("create extras directory");
    let json = serde_json::json!({ scenario: { metric: value } });
    fs::write(
        directory.join(format!("{baseline}.json")),
        serde_json::to_string_pretty(&json).expect("encode extras"),
    )
    .expect("write extras");
}

fn criterion_home() -> PathBuf {
    std::env::var_os("CRITERION_HOME").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/criterion"),
        PathBuf::from,
    )
}

/// One scenario's paired result: per-round mean time per iteration for each binary.
struct PairedResult {
    scenario: Scenario,
    base: Vec<f64>,
    head: Vec<f64>,
    base_doh_per_burst: Option<f64>,
    head_doh_per_burst: Option<f64>,
}

/// Measures base and HEAD in alternating rounds (A B B A A B …) and prints the gate table.
/// Exits non-zero when a scenario regressed beyond [`REGRESSION_GATE`], or, with
/// [`REQUIRE_FASTER_ENV`], when no scenario got faster.
fn paired_compare(base_binary: &Path) {
    let runtime = runtime();
    let head_binary = head_binary();
    let filter = cli_filter();
    let counter = AtomicU64::new(0);
    let mut results = Vec::new();

    for scenario in Scenario::ALL {
        if !scenario.name().contains(&filter) {
            continue;
        }
        runtime.block_on(drain_tunnel_budget());
        let (rounds, iterations) = scenario.paired_shape();
        eprintln!(
            "paired {:<28} {rounds} rounds × {iterations} iterations",
            scenario.name()
        );
        let base = runtime.block_on(Variant::start(base_binary, scenario));
        let head = runtime.block_on(Variant::start(&head_binary, scenario));
        let mut samples = [Vec::with_capacity(rounds), Vec::with_capacity(rounds)];
        // One discarded warm-up round per binary, then ABBA ordering so neither binary
        // systematically runs first.
        for round in 0..=rounds {
            let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
            for index in order {
                let variant = if index == 0 { &base } else { &head };
                let total = runtime.block_on(async {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += variant.iteration(scenario, &counter).await;
                    }
                    total
                });
                if round > 0 {
                    samples[index].push(total.as_secs_f64() * 1e9 / iterations as f64);
                }
            }
        }
        let per_burst = |variant: &Variant| {
            (scenario == Scenario::BurstCold20)
                .then(|| variant.doh_queries() as f64 / ((rounds as u64 + 1) * iterations) as f64)
        };
        let [base_samples, head_samples] = samples;
        results.push(PairedResult {
            scenario,
            base_doh_per_burst: per_burst(&base),
            head_doh_per_burst: per_burst(&head),
            base: base_samples,
            head: head_samples,
        });
    }

    let verdicts = print_paired_table(&results);
    write_paired_json(&results);
    let regressed = verdicts.names(Verdict::Regressed);
    let faster = verdicts.names(Verdict::Faster);
    let require_faster = std::env::var_os(REQUIRE_FASTER_ENV).is_some_and(|value| value == "1");
    println!();
    if !regressed.is_empty() {
        println!(
            "**Gate: FAIL**: regressed beyond {:.0}%: {}",
            REGRESSION_GATE * 100.0,
            regressed.join(", ")
        );
        std::process::exit(1);
    }
    if require_faster && faster.is_empty() {
        println!("**Gate: FAIL**: no scenario is faster than the base (95% CI below -1%)");
        std::process::exit(1);
    }
    let faster = if faster.is_empty() {
        "none".to_owned()
    } else {
        faster.join(", ")
    };
    println!("**Gate: PASS**: no regression; faster: {faster}");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Verdict {
    Regressed,
    Slower,
    Same,
    Faster,
}

impl Verdict {
    /// `lower`/`upper` bound the 95% CI of the median change (HEAD / base − 1).
    fn from_change_ci(lower: f64, upper: f64) -> Self {
        if lower > REGRESSION_GATE {
            Self::Regressed
        } else if lower > NOISE {
            Self::Slower
        } else if upper < -NOISE {
            Self::Faster
        } else {
            Self::Same
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Regressed => "❌ regressed",
            Self::Slower => "slower",
            Self::Same => "≈ same",
            Self::Faster => "✅ faster",
        }
    }
}

struct Verdicts(Vec<(&'static str, Verdict)>);

impl Verdicts {
    fn names(&self, verdict: Verdict) -> Vec<&'static str> {
        self.0
            .iter()
            .filter(|(_, candidate)| *candidate == verdict)
            .map(|(name, _)| *name)
            .collect()
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

/// Median of per-round HEAD/base ratios with a 95% percentile-bootstrap CI.
/// Pairing by round cancels drift that affects both binaries alike.
fn ratio_with_ci(base: &[f64], head: &[f64]) -> (f64, f64, f64) {
    let ratios = base
        .iter()
        .zip(head)
        .map(|(base, head)| head / base)
        .collect::<Vec<_>>();
    let point = median(&mut ratios.clone());
    // xorshift64*: deterministic, dependency-free resampling.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    let mut medians = (0..2_000)
        .map(|_| {
            let mut resample = (0..ratios.len())
                .map(|_| ratios[(next() % ratios.len() as u64) as usize])
                .collect::<Vec<_>>();
            median(&mut resample)
        })
        .collect::<Vec<_>>();
    medians.sort_by(f64::total_cmp);
    (point, medians[50], medians[1_949])
}

fn format_ns(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else {
        format!("{:.1} µs", ns / 1e3)
    }
}

fn format_sample(scenario: Scenario, ns: f64) -> String {
    match scenario.throughput_bytes() {
        Some(bytes) => format!(
            "{} ({:.0} MiB/s)",
            format_ns(ns),
            bytes as f64 / (1024.0 * 1024.0) / (ns / 1e9)
        ),
        None => format_ns(ns),
    }
}

fn print_paired_table(results: &[PairedResult]) -> Verdicts {
    let mut verdicts = Vec::new();
    println!();
    println!("| Scenario | base (median) | HEAD (median) | Δ median | 95% CI | Verdict |");
    println!("|---|---|---|---|---|---|");
    for result in results {
        let (ratio, lower, upper) = ratio_with_ci(&result.base, &result.head);
        let (change, lower, upper) = (ratio - 1.0, lower - 1.0, upper - 1.0);
        let verdict = Verdict::from_change_ci(lower, upper);
        verdicts.push((result.scenario.name(), verdict));
        println!(
            "| `{}` | {} | {} | {:+.1}% | [{:+.1}%, {:+.1}%] | {} |",
            result.scenario.name(),
            format_sample(result.scenario, median(&mut result.base.clone())),
            format_sample(result.scenario, median(&mut result.head.clone())),
            change * 100.0,
            lower * 100.0,
            upper * 100.0,
            verdict.label(),
        );
        if let (Some(base), Some(head)) = (result.base_doh_per_burst, result.head_doh_per_burst) {
            println!(
                "| `{}` DoH queries per burst | {base:.2} | {head:.2} | | | |",
                result.scenario.name()
            );
        }
    }
    Verdicts(verdicts)
}

fn write_paired_json(results: &[PairedResult]) {
    let scenarios = results
        .iter()
        .map(|result| {
            let (ratio, lower, upper) = ratio_with_ci(&result.base, &result.head);
            (
                result.scenario.name().to_owned(),
                serde_json::json!({
                    "base_median_ns": median(&mut result.base.clone()),
                    "head_median_ns": median(&mut result.head.clone()),
                    "ratio": ratio,
                    "ratio_ci95": [lower, upper],
                    "base_doh_queries_per_burst": result.base_doh_per_burst,
                    "head_doh_queries_per_burst": result.head_doh_per_burst,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let path = criterion_home().join("paired.json");
    fs::create_dir_all(criterion_home()).expect("create criterion home");
    fs::write(
        &path,
        serde_json::to_string_pretty(&scenarios).expect("encode paired results"),
    )
    .expect("write paired results");
    eprintln!("wrote {}", path.display());
}
