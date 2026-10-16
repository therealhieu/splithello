#![allow(dead_code)]

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::domain::DomainName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Ready,
    DirectRelay,
    TargetTransformed,
    UnsupportedRequest,
    ConfigError,
    DnsError,
    ConnectError,
    TlsError,
    Timeout,
    /// A relayed tunnel closed after `idle_timeout` with no traffic: housekeeping, not a failure.
    Idle,
    /// A peer hung up abruptly after the site had replied: an ending, not a failure.
    Reset,
    Cancelled,
    InternalError,
}

impl Outcome {
    pub(crate) const ALL: [Self; 13] = [
        Self::Ready,
        Self::DirectRelay,
        Self::TargetTransformed,
        Self::UnsupportedRequest,
        Self::ConfigError,
        Self::DnsError,
        Self::ConnectError,
        Self::TlsError,
        Self::Timeout,
        Self::Idle,
        Self::Reset,
        Self::Cancelled,
        Self::InternalError,
    ];

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|outcome| outcome.as_str() == value)
    }

    /// Short column label for human-readable logs.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::DirectRelay => "direct",
            Self::TargetTransformed => "split",
            Self::UnsupportedRequest => "request",
            Self::ConfigError => "config",
            Self::DnsError => "dns",
            Self::ConnectError => "connect",
            Self::TlsError => "tls",
            Self::Timeout => "timeout",
            Self::Idle => "idle",
            Self::Reset => "reset",
            Self::Cancelled => "cancel",
            Self::InternalError => "internal",
        }
    }

    /// What a user can do about a failure; `None` for successful outcomes.
    pub(crate) const fn hint(self) -> Option<&'static str> {
        match self {
            Self::Ready
            | Self::DirectRelay
            | Self::TargetTransformed
            | Self::Idle
            | Self::Reset
            | Self::Cancelled => None,
            Self::UnsupportedRequest => {
                Some("only HTTPS CONNECT is proxied; check the browser proxy settings")
            }
            Self::ConfigError => Some("fix the configuration, then run `splithello check`"),
            Self::DnsError => Some("check that [dns].upstream is reachable (`splithello check`)"),
            Self::ConnectError => Some("the site or its address may be blocked or down"),
            Self::TlsError => Some("the TLS ClientHello was malformed or named another host"),
            Self::Timeout => Some("a deadline expired; the network or site may be slow"),
            Self::InternalError => Some("unexpected; please report it with the log line"),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::DirectRelay => "direct-relay",
            Self::TargetTransformed => "target-transformed",
            Self::UnsupportedRequest => "unsupported-request",
            Self::ConfigError => "config-error",
            Self::DnsError => "dns-error",
            Self::ConnectError => "connect-error",
            Self::TlsError => "tls-error",
            Self::Timeout => "timeout",
            Self::Idle => "idle-timeout",
            Self::Reset => "peer-reset",
            Self::Cancelled => "cancelled",
            Self::InternalError => "internal-error",
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TunnelReport {
    host: DomainName,
    port: u16,
    target: bool,
    outcome: Outcome,
    duration: Duration,
    bytes_up: u64,
    bytes_down: u64,
    error: Option<String>,
}

impl TunnelReport {
    pub(crate) fn new(
        host: DomainName,
        port: u16,
        target: bool,
        outcome: Outcome,
        duration: Duration,
        bytes_up: u64,
        bytes_down: u64,
    ) -> Self {
        Self {
            host,
            port,
            target,
            outcome,
            duration,
            bytes_up,
            bytes_down,
            error: None,
        }
    }

    /// Attaches the failure's cause, rendered with its whole `source()` chain.
    pub(crate) fn with_error(mut self, error: &dyn std::error::Error) -> Self {
        self.error = Some(error_chain(error));
        self
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn host(&self) -> &DomainName {
        &self.host
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn is_target(&self) -> bool {
        self.target
    }

    pub(crate) fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub(crate) fn duration(&self) -> Duration {
        self.duration
    }

    pub(crate) fn bytes_up(&self) -> u64 {
        self.bytes_up
    }

    pub(crate) fn bytes_down(&self) -> u64 {
        self.bytes_down
    }
}

impl fmt::Display for TunnelReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "host={} port={} target={} outcome={} duration_ms={} bytes_up={} bytes_down={}",
            self.host,
            self.port,
            self.target,
            self.outcome,
            self.duration.as_millis(),
            self.bytes_up,
            self.bytes_down
        )
    }
}

/// Emits one tunnel's report: splits at INFO, direct relays and cancellations at DEBUG
/// (the bulk of browser traffic), failures at WARN with a hint.
pub(crate) fn log_tunnel(report: &TunnelReport) {
    macro_rules! emit {
        ($level:expr) => {
            tracing::event!(
                $level,
                host = %report.host(),
                port = report.port(),
                target = report.is_target(),
                outcome = %report.outcome(),
                duration_ms = report.duration().as_millis() as u64,
                bytes_up = report.bytes_up(),
                bytes_down = report.bytes_down(),
                error = report.error(),
                hint = report.outcome().hint(),
                "tunnel completed"
            )
        };
    }
    match report.outcome() {
        Outcome::TargetTransformed => emit!(tracing::Level::INFO),
        Outcome::Idle | Outcome::Reset if report.is_target() => emit!(tracing::Level::INFO),
        Outcome::DirectRelay | Outcome::Idle | Outcome::Reset | Outcome::Cancelled => {
            emit!(tracing::Level::DEBUG)
        }
        _ => emit!(tracing::Level::WARN),
    }
}

/// Emits a tunnel that ended before a CONNECT host was known.
pub(crate) fn log_tunnel_error(outcome: Outcome, error: &dyn fmt::Display) {
    macro_rules! emit {
        ($level:expr) => {
            tracing::event!(
                $level,
                outcome = %outcome,
                error = %error,
                hint = outcome.hint(),
                "tunnel ended before a report was available"
            )
        };
    }
    if outcome == Outcome::Cancelled {
        emit!(tracing::Level::DEBUG);
    } else {
        emit!(tracing::Level::WARN);
    }
}

/// `outer: inner: innermost`, so a log line shows the root cause (e.g. a socket error)
/// and not only the top-level "DoH request failed".
pub(crate) fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        // `#[error(transparent)]` and some wrappers repeat their source's message.
        if !chain.ends_with(&cause_text) {
            chain.push_str(": ");
            chain.push_str(&cause_text);
        }
        source = cause.source();
    }
    chain
}

/// Live tunnel counters for the periodic summary line.
#[derive(Debug, Default)]
pub(crate) struct ProxyStats {
    active: AtomicU64,
    split: AtomicU64,
    direct: AtomicU64,
    failed: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WindowSummary {
    pub(crate) split: u64,
    pub(crate) direct: u64,
    pub(crate) failed: u64,
    pub(crate) active: u64,
}

impl WindowSummary {
    pub(crate) const fn tunnels(&self) -> u64 {
        self.split + self.direct + self.failed
    }
}

impl ProxyStats {
    pub(crate) fn opened(&self) {
        self.active.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn closed(&self, outcome: Outcome) {
        self.active.fetch_sub(1, Ordering::Relaxed);
        let counter = match outcome {
            Outcome::TargetTransformed => &self.split,
            Outcome::DirectRelay => &self.direct,
            // Endings, not results: neither counts as split, direct, or failed.
            Outcome::Idle | Outcome::Reset | Outcome::Cancelled => return,
            _ => &self.failed,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns the counts since the previous call and resets them.
    pub(crate) fn take_window(&self) -> WindowSummary {
        WindowSummary {
            split: self.split.swap(0, Ordering::Relaxed),
            direct: self.direct.swap(0, Ordering::Relaxed),
            failed: self.failed.swap(0, Ordering::Relaxed),
            active: self.active.load(Ordering::Relaxed),
        }
    }
}

pub(crate) fn log_summary(window: Duration, summary: WindowSummary) {
    tracing::info!(
        window = %humantime::format_duration(window),
        tunnels = summary.tunnels(),
        split = summary.split,
        direct = summary.direct,
        failed = summary.failed,
        active = summary.active,
        "summary"
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::DomainName;

    use super::{Outcome, ProxyStats, TunnelReport, WindowSummary, error_chain};

    #[test]
    fn every_outcome_parses_back_and_failures_carry_hints() {
        for outcome in Outcome::ALL {
            assert_eq!(Outcome::parse(outcome.as_str()), Some(outcome));
            let failure = !matches!(
                outcome,
                Outcome::Ready
                    | Outcome::DirectRelay
                    | Outcome::TargetTransformed
                    | Outcome::Idle
                    | Outcome::Reset
                    | Outcome::Cancelled
            );
            assert_eq!(outcome.hint().is_some(), failure, "{outcome}");
        }
        assert_eq!(Outcome::parse("unknown"), None);
    }

    #[test]
    fn stats_count_a_window_and_reset() {
        let stats = ProxyStats::default();
        for outcome in [
            Outcome::TargetTransformed,
            Outcome::DirectRelay,
            Outcome::DirectRelay,
            Outcome::DnsError,
            Outcome::Idle,
            Outcome::Reset,
            Outcome::Cancelled,
        ] {
            stats.opened();
            stats.closed(outcome);
        }
        stats.opened();
        let window = stats.take_window();
        assert_eq!(
            window,
            WindowSummary {
                split: 1,
                direct: 2,
                failed: 1,
                active: 1,
            }
        );
        assert_eq!(window.tunnels(), 4);
        assert_eq!(stats.take_window().tunnels(), 0);
    }

    #[test]
    fn outcome_strings_are_stable_kebab_case() {
        let cases = [
            (Outcome::Ready, "ready"),
            (Outcome::DirectRelay, "direct-relay"),
            (Outcome::TargetTransformed, "target-transformed"),
            (Outcome::UnsupportedRequest, "unsupported-request"),
            (Outcome::ConfigError, "config-error"),
            (Outcome::DnsError, "dns-error"),
            (Outcome::ConnectError, "connect-error"),
            (Outcome::TlsError, "tls-error"),
            (Outcome::Timeout, "timeout"),
            (Outcome::Idle, "idle-timeout"),
            (Outcome::Reset, "peer-reset"),
            (Outcome::Cancelled, "cancelled"),
            (Outcome::InternalError, "internal-error"),
        ];

        for (outcome, expected) in cases {
            assert_eq!(outcome.as_str(), expected);
            assert_eq!(outcome.to_string(), expected);
        }
    }

    #[test]
    fn tunnel_report_formats_only_bounded_secret_safe_metadata() {
        let report = TunnelReport::new(
            DomainName::parse("ReDdIt.CoM.").unwrap(),
            443,
            true,
            Outcome::DnsError,
            Duration::from_millis(1250),
            12,
            34,
        );
        let secret = "authorization: bearer test-secret";

        let display = report.to_string();
        let debug = format!("{report:?}");

        assert_eq!(
            display,
            "host=reddit.com port=443 target=true outcome=dns-error duration_ms=1250 bytes_up=12 bytes_down=34"
        );
        assert!(!display.contains(secret));
        assert!(!debug.contains(secret));
        assert!(!display.contains("CONNECT "));
        assert!(!display.contains("ClientHello"));
    }

    #[test]
    fn error_chain_joins_sources_and_skips_repeated_messages() {
        #[derive(Debug, thiserror::Error)]
        enum Inner {
            #[error("connection reset by peer")]
            Reset,
        }
        #[derive(Debug, thiserror::Error)]
        enum Outer {
            #[error("DoH request failed")]
            Request(#[source] Inner),
            #[error(transparent)]
            Transparent(Inner),
        }

        assert_eq!(
            error_chain(&Outer::Request(Inner::Reset)),
            "DoH request failed: connection reset by peer"
        );
        assert_eq!(
            error_chain(&Outer::Transparent(Inner::Reset)),
            "connection reset by peer"
        );

        let report = TunnelReport::new(
            DomainName::parse("www.reddit.com").unwrap(),
            443,
            true,
            Outcome::DnsError,
            Duration::from_millis(83),
            0,
            0,
        );
        assert_eq!(report.error(), None);
        let report = report.with_error(&Outer::Request(Inner::Reset));
        assert_eq!(
            report.error(),
            Some("DoH request failed: connection reset by peer")
        );
    }
}
