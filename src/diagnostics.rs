#![allow(dead_code)]

use std::fmt;
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
    Cancelled,
    InternalError,
}

impl Outcome {
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
        }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::DomainName;

    use super::{Outcome, TunnelReport};

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
}
