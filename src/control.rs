//! Loopback control channel: how `splithello status` and `splithello stop` reach a running
//! instance on every OS, including Windows, where a detached process gets no Ctrl-C.
//!
//! One request per connection, one line each way:
//!
//! ```text
//! STATUS <token>\n  →  OK pid=4242 version=0.1.0 active=2 split=120 direct=340 failed=3\n
//! STOP <token>\n    →  OK drain_ms=10000\n   then the instance shuts down as on Ctrl-C
//! anything else     →  ERR unauthorized | bad-request | unknown-command | too-long\n
//! ```
//!
//! The token lives only in the owner-only `run.toml`; a web page can reach the port but can
//! neither read the token nor speak this protocol from a browser.

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::diagnostics::ProxyStats;
use crate::update::CURRENT_VERSION;

const TOKEN_BYTES: usize = 32;
/// `STATUS ` + 64 hex digits + newline fits with room to spare.
const MAX_REQUEST_BYTES: usize = 128;
const MAX_REPLY_BYTES: usize = 512;
/// Per connection, for reading the request and for writing the reply.
const IO_TIMEOUT: Duration = Duration::from_secs(2);
/// Requests come from one person's commands; more at once means something else is knocking.
const MAX_CONNECTIONS: usize = 4;
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub(crate) enum ControlError {
    #[error("could not generate a control token")]
    Random(#[source] getrandom::Error),
    #[error("could not bind the control listener")]
    Bind(#[source] std::io::Error),
    #[error("could not reach the running instance at {address}")]
    Connect {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("the running instance did not answer within {}", humantime::format_duration(*.0))]
    Timeout(Duration),
    #[error("the running instance refused the request: {0}")]
    Rejected(String),
    #[error("the running instance sent an unexpected reply")]
    Protocol,
}

impl ControlError {
    /// Whether the instance could not be reached at all, as opposed to answering "no".
    pub(crate) const fn is_unreachable(&self) -> bool {
        matches!(
            self,
            Self::Connect { .. } | Self::Timeout(_) | Self::Protocol
        )
    }
}

/// The shared secret a control request must carry. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ControlToken(String);

impl ControlToken {
    pub(crate) fn generate() -> Result<Self, ControlError> {
        let mut bytes = [0_u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(ControlError::Random)?;
        Ok(Self(
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ))
    }

    fn parse(text: &str) -> Option<Self> {
        (text.len() == TOKEN_BYTES * 2
            && text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| Self(text.to_owned()))
    }

    /// For tests that check the token never leaks into output.
    #[cfg(test)]
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }

    /// Constant-time, so response timing does not reveal how much of a guess was right.
    fn matches(&self, presented: &[u8]) -> bool {
        let expected = self.0.as_bytes();
        expected.len() == presented.len()
            && std::hint::black_box(
                expected
                    .iter()
                    .zip(presented)
                    .fold(0_u8, |difference, (a, b)| difference | (a ^ b)),
            ) == 0
    }
}

impl fmt::Debug for ControlToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ControlToken(<redacted>)")
    }
}

impl Serialize for ControlToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ControlToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom("invalid control token"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verb {
    Status,
    Stop,
}

impl Verb {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "STATUS",
            Self::Stop => "STOP",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reject {
    TooLong,
    BadRequest,
    UnknownCommand,
    Unauthorized,
}

impl Reject {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TooLong => "too-long",
            Self::BadRequest => "bad-request",
            Self::UnknownCommand => "unknown-command",
            Self::Unauthorized => "unauthorized",
        }
    }
}

/// Parses `VERB SP TOKEN` (the newline already removed) and checks the token.
fn parse_request(line: &[u8], token: &ControlToken) -> Result<Verb, Reject> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let mut parts = line.split(|byte| *byte == b' ');
    let (Some(verb), Some(presented), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(Reject::BadRequest);
    };
    let verb = match verb {
        b"STATUS" => Verb::Status,
        b"STOP" => Verb::Stop,
        _ => return Err(Reject::UnknownCommand),
    };
    if !token.matches(presented) {
        return Err(Reject::Unauthorized);
    }
    Ok(verb)
}

/// Reads one line of at most `limit` bytes; a missing final newline at EOF is accepted.
async fn read_line(
    stream: &mut TcpStream,
    limit: usize,
) -> std::io::Result<Result<Vec<u8>, Reject>> {
    let mut line = Vec::with_capacity(64);
    let mut chunk = [0_u8; 64];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(Ok(line));
        }
        if let Some(end) = chunk[..read].iter().position(|byte| *byte == b'\n') {
            line.extend_from_slice(&chunk[..end]);
            return Ok(if line.len() > limit {
                Err(Reject::TooLong)
            } else {
                Ok(line)
            });
        }
        line.extend_from_slice(&chunk[..read]);
        if line.len() > limit {
            return Ok(Err(Reject::TooLong));
        }
    }
}

pub(crate) struct ControlServer {
    listener: TcpListener,
    shared: Arc<Shared>,
}

struct Shared {
    token: ControlToken,
    stats: Arc<ProxyStats>,
    /// The runtime's root token: `STOP` cancels it, exactly as Ctrl-C does.
    shutdown: CancellationToken,
    drain: Duration,
}

impl ControlServer {
    pub(crate) fn new(
        listener: TcpListener,
        token: ControlToken,
        stats: Arc<ProxyStats>,
        shutdown: CancellationToken,
        drain: Duration,
    ) -> Self {
        Self {
            listener,
            shared: Arc::new(Shared {
                token,
                stats,
                shutdown,
                drain,
            }),
        }
    }

    /// Serves until the runtime shuts down. Never fails: a broken control channel must not
    /// take the proxy down with it, so accept errors are logged and retried.
    pub(crate) async fn run(self) {
        let mut connections = JoinSet::new();
        loop {
            let accepted = tokio::select! {
                _ = self.shared.shutdown.cancelled() => break,
                _ = connections.join_next(), if !connections.is_empty() => continue,
                accepted = self.listener.accept() => accepted,
            };
            match accepted {
                Ok((stream, _)) if connections.len() < MAX_CONNECTIONS => {
                    connections.spawn(serve(stream, Arc::clone(&self.shared)));
                }
                Ok(_) => tracing::debug!("control connection dropped: too many at once"),
                Err(error) => {
                    tracing::warn!(error = %error, "control listener accept failed");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            }
        }
        connections.shutdown().await;
    }
}

async fn serve(mut stream: TcpStream, shared: Arc<Shared>) {
    let request = match timeout(IO_TIMEOUT, read_line(&mut stream, MAX_REQUEST_BYTES)).await {
        Ok(Ok(request)) => request,
        Ok(Err(_)) | Err(_) => return,
    };
    let verb = request.and_then(|line| parse_request(&line, &shared.token));
    let reply = match verb {
        Ok(Verb::Status) => {
            let counts = shared.stats.snapshot();
            format!(
                "OK pid={} version={CURRENT_VERSION} active={} split={} direct={} failed={}\n",
                std::process::id(),
                counts.active,
                counts.split,
                counts.direct,
                counts.failed,
            )
        }
        Ok(Verb::Stop) => format!("OK drain_ms={}\n", shared.drain.as_millis()),
        Err(reject) => {
            if reject == Reject::Unauthorized {
                tracing::warn!("control request with a wrong token refused");
            }
            format!("ERR {}\n", reject.as_str())
        }
    };
    let _ = timeout(IO_TIMEOUT, stream.write_all(reply.as_bytes())).await;
    if verb == Ok(Verb::Stop) {
        tracing::info!("stop requested by `splithello stop`");
        shared.shutdown.cancel();
    }
}

/// The `key=value` pairs of an `OK` reply. Unknown keys are kept, so newer instances can add
/// fields without breaking older clients.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct Reply(BTreeMap<String, String>);

impl Reply {
    pub(crate) fn parse(line: &str) -> Result<Self, ControlError> {
        if let Some(reason) = line.strip_prefix("ERR ") {
            return Err(ControlError::Rejected(reason.to_owned()));
        }
        let fields = line
            .strip_prefix("OK")
            .ok_or(ControlError::Protocol)?
            .split_whitespace()
            .map(|pair| {
                pair.split_once('=')
                    .map(|(key, value)| (key.to_owned(), value.to_owned()))
                    .ok_or(ControlError::Protocol)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self(fields))
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub(crate) fn number(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|value| value.parse().ok())
    }
}

/// Sends one request to the instance at `address` and returns its reply.
pub(crate) async fn request(
    address: SocketAddr,
    token: &ControlToken,
    verb: Verb,
) -> Result<Reply, ControlError> {
    let exchange = async {
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|source| ControlError::Connect { address, source })?;
        let line = format!("{} {}\n", verb.as_str(), token.0);
        let protocol = |_| ControlError::Protocol;
        stream.write_all(line.as_bytes()).await.map_err(protocol)?;
        let reply = read_line(&mut stream, MAX_REPLY_BYTES)
            .await
            .map_err(protocol)?
            .map_err(|_| ControlError::Protocol)?;
        let reply = String::from_utf8(reply).map_err(|_| ControlError::Protocol)?;
        Reply::parse(reply.trim_end())
    };
    let deadline = IO_TIMEOUT * 2;
    timeout(deadline, exchange)
        .await
        .map_err(|_| ControlError::Timeout(deadline))?
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinHandle;
    use tokio_util::sync::CancellationToken;

    use crate::diagnostics::{Outcome, ProxyStats};

    use super::{
        ControlError, ControlServer, ControlToken, Reject, Reply, Verb, parse_request, request,
    };

    fn token() -> ControlToken {
        ControlToken::generate().unwrap()
    }

    #[test]
    fn tokens_are_random_hex_and_never_debug_printed() {
        let (a, b) = (token(), token());
        assert_ne!(a, b);
        assert_eq!(a.0.len(), 64);
        assert!(ControlToken::parse(&a.0).is_some());
        assert!(!format!("{a:?}").contains(&a.0));
        assert!(ControlToken::parse("abc").is_none());
        assert!(ControlToken::parse(&"G".repeat(64)).is_none());
    }

    #[test]
    fn token_comparison_needs_an_exact_match() {
        let token = token();
        assert!(token.matches(token.0.as_bytes()));
        assert!(!token.matches(&token.0.as_bytes()[1..]));
        let mut wrong = token.0.clone().into_bytes();
        wrong[63] = if wrong[63] == b'0' { b'1' } else { b'0' };
        assert!(!token.matches(&wrong));
        assert!(!token.matches(b""));
    }

    #[test]
    fn requests_parse_strictly() {
        let token = token();
        let line = |text: String| parse_request(text.as_bytes(), &token);
        assert_eq!(line(format!("STATUS {}", token.0)), Ok(Verb::Status));
        assert_eq!(line(format!("STOP {}\r", token.0)), Ok(Verb::Stop));
        assert_eq!(
            line(format!("RESTART {}", token.0)),
            Err(Reject::UnknownCommand)
        );
        assert_eq!(
            line(format!("stop {}", token.0)),
            Err(Reject::UnknownCommand)
        );
        assert_eq!(line("STOP".to_owned()), Err(Reject::BadRequest));
        assert_eq!(
            line(format!("STOP {} extra", token.0)),
            Err(Reject::BadRequest)
        );
        assert_eq!(line(format!("STOP  {}", token.0)), Err(Reject::BadRequest));
        assert_eq!(
            line(format!("STOP {}", "0".repeat(64))),
            Err(Reject::Unauthorized)
        );
    }

    #[test]
    fn replies_parse_ok_and_err() {
        let reply = Reply::parse("OK pid=7 active=2 future=yes").unwrap();
        assert_eq!(reply.number("pid"), Some(7));
        assert_eq!(reply.get("future"), Some("yes"));
        assert!(matches!(
            Reply::parse("ERR unauthorized"),
            Err(ControlError::Rejected(reason)) if reason == "unauthorized"
        ));
        assert!(matches!(Reply::parse("HELLO"), Err(ControlError::Protocol)));
        assert!(matches!(
            Reply::parse("OK novalue"),
            Err(ControlError::Protocol)
        ));
    }

    struct Running {
        address: std::net::SocketAddr,
        token: ControlToken,
        stats: Arc<ProxyStats>,
        shutdown: CancellationToken,
        task: JoinHandle<()>,
    }

    async fn serve() -> Running {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let (token, stats, shutdown) = (
            token(),
            Arc::new(ProxyStats::default()),
            CancellationToken::new(),
        );
        let server = ControlServer::new(
            listener,
            token.clone(),
            Arc::clone(&stats),
            shutdown.clone(),
            Duration::from_secs(3),
        );
        Running {
            address,
            token,
            stats,
            shutdown,
            task: tokio::spawn(server.run()),
        }
    }

    #[tokio::test]
    async fn status_reports_live_counts_without_stopping() {
        let running = serve().await;
        running.stats.opened();
        running.stats.opened();
        running.stats.closed(Outcome::TargetTransformed);
        let reply = request(running.address, &running.token, Verb::Status)
            .await
            .unwrap();
        assert_eq!(reply.number("pid"), Some(u64::from(std::process::id())));
        assert_eq!(reply.get("version"), Some(crate::update::CURRENT_VERSION));
        assert_eq!(reply.number("active"), Some(1));
        assert_eq!(reply.number("split"), Some(1));
        assert!(!running.shutdown.is_cancelled());
        running.shutdown.cancel();
        running.task.await.unwrap();
    }

    #[tokio::test]
    async fn stop_replies_with_the_drain_then_cancels() {
        let running = serve().await;
        let reply = request(running.address, &running.token, Verb::Stop)
            .await
            .unwrap();
        assert_eq!(reply.number("drain_ms"), Some(3_000));
        tokio::time::timeout(Duration::from_secs(1), running.shutdown.cancelled())
            .await
            .expect("STOP cancels the runtime");
        tokio::time::timeout(Duration::from_secs(1), running.task)
            .await
            .expect("server exits after STOP")
            .unwrap();
    }

    #[tokio::test]
    async fn a_wrong_token_is_refused_and_does_not_stop() {
        let running = serve().await;
        let error = request(running.address, &token(), Verb::Stop)
            .await
            .unwrap_err();
        assert!(matches!(error, ControlError::Rejected(ref reason) if reason == "unauthorized"));
        assert!(!error.is_unreachable());
        assert!(!running.shutdown.is_cancelled());
        running.shutdown.cancel();
    }

    #[tokio::test]
    async fn oversized_requests_are_refused() {
        let running = serve().await;
        let mut stream = TcpStream::connect(running.address).await.unwrap();
        // One byte over the limit, all of it read before the refusal: closing a socket with
        // unread input resets it, which would hide the reply.
        let mut request = vec![b'A'; super::MAX_REQUEST_BYTES + 1];
        request.push(b'\n');
        stream.write_all(&request).await.unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).await.unwrap();
        assert_eq!(reply, "ERR too-long\n");
        running.shutdown.cancel();
    }

    #[tokio::test]
    async fn idle_clients_do_not_block_others() {
        let running = serve().await;
        let mut idle = Vec::new();
        for _ in 0..3 {
            idle.push(TcpStream::connect(running.address).await.unwrap());
        }
        let reply = request(running.address, &running.token, Verb::Status)
            .await
            .unwrap();
        assert!(reply.number("pid").is_some());
        running.shutdown.cancel();
    }

    #[tokio::test]
    async fn an_unreachable_instance_is_reported_as_such() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = request(address, &token(), Verb::Status).await.unwrap_err();
        assert!(matches!(error, ControlError::Connect { .. }));
        assert!(error.is_unreachable());
    }
}
