#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{RData, Record, RecordType};
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, KeyUsagePurpose};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

pub const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const KEEP_ALIVE_IDLE: Duration = Duration::from_secs(120);
#[derive(Clone, Debug)]
pub enum DohResponse {
    Addresses(Vec<Ipv4Addr>),
    Raw(Vec<u8>),
    HttpStatus(u16),
}

/// Network model for [`TestDoh`]. The defaults match the original test server:
/// one request per connection, no added latency, 30 second answer TTL.
#[derive(Clone, Debug)]
pub struct DohOptions {
    pub response: DohResponse,
    /// Serve several requests per TLS connection, as public DoH servers do.
    pub keep_alive: bool,
    /// Simulated round-trip time: a new connection waits `2 * rtt` (TCP + TLS 1.3)
    /// before the handshake and each query waits `rtt` before its answer.
    pub rtt: Duration,
    pub ttl: u32,
}

impl DohOptions {
    pub fn new(response: DohResponse) -> Self {
        Self {
            response,
            keep_alive: false,
            rtt: Duration::ZERO,
            ttl: 30,
        }
    }
}

pub struct TestDoh {
    pub addr: SocketAddr,
    pub ca_pem: PathBuf,
    queries: Arc<Mutex<Vec<Message>>>,
    response: Arc<Mutex<DohResponse>>,
    task: JoinHandle<()>,
    directory: TempDir,
}

impl TestDoh {
    pub async fn start(response: DohResponse) -> Self {
        Self::start_with(DohOptions::new(response)).await
    }

    pub async fn start_with(options: DohOptions) -> Self {
        let directory = tempfile::tempdir().expect("create DoH certificate directory");
        let material = tls_material(directory.path(), &["localhost"]);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind HTTPS DoH listener");
        let addr = listener.local_addr().expect("inspect DoH listener");
        let queries = Arc::new(Mutex::new(Vec::new()));
        let response = Arc::new(Mutex::new(options.response.clone()));
        let task_queries = Arc::clone(&queries);
        let task_response = Arc::clone(&response);
        let acceptor = TlsAcceptor::from(Arc::new(material.server));
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let acceptor = acceptor.clone();
                let queries = Arc::clone(&task_queries);
                let response = Arc::clone(&task_response);
                let options = options.clone();
                tokio::spawn(async move {
                    sleep_rtt(options.rtt * 2).await;
                    let Ok(mut stream) = acceptor.accept(stream).await else {
                        return;
                    };
                    let idle = if options.keep_alive {
                        KEEP_ALIVE_IDLE
                    } else {
                        TEST_TIMEOUT
                    };
                    loop {
                        let Ok(request) = read_http_request(&mut stream, idle).await else {
                            return;
                        };
                        let Some(query) = parse_doh_request(&request) else {
                            return;
                        };
                        queries.lock().expect("query lock").push(query.clone());
                        sleep_rtt(options.rtt).await;
                        let configured = response.lock().expect("response lock").clone();
                        let (status, content_type, body) = match configured {
                            DohResponse::Addresses(addresses) => {
                                let mut dns = Message::new(
                                    query.metadata.id,
                                    MessageType::Response,
                                    OpCode::Query,
                                );
                                dns.add_query(query.queries[0].clone());
                                for address in addresses {
                                    dns.add_answer(Record::from_rdata(
                                        query.queries[0].name().clone(),
                                        options.ttl,
                                        RData::A(A(address)),
                                    ));
                                }
                                let body = dns.to_vec().expect("encode test DNS response");
                                (200, "application/dns-message", body)
                            }
                            DohResponse::Raw(body) => (200, "application/dns-message", body),
                            DohResponse::HttpStatus(status) => (status, "text/plain", Vec::new()),
                        };
                        write_http_response(
                            &mut stream,
                            status,
                            content_type,
                            &body,
                            options.keep_alive,
                        )
                        .await;
                        if !options.keep_alive {
                            return;
                        }
                    }
                });
            }
        });
        Self {
            addr,
            ca_pem: material.ca_pem,
            queries,
            response,
            task,
            directory,
        }
    }

    pub fn url(&self) -> String {
        format!("https://localhost:{}/dns-query", self.addr.port())
    }

    pub fn queries(&self) -> Vec<Message> {
        self.queries.lock().expect("query lock").clone()
    }

    /// Lower-case names of every query received, without the trailing dot.
    pub fn queried_names(&self) -> Vec<String> {
        self.queries()
            .iter()
            .flat_map(|message| message.queries.iter())
            .map(|query| {
                query
                    .name()
                    .to_ascii()
                    .trim_end_matches('.')
                    .to_ascii_lowercase()
            })
            .collect()
    }

    pub async fn wait_for_queries(&self, count: usize) -> Vec<Message> {
        timeout(TEST_TIMEOUT, async {
            loop {
                let queries = self.queries();
                if queries.len() >= count {
                    return queries;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        })
        .await
        .expect("timed out waiting for DoH query")
    }

    pub fn set_response(&self, response: DohResponse) {
        *self.response.lock().expect("response lock") = response;
    }
}

impl Drop for TestDoh {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct TestOrigin {
    pub addr: SocketAddr,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
    active: Arc<AtomicUsize>,
    peak_active: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl TestOrigin {
    pub async fn start_ipv4(reply: Vec<u8>) -> Self {
        Self::start_on(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), reply).await
    }

    pub async fn start_ipv6(port: u16, reply: Vec<u8>) -> Option<Self> {
        Self::try_start_on(SocketAddr::from((Ipv6Addr::LOCALHOST, port)), reply).await
    }

    async fn start_on(address: SocketAddr, reply: Vec<u8>) -> Self {
        Self::try_start_on(address, reply)
            .await
            .expect("bind test origin")
    }

    async fn try_start_on(address: SocketAddr, reply: Vec<u8>) -> Option<Self> {
        let listener = TcpListener::bind(address).await.ok()?;
        let addr = listener.local_addr().ok()?;
        let received = Arc::new(Mutex::new(Vec::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let peak_active = Arc::new(AtomicUsize::new(0));
        let task_received = Arc::clone(&received);
        let task_active = Arc::clone(&active);
        let task_peak = Arc::clone(&peak_active);
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let received = Arc::clone(&task_received);
                let reply = reply.clone();
                let active = Arc::clone(&task_active);
                let peak = Arc::clone(&task_peak);
                tokio::spawn(async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    let mut bytes = Vec::new();
                    let _ = timeout(TEST_TIMEOUT, stream.read_to_end(&mut bytes)).await;
                    received.lock().expect("origin receive lock").push(bytes);
                    let _ = stream.write_all(&reply).await;
                    let _ = stream.shutdown().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Some(Self {
            addr,
            received,
            active,
            peak_active,
            task,
        })
    }

    pub async fn wait_for_connections(&self, count: usize) -> Vec<Vec<u8>> {
        timeout(TEST_TIMEOUT, async {
            loop {
                let received = self.received.lock().expect("origin receive lock").clone();
                if received.len() >= count {
                    return received;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        })
        .await
        .expect("timed out waiting for origin connection")
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    pub fn peak_active(&self) -> usize {
        self.peak_active.load(Ordering::SeqCst)
    }

    pub async fn wait_for_connections_count(&self, wait: Duration) -> usize {
        tokio::time::sleep(wait).await;
        self.received.lock().expect("origin receive lock").len()
    }
}

impl Drop for TestOrigin {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct RunningSplitHello {
    child: Child,
    pub proxy_addr: SocketAddr,
    pub pac_addr: Option<SocketAddr>,
    output: Arc<Mutex<String>>,
    directory: TempDir,
}

impl RunningSplitHello {
    pub fn start(config: TestConfig<'_>) -> Self {
        let configured_proxy = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let configured_pac = config
            .with_pac
            .then_some(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)));
        // Tests assert on every tunnel outcome, including DEBUG-level direct relays.
        Self::spawn(
            Path::new(env!("CARGO_BIN_EXE_splithello")),
            &config.render(configured_proxy, configured_pac),
            config.with_pac,
            "debug",
        )
    }

    /// Starts the compiled binary with literal configuration text whose listeners
    /// use kernel-assigned loopback ports.
    pub fn start_with_config(config_text: &str, with_pac: bool) -> Self {
        Self::start_binary(
            Path::new(env!("CARGO_BIN_EXE_splithello")),
            config_text,
            with_pac,
        )
    }

    /// Like [`Self::start_with_config`] for an arbitrary build, e.g. a base-ref binary
    /// in a paired benchmark. Logs at the default `info` level, as a user would run it.
    pub fn start_binary(binary: &Path, config_text: &str, with_pac: bool) -> Self {
        Self::spawn(binary, config_text, with_pac, "info")
    }

    fn spawn(binary: &Path, config_text: &str, with_pac: bool, rust_log: &str) -> Self {
        let directory = tempfile::tempdir().expect("create process config directory");
        let config_path = directory.path().join("config.toml");
        fs::write(&config_path, config_text).expect("write process configuration");

        let mut child = Command::new(binary)
            .arg("start")
            .arg("--config")
            .arg(&config_path)
            .env("RUST_LOG", rust_log)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start compiled splithello child");
        let output = Arc::new(Mutex::new(String::new()));
        capture_pipe(
            child.stdout.take().expect("child stdout"),
            Arc::clone(&output),
        );
        capture_pipe(
            child.stderr.take().expect("child stderr"),
            Arc::clone(&output),
        );
        let ready_output = wait_for_captured_output(&output, "outcome=ready");
        let proxy_addr = parse_ready_socket(&ready_output, "proxy_addr=")
            .expect("ready output contains actual proxy address");
        let pac_addr = parse_ready_socket(&ready_output, "pac_url=\"http://");
        assert_eq!(pac_addr.is_some(), with_pac);
        if let Some(pac_addr) = pac_addr {
            assert_ne!(
                proxy_addr, pac_addr,
                "kernel-assigned proxy and PAC binds must differ"
            );
        }
        Self {
            child,
            proxy_addr,
            pac_addr,
            output,
            directory,
        }
    }

    pub fn output(&self) -> String {
        self.output.lock().expect("output lock").clone()
    }

    pub fn directory_path(&self) -> &Path {
        self.directory.path()
    }

    pub fn wait_for_output(&self, needle: &str) -> String {
        wait_for_captured_output(&self.output, needle)
    }

    #[cfg(unix)]
    pub fn signal(&mut self, signal: &str) {
        let status = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(self.child.id().to_string())
            .status()
            .expect("send child signal");
        assert!(status.success(), "kill command failed");
    }

    pub fn wait_for_exit(&mut self, deadline: Duration) -> std::process::ExitStatus {
        let end = Instant::now() + deadline;
        loop {
            if let Some(status) = self.child.try_wait().expect("inspect child status") {
                return status;
            }
            assert!(Instant::now() < end, "child did not exit before deadline");
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for RunningSplitHello {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

pub struct TestConfig<'a> {
    pub doh: &'a TestDoh,
    pub origin_port: u16,
    pub target_host: &'a str,
    pub with_pac: bool,
    pub max_connections: usize,
    pub shutdown_seconds: u64,
    pub injected_secret: Option<&'a str>,
}

impl TestConfig<'_> {
    pub fn render(&self, proxy: SocketAddr, pac: Option<SocketAddr>) -> String {
        let pac = pac.map_or_else(|| "false".to_owned(), |addr| format!("\"{addr}\""));
        let secret = self
            .injected_secret
            .map(|secret| format!("# {secret}\n"))
            .unwrap_or_default();
        format!(
            "{secret}targets = [{{ host = \"{}\", subdomains = true }}]\n\n[proxy]\nlisten = \"{proxy}\"\npac = {pac}\nports = [{}]\n\n[dns]\nupstream = \"{}\"\nca_certificate = \"{}\"\ntimeout = \"1s\"\ncache_ttl = \"30s\"\ncache_size = 8\nmax_response_bytes = 4096\n\n[limits]\nmax_connect_header_bytes = 4096\nmax_client_hello_bytes = 4096\nmax_connections = {}\nconnect_timeout = \"1s\"\nclient_hello_timeout = \"1s\"\nidle_timeout = \"2s\"\nshutdown_timeout = \"{}s\"\n",
            self.target_host,
            self.origin_port,
            self.doh.url(),
            self.doh.ca_pem.display(),
            self.max_connections,
            self.shutdown_seconds,
        )
    }
}

pub async fn connect_via_proxy(proxy: SocketAddr, authority: &str) -> TcpStream {
    let mut stream = open_proxy_request(proxy, authority).await;
    let response = read_http_headers(&mut stream).await;
    assert!(
        response.starts_with(b"HTTP/1.1 200"),
        "CONNECT failed: {}",
        String::from_utf8_lossy(&response)
    );
    stream
}

pub async fn request_via_proxy(proxy: SocketAddr, authority: &str) -> Vec<u8> {
    let mut stream = open_proxy_request(proxy, authority).await;
    read_http_headers(&mut stream).await
}

async fn open_proxy_request(proxy: SocketAddr, authority: &str) -> TcpStream {
    let mut stream = timeout(TEST_TIMEOUT, TcpStream::connect(proxy))
        .await
        .expect("proxy connect timeout")
        .expect("connect to proxy");
    let request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n");
    timeout(TEST_TIMEOUT, stream.write_all(request.as_bytes()))
        .await
        .expect("CONNECT write timeout")
        .expect("write CONNECT request");
    stream
}

pub async fn fetch_pac(address: SocketAddr) -> Vec<u8> {
    let mut stream = timeout(TEST_TIMEOUT, TcpStream::connect(address))
        .await
        .expect("PAC connect timeout")
        .expect("connect to PAC");
    stream
        .write_all(b"GET /proxy.pac HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write PAC request");
    let mut response = Vec::new();
    timeout(TEST_TIMEOUT, stream.read_to_end(&mut response))
        .await
        .expect("PAC read timeout")
        .expect("read PAC response");
    response
}

pub fn build_client_hello(sni: &str, splits: &[usize]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&[0x11; 32]);
    body.push(0);
    body.extend_from_slice(&2_u16.to_be_bytes());
    body.extend_from_slice(&[0x13, 0x01]);
    body.push(1);
    body.push(0);
    let mut server_name = vec![0];
    server_name.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    server_name.extend_from_slice(sni.as_bytes());
    let mut extension = Vec::new();
    extension.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
    extension.extend_from_slice(&server_name);
    let mut extensions = Vec::new();
    extensions.extend_from_slice(&0_u16.to_be_bytes());
    extensions.extend_from_slice(&(extension.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&extension);
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);
    let mut handshake = vec![1];
    let length = body.len();
    handshake.extend_from_slice(&[
        ((length >> 16) & 0xff) as u8,
        ((length >> 8) & 0xff) as u8,
        (length & 0xff) as u8,
    ]);
    handshake.extend_from_slice(&body);
    encode_records(&handshake, splits)
}

pub fn decode_handshake_records(mut bytes: &[u8], count: usize) -> (Vec<u8>, usize) {
    let mut handshake = Vec::new();
    let mut consumed = 0;
    for _ in 0..count {
        assert!(bytes.len() >= 5, "missing TLS record header");
        assert_eq!(bytes[0], 22, "expected handshake record");
        let length = usize::from(u16::from_be_bytes([bytes[3], bytes[4]]));
        assert!(length <= 16_384);
        assert!(bytes.len() >= 5 + length, "truncated TLS record");
        handshake.extend_from_slice(&bytes[5..5 + length]);
        consumed += 5 + length;
        bytes = &bytes[5 + length..];
    }
    (handshake, consumed)
}

fn encode_records(handshake: &[u8], splits: &[usize]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut start = 0;
    for end in splits
        .iter()
        .copied()
        .chain(std::iter::once(handshake.len()))
    {
        let payload = &handshake[start..end];
        output.extend_from_slice(&[22, 0x03, 0x03]);
        output.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        output.extend_from_slice(payload);
        start = end;
    }
    output
}

pub struct TlsMaterial {
    pub ca_pem: PathBuf,
    pub ca_der: CertificateDer<'static>,
    pub server: ServerConfig,
}

/// Generates a throwaway CA plus a server certificate for `names`
/// (wildcards such as `*.bench.test` are allowed).
pub fn tls_material(directory: &Path, names: &[&str]) -> TlsMaterial {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let mut ca_params = CertificateParams::new(vec!["SplitHello test CA".to_owned()])
        .expect("create CA parameters");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca_key = KeyPair::generate().expect("generate CA key");
    let ca_cert = ca_params
        .clone()
        .self_signed(&ca_key)
        .expect("sign CA certificate");
    let ca_pem = directory.join("ca.pem");
    fs::write(&ca_pem, ca_cert.pem()).expect("write CA PEM");
    let ca_der = CertificateDer::from(ca_cert.der().to_vec());
    let issuer = Issuer::new(ca_params, ca_key);

    let names = names
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let server_params =
        CertificateParams::new(names).expect("create server certificate parameters");
    let server_key = KeyPair::generate().expect("generate server key");
    let server_cert = server_params
        .signed_by(&server_key, &issuer)
        .expect("sign server certificate");
    let cert_chain = vec![CertificateDer::from(server_cert.der().to_vec())];
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(server_key.serialize_der()));
    let server = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .expect("build test TLS server config");
    TlsMaterial {
        ca_pem,
        ca_der,
        server,
    }
}

async fn sleep_rtt(duration: Duration) {
    if !duration.is_zero() {
        tokio::time::sleep(duration).await;
    }
}

fn parse_doh_request(request: &[u8]) -> Option<Message> {
    let header_end = request
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .map(|p| p + 4)?;
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    if request.len() < header_end + content_length {
        return None;
    }
    Message::from_vec(&request[header_end..header_end + content_length]).ok()
}

async fn read_http_request<S>(stream: &mut S, idle: Duration) -> std::io::Result<Vec<u8>>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let mut request = Vec::new();
    loop {
        let mut buffer = [0_u8; 1024];
        let read = timeout(idle, stream.read(&mut buffer))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "HTTP request timeout")
            })??;
        if read == 0 {
            return Ok(request);
        }
        request.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = request
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .map(|p| p + 4)
        {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + content_length {
                return Ok(request);
            }
        }
    }
}

async fn write_http_response<S>(
    stream: &mut S,
    status: u16,
    content_type: &str,
    body: &[u8],
    keep_alive: bool,
) where
    S: tokio::io::AsyncWrite + Unpin,
{
    let reason = if status == 200 { "OK" } else { "Failure" };
    let connection = if keep_alive { "keep-alive" } else { "close" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: {connection}\r\n\r\n",
        body.len()
    );
    let mut response = head.into_bytes();
    response.extend_from_slice(body);
    let _ = stream.write_all(&response).await;
    if keep_alive {
        let _ = stream.flush().await;
    } else {
        let _ = stream.shutdown().await;
    }
}

async fn read_http_headers(stream: &mut TcpStream) -> Vec<u8> {
    timeout(TEST_TIMEOUT, async {
        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while !response.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .await
                .expect("read HTTP response");
            response.push(byte[0]);
        }
        response
    })
    .await
    .expect("HTTP response timeout")
}

fn wait_for_captured_output(output: &Arc<Mutex<String>>, needle: &str) -> String {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let captured = output.lock().expect("output lock").clone();
        if captured.contains(needle) {
            return captured;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for `{needle}`; output: {captured}"
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn parse_ready_socket(output: &str, prefix: &str) -> Option<SocketAddr> {
    let start = output.find(prefix)? + prefix.len();
    let value = output[start..]
        .split(|character: char| character.is_ascii_whitespace() || character == '/')
        .next()?;
    value.parse().ok()
}

fn capture_pipe(pipe: impl std::io::Read + Send + 'static, output: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        for line in BufReader::new(pipe).lines().map_while(Result::ok) {
            let mut captured = output.lock().expect("output lock");
            captured.push_str(&line);
            captured.push('\n');
        }
    });
}

#[tokio::test]
async fn authenticated_doh_requires_the_generated_ca() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let untrusted = reqwest::Client::builder()
        .use_rustls_tls()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("build untrusted client");
    assert!(
        untrusted
            .post(doh.url())
            .body(Vec::new())
            .send()
            .await
            .is_err()
    );

    let ca = fs::read(&doh.ca_pem).expect("read generated CA");
    let certificates = reqwest::tls::Certificate::from_pem_bundle(&ca).expect("parse generated CA");
    let trusted = reqwest::Client::builder()
        .use_rustls_tls()
        .no_proxy()
        .tls_certs_merge(certificates)
        .timeout(Duration::from_secs(1))
        .build()
        .expect("build trusted client");
    let response = trusted
        .post(doh.url())
        .header(reqwest::header::CONTENT_TYPE, "application/dns-message")
        .body(build_query_bytes("selected.test"))
        .send()
        .await
        .expect("generated CA authenticates localhost DoH");
    assert!(response.status().is_success());
    assert_eq!(doh.wait_for_queries(1).await.len(), 1);
}

fn build_query_bytes(host: &str) -> Vec<u8> {
    let mut message = Message::new(7, MessageType::Query, OpCode::Query);
    let name = hickory_proto::rr::Name::from_ascii(format!("{host}.")).expect("test query name");
    message.add_query(Query::query(name, RecordType::A));
    message.to_vec().expect("encode test query")
}
