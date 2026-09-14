#![allow(dead_code)]

use std::fmt::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::diagnostics::Outcome;
use crate::domain::TargetRule;

const PAC_PATH: &str = "/proxy.pac";
const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";

pub(crate) fn render_pac(proxy_addr: SocketAddr, rules: &[TargetRule]) -> String {
    let proxy = escape_javascript_string(&proxy_addr.to_string());
    let mut body = String::from(
        "function FindProxyForURL(url, host) {\n\
         \x20 host = host.toLowerCase();\n\
         \x20 if (host.charAt(host.length - 1) === \".\") host = host.slice(0, -1);\n",
    );

    for rule in rules {
        let host = escape_javascript_string(rule.host().as_str());
        write!(body, "  if (host === \"{host}\"").expect("writing to String cannot fail");
        if rule.includes_subdomains() {
            write!(body, " || dnsDomainIs(host, \".{host}\")")
                .expect("writing to String cannot fail");
        }
        writeln!(body, ") return \"PROXY {proxy}\";").expect("writing to String cannot fail");
    }

    body.push_str("  return \"DIRECT\";\n}\n");
    body
}

pub(crate) struct PacServer {
    listener: TcpListener,
    body: Arc<str>,
    cancel: CancellationToken,
    max_request_bytes: usize,
    request_timeout: Duration,
    shutdown_timeout: Duration,
    concurrency: Arc<Semaphore>,
}

#[derive(Debug, Error)]
pub(crate) enum PacServiceError {
    #[error("PAC listener accept failed")]
    Accept(#[source] std::io::Error),
}

impl PacServer {
    pub(crate) fn new(
        listener: TcpListener,
        body: Arc<str>,
        cancel: CancellationToken,
        max_request_bytes: usize,
        request_timeout: Duration,
        shutdown_timeout: Duration,
        max_connections: usize,
    ) -> Self {
        Self {
            listener,
            body,
            cancel,
            max_request_bytes,
            request_timeout,
            shutdown_timeout,
            concurrency: Arc::new(Semaphore::new(max_connections)),
        }
    }

    pub(crate) async fn run(self) -> Result<(), PacServiceError> {
        let mut children = JoinSet::new();
        let result = self.accept_loop(&mut children).await;
        self.cancel.cancel();
        drain_pac_children(&mut children, self.shutdown_timeout).await;
        result
    }

    async fn accept_loop(&self, children: &mut JoinSet<()>) -> Result<(), PacServiceError> {
        loop {
            let permit = tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                joined = children.join_next(), if !children.is_empty() => {
                    log_pac_child(joined);
                    continue;
                }
                permit = Arc::clone(&self.concurrency).acquire_owned() => {
                    match permit {
                        Ok(permit) => permit,
                        Err(_) => return Ok(()),
                    }
                }
            };
            let accepted = tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                joined = children.join_next(), if !children.is_empty() => {
                    log_pac_child(joined);
                    drop(permit);
                    continue;
                }
                accepted = self.listener.accept() => accepted,
            };
            let (stream, _) = accepted.map_err(PacServiceError::Accept)?;
            let body = Arc::clone(&self.body);
            let cancel = self.cancel.child_token();
            let max_request_bytes = self.max_request_bytes;
            let request_timeout = self.request_timeout;
            children.spawn(async move {
                let _permit = permit;
                if let Err(error) =
                    serve_pac_client(stream, &body, max_request_bytes, request_timeout, &cancel)
                        .await
                {
                    tracing::info!(
                        outcome = %error.outcome(),
                        error = %error,
                        "PAC request failed"
                    );
                }
            });
        }
    }
}

#[derive(Debug, Error)]
enum PacRequestError {
    #[error("PAC request timed out")]
    Timeout,
    #[error("PAC request was cancelled")]
    Cancelled,
    #[error("PAC request I/O failed")]
    Io(#[source] std::io::Error),
}

impl PacRequestError {
    const fn outcome(&self) -> Outcome {
        match self {
            Self::Timeout => Outcome::Timeout,
            Self::Cancelled => Outcome::Cancelled,
            Self::Io(_) => Outcome::ConnectError,
        }
    }
}

async fn serve_pac_client(
    mut stream: TcpStream,
    body: &str,
    max_request_bytes: usize,
    request_timeout: Duration,
    cancel: &CancellationToken,
) -> Result<(), PacRequestError> {
    let deadline = Instant::now() + request_timeout;
    let mut request = Vec::with_capacity(max_request_bytes.min(1024));
    let status = loop {
        if request
            .windows(HEADER_TERMINATOR.len())
            .any(|window| window == HEADER_TERMINATOR)
        {
            break pac_status(&request);
        }
        if request.len() >= max_request_bytes {
            break 431;
        }
        let remaining = max_request_bytes - request.len();
        let mut buffer = [0_u8; 1024];
        let read_limit = remaining.min(buffer.len());
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(PacRequestError::Cancelled),
            result = timeout_at(deadline, stream.read(&mut buffer[..read_limit])) => result,
        };
        match result {
            Ok(Ok(0)) => break 400,
            Ok(Ok(read)) => request.extend_from_slice(&buffer[..read]),
            Ok(Err(error)) => return Err(PacRequestError::Io(error)),
            Err(_) => return Err(PacRequestError::Timeout),
        }
    };

    let response_body = if status == 200 { body.as_bytes() } else { &[] };
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        _ => "Bad Request",
    };
    let content_type = if status == 200 {
        "application/x-ns-proxy-autoconfig"
    } else {
        "text/plain"
    };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response_body.len()
    );
    tokio::select! {
        _ = cancel.cancelled() => Err(PacRequestError::Cancelled),
        result = async {
            stream.write_all(headers.as_bytes()).await?;
            stream.write_all(response_body).await
        } => result.map_err(PacRequestError::Io),
    }
}

fn pac_status(request: &[u8]) -> u16 {
    let Some(end) = request
        .windows(HEADER_TERMINATOR.len())
        .position(|window| window == HEADER_TERMINATOR)
    else {
        return 400;
    };
    if end + HEADER_TERMINATOR.len() != request.len() {
        return 400;
    }
    let Ok(text) = std::str::from_utf8(&request[..end]) else {
        return 400;
    };
    let Some(line) = text.split("\r\n").next() else {
        return 400;
    };
    let mut parts = line.split(' ');
    let (Some(method), Some(path), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return 400;
    };
    if method != "GET" {
        return 405;
    }
    if version != "HTTP/1.1" {
        return 400;
    }
    if path != PAC_PATH {
        return 404;
    }
    200
}

fn log_pac_child(joined: Option<Result<(), tokio::task::JoinError>>) {
    if let Some(Err(error)) = joined {
        tracing::error!(
            outcome = %Outcome::InternalError,
            error = %error,
            "PAC client task failed"
        );
    }
}

async fn drain_pac_children(children: &mut JoinSet<()>, shutdown_timeout: Duration) {
    let deadline = Instant::now() + shutdown_timeout;
    while !children.is_empty() {
        match timeout_at(deadline, children.join_next()).await {
            Ok(joined) => log_pac_child(joined),
            Err(_) => {
                children.abort_all();
                while let Some(joined) = children.join_next().await {
                    log_pac_child(Some(joined));
                }
                return;
            }
        }
    }
}

fn escape_javascript_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::timeout;
    use tokio_util::sync::CancellationToken;

    use crate::config::Strategy;
    use crate::domain::{DomainName, TargetRule};

    use super::{PacServer, render_pac};

    fn rule(host: &str, include_subdomains: bool) -> TargetRule {
        TargetRule::new(
            DomainName::parse(host).expect("valid test hostname"),
            include_subdomains,
            Strategy::TlsRecordSplit,
        )
    }

    #[test]
    fn renders_deterministic_target_only_policy() {
        let proxy = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let rules = vec![rule("reddit.com", true), rule("medium.com", false)];

        let first = render_pac(proxy, &rules);
        let second = render_pac(proxy, &rules);

        assert_eq!(first, second);
        assert_eq!(
            first,
            "function FindProxyForURL(url, host) {\n  host = host.toLowerCase();\n  if (host.charAt(host.length - 1) === \".\") host = host.slice(0, -1);\n  if (host === \"reddit.com\" || dnsDomainIs(host, \".reddit.com\")) return \"PROXY 127.0.0.1:8080\";\n  if (host === \"medium.com\") return \"PROXY 127.0.0.1:8080\";\n  return \"DIRECT\";\n}\n"
        );
    }

    #[test]
    fn uses_leading_dot_only_for_enabled_subdomains() {
        let body = render_pac(
            "127.0.0.1:8080".parse().unwrap(),
            &[rule("reddit.com", true), rule("medium.com", false)],
        );

        assert!(body.contains("host === \"reddit.com\""));
        assert!(body.contains("dnsDomainIs(host, \".reddit.com\")"));
        assert!(body.contains("host === \"medium.com\""));
        assert!(!body.contains("dnsDomainIs(host, \".medium.com\")"));
        assert!(!body.contains("dnsDomainIs(host, \"reddit.com\")"));
        assert!(body.ends_with("  return \"DIRECT\";\n}\n"));
    }

    #[test]
    fn emits_only_canonical_ascii_host_constants() {
        let body = render_pac(
            "127.0.0.1:8080".parse().unwrap(),
            &[rule("BÜCHER.Example.", true)],
        );

        assert!(body.contains("xn--bcher-kva.example"));
        assert!(!body.contains("BÜCHER"));
    }

    async fn read_response(stream: &mut TcpStream) -> Vec<u8> {
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    }

    #[tokio::test]
    async fn pac_server_serves_only_bounded_get_proxy_pac_and_survives_bad_clients() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let server = PacServer::new(
            listener,
            Arc::<str>::from("fixed-body"),
            cancel.clone(),
            128,
            Duration::from_secs(1),
            Duration::from_millis(100),
            2,
        );
        let task = tokio::spawn(server.run());

        let cases = [
            ("POST /proxy.pac HTTP/1.1\r\n\r\n", 405, ""),
            ("GET /other HTTP/1.1\r\n\r\n", 404, ""),
            ("GET /proxy.pac HTTP/1.1\r\n\r\n", 200, "fixed-body"),
        ];
        for (request, status, body) in cases {
            let mut client = TcpStream::connect(address).await.unwrap();
            client.write_all(request.as_bytes()).await.unwrap();
            let response = read_response(&mut client).await;
            let text = String::from_utf8(response).unwrap();
            assert!(text.starts_with(&format!("HTTP/1.1 {status} ")));
            assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
            assert!(text.ends_with(body));
        }

        let mut oversized = TcpStream::connect(address).await.unwrap();
        oversized.write_all(&[b'a'; 128]).await.unwrap();
        let response = read_response(&mut oversized).await;
        assert!(response.starts_with(b"HTTP/1.1 431"));

        cancel.cancel();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(TcpStream::connect(address).await.is_err());
    }

    #[tokio::test]
    async fn pac_cancellation_aborts_a_stalled_child_after_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(
            PacServer::new(
                listener,
                Arc::<str>::from("body"),
                cancel.clone(),
                128,
                Duration::from_secs(10),
                Duration::from_millis(20),
                1,
            )
            .run(),
        );
        let mut client = TcpStream::connect(address).await.unwrap();
        client.write_all(b"GET ").await.unwrap();
        cancel.cancel();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let mut rest = Vec::new();
        let _ = client.read_to_end(&mut rest).await;
    }
}
