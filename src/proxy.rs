#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant as StdInstant};

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::diagnostics::{Outcome, TunnelReport};
use crate::dns::{ResolveError, SystemResolver, TargetResolver};
use crate::domain::{DomainError, DomainName, TargetMatcher};
use crate::tls::RewriteOutcome;

const MAX_CONNECT_HEADER_BYTES: usize = 65_536;
const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConnectRequest {
    host: DomainName,
    port: u16,
}

impl ConnectRequest {
    pub(crate) fn host(&self) -> &DomainName {
        &self.host
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ProxyError {
    #[error("CONNECT header exceeds the configured limit")]
    HeaderTooLarge,
    #[error("CONNECT header is incomplete")]
    HeaderIncomplete,
    #[error("request line is malformed")]
    MalformedRequestLine,
    #[error("only CONNECT requests are supported")]
    UnsupportedMethod,
    #[error("only HTTP/1.1 CONNECT requests are supported")]
    UnsupportedVersion,
    #[error("CONNECT authority must be a hostname and port")]
    MalformedAuthority,
    #[error("IP-form CONNECT authorities are not supported")]
    IpAuthority,
    #[error("CONNECT port is not allowed")]
    PortNotAllowed,
    #[error("header syntax is malformed")]
    MalformedHeader,
    #[error("CONNECT request bodies are not supported")]
    RequestBodyNotAllowed,
}

impl ProxyError {
    pub(crate) const fn status_code(self) -> u16 {
        match self {
            Self::UnsupportedMethod => 405,
            Self::PortNotAllowed => 403,
            Self::HeaderTooLarge | Self::HeaderIncomplete => 431,
            Self::MalformedRequestLine
            | Self::UnsupportedVersion
            | Self::MalformedAuthority
            | Self::IpAuthority
            | Self::MalformedHeader
            | Self::RequestBodyNotAllowed => 400,
        }
    }
}

pub(crate) fn parse_connect(
    header: &[u8],
    allowed_ports: &BTreeSet<u16>,
) -> Result<ConnectRequest, ProxyError> {
    if header.len() > MAX_CONNECT_HEADER_BYTES {
        return Err(ProxyError::HeaderTooLarge);
    }
    if has_invalid_line_endings(header) {
        return Err(ProxyError::MalformedHeader);
    }

    let terminator = header
        .windows(HEADER_TERMINATOR.len())
        .position(|window| window == HEADER_TERMINATOR)
        .ok_or(ProxyError::HeaderIncomplete)?;
    let header_end = terminator + HEADER_TERMINATOR.len();
    if header_end != header.len() {
        return Err(ProxyError::RequestBodyNotAllowed);
    }

    let text =
        std::str::from_utf8(&header[..terminator]).map_err(|_| ProxyError::MalformedRequestLine)?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(ProxyError::MalformedRequestLine)?;
    let request = parse_request_line(request_line, allowed_ports)?;

    for line in lines {
        parse_header_line(line)?;
    }

    Ok(request)
}

fn has_invalid_line_endings(header: &[u8]) -> bool {
    header.iter().enumerate().any(|(index, byte)| match *byte {
        b'\n' => index == 0 || header[index - 1] != b'\r',
        b'\r' => header.get(index + 1).is_some_and(|next| *next != b'\n'),
        _ => false,
    })
}

fn parse_request_line(
    request_line: &str,
    allowed_ports: &BTreeSet<u16>,
) -> Result<ConnectRequest, ProxyError> {
    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or(ProxyError::MalformedRequestLine)?;
    let authority = parts.next().ok_or(ProxyError::MalformedRequestLine)?;
    let version = parts.next().ok_or(ProxyError::MalformedRequestLine)?;
    if parts.next().is_some() || method.is_empty() || authority.is_empty() || version.is_empty() {
        return Err(ProxyError::MalformedRequestLine);
    }
    if method != "CONNECT" {
        return Err(ProxyError::UnsupportedMethod);
    }
    if version != "HTTP/1.1" {
        return Err(ProxyError::UnsupportedVersion);
    }

    parse_authority(authority, allowed_ports)
}

fn parse_authority(
    authority: &str,
    allowed_ports: &BTreeSet<u16>,
) -> Result<ConnectRequest, ProxyError> {
    if authority.contains(['@', '/', '\\', '?', '#']) {
        return Err(ProxyError::MalformedAuthority);
    }
    if authority.starts_with('[') || authority.ends_with(']') {
        return Err(ProxyError::IpAuthority);
    }

    let (host, port) = authority
        .rsplit_once(':')
        .ok_or(ProxyError::MalformedAuthority)?;
    if host.is_empty() || host.contains(':') {
        return Err(ProxyError::MalformedAuthority);
    }
    if host.parse::<IpAddr>().is_ok() {
        return Err(ProxyError::IpAuthority);
    }

    let port = port
        .parse::<u16>()
        .map_err(|_| ProxyError::MalformedAuthority)?;
    if port == 0 {
        return Err(ProxyError::MalformedAuthority);
    }
    let host = DomainName::parse(host).map_err(map_domain_error)?;
    if !allowed_ports.contains(&port) {
        return Err(ProxyError::PortNotAllowed);
    }

    Ok(ConnectRequest { host, port })
}

fn map_domain_error(error: DomainError) -> ProxyError {
    match error {
        DomainError::IpLiteral { .. } => ProxyError::IpAuthority,
        DomainError::Empty
        | DomainError::InvalidIdna { .. }
        | DomainError::InvalidLength { .. }
        | DomainError::Duplicate { .. } => ProxyError::MalformedAuthority,
    }
}

fn parse_header_line(line: &str) -> Result<(), ProxyError> {
    let (name, value) = line.split_once(':').ok_or(ProxyError::MalformedHeader)?;
    if name.is_empty() || !name.bytes().all(is_header_name_byte) {
        return Err(ProxyError::MalformedHeader);
    }
    if value
        .bytes()
        .any(|byte| (byte < b' ' && byte != b'\t') || byte == 0x7f)
    {
        return Err(ProxyError::MalformedHeader);
    }

    if name.eq_ignore_ascii_case("transfer-encoding") {
        return Err(ProxyError::RequestBodyNotAllowed);
    }
    if name.eq_ignore_ascii_case("content-length") {
        return Err(ProxyError::RequestBodyNotAllowed);
    }

    Ok(())
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

impl fmt::Display for ConnectRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProxyLimits {
    pub(crate) max_connect_header_bytes: usize,
    pub(crate) max_client_hello_bytes: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) client_hello_timeout: Duration,
    pub(crate) idle_timeout: Duration,
}

pub(crate) trait DirectResolver: Send + Sync {
    fn resolve<'a>(
        &'a self,
        host: &'a DomainName,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, ResolveError>> + Send + 'a>>;
}

impl DirectResolver for SystemResolver {
    fn resolve<'a>(
        &'a self,
        host: &'a DomainName,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, ResolveError>> + Send + 'a>> {
        Box::pin(SystemResolver::resolve(host, port))
    }
}

pub(crate) struct ProxyState {
    matcher: TargetMatcher,
    target_resolver: Arc<dyn TargetResolver>,
    direct_resolver: Arc<dyn DirectResolver>,
    allowed_ports: BTreeSet<u16>,
    limits: ProxyLimits,
    concurrency: Arc<Semaphore>,
}

pub(crate) struct ProxyServer {
    listener: TcpListener,
    state: Arc<ProxyState>,
    cancel: CancellationToken,
    shutdown_timeout: Duration,
}

#[derive(Debug, Error)]
pub(crate) enum ProxyServiceError {
    #[error("proxy listener accept failed")]
    Accept(#[source] std::io::Error),
}

impl ProxyServer {
    pub(crate) fn new(
        listener: TcpListener,
        state: Arc<ProxyState>,
        cancel: CancellationToken,
        shutdown_timeout: Duration,
    ) -> Self {
        Self {
            listener,
            state,
            cancel,
            shutdown_timeout,
        }
    }

    pub(crate) async fn run(self) -> Result<(), ProxyServiceError> {
        let mut children = JoinSet::new();
        let result = self.accept_loop(&mut children).await;
        self.cancel.cancel();
        drain_proxy_children(&mut children, self.shutdown_timeout).await;
        result
    }

    async fn accept_loop(&self, children: &mut JoinSet<()>) -> Result<(), ProxyServiceError> {
        loop {
            let permit = tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                joined = children.join_next(), if !children.is_empty() => {
                    log_proxy_child(joined);
                    continue;
                }
                permit = Arc::clone(&self.state.concurrency).acquire_owned() => {
                    match permit {
                        Ok(permit) => permit,
                        Err(_) => return Ok(()),
                    }
                }
            };

            let accepted = tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                joined = children.join_next(), if !children.is_empty() => {
                    log_proxy_child(joined);
                    drop(permit);
                    continue;
                }
                accepted = self.listener.accept() => accepted,
            };
            let (client, _) = accepted.map_err(ProxyServiceError::Accept)?;
            let state = Arc::clone(&self.state);
            let cancel = self.cancel.child_token();
            children.spawn(async move {
                match handle_client_with_permit(client, state, cancel, permit).await {
                    Ok(report) => tracing::info!(
                        host = %report.host(),
                        port = report.port(),
                        target = report.is_target(),
                        outcome = %report.outcome(),
                        duration_ms = report.duration().as_millis(),
                        bytes_up = report.bytes_up(),
                        bytes_down = report.bytes_down(),
                        "tunnel completed"
                    ),
                    Err(error) => tracing::info!(
                        outcome = %error.outcome(),
                        error = %error,
                        "tunnel ended before a report was available"
                    ),
                }
            });
        }
    }
}

fn log_proxy_child(joined: Option<Result<(), tokio::task::JoinError>>) {
    if let Some(Err(error)) = joined {
        tracing::error!(
            outcome = %Outcome::InternalError,
            error = %error,
            "proxy tunnel task failed"
        );
    }
}

async fn drain_proxy_children(children: &mut JoinSet<()>, shutdown_timeout: Duration) {
    let deadline = Instant::now() + shutdown_timeout;
    while !children.is_empty() {
        match timeout_at(deadline, children.join_next()).await {
            Ok(joined) => log_proxy_child(joined),
            Err(_) => {
                children.abort_all();
                while let Some(joined) = children.join_next().await {
                    log_proxy_child(Some(joined));
                }
                return;
            }
        }
    }
}

impl ProxyState {
    pub(crate) fn new(
        matcher: TargetMatcher,
        target_resolver: Arc<dyn TargetResolver>,
        allowed_ports: impl IntoIterator<Item = u16>,
        limits: ProxyLimits,
        max_connections: usize,
    ) -> Self {
        Self {
            matcher,
            target_resolver,
            direct_resolver: Arc::new(SystemResolver),
            allowed_ports: allowed_ports.into_iter().collect(),
            limits,
            concurrency: Arc::new(Semaphore::new(max_connections)),
        }
    }

    #[cfg(test)]
    fn with_direct_resolver(mut self, resolver: Arc<dyn DirectResolver>) -> Self {
        self.direct_resolver = resolver;
        self
    }
}

#[derive(Debug, Error)]
pub(crate) enum TunnelError {
    #[error("client I/O failed")]
    ClientIo(#[source] std::io::Error),
    #[error("CONNECT request was rejected")]
    Request(#[source] ProxyError),
    #[error("CONNECT request timed out")]
    Timeout,
    #[error("tunnel was cancelled")]
    Cancelled,
}

impl TunnelError {
    pub(crate) const fn outcome(&self) -> Outcome {
        match self {
            Self::Request(_) => Outcome::UnsupportedRequest,
            Self::Timeout => Outcome::Timeout,
            Self::Cancelled => Outcome::Cancelled,
            Self::ClientIo(_) => Outcome::ConnectError,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum ConnectError {
    #[error("upstream connection timed out")]
    Timeout,
    #[error("upstream connection was cancelled")]
    Cancelled,
    #[error("no upstream address was reachable")]
    Unreachable,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RelayStats {
    pub(crate) bytes_up: u64,
    pub(crate) bytes_down: u64,
}

pub(crate) async fn connect_socket_candidates(
    addrs: &[SocketAddr],
    deadline: Instant,
    cancel: &CancellationToken,
) -> Result<TcpStream, ConnectError> {
    for &address in addrs {
        if Instant::now() >= deadline {
            return Err(ConnectError::Timeout);
        }
        let attempt = timeout_at(deadline, TcpStream::connect(address));
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(ConnectError::Cancelled),
            result = attempt => result,
        };
        match result {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(_)) => continue,
            Err(_) => return Err(ConnectError::Timeout),
        }
    }
    Err(ConnectError::Unreachable)
}

pub(crate) async fn handle_client(
    client: TcpStream,
    state: Arc<ProxyState>,
    cancel: CancellationToken,
) -> Result<TunnelReport, TunnelError> {
    let permit = tokio::select! {
        _ = cancel.cancelled() => return Err(TunnelError::Cancelled),
        permit = Arc::clone(&state.concurrency).acquire_owned() => {
            permit.map_err(|_| TunnelError::Cancelled)?
        }
    };
    handle_client_with_permit(client, state, cancel, permit).await
}

async fn handle_client_with_permit(
    mut client: TcpStream,
    state: Arc<ProxyState>,
    cancel: CancellationToken,
    _permit: OwnedSemaphorePermit,
) -> Result<TunnelReport, TunnelError> {
    let started = StdInstant::now();
    let request = match read_connect_request(&mut client, &state, &cancel).await {
        Ok(request) => request,
        Err(RequestReadError::Request(error)) => {
            let _ = write_http_status(&mut client, error.status_code(), &cancel).await;
            return Err(TunnelError::Request(error));
        }
        Err(RequestReadError::Timeout) => {
            let _ = write_http_status(&mut client, 431, &cancel).await;
            return Err(TunnelError::Timeout);
        }
        Err(RequestReadError::Cancelled) => return Err(TunnelError::Cancelled),
        Err(RequestReadError::Io(error)) => return Err(TunnelError::ClientIo(error)),
    };
    let is_target = state.matcher.find(request.host()).is_some();
    if is_target {
        return handle_target(client, state, cancel, request, started).await;
    }

    let deadline = Instant::now() + state.limits.connect_timeout;
    let resolve_result = tokio::select! {
        _ = cancel.cancelled() => {
            return Ok(report(
                request,
                false,
                Outcome::Cancelled,
                started,
                RelayStats::default(),
            ));
        }
        result = timeout_at(deadline, state.direct_resolver.resolve(request.host(), request.port())) => result,
    };
    let addresses = match resolve_result {
        Ok(Ok(addresses)) => addresses,
        Ok(Err(error)) => {
            let status = resolve_status(&error);
            let _ = write_http_status(&mut client, status, &cancel).await;
            return Ok(report(
                request,
                false,
                error.outcome(),
                started,
                RelayStats::default(),
            ));
        }
        Err(_) => {
            let _ = write_http_status(&mut client, 504, &cancel).await;
            return Ok(report(
                request,
                false,
                Outcome::Timeout,
                started,
                RelayStats::default(),
            ));
        }
    };
    let mut upstream = match connect_socket_candidates(&addresses, deadline, &cancel).await {
        Ok(upstream) => upstream,
        Err(error) => {
            let _ = write_http_status(&mut client, connect_status(&error), &cancel).await;
            let outcome = connect_outcome(&error);
            return Ok(report(
                request,
                false,
                outcome,
                started,
                RelayStats::default(),
            ));
        }
    };

    write_http_status(&mut client, 200, &cancel).await?;
    let (stats, outcome) = match relay_streams(
        &mut client,
        &mut upstream,
        state.limits.idle_timeout,
        &cancel,
    )
    .await
    {
        Ok(stats) => (stats, Outcome::DirectRelay),
        Err(RelayError::Cancelled) => (RelayStats::default(), Outcome::Cancelled),
        Err(RelayError::Timeout) => (RelayStats::default(), Outcome::Timeout),
        Err(RelayError::Io) => (RelayStats::default(), Outcome::ConnectError),
    };
    Ok(report(request, false, outcome, started, stats))
}

async fn handle_target(
    mut client: TcpStream,
    state: Arc<ProxyState>,
    cancel: CancellationToken,
    request: ConnectRequest,
    started: StdInstant,
) -> Result<TunnelReport, TunnelError> {
    let resolve_result = tokio::select! {
        _ = cancel.cancelled() => {
            return Ok(report(
                request,
                true,
                Outcome::Cancelled,
                started,
                RelayStats::default(),
            ));
        }
        result = state.target_resolver.resolve_ipv4(request.host()) => result,
    };
    let ipv4 = match resolve_result {
        Ok(addresses) => addresses,
        Err(error) => {
            let _ = write_http_status(&mut client, resolve_status(&error), &cancel).await;
            return Ok(report(
                request,
                true,
                error.outcome(),
                started,
                RelayStats::default(),
            ));
        }
    };
    let addresses = ipv4
        .into_iter()
        .map(|address| SocketAddr::from((address, request.port())))
        .collect::<Vec<_>>();
    let deadline = Instant::now() + state.limits.connect_timeout;
    let mut upstream = match connect_socket_candidates(&addresses, deadline, &cancel).await {
        Ok(upstream) => upstream,
        Err(error) => {
            let _ = write_http_status(&mut client, connect_status(&error), &cancel).await;
            let outcome = connect_outcome(&error);
            return Ok(report(
                request,
                true,
                outcome,
                started,
                RelayStats::default(),
            ));
        }
    };

    write_http_status(&mut client, 200, &cancel).await?;
    let (prefix, transformed) = match decide_client_hello_prefix(
        &mut client,
        request.host(),
        state.limits.max_client_hello_bytes,
        state.limits.client_hello_timeout,
        &cancel,
    )
    .await
    {
        PrefixDecision::Forward {
            prefix,
            transformed,
        } => (prefix, transformed),
        PrefixDecision::Reject(outcome) => {
            return Ok(report(
                request,
                true,
                outcome,
                started,
                RelayStats::default(),
            ));
        }
    };

    let prefix_len = prefix.len() as u64;
    let prefix_deadline = Instant::now() + state.limits.idle_timeout;
    if let Err(error) = write_relay_bytes(&mut upstream, &prefix, prefix_deadline, &cancel).await {
        let outcome = relay_outcome(error);
        return Ok(report(
            request,
            true,
            outcome,
            started,
            RelayStats::default(),
        ));
    }
    let (mut stats, outcome) = match relay_streams(
        &mut client,
        &mut upstream,
        state.limits.idle_timeout,
        &cancel,
    )
    .await
    {
        Ok(stats) => (
            stats,
            if transformed {
                Outcome::TargetTransformed
            } else {
                Outcome::DirectRelay
            },
        ),
        Err(error) => (RelayStats::default(), relay_outcome(error)),
    };
    stats.bytes_up = stats.bytes_up.saturating_add(prefix_len);
    Ok(report(request, true, outcome, started, stats))
}

enum PrefixDecision {
    Forward { prefix: Vec<u8>, transformed: bool },
    Reject(Outcome),
}

async fn decide_client_hello_prefix(
    client: &mut TcpStream,
    expected: &DomainName,
    limit: usize,
    decision_timeout: Duration,
    cancel: &CancellationToken,
) -> PrefixDecision {
    let deadline = Instant::now() + decision_timeout;
    let mut prefix = Vec::with_capacity(limit.min(4096));
    loop {
        match crate::tls::rewrite_client_hello(&prefix, expected, limit) {
            RewriteOutcome::NeedMore => {}
            RewriteOutcome::Rewritten(prefix) => {
                return PrefixDecision::Forward {
                    prefix,
                    transformed: true,
                };
            }
            RewriteOutcome::PassThrough { prefix, .. } => {
                return PrefixDecision::Forward {
                    prefix,
                    transformed: false,
                };
            }
            RewriteOutcome::Reject(_) => return PrefixDecision::Reject(Outcome::TlsError),
        }
        if prefix.len() >= limit {
            return PrefixDecision::Reject(Outcome::TlsError);
        }
        let remaining = limit - prefix.len();
        let mut buffer = [0_u8; 4096];
        let read_limit = remaining.min(buffer.len());
        let result = tokio::select! {
            _ = cancel.cancelled() => return PrefixDecision::Reject(Outcome::Cancelled),
            result = timeout_at(deadline, client.read(&mut buffer[..read_limit])) => result,
        };
        match result {
            Ok(Ok(0)) => return PrefixDecision::Reject(Outcome::TlsError),
            Ok(Ok(read)) => prefix.extend_from_slice(&buffer[..read]),
            Ok(Err(_)) => return PrefixDecision::Reject(Outcome::TlsError),
            Err(_) => return PrefixDecision::Reject(Outcome::Timeout),
        }
    }
}

fn relay_outcome(error: RelayError) -> Outcome {
    match error {
        RelayError::Cancelled => Outcome::Cancelled,
        RelayError::Timeout => Outcome::Timeout,
        RelayError::Io => Outcome::ConnectError,
    }
}

enum RequestReadError {
    Request(ProxyError),
    Timeout,
    Cancelled,
    Io(std::io::Error),
}

async fn read_connect_request(
    client: &mut TcpStream,
    state: &ProxyState,
    cancel: &CancellationToken,
) -> Result<ConnectRequest, RequestReadError> {
    let total_deadline = Instant::now() + state.limits.connect_timeout;
    let mut idle_deadline = Instant::now() + state.limits.idle_timeout;
    let mut header = Vec::with_capacity(state.limits.max_connect_header_bytes.min(4096));
    loop {
        if header
            .windows(HEADER_TERMINATOR.len())
            .any(|window| window == HEADER_TERMINATOR)
        {
            return parse_connect(&header, &state.allowed_ports).map_err(RequestReadError::Request);
        }
        if header.len() >= state.limits.max_connect_header_bytes {
            return Err(RequestReadError::Request(ProxyError::HeaderTooLarge));
        }
        let remaining = state.limits.max_connect_header_bytes - header.len();
        let mut buffer = [0_u8; 2048];
        let read_limit = remaining.min(buffer.len());
        let deadline = total_deadline.min(idle_deadline);
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(RequestReadError::Cancelled),
            result = timeout_at(deadline, client.read(&mut buffer[..read_limit])) => result,
        };
        let read = match result {
            Ok(Ok(read)) => read,
            Ok(Err(error)) => return Err(RequestReadError::Io(error)),
            Err(_) => return Err(RequestReadError::Timeout),
        };
        if read == 0 {
            return Err(RequestReadError::Request(ProxyError::HeaderIncomplete));
        }
        header.extend_from_slice(&buffer[..read]);
        idle_deadline = Instant::now() + state.limits.idle_timeout;
    }
}

async fn write_http_status(
    stream: &mut TcpStream,
    status: u16,
    cancel: &CancellationToken,
) -> Result<(), TunnelError> {
    let reason = match status {
        200 => "Connection Established",
        400 => "Bad Request",
        403 => "Forbidden",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        504 => "Gateway Timeout",
        _ => "Bad Gateway",
    };
    let response = format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\n\r\n");
    tokio::select! {
        _ = cancel.cancelled() => Err(TunnelError::Cancelled),
        result = stream.write_all(response.as_bytes()) => result.map_err(TunnelError::ClientIo),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelayError {
    Cancelled,
    Timeout,
    Io,
}

async fn relay_streams(
    client: &mut TcpStream,
    upstream: &mut TcpStream,
    idle_timeout: Duration,
    cancel: &CancellationToken,
) -> Result<RelayStats, RelayError> {
    let (mut client_read, mut client_write) = client.split();
    let (mut upstream_read, mut upstream_write) = upstream.split();
    let mut client_eof = false;
    let mut upstream_eof = false;
    let mut up_buffer = [0_u8; 16_384];
    let mut down_buffer = [0_u8; 16_384];
    let mut stats = RelayStats::default();
    let mut idle_deadline = Instant::now() + idle_timeout;

    while !client_eof || !upstream_eof {
        tokio::select! {
            _ = cancel.cancelled() => return Err(RelayError::Cancelled),
            _ = tokio::time::sleep_until(idle_deadline) => return Err(RelayError::Timeout),
            result = client_read.read(&mut up_buffer), if !client_eof => {
                let read = result.map_err(|_| RelayError::Io)?;
                if read == 0 {
                    client_eof = true;
                    upstream_write.shutdown().await.map_err(|_| RelayError::Io)?;
                } else {
                    idle_deadline = Instant::now() + idle_timeout;
                    write_relay_bytes(&mut upstream_write, &up_buffer[..read], idle_deadline, cancel).await?;
                    stats.bytes_up = stats.bytes_up.saturating_add(read as u64);
                }
            }
            result = upstream_read.read(&mut down_buffer), if !upstream_eof => {
                let read = result.map_err(|_| RelayError::Io)?;
                if read == 0 {
                    upstream_eof = true;
                    client_write.shutdown().await.map_err(|_| RelayError::Io)?;
                } else {
                    idle_deadline = Instant::now() + idle_timeout;
                    write_relay_bytes(&mut client_write, &down_buffer[..read], idle_deadline, cancel).await?;
                    stats.bytes_down = stats.bytes_down.saturating_add(read as u64);
                }
            }
        }
    }
    Ok(stats)
}

async fn write_relay_bytes<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    deadline: Instant,
    cancel: &CancellationToken,
) -> Result<(), RelayError> {
    tokio::select! {
        _ = cancel.cancelled() => Err(RelayError::Cancelled),
        result = timeout_at(deadline, writer.write_all(bytes)) => match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(RelayError::Io),
            Err(_) => Err(RelayError::Timeout),
        }
    }
}

fn connect_status(error: &ConnectError) -> u16 {
    if matches!(error, ConnectError::Timeout) {
        504
    } else {
        502
    }
}

fn connect_outcome(error: &ConnectError) -> Outcome {
    match error {
        ConnectError::Timeout => Outcome::Timeout,
        ConnectError::Cancelled => Outcome::Cancelled,
        ConnectError::Unreachable => Outcome::ConnectError,
    }
}

fn resolve_status(error: &ResolveError) -> u16 {
    if matches!(error, ResolveError::Timeout) {
        504
    } else {
        502
    }
}

fn report(
    request: ConnectRequest,
    target: bool,
    outcome: Outcome,
    started: StdInstant,
    stats: RelayStats,
) -> TunnelReport {
    TunnelReport::new(
        request.host,
        request.port,
        target,
        outcome,
        started.elapsed(),
        stats.bytes_up,
        stats.bytes_down,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::{Instant, timeout};
    use tokio_util::sync::CancellationToken;

    use crate::config::Strategy;
    use crate::dns::{ResolveError, TargetResolver};
    use crate::domain::{DomainName, TargetMatcher, TargetRule};

    use super::{
        ConnectError, DirectResolver, ProxyError, ProxyLimits, ProxyServer, ProxyState,
        TunnelError, connect_socket_candidates, handle_client, parse_connect,
    };

    fn allowed_ports() -> BTreeSet<u16> {
        BTreeSet::from([443, 8443])
    }

    #[test]
    fn parses_normalized_hostname_connect_request() {
        let request = parse_connect(
            b"CONNECT ReDdIt.CoM.:443 HTTP/1.1\r\nHost: ReDdIt.CoM.:443\r\nUser-Agent: test\r\n\r\n",
            &allowed_ports(),
        )
        .expect("valid CONNECT request");

        assert_eq!(request.host().as_str(), "reddit.com");
        assert_eq!(request.port(), 443);
    }

    #[test]
    fn rejects_unsupported_method_and_version_with_exact_status() {
        let method =
            parse_connect(b"GET reddit.com:443 HTTP/1.1\r\n\r\n", &allowed_ports()).unwrap_err();
        assert_eq!(method, ProxyError::UnsupportedMethod);
        assert_eq!(method.status_code(), 405);

        let version = parse_connect(b"CONNECT reddit.com:443 HTTP/1.0\r\n\r\n", &allowed_ports())
            .unwrap_err();
        assert_eq!(version, ProxyError::UnsupportedVersion);
        assert_eq!(version.status_code(), 400);
    }

    #[test]
    fn rejects_authority_matrix_with_exact_errors_and_statuses() {
        let cases = [
            (
                "missing port",
                &b"CONNECT reddit.com HTTP/1.1\r\n\r\n"[..],
                ProxyError::MalformedAuthority,
                400,
            ),
            (
                "userinfo",
                &b"CONNECT user@reddit.com:443 HTTP/1.1\r\n\r\n"[..],
                ProxyError::MalformedAuthority,
                400,
            ),
            (
                "zero port",
                &b"CONNECT reddit.com:0 HTTP/1.1\r\n\r\n"[..],
                ProxyError::MalformedAuthority,
                400,
            ),
            (
                "IPv4 authority",
                &b"CONNECT 127.0.0.1:443 HTTP/1.1\r\n\r\n"[..],
                ProxyError::IpAuthority,
                400,
            ),
            (
                "bracketed IPv6 authority",
                &b"CONNECT [::1]:443 HTTP/1.1\r\n\r\n"[..],
                ProxyError::IpAuthority,
                400,
            ),
        ];

        for (name, input, expected_error, expected_status) in cases {
            let error = parse_connect(input, &allowed_ports()).unwrap_err();
            assert_eq!(error, expected_error, "{name}");
            assert_eq!(error.status_code(), expected_status, "{name}");
            assert!(!error.to_string().contains("reddit.com"), "{name}");
            assert!(!error.to_string().contains("127.0.0.1"), "{name}");
        }
    }

    #[test]
    fn explicitly_configured_dynamic_port_does_not_allow_other_ports() {
        let dynamic_port = 49_199;
        let allowed_ports = BTreeSet::from([dynamic_port]);
        let accepted = parse_connect(
            format!("CONNECT selected.test:{dynamic_port} HTTP/1.1\r\n\r\n").as_bytes(),
            &allowed_ports,
        )
        .expect("explicit dynamic port must be accepted");
        assert_eq!(accepted.port(), dynamic_port);

        let error = parse_connect(
            b"CONNECT selected.test:49198 HTTP/1.1\r\n\r\n",
            &allowed_ports,
        )
        .unwrap_err();
        assert_eq!(error, ProxyError::PortNotAllowed);
        assert_eq!(error.status_code(), 403);
    }

    #[test]
    fn rejects_disallowed_ports_and_body_semantics() {
        let port =
            parse_connect(b"CONNECT reddit.com:80 HTTP/1.1\r\n\r\n", &allowed_ports()).unwrap_err();
        assert_eq!(port, ProxyError::PortNotAllowed);
        assert_eq!(port.status_code(), 403);

        for input in [
            &b"CONNECT reddit.com:443 HTTP/1.1\r\nContent-Length: 1\r\n\r\n"[..],
            &b"CONNECT reddit.com:443 HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"[..],
            &b"CONNECT reddit.com:443 HTTP/1.1\r\n\r\nbody"[..],
        ] {
            let error = parse_connect(input, &allowed_ports()).unwrap_err();
            assert_eq!(error, ProxyError::RequestBodyNotAllowed);
            assert_eq!(error.status_code(), 400);
        }
    }

    #[test]
    fn rejects_incomplete_and_defensively_oversized_headers() {
        let incomplete = parse_connect(
            b"CONNECT reddit.com:443 HTTP/1.1\r\nHost: reddit.com:443\r\n",
            &allowed_ports(),
        )
        .unwrap_err();
        assert_eq!(incomplete, ProxyError::HeaderIncomplete);
        assert_eq!(incomplete.status_code(), 431);

        let mut oversized = vec![b'a'; 65_537];
        oversized.extend_from_slice(b"\r\n\r\n");
        let error = parse_connect(&oversized, &allowed_ports()).unwrap_err();
        assert_eq!(error, ProxyError::HeaderTooLarge);
        assert_eq!(error.status_code(), 431);
    }

    #[test]
    fn rejects_header_matrix_with_exact_errors_and_statuses_without_panicking() {
        let cases = [
            (
                "LF-only framing",
                &b"CONNECT reddit.com:443 HTTP/1.1\n\n"[..],
                ProxyError::MalformedHeader,
                400,
            ),
            (
                "invalid header name",
                &b"CONNECT reddit.com:443 HTTP/1.1\r\nBad Header: x\r\n\r\n"[..],
                ProxyError::MalformedHeader,
                400,
            ),
            (
                "invalid header value",
                &b"CONNECT reddit.com:443 HTTP/1.1\r\nX-Test: bad\0value\r\n\r\n"[..],
                ProxyError::MalformedHeader,
                400,
            ),
            (
                "invalid request bytes",
                &[0xff, 0xfe, b'\r', b'\n', b'\r', b'\n'][..],
                ProxyError::MalformedRequestLine,
                400,
            ),
        ];

        for (name, input, expected_error, expected_status) in cases {
            let result = std::panic::catch_unwind(|| parse_connect(input, &allowed_ports()));
            assert!(result.is_ok(), "{name}");
            let error = result.unwrap().unwrap_err();
            assert_eq!(error, expected_error, "{name}");
            assert_eq!(error.status_code(), expected_status, "{name}");
        }
    }

    struct CountingResolver {
        calls: Arc<AtomicUsize>,
    }

    impl TargetResolver for CountingResolver {
        fn resolve_ipv4<'a>(
            &'a self,
            _host: &'a DomainName,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(ResolveError::EmptyAnswer) })
        }
    }

    fn direct_state(calls: Arc<AtomicUsize>, allowed_port: u16) -> Arc<ProxyState> {
        Arc::new(ProxyState::new(
            TargetMatcher::new(Vec::new()).unwrap(),
            Arc::new(CountingResolver { calls }),
            [allowed_port],
            test_limits(),
            2,
        ))
    }

    enum DirectResolverResult {
        Addresses(Vec<SocketAddr>),
        Pending,
    }

    struct TestDirectResolver {
        calls: Arc<AtomicUsize>,
        result: DirectResolverResult,
    }

    impl DirectResolver for TestDirectResolver {
        fn resolve<'a>(
            &'a self,
            _host: &'a DomainName,
            _port: u16,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, ResolveError>> + Send + 'a>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match &self.result {
                DirectResolverResult::Addresses(addresses) => {
                    let addresses = addresses.clone();
                    Box::pin(async move { Ok(addresses) })
                }
                DirectResolverResult::Pending => Box::pin(std::future::pending()),
            }
        }
    }

    fn state_with_direct_resolver(
        resolver: Arc<dyn DirectResolver>,
        allowed_port: u16,
        limits: ProxyLimits,
        max_connections: usize,
    ) -> Arc<ProxyState> {
        Arc::new(
            ProxyState::new(
                TargetMatcher::new(Vec::new()).unwrap(),
                Arc::new(CountingResolver {
                    calls: Arc::new(AtomicUsize::new(0)),
                }),
                [allowed_port],
                limits,
                max_connections,
            )
            .with_direct_resolver(resolver),
        )
    }

    struct FixedResolver {
        calls: Arc<AtomicUsize>,
        result: Result<Vec<Ipv4Addr>, ResolveError>,
    }

    impl TargetResolver for FixedResolver {
        fn resolve_ipv4<'a>(
            &'a self,
            _host: &'a DomainName,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let result = match &self.result {
                Ok(addresses) => Ok(addresses.clone()),
                Err(ResolveError::EmptyAnswer) => Err(ResolveError::EmptyAnswer),
                Err(_) => panic!("test resolver uses only EmptyAnswer failures"),
            };
            Box::pin(async move { result })
        }
    }

    fn selected_state(
        calls: Arc<AtomicUsize>,
        result: Result<Vec<Ipv4Addr>, ResolveError>,
        allowed_port: u16,
        limits: ProxyLimits,
        max_connections: usize,
    ) -> Arc<ProxyState> {
        let rule = TargetRule::new(
            DomainName::parse("selected.test").unwrap(),
            true,
            Strategy::TlsRecordSplit,
        );
        Arc::new(ProxyState::new(
            TargetMatcher::new(vec![rule]).unwrap(),
            Arc::new(FixedResolver { calls, result }),
            [allowed_port],
            limits,
            max_connections,
        ))
    }

    fn test_limits() -> ProxyLimits {
        ProxyLimits {
            max_connect_header_bytes: 4096,
            max_client_hello_bytes: 4096,
            connect_timeout: Duration::from_secs(1),
            client_hello_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(1),
        }
    }

    async fn read_response(stream: &mut TcpStream) -> Vec<u8> {
        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while !response.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).await.unwrap();
            response.push(byte[0]);
        }
        response
    }

    fn client_hello(sni: &str) -> Vec<u8> {
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
        let mut record = vec![22, 0x03, 0x03];
        record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        record.extend_from_slice(&handshake);
        record
    }

    #[tokio::test]
    async fn stalled_connect_header_times_out_with_431_and_releases_waiting_permit() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });
        let mut limits = test_limits();
        limits.connect_timeout = Duration::from_millis(80);
        limits.idle_timeout = Duration::from_secs(1);
        let state = Arc::new(ProxyState::new(
            TargetMatcher::new(Vec::new()).unwrap(),
            Arc::new(CountingResolver {
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            [origin_port],
            limits,
            1,
        ));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let accept_task = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                let (stalled, _) = listener.accept().await.unwrap();
                let stalled_task =
                    tokio::spawn(handle_client(stalled, Arc::clone(&state), cancel.clone()));
                let (waiting, _) = listener.accept().await.unwrap();
                let waiting_task = tokio::spawn(handle_client(waiting, state, cancel));
                (stalled_task, waiting_task)
            }
        });

        let mut stalled = TcpStream::connect(address).await.unwrap();
        stalled
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\nX:").as_bytes())
            .await
            .unwrap();
        let mut waiting = TcpStream::connect(address).await.unwrap();
        waiting
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut waiting_byte = [0_u8; 1];
        assert!(
            timeout(Duration::from_millis(30), waiting.read(&mut waiting_byte))
                .await
                .is_err(),
            "waiting handler must not pass the occupied permit"
        );

        let response = read_response(&mut stalled).await;
        assert!(response.starts_with(b"HTTP/1.1 431"));
        let (stalled_task, waiting_task) = accept_task.await.unwrap();
        let stalled_error = stalled_task.await.unwrap().unwrap_err();
        assert!(matches!(stalled_error, TunnelError::Timeout));
        assert_eq!(
            stalled_error.outcome(),
            crate::diagnostics::Outcome::Timeout
        );

        assert!(
            read_response(&mut waiting)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        waiting.write_all(b"ping").await.unwrap();
        let mut reply = [0_u8; 4];
        waiting.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        waiting.shutdown().await.unwrap();
        assert_eq!(
            waiting_task.await.unwrap().unwrap().outcome().as_str(),
            "direct-relay"
        );
        assert_eq!(state.concurrency.available_permits(), 1);
        origin_task.await.unwrap();
    }

    #[tokio::test]
    async fn direct_resolution_timeout_returns_504_report_and_releases_permit() {
        let mut limits = test_limits();
        limits.connect_timeout = Duration::from_millis(50);
        let calls = Arc::new(AtomicUsize::new(0));
        let state = state_with_direct_resolver(
            Arc::new(TestDirectResolver {
                calls: Arc::clone(&calls),
                result: DirectResolverResult::Pending,
            }),
            443,
            limits,
            1,
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let handler = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_client(stream, state, cancel).await.unwrap()
            }
        });
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"CONNECT direct.test:443 HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let response = read_response(&mut client).await;
        assert!(response.starts_with(b"HTTP/1.1 504"));
        assert!(!response.starts_with(b"HTTP/1.1 200"));
        assert_eq!(handler.await.unwrap().outcome().as_str(), "timeout");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(state.concurrency.available_permits(), 1);
    }

    #[tokio::test]
    async fn direct_resolution_cancellation_returns_stable_report_without_200() {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = state_with_direct_resolver(
            Arc::new(TestDirectResolver {
                calls: Arc::clone(&calls),
                result: DirectResolverResult::Pending,
            }),
            443,
            test_limits(),
            1,
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let handler = tokio::spawn({
            let state = Arc::clone(&state);
            let cancel = cancel.clone();
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_client(stream, state, cancel).await.unwrap()
            }
        });
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"CONNECT direct.test:443 HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        cancel.cancel();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(!response.starts_with(b"HTTP/1.1 200"));
        assert_eq!(handler.await.unwrap().outcome().as_str(), "cancelled");
        assert_eq!(state.concurrency.available_permits(), 1);
    }

    #[tokio::test]
    async fn injected_direct_resolution_preserves_candidate_family_and_order() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ipv4 = origin.local_addr().unwrap();
        let ipv6 = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), ipv4.port());
        let calls = Arc::new(AtomicUsize::new(0));
        let state = state_with_direct_resolver(
            Arc::new(TestDirectResolver {
                calls: Arc::clone(&calls),
                result: DirectResolverResult::Addresses(vec![ipv6, ipv4]),
            }),
            ipv4.port(),
            test_limits(),
            1,
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).await.unwrap();
            assert_eq!(byte, [7]);
            stream.write_all(&[8]).await.unwrap();
        });
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(format!("CONNECT direct.test:{} HTTP/1.1\r\n\r\n", ipv4.port()).as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut client)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        client.write_all(&[7]).await.unwrap();
        let mut reply = [0_u8; 1];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, [8]);
        client.shutdown().await.unwrap();
        assert_eq!(handler.await.unwrap().outcome().as_str(), "direct-relay");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        origin_task.await.unwrap();
    }

    #[tokio::test]
    async fn direct_handler_sends_200_only_after_upstream_and_relays_both_directions() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let state = direct_state(Arc::clone(&calls), origin_port);
        let cancel = CancellationToken::new();
        let handler = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_client(stream, state, cancel).await.unwrap()
            }
        });

        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while !response.ends_with(b"\r\n\r\n") {
            client.read_exact(&mut byte).await.unwrap();
            response.push(byte[0]);
        }
        assert!(response.starts_with(b"HTTP/1.1 200 Connection Established\r\n"));
        client.write_all(b"ping").await.unwrap();
        let mut reply = [0_u8; 4];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        client.shutdown().await.unwrap();

        let report = timeout(Duration::from_secs(2), handler)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.outcome().as_str(), "direct-relay");
        assert_eq!(report.bytes_up(), 4);
        assert_eq!(report.bytes_down(), 4);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        origin_task.await.unwrap();
    }

    #[tokio::test]
    async fn direct_handler_never_acknowledges_an_unreachable_upstream() {
        let unavailable = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = unavailable.local_addr().unwrap().port();
        drop(unavailable);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let state = direct_state(Arc::new(AtomicUsize::new(0)), port);
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(format!("CONNECT localhost:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(!response.starts_with(b"HTTP/1.1 200"));
        assert!(response.starts_with(b"HTTP/1.1 502") || response.starts_with(b"HTTP/1.1 504"));
        let report = handler.await.unwrap();
        assert!(matches!(
            report.outcome().as_str(),
            "connect-error" | "timeout" | "dns-error"
        ));
    }

    #[tokio::test]
    async fn candidate_connector_preserves_order_and_accepts_both_address_families() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ipv4 = listener.local_addr().unwrap();
        let ipv6 = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), ipv4.port());
        let cancel = CancellationToken::new();
        let accept = tokio::spawn(async move { listener.accept().await.unwrap() });

        let stream = connect_socket_candidates(
            &[ipv6, ipv4],
            Instant::now() + Duration::from_secs(1),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(stream.peer_addr().unwrap(), ipv4);
        accept.await.unwrap();

        let error = connect_socket_candidates(&[], Instant::now(), &cancel)
            .await
            .unwrap_err();
        assert!(matches!(error, ConnectError::Unreachable));

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let error = connect_socket_candidates(
            &[SocketAddr::from(([192, 0, 2, 1], 443))],
            Instant::now() + Duration::from_secs(1),
            &cancelled,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ConnectError::Cancelled));
    }

    #[tokio::test]
    async fn selected_handler_uses_only_target_resolver_and_writes_one_rewritten_prefix() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut handshake = Vec::new();
            for _ in 0..2 {
                let mut header = [0_u8; 5];
                stream.read_exact(&mut header).await.unwrap();
                assert_eq!(header[0], 22);
                let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
                let mut payload = vec![0_u8; length];
                stream.read_exact(&mut payload).await.unwrap();
                handshake.extend_from_slice(&payload);
            }
            let mut opaque = [0_u8; 4];
            stream.read_exact(&mut opaque).await.unwrap();
            assert_eq!(&opaque, b"tail");
            stream.write_all(b"pong").await.unwrap();
            handshake
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let state = selected_state(
            Arc::clone(&calls),
            Ok(vec![Ipv4Addr::LOCALHOST]),
            origin_port,
            test_limits(),
            2,
        );
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(proxy_address).await.unwrap();
        client
            .write_all(format!("CONNECT selected.test:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut client)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        let input = client_hello("selected.test");
        client.write_all(&input).await.unwrap();
        client.write_all(b"tail").await.unwrap();
        let mut reply = [0_u8; 4];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        client.shutdown().await.unwrap();

        let report = handler.await.unwrap();
        assert!(report.is_target());
        assert_eq!(report.outcome().as_str(), "target-transformed");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let transformed_handshake = origin_task.await.unwrap();
        assert_eq!(transformed_handshake, input[5..]);
    }

    #[tokio::test]
    async fn selected_safe_pass_through_is_forwarded_once_then_relay_stays_opaque() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut bytes = [0_u8; 5];
            stream.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"plain");
            stream.write_all(b"reply").await.unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let state = selected_state(
            Arc::new(AtomicUsize::new(0)),
            Ok(vec![Ipv4Addr::LOCALHOST]),
            origin_port,
            test_limits(),
            1,
        );
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(proxy_address).await.unwrap();
        client
            .write_all(format!("CONNECT selected.test:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut client)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        client.write_all(b"plain").await.unwrap();
        let mut reply = [0_u8; 5];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"reply");
        client.shutdown().await.unwrap();
        let report = handler.await.unwrap();
        assert!(report.is_target());
        assert_eq!(report.outcome().as_str(), "direct-relay");
        origin_task.await.unwrap();
    }

    #[tokio::test]
    async fn selected_reject_closes_only_tunnel_without_forwarding_prefix() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut byte = [0_u8; 1];
            stream.read(&mut byte).await.unwrap()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let state = selected_state(
            Arc::new(AtomicUsize::new(0)),
            Ok(vec![Ipv4Addr::LOCALHOST]),
            origin_port,
            test_limits(),
            1,
        );
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(proxy_address).await.unwrap();
        client
            .write_all(format!("CONNECT selected.test:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut client)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        client.write_all(&client_hello("other.test")).await.unwrap();
        let mut rest = Vec::new();
        client.read_to_end(&mut rest).await.unwrap();
        let report = handler.await.unwrap();
        assert_eq!(report.outcome().as_str(), "tls-error");
        assert_eq!(
            origin_task.await.unwrap(),
            0,
            "rejected prefix must not be sent"
        );
    }

    #[tokio::test]
    async fn selected_resolution_failure_happens_before_200_without_os_fallback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let state = selected_state(
            Arc::clone(&calls),
            Err(ResolveError::EmptyAnswer),
            443,
            test_limits(),
            1,
        );
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(proxy_address).await.unwrap();
        client
            .write_all(b"CONNECT selected.test:443 HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let response = read_response(&mut client).await;
        assert!(response.starts_with(b"HTTP/1.1 502"));
        assert!(!response.starts_with(b"HTTP/1.1 200"));
        assert_eq!(handler.await.unwrap().outcome().as_str(), "dns-error");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn selected_client_hello_decision_has_one_total_timeout() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut byte = [0_u8; 1];
            stream.read(&mut byte).await.unwrap()
        });
        let mut limits = test_limits();
        limits.client_hello_timeout = Duration::from_millis(50);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let state = selected_state(
            Arc::new(AtomicUsize::new(0)),
            Ok(vec![Ipv4Addr::LOCALHOST]),
            origin_port,
            limits,
            1,
        );
        let cancel = CancellationToken::new();
        let handler = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_client(stream, state, cancel).await.unwrap()
        });

        let mut client = TcpStream::connect(proxy_address).await.unwrap();
        client
            .write_all(format!("CONNECT selected.test:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut client)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        let report = timeout(Duration::from_secs(1), handler)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.outcome().as_str(), "timeout");
        assert_eq!(origin_task.await.unwrap(), 0);
    }

    #[tokio::test]
    async fn concurrent_malformed_selected_tunnel_does_not_break_direct_relay() {
        let selected_origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = selected_origin.local_addr().unwrap().port();
        let direct_origin = TcpListener::bind((Ipv6Addr::LOCALHOST, port))
            .await
            .unwrap();
        let selected_origin_task = tokio::spawn(async move {
            let (mut stream, _) = selected_origin.accept().await.unwrap();
            let mut byte = [0_u8; 1];
            stream.read(&mut byte).await.unwrap()
        });
        let direct_origin_task = tokio::spawn(async move {
            let (mut stream, _) = direct_origin.accept().await.unwrap();
            let mut request = [0_u8; 6];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"direct");
            stream.write_all(b"success").await.unwrap();
        });

        let rule = TargetRule::new(
            DomainName::parse("selected.test").unwrap(),
            true,
            Strategy::TlsRecordSplit,
        );
        let state = Arc::new(
            ProxyState::new(
                TargetMatcher::new(vec![rule]).unwrap(),
                Arc::new(FixedResolver {
                    calls: Arc::new(AtomicUsize::new(0)),
                    result: Ok(vec![Ipv4Addr::LOCALHOST]),
                }),
                [port],
                test_limits(),
                2,
            )
            .with_direct_resolver(Arc::new(TestDirectResolver {
                calls: Arc::new(AtomicUsize::new(0)),
                result: DirectResolverResult::Addresses(vec![SocketAddr::from((
                    Ipv6Addr::LOCALHOST,
                    port,
                ))]),
            })),
        );
        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = proxy.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let handlers = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                let (selected, _) = proxy.accept().await.unwrap();
                let selected_task =
                    tokio::spawn(handle_client(selected, Arc::clone(&state), cancel.clone()));
                let (direct, _) = proxy.accept().await.unwrap();
                let direct_task = tokio::spawn(handle_client(direct, state, cancel));
                (selected_task, direct_task)
            }
        });

        let mut selected = TcpStream::connect(proxy_address).await.unwrap();
        selected
            .write_all(format!("CONNECT selected.test:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut selected)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        let mut direct = TcpStream::connect(proxy_address).await.unwrap();
        direct
            .write_all(format!("CONNECT direct.test:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(
            read_response(&mut direct)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        assert_eq!(
            state.concurrency.available_permits(),
            0,
            "both handlers hold the configured two permits"
        );

        selected
            .write_all(&client_hello("mismatched.test"))
            .await
            .unwrap();
        direct.write_all(b"direct").await.unwrap();
        let mut reply = [0_u8; 7];
        direct.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"success");
        direct.shutdown().await.unwrap();
        let mut selected_rest = Vec::new();
        selected.read_to_end(&mut selected_rest).await.unwrap();

        let (selected_task, direct_task) = handlers.await.unwrap();
        let selected_report = selected_task.await.unwrap().unwrap();
        let direct_report = direct_task.await.unwrap().unwrap();
        assert_eq!(selected_report.outcome().as_str(), "tls-error");
        assert!(selected_report.is_target());
        assert_eq!(direct_report.outcome().as_str(), "direct-relay");
        assert!(!direct_report.is_target());
        assert_eq!(direct_report.bytes_up(), 6);
        assert_eq!(direct_report.bytes_down(), 7);
        assert_eq!(selected_origin_task.await.unwrap(), 0);
        direct_origin_task.await.unwrap();
        assert_eq!(state.concurrency.available_permits(), 2);
    }

    #[tokio::test]
    async fn concurrency_permit_is_held_for_the_full_tunnel_lifetime() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let first = origin.accept().await.unwrap().0;
            let second = origin.accept().await.unwrap().0;
            (first, second)
        });
        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = proxy.local_addr().unwrap();
        let state = Arc::new(ProxyState::new(
            TargetMatcher::new(Vec::new()).unwrap(),
            Arc::new(CountingResolver {
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            [origin_port],
            test_limits(),
            1,
        ));
        let cancel = CancellationToken::new();
        let accept_task = tokio::spawn(async move {
            let (first, _) = proxy.accept().await.unwrap();
            let first_task = tokio::spawn(handle_client(first, Arc::clone(&state), cancel.clone()));
            let (second, _) = proxy.accept().await.unwrap();
            let second_task = tokio::spawn(handle_client(second, state, cancel));
            (first_task, second_task)
        });

        let mut first = TcpStream::connect(proxy_address).await.unwrap();
        first
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(read_response(&mut first).await.starts_with(b"HTTP/1.1 200"));
        let mut second = TcpStream::connect(proxy_address).await.unwrap();
        second
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut byte = [0_u8; 1];
        assert!(
            timeout(Duration::from_millis(50), second.read(&mut byte))
                .await
                .is_err()
        );
        first.shutdown().await.unwrap();
        assert!(
            read_response(&mut second)
                .await
                .starts_with(b"HTTP/1.1 200")
        );
        second.shutdown().await.unwrap();
        let (first_task, second_task) = accept_task.await.unwrap();
        first_task.await.unwrap().unwrap();
        second_task.await.unwrap().unwrap();
        let _ = origin_task.await.unwrap();
    }

    #[tokio::test]
    async fn proxy_server_isolates_client_errors_and_stops_accepting_on_cancellation() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let state = direct_state(Arc::new(AtomicUsize::new(0)), origin_port);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(
            ProxyServer::new(listener, state, cancel.clone(), Duration::from_millis(100)).run(),
        );

        let mut invalid = TcpStream::connect(address).await.unwrap();
        invalid.write_all(b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
        assert!(
            read_response(&mut invalid)
                .await
                .starts_with(b"HTTP/1.1 405")
        );

        let mut valid = TcpStream::connect(address).await.unwrap();
        valid
            .write_all(format!("CONNECT localhost:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert!(read_response(&mut valid).await.starts_with(b"HTTP/1.1 200"));
        valid.write_all(b"ping").await.unwrap();
        let mut reply = [0_u8; 4];
        valid.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        valid.shutdown().await.unwrap();
        origin_task.await.unwrap();

        cancel.cancel();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(TcpStream::connect(address).await.is_err());
    }

    #[test]
    fn selected_rule_is_explicit_in_state_without_affecting_direct_policy() {
        let rule = TargetRule::new(
            DomainName::parse("reddit.com").unwrap(),
            true,
            Strategy::TlsRecordSplit,
        );
        let matcher = TargetMatcher::new(vec![rule]).unwrap();
        assert!(
            matcher
                .find(&DomainName::parse("www.reddit.com").unwrap())
                .is_some()
        );
        assert!(
            matcher
                .find(&DomainName::parse("localhost").unwrap())
                .is_none()
        );
    }
}
