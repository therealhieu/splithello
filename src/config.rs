use std::{
    collections::HashSet,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::{DomainError, DomainName, TargetMatcher, TargetRule};

const DEFAULT_ALLOWED_PORTS: &[u16] = &[443];
const SAFE_CONNECT_PORTS: &[u16] = &[443, 8443];
const MAX_CONNECT_HEADER_BYTES: usize = 65_536;
const MAX_CLIENT_HELLO_BYTES: usize = 1_048_576;
const MAX_CONNECTIONS: usize = 4_096;
const MAX_DNS_RESPONSE_BYTES: usize = 1_048_576;
const MAX_DNS_CACHE_CAPACITY: usize = 4_096;
const MAX_TIMEOUT_SECONDS: u64 = 300;
const MAX_SHUTDOWN_SECONDS: u64 = 60;
const MAX_CACHE_TTL_SECONDS: u64 = 86_400;

// These validated contracts are consumed incrementally by later implementation groups.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct Config {
    proxy: ProxyConfig,
    dns: DnsConfig,
    limits: RuntimeLimits,
    targets: Vec<TargetRule>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ProxyConfig {
    listen: SocketAddr,
    pac_listen: Option<SocketAddr>,
    allowed_ports: Vec<u16>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct DnsConfig {
    upstream: reqwest::Url,
    ca_certificate_pem: Option<Vec<u8>>,
    timeout: Duration,
    cache_max_ttl: Duration,
    cache_capacity: usize,
    max_response_bytes: usize,
}

#[allow(dead_code)]
impl DnsConfig {
    pub(crate) fn upstream(&self) -> &reqwest::Url {
        &self.upstream
    }

    pub(crate) fn ca_certificate_pem(&self) -> Option<&[u8]> {
        self.ca_certificate_pem.as_deref()
    }

    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }

    pub(crate) fn cache_max_ttl(&self) -> Duration {
        self.cache_max_ttl
    }

    pub(crate) fn cache_capacity(&self) -> usize {
        self.cache_capacity
    }

    pub(crate) fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct RuntimeLimits {
    max_connect_header_bytes: usize,
    max_client_hello_bytes: usize,
    max_connections: usize,
    connect_timeout: Duration,
    client_hello_timeout: Duration,
    idle_timeout: Duration,
    shutdown_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Strategy {
    TlsRecordSplit,
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("could not read configuration {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML configuration: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid proxy listener `{value}`")]
    InvalidListener { value: String },
    #[error("proxy listener must be loopback: {address}")]
    NonLoopbackListener { address: SocketAddr },
    #[error("proxy and PAC listeners must use different addresses")]
    ListenerConflict,
    #[error("allowed_ports must not be empty")]
    EmptyAllowedPorts,
    #[error("CONNECT port {port} is not approved; allowed values are 443 and 8443")]
    UnsafePort { port: u16 },
    #[error("CONNECT port {port} is duplicated")]
    DuplicatePort { port: u16 },
    #[error("invalid DoH upstream URL: {value}")]
    InvalidDohUrl { value: String },
    #[error("DoH upstream must use HTTPS")]
    InsecureDohUrl,
    #[error("DoH upstream must not contain credentials or a fragment")]
    UnsafeDohUrl,
    #[error("could not read additional CA certificate {path}: {source}")]
    ReadCaCertificate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("additional CA certificate {path} is not valid PEM")]
    InvalidCaCertificate { path: PathBuf },
    #[error("at least one target is required")]
    EmptyTargets,
    #[error("invalid target policy: {0}")]
    Domain(#[from] DomainError),
    #[error("{field} must be between {min} and {max}, got {actual}")]
    OutOfBounds {
        field: &'static str,
        min: u64,
        max: u64,
        actual: u64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    proxy: RawProxyConfig,
    dns: RawDnsConfig,
    #[serde(default)]
    limits: RawRuntimeLimits,
    targets: Vec<RawTargetConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProxyConfig {
    listen: String,
    pac_listen: Option<String>,
    #[serde(default = "default_allowed_ports")]
    allowed_ports: Vec<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDnsConfig {
    upstream: String,
    ca_certificate: Option<PathBuf>,
    #[serde(default = "default_dns_timeout_seconds")]
    timeout_seconds: u64,
    #[serde(default = "default_cache_max_ttl_seconds")]
    cache_max_ttl_seconds: u64,
    #[serde(default = "default_cache_capacity")]
    cache_capacity: usize,
    #[serde(default = "default_dns_response_bytes")]
    max_response_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTargetConfig {
    host: String,
    #[serde(default)]
    include_subdomains: bool,
    strategy: Strategy,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawRuntimeLimits {
    max_connect_header_bytes: usize,
    max_client_hello_bytes: usize,
    max_connections: usize,
    connect_timeout_seconds: u64,
    client_hello_timeout_seconds: u64,
    idle_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
}

impl Default for RawRuntimeLimits {
    fn default() -> Self {
        Self {
            max_connect_header_bytes: 16_384,
            max_client_hello_bytes: 262_144,
            max_connections: 256,
            connect_timeout_seconds: 10,
            client_hello_timeout_seconds: 5,
            idle_timeout_seconds: 300,
            shutdown_timeout_seconds: 10,
        }
    }
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Self::parse(&contents, base_dir)
    }

    fn parse(contents: &str, base_dir: &Path) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(contents)?;
        Self::validate(raw, base_dir)
    }

    fn validate(raw: RawConfig, base_dir: &Path) -> Result<Self, ConfigError> {
        let listen = parse_loopback_listener(&raw.proxy.listen)?;
        let pac_listen = raw
            .proxy
            .pac_listen
            .as_deref()
            .map(parse_loopback_listener)
            .transpose()?;
        if pac_listen == Some(listen) && listen.port() != 0 {
            return Err(ConfigError::ListenerConflict);
        }

        let allowed_ports = validate_allowed_ports(raw.proxy.allowed_ports, listen.port() == 0)?;
        let upstream = validate_doh_url(&raw.dns.upstream)?;
        let ca_certificate_pem = load_ca_certificate(raw.dns.ca_certificate, base_dir)?;

        let dns = DnsConfig {
            upstream,
            ca_certificate_pem,
            timeout: duration(
                "dns.timeout_seconds",
                raw.dns.timeout_seconds,
                MAX_TIMEOUT_SECONDS,
            )?,
            cache_max_ttl: duration(
                "dns.cache_max_ttl_seconds",
                raw.dns.cache_max_ttl_seconds,
                MAX_CACHE_TTL_SECONDS,
            )?,
            cache_capacity: bounded_usize(
                "dns.cache_capacity",
                raw.dns.cache_capacity,
                MAX_DNS_CACHE_CAPACITY,
            )?,
            max_response_bytes: bounded_usize(
                "dns.max_response_bytes",
                raw.dns.max_response_bytes,
                MAX_DNS_RESPONSE_BYTES,
            )?,
        };

        let limits = RuntimeLimits {
            max_connect_header_bytes: bounded_usize(
                "limits.max_connect_header_bytes",
                raw.limits.max_connect_header_bytes,
                MAX_CONNECT_HEADER_BYTES,
            )?,
            max_client_hello_bytes: bounded_usize(
                "limits.max_client_hello_bytes",
                raw.limits.max_client_hello_bytes,
                MAX_CLIENT_HELLO_BYTES,
            )?,
            max_connections: bounded_usize(
                "limits.max_connections",
                raw.limits.max_connections,
                MAX_CONNECTIONS,
            )?,
            connect_timeout: duration(
                "limits.connect_timeout_seconds",
                raw.limits.connect_timeout_seconds,
                MAX_TIMEOUT_SECONDS,
            )?,
            client_hello_timeout: duration(
                "limits.client_hello_timeout_seconds",
                raw.limits.client_hello_timeout_seconds,
                MAX_TIMEOUT_SECONDS,
            )?,
            idle_timeout: duration(
                "limits.idle_timeout_seconds",
                raw.limits.idle_timeout_seconds,
                MAX_TIMEOUT_SECONDS,
            )?,
            shutdown_timeout: duration(
                "limits.shutdown_timeout_seconds",
                raw.limits.shutdown_timeout_seconds,
                MAX_SHUTDOWN_SECONDS,
            )?,
        };

        let targets = validate_targets(raw.targets)?;

        Ok(Self {
            proxy: ProxyConfig {
                listen,
                pac_listen,
                allowed_ports,
            },
            dns,
            limits,
            targets,
        })
    }

    pub(crate) fn proxy(&self) -> &ProxyConfig {
        &self.proxy
    }

    pub(crate) fn dns(&self) -> &DnsConfig {
        &self.dns
    }

    pub(crate) fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    pub(crate) fn targets(&self) -> &[TargetRule] {
        &self.targets
    }
}

impl ProxyConfig {
    pub(crate) fn listen(&self) -> SocketAddr {
        self.listen
    }

    pub(crate) fn pac_listen(&self) -> Option<SocketAddr> {
        self.pac_listen
    }

    pub(crate) fn allowed_ports(&self) -> &[u16] {
        &self.allowed_ports
    }
}

impl RuntimeLimits {
    pub(crate) fn max_connect_header_bytes(&self) -> usize {
        self.max_connect_header_bytes
    }

    pub(crate) fn max_client_hello_bytes(&self) -> usize {
        self.max_client_hello_bytes
    }

    pub(crate) fn max_connections(&self) -> usize {
        self.max_connections
    }

    pub(crate) fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub(crate) fn client_hello_timeout(&self) -> Duration {
        self.client_hello_timeout
    }

    pub(crate) fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    pub(crate) fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }
}

fn parse_loopback_listener(value: &str) -> Result<SocketAddr, ConfigError> {
    let address = SocketAddr::from_str(value).map_err(|_| ConfigError::InvalidListener {
        value: value.to_owned(),
    })?;
    if !address.ip().is_loopback() {
        return Err(ConfigError::NonLoopbackListener { address });
    }
    Ok(address)
}

fn validate_allowed_ports(
    ports: Vec<u16>,
    allow_ephemeral_test_ports: bool,
) -> Result<Vec<u16>, ConfigError> {
    if ports.is_empty() {
        return Err(ConfigError::EmptyAllowedPorts);
    }
    let mut unique = HashSet::with_capacity(ports.len());
    for &port in &ports {
        if !SAFE_CONNECT_PORTS.contains(&port) && !allow_ephemeral_test_ports {
            return Err(ConfigError::UnsafePort { port });
        }
        if !unique.insert(port) {
            return Err(ConfigError::DuplicatePort { port });
        }
    }
    Ok(ports)
}

fn validate_doh_url(value: &str) -> Result<reqwest::Url, ConfigError> {
    let url = reqwest::Url::parse(value).map_err(|_| ConfigError::InvalidDohUrl {
        value: value.to_owned(),
    })?;
    if url.scheme() != "https" || url.host().is_none() {
        return Err(ConfigError::InsecureDohUrl);
    }
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(ConfigError::UnsafeDohUrl);
    }
    Ok(url)
}

fn load_ca_certificate(
    path: Option<PathBuf>,
    base_dir: &Path,
) -> Result<Option<Vec<u8>>, ConfigError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let resolved = if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    };
    let pem = fs::read(&resolved).map_err(|source| ConfigError::ReadCaCertificate {
        path: resolved.clone(),
        source,
    })?;
    let certificates = reqwest::tls::Certificate::from_pem_bundle(&pem).map_err(|_| {
        ConfigError::InvalidCaCertificate {
            path: resolved.clone(),
        }
    })?;
    if certificates.is_empty() {
        return Err(ConfigError::InvalidCaCertificate { path: resolved });
    }
    Ok(Some(pem))
}

fn validate_targets(raw_targets: Vec<RawTargetConfig>) -> Result<Vec<TargetRule>, ConfigError> {
    if raw_targets.is_empty() {
        return Err(ConfigError::EmptyTargets);
    }
    let rules = raw_targets
        .into_iter()
        .map(|raw| {
            Ok(TargetRule::new(
                DomainName::parse(&raw.host)?,
                raw.include_subdomains,
                raw.strategy,
            ))
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    Ok(TargetMatcher::new(rules)?.into_rules())
}

fn bounded_usize(field: &'static str, value: usize, max: usize) -> Result<usize, ConfigError> {
    if value == 0 || value > max {
        return Err(ConfigError::OutOfBounds {
            field,
            min: 1,
            max: max as u64,
            actual: value as u64,
        });
    }
    Ok(value)
}

fn duration(field: &'static str, seconds: u64, max: u64) -> Result<Duration, ConfigError> {
    if seconds == 0 || seconds > max {
        return Err(ConfigError::OutOfBounds {
            field,
            min: 1,
            max,
            actual: seconds,
        });
    }
    Ok(Duration::from_secs(seconds))
}

fn default_allowed_ports() -> Vec<u16> {
    DEFAULT_ALLOWED_PORTS.to_vec()
}

fn default_dns_timeout_seconds() -> u64 {
    10
}

fn default_cache_max_ttl_seconds() -> u64 {
    300
}

fn default_cache_capacity() -> usize {
    256
}

fn default_dns_response_bytes() -> usize {
    65_536
}

#[cfg(test)]
mod tests {
    use std::{fs, net::SocketAddr, time::Duration};

    use tempfile::tempdir;

    use crate::domain::DomainError;

    use super::{Config, ConfigError, Strategy};

    #[derive(Debug)]
    enum ExpectedConfigError {
        Toml,
        InvalidListener(&'static str),
        NonLoopbackListener(SocketAddr),
        ListenerConflict,
        InsecureDohUrl,
        UnsafeDohUrl,
        DomainDuplicate(&'static str),
        DomainIpLiteral(&'static str),
        EmptyTargets,
        UnsafePort(u16),
        DuplicatePort(u16),
        OutOfBounds {
            field: &'static str,
            actual: u64,
            max: u64,
        },
    }

    impl ExpectedConfigError {
        fn matches(&self, actual: &ConfigError) -> bool {
            match (self, actual) {
                (Self::Toml, ConfigError::Toml(_))
                | (Self::ListenerConflict, ConfigError::ListenerConflict)
                | (Self::InsecureDohUrl, ConfigError::InsecureDohUrl)
                | (Self::UnsafeDohUrl, ConfigError::UnsafeDohUrl)
                | (Self::EmptyTargets, ConfigError::EmptyTargets) => true,
                (
                    Self::InvalidListener(expected),
                    ConfigError::InvalidListener { value: actual },
                ) => expected == actual,
                (
                    Self::NonLoopbackListener(expected),
                    ConfigError::NonLoopbackListener { address: actual },
                ) => expected == actual,
                (
                    Self::DomainDuplicate(expected),
                    ConfigError::Domain(DomainError::Duplicate { host: actual }),
                ) => expected == &actual.as_str(),
                (
                    Self::DomainIpLiteral(expected),
                    ConfigError::Domain(DomainError::IpLiteral { input: actual }),
                ) => expected == actual,
                (Self::UnsafePort(expected), ConfigError::UnsafePort { port: actual })
                | (Self::DuplicatePort(expected), ConfigError::DuplicatePort { port: actual }) => {
                    expected == actual
                }
                (
                    Self::OutOfBounds {
                        field: expected_field,
                        actual: expected_actual,
                        max: expected_max,
                    },
                    ConfigError::OutOfBounds {
                        field: actual_field,
                        min: 1,
                        max: actual_max,
                        actual: actual_value,
                    },
                ) => {
                    expected_field == actual_field
                        && expected_actual == actual_value
                        && expected_max == actual_max
                }
                _ => false,
            }
        }
    }

    const VALID: &str = r#"
[proxy]
listen = "127.0.0.1:8080"
pac_listen = "[::1]:8081"

[dns]
upstream = "https://1.1.1.1/dns-query"

[[targets]]
host = "reddit.com"
include_subdomains = true
strategy = "tls-record-split"

[[targets]]
host = "medium.com"
include_subdomains = true
strategy = "tls-record-split"
"#;

    #[test]
    fn representative_config_loads_and_materializes_defaults() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        fs::write(&path, VALID).expect("write valid configuration");
        let config = Config::load(&path).expect("valid configuration");

        assert_eq!(
            config.proxy.listen,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.proxy.pac_listen, Some("[::1]:8081".parse().unwrap()));
        assert_eq!(config.proxy.allowed_ports, vec![443]);
        assert_eq!(config.dns.upstream.as_str(), "https://1.1.1.1/dns-query");
        assert!(config.dns.ca_certificate_pem.is_none());
        assert_eq!(config.dns.timeout, Duration::from_secs(10));
        assert_eq!(config.dns.cache_max_ttl, Duration::from_secs(300));
        assert_eq!(config.dns.cache_capacity, 256);
        assert_eq!(config.dns.max_response_bytes, 65_536);
        assert_eq!(config.limits.max_connect_header_bytes, 16_384);
        assert_eq!(config.limits.max_client_hello_bytes, 262_144);
        assert_eq!(config.limits.max_connections, 256);
        assert_eq!(config.limits.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.limits.client_hello_timeout, Duration::from_secs(5));
        assert_eq!(config.limits.idle_timeout, Duration::from_secs(300));
        assert_eq!(config.limits.shutdown_timeout, Duration::from_secs(10));
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.targets[0].host().as_str(), "reddit.com");
        assert!(config.targets[0].includes_subdomains());
        assert_eq!(config.targets[0].strategy(), Strategy::TlsRecordSplit);
    }

    #[test]
    fn rejects_unknown_missing_and_unsafe_values_with_exact_errors() {
        let directory = tempdir().expect("temporary directory");
        let cases = [
            (
                "unknown top-level field",
                VALID.replace("[proxy]", "unknown = true\n[proxy]"),
                ExpectedConfigError::Toml,
            ),
            (
                "unknown nested field",
                VALID.replace("[dns]", "[dns]\nunknown = true"),
                ExpectedConfigError::Toml,
            ),
            (
                "missing required listener",
                VALID.replace("listen = \"127.0.0.1:8080\"", ""),
                ExpectedConfigError::Toml,
            ),
            (
                "malformed listener",
                VALID.replace("127.0.0.1:8080", "not-an-address"),
                ExpectedConfigError::InvalidListener("not-an-address"),
            ),
            (
                "non-loopback listener",
                VALID.replace("127.0.0.1:8080", "0.0.0.0:8080"),
                ExpectedConfigError::NonLoopbackListener("0.0.0.0:8080".parse().unwrap()),
            ),
            (
                "conflicting listeners",
                VALID.replace("[::1]:8081", "127.0.0.1:8080"),
                ExpectedConfigError::ListenerConflict,
            ),
            (
                "insecure DoH URL",
                VALID.replace("https://1.1.1.1", "http://1.1.1.1"),
                ExpectedConfigError::InsecureDohUrl,
            ),
            (
                "credential-bearing DoH URL",
                VALID.replace(
                    "https://1.1.1.1/dns-query",
                    "https://user:pass@1.1.1.1/dns-query",
                ),
                ExpectedConfigError::UnsafeDohUrl,
            ),
            (
                "unsupported strategy",
                VALID.replace("strategy = \"tls-record-split\"", "strategy = \"unknown\""),
                ExpectedConfigError::Toml,
            ),
            (
                "canonical duplicate target",
                VALID.replace("medium.com", "Reddit.com."),
                ExpectedConfigError::DomainDuplicate("reddit.com"),
            ),
            (
                "IP literal target",
                VALID.replace("reddit.com", "127.0.0.1"),
                ExpectedConfigError::DomainIpLiteral("127.0.0.1"),
            ),
            (
                "empty targets",
                format!(
                    "targets = []\n{}",
                    &VALID[..VALID.find("[[targets]]").unwrap()]
                ),
                ExpectedConfigError::EmptyTargets,
            ),
            (
                "zero bound",
                format!("{VALID}\n[limits]\nmax_connections = 0\n"),
                ExpectedConfigError::OutOfBounds {
                    field: "limits.max_connections",
                    actual: 0,
                    max: 4_096,
                },
            ),
            (
                "excessive bound",
                format!("{VALID}\n[limits]\nmax_connections = 4097\n"),
                ExpectedConfigError::OutOfBounds {
                    field: "limits.max_connections",
                    actual: 4_097,
                    max: 4_096,
                },
            ),
            (
                "disallowed port",
                VALID.replace("[proxy]", "[proxy]\nallowed_ports = [80]"),
                ExpectedConfigError::UnsafePort(80),
            ),
            (
                "duplicate port",
                VALID.replace("[proxy]", "[proxy]\nallowed_ports = [443, 443]"),
                ExpectedConfigError::DuplicatePort(443),
            ),
        ];

        for (case, invalid, expected) in cases {
            let error = Config::parse(&invalid, directory.path())
                .expect_err("invalid configuration must be rejected");
            assert!(
                expected.matches(&error),
                "{case}: expected {expected:?}, got {error:?}"
            );
        }
    }

    #[test]
    fn ephemeral_loopback_listener_mode_is_scoped_and_keeps_production_ports_safe() {
        let directory = tempdir().expect("temporary directory");
        let ephemeral = VALID
            .replace("127.0.0.1:8080", "127.0.0.1:0")
            .replace("[::1]:8081", "127.0.0.1:0")
            .replace("[proxy]", "[proxy]\nallowed_ports = [49199]");
        let config = Config::parse(&ephemeral, directory.path()).expect("ephemeral test config");
        assert_eq!(config.proxy.listen.port(), 0);
        assert_eq!(config.proxy.pac_listen.unwrap().port(), 0);
        assert_eq!(config.proxy.allowed_ports, vec![49199]);
        assert!(config.proxy.listen.port() == 0);

        let production = VALID.replace("[proxy]", "[proxy]\nallowed_ports = [49199]");
        assert!(matches!(
            Config::parse(&production, directory.path()),
            Err(ConfigError::UnsafePort { port: 49199 })
        ));
    }

    #[test]
    fn repository_example_and_readme_define_the_same_operating_contract() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let config = Config::load(&root.join("config.example.toml"))
            .expect("repository example must load through production Config::load");

        assert_eq!(
            config.proxy.listen,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.proxy.pac_listen,
            Some("127.0.0.1:8081".parse::<SocketAddr>().unwrap())
        );
        assert_eq!(config.proxy.allowed_ports, vec![443]);
        assert_ne!(config.proxy.listen.port(), 0);
        assert_eq!(config.dns.upstream.as_str(), "https://1.1.1.1/dns-query");
        assert!(config.dns.ca_certificate_pem.is_none());
        assert_eq!(config.dns.timeout, Duration::from_secs(10));
        assert_eq!(config.dns.cache_max_ttl, Duration::from_secs(300));
        assert_eq!(config.dns.cache_capacity, 256);
        assert_eq!(config.dns.max_response_bytes, 65_536);
        assert_eq!(config.limits.max_connect_header_bytes, 16_384);
        assert_eq!(config.limits.max_client_hello_bytes, 262_144);
        assert_eq!(config.limits.max_connections, 256);
        assert_eq!(config.limits.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.limits.client_hello_timeout, Duration::from_secs(5));
        assert_eq!(config.limits.idle_timeout, Duration::from_secs(300));
        assert_eq!(config.limits.shutdown_timeout, Duration::from_secs(10));
        assert_eq!(config.targets.len(), 2);
        for (rule, host) in config.targets.iter().zip(["reddit.com", "medium.com"]) {
            assert_eq!(rule.host().as_str(), host);
            assert!(rule.includes_subdomains());
            assert_eq!(rule.strategy(), Strategy::TlsRecordSplit);
        }

        let readme = fs::read_to_string(root.join("README.md")).expect("read repository README");
        for required in [
            "cargo build --release",
            "./target/release/splithello start --config config.example.toml",
            "127.0.0.1:8080",
            "http://127.0.0.1:8081/proxy.pac",
            "outcome=ready",
            "outcome=target-transformed",
            "outcome=direct-relay",
            "outcome=dns-error",
            "outcome=connect-error",
            "outcome=tls-error",
            "not a VPN",
            "does not provide anonymity",
            "does not hide your client IP",
            "cannot guarantee access when destination IPs are blocked",
            "IPv6, QUIC, and HTTP/3 are not covered for selected targets",
            "does not require root, firewall rules, raw sockets, or system changes",
            "The DoH endpoint host must be bootstrap-reachable without resolving any configured selected or blocked domain",
            "Prefer a literal-IP HTTPS endpoint whose certificate is valid for that IP",
            "an independently resolvable non-target hostname",
            "HTTPS certificate authentication is mandatory",
            "dns.ca_certificate",
            "may add trusted CA roots but never disables verification",
            "Successful startup does not prove bypass success",
            "Stop the process",
            "remove the HTTPS proxy or PAC setting",
        ] {
            assert!(readme.contains(required), "README is missing `{required}`");
        }
        let example =
            fs::read_to_string(root.join("config.example.toml")).expect("read repository example");
        for required in [
            "bootstrap-reachable without resolving a selected/blocked domain",
            "literal-IP HTTPS endpoint with a certificate valid for that IP",
            "independently resolvable non-target host",
            "HTTPS authentication is mandatory",
            "ca_certificate may add trusted roots but never disables certificate verification",
        ] {
            assert!(
                example.contains(required),
                "config example is missing `{required}`"
            );
        }

        for prohibited in [
            "iptables ",
            "nft ",
            "tcp sequence",
            "low TTL",
            "fake packet",
        ] {
            assert!(
                !readme
                    .to_ascii_lowercase()
                    .contains(&prohibited.to_ascii_lowercase()),
                "README must not include packet recipe surface `{prohibited}`"
            );
        }
    }

    #[test]
    fn validates_additional_ca_pem_during_loading() {
        let directory = tempdir().expect("temporary directory");
        let invalid_ca = directory.path().join("invalid.pem");
        fs::write(&invalid_ca, "not a certificate").expect("write invalid CA");

        let with_missing_ca = VALID.replace(
            "upstream = \"https://1.1.1.1/dns-query\"",
            "upstream = \"https://1.1.1.1/dns-query\"\nca_certificate = \"missing.pem\"",
        );
        assert!(matches!(
            Config::parse(&with_missing_ca, directory.path()),
            Err(ConfigError::ReadCaCertificate { .. })
        ));

        let with_invalid_ca = VALID.replace(
            "upstream = \"https://1.1.1.1/dns-query\"",
            "upstream = \"https://1.1.1.1/dns-query\"\nca_certificate = \"invalid.pem\"",
        );
        let result = Config::parse(&with_invalid_ca, directory.path());
        assert!(
            matches!(result, Err(ConfigError::InvalidCaCertificate { .. })),
            "unexpected result: {result:?}"
        );
    }
}
