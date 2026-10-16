use std::{
    collections::HashSet,
    fmt, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::domain::{DomainError, DomainName, TargetMatcher, TargetRule};

const DEFAULT_LISTEN: &str = "127.0.0.1:8080";
const DEFAULT_PAC_LISTEN: &str = "127.0.0.1:8081";
const DEFAULT_DOH_UPSTREAM: &str = "https://1.1.1.1/dns-query";
const DEFAULT_PORTS: &[u16] = &[443];
const SAFE_CONNECT_PORTS: &[u16] = &[443, 8443];
const MAX_CONNECT_HEADER_BYTES: usize = 65_536;
const MAX_CLIENT_HELLO_BYTES: usize = 1_048_576;
const MAX_CONNECTIONS: usize = 4_096;
const MAX_DNS_RESPONSE_BYTES: usize = 1_048_576;
const MAX_DNS_CACHE_SIZE: usize = 4_096;
const MAX_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_CACHE_TTL: Duration = Duration::from_secs(86_400);

// These validated contracts are consumed incrementally by later implementation groups.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct Config {
    proxy: ProxyConfig,
    dns: DnsConfig,
    limits: RuntimeLimits,
    update: UpdateConfig,
    targets: Vec<TargetRule>,
}

#[derive(Debug)]
pub(crate) struct UpdateConfig {
    check: bool,
}

impl UpdateConfig {
    /// Whether `start` and `check` look for a newer release.
    pub(crate) fn check(&self) -> bool {
        self.check
    }
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

impl Default for DnsConfig {
    /// The `[dns]` defaults, for commands that run without a usable configuration file.
    fn default() -> Self {
        Self::validate(RawDnsConfig::default(), Path::new("."))
            .expect("default DNS settings are valid")
    }
}

#[allow(dead_code)]
impl DnsConfig {
    fn validate(raw: RawDnsConfig, base_dir: &Path) -> Result<Self, ConfigError> {
        Ok(Self {
            upstream: validate_doh_url(&raw.upstream)?,
            ca_certificate_pem: load_ca_certificate(raw.ca_certificate, base_dir)?,
            timeout: bounded_duration("dns.timeout", raw.timeout, MAX_TIMEOUT)?,
            cache_max_ttl: bounded_duration("dns.cache_ttl", raw.cache_ttl, MAX_CACHE_TTL)?,
            cache_capacity: bounded_usize("dns.cache_size", raw.cache_size, MAX_DNS_CACHE_SIZE)?,
            max_response_bytes: bounded_usize(
                "dns.max_response_bytes",
                raw.max_response_bytes,
                MAX_DNS_RESPONSE_BYTES,
            )?,
        })
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Strategy {
    TlsRecordSplit,
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("no configuration at {}", path.display())]
    NotFound { path: PathBuf },
    #[error("could not read configuration {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML configuration: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid listener address `{value}`")]
    InvalidListener { value: String },
    #[error("listener must be loopback: {address}")]
    NonLoopbackListener { address: SocketAddr },
    #[error("proxy.listen and proxy.pac must use different addresses")]
    ListenerConflict,
    #[error("proxy.ports must not be empty")]
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
    #[error("could not read additional CA certificate {}: {source}", path.display())]
    ReadCaCertificate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("additional CA certificate {} is not valid PEM", path.display())]
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
    #[error(
        "{field} must be greater than 0 and at most {}, got {}",
        humantime::format_duration(*max),
        humantime::format_duration(*actual)
    )]
    DurationOutOfBounds {
        field: &'static str,
        max: Duration,
        actual: Duration,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    targets: Vec<RawTarget>,
    #[serde(default)]
    proxy: RawProxyConfig,
    #[serde(default)]
    dns: RawDnsConfig,
    #[serde(default)]
    limits: RawRuntimeLimits,
    #[serde(default)]
    update: RawUpdateConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawUpdateConfig {
    check: bool,
}

impl Default for RawUpdateConfig {
    fn default() -> Self {
        Self { check: true }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawProxyConfig {
    listen: String,
    pac: RawPac,
    ports: Vec<u16>,
}

impl Default for RawProxyConfig {
    fn default() -> Self {
        Self {
            listen: DEFAULT_LISTEN.to_owned(),
            pac: RawPac::Enabled(true),
            ports: DEFAULT_PORTS.to_vec(),
        }
    }
}

/// `pac = "127.0.0.1:8081"` sets the PAC listener, `pac = false` disables it,
/// `pac = true` (the default) uses the default address.
#[derive(Debug)]
enum RawPac {
    Enabled(bool),
    Address(String),
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawDnsConfig {
    upstream: String,
    ca_certificate: Option<PathBuf>,
    #[serde(deserialize_with = "human_duration")]
    timeout: Duration,
    #[serde(deserialize_with = "human_duration")]
    cache_ttl: Duration,
    cache_size: usize,
    max_response_bytes: usize,
}

impl Default for RawDnsConfig {
    fn default() -> Self {
        Self {
            upstream: DEFAULT_DOH_UPSTREAM.to_owned(),
            ca_certificate: None,
            timeout: Duration::from_secs(10),
            cache_ttl: Duration::from_secs(300),
            cache_size: 256,
            max_response_bytes: 65_536,
        }
    }
}

/// A target is either `"example.com"` (subdomains included) or
/// `{ host = "example.com", subdomains = false }`.
#[derive(Debug)]
struct RawTarget {
    host: String,
    subdomains: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTargetTable {
    host: String,
    #[serde(default = "subdomains_by_default")]
    subdomains: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawRuntimeLimits {
    max_connect_header_bytes: usize,
    max_client_hello_bytes: usize,
    max_connections: usize,
    #[serde(deserialize_with = "human_duration")]
    connect_timeout: Duration,
    #[serde(deserialize_with = "human_duration")]
    client_hello_timeout: Duration,
    #[serde(deserialize_with = "human_duration")]
    idle_timeout: Duration,
    #[serde(deserialize_with = "human_duration")]
    shutdown_timeout: Duration,
}

impl Default for RawRuntimeLimits {
    fn default() -> Self {
        Self {
            max_connect_header_bytes: 16_384,
            max_client_hello_bytes: 262_144,
            max_connections: 256,
            connect_timeout: Duration::from_secs(10),
            client_hello_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            shutdown_timeout: Duration::from_secs(10),
        }
    }
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ConfigError::NotFound {
                    path: path.to_path_buf(),
                }
            } else {
                ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                }
            }
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
        let pac_listen = match &raw.proxy.pac {
            RawPac::Enabled(false) => None,
            RawPac::Enabled(true) => Some(parse_loopback_listener(DEFAULT_PAC_LISTEN)?),
            RawPac::Address(address) => Some(parse_loopback_listener(address)?),
        };
        if pac_listen == Some(listen) && listen.port() != 0 {
            return Err(ConfigError::ListenerConflict);
        }

        let allowed_ports = validate_allowed_ports(raw.proxy.ports, listen.port() == 0)?;
        let dns = DnsConfig::validate(raw.dns, base_dir)?;

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
            connect_timeout: bounded_duration(
                "limits.connect_timeout",
                raw.limits.connect_timeout,
                MAX_TIMEOUT,
            )?,
            client_hello_timeout: bounded_duration(
                "limits.client_hello_timeout",
                raw.limits.client_hello_timeout,
                MAX_TIMEOUT,
            )?,
            idle_timeout: bounded_duration(
                "limits.idle_timeout",
                raw.limits.idle_timeout,
                MAX_TIMEOUT,
            )?,
            shutdown_timeout: bounded_duration(
                "limits.shutdown_timeout",
                raw.limits.shutdown_timeout,
                MAX_SHUTDOWN_TIMEOUT,
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
            update: UpdateConfig {
                check: raw.update.check,
            },
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

    pub(crate) fn update(&self) -> &UpdateConfig {
        &self.update
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

impl<'de> Deserialize<'de> for RawTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TargetVisitor;

        impl<'de> Visitor<'de> for TargetVisitor {
            type Value = RawTarget;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(
                    "a host name like \"example.com\" or a table like \
                     { host = \"example.com\", subdomains = false }",
                )
            }

            fn visit_str<E: de::Error>(self, host: &str) -> Result<RawTarget, E> {
                Ok(RawTarget {
                    host: host.to_owned(),
                    subdomains: subdomains_by_default(),
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<RawTarget, A::Error> {
                let table =
                    RawTargetTable::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(RawTarget {
                    host: table.host,
                    subdomains: table.subdomains,
                })
            }
        }

        deserializer.deserialize_any(TargetVisitor)
    }
}

impl<'de> Deserialize<'de> for RawPac {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PacVisitor;

        impl Visitor<'_> for PacVisitor {
            type Value = RawPac;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a loopback address like \"127.0.0.1:8081\", or false")
            }

            fn visit_bool<E: de::Error>(self, enabled: bool) -> Result<RawPac, E> {
                Ok(RawPac::Enabled(enabled))
            }

            fn visit_str<E: de::Error>(self, address: &str) -> Result<RawPac, E> {
                Ok(RawPac::Address(address.to_owned()))
            }
        }

        deserializer.deserialize_any(PacVisitor)
    }
}

fn human_duration<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    struct DurationVisitor;

    impl Visitor<'_> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a duration string like \"500ms\", \"10s\", or \"5m\"")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Duration, E> {
            humantime::parse_duration(value).map_err(|error| {
                E::custom(format_args!(
                    "invalid duration `{value}` ({error}); use values like \"500ms\", \"10s\", or \"5m\""
                ))
            })
        }
    }

    deserializer.deserialize_str(DurationVisitor)
}

const fn subdomains_by_default() -> bool {
    true
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

fn validate_targets(raw_targets: Vec<RawTarget>) -> Result<Vec<TargetRule>, ConfigError> {
    if raw_targets.is_empty() {
        return Err(ConfigError::EmptyTargets);
    }
    let rules = raw_targets
        .into_iter()
        .map(|raw| {
            Ok(TargetRule::new(
                DomainName::parse(&raw.host)?,
                raw.subdomains,
                Strategy::TlsRecordSplit,
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

fn bounded_duration(
    field: &'static str,
    value: Duration,
    max: Duration,
) -> Result<Duration, ConfigError> {
    if value.is_zero() || value > max {
        return Err(ConfigError::DurationOutOfBounds {
            field,
            max,
            actual: value,
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::{fs, net::SocketAddr, time::Duration};

    use tempfile::tempdir;

    use crate::domain::DomainError;

    use super::{Config, ConfigError, DnsConfig, Strategy};

    #[derive(Debug)]
    enum ExpectedConfigError {
        Toml(&'static str),
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
        DurationOutOfBounds {
            field: &'static str,
            actual: Duration,
        },
    }

    impl ExpectedConfigError {
        fn matches(&self, actual: &ConfigError) -> bool {
            match (self, actual) {
                (Self::Toml(fragment), ConfigError::Toml(error)) => {
                    error.to_string().contains(fragment)
                }
                (Self::ListenerConflict, ConfigError::ListenerConflict)
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
                (
                    Self::DurationOutOfBounds {
                        field: expected_field,
                        actual: expected_actual,
                    },
                    ConfigError::DurationOutOfBounds {
                        field: actual_field,
                        actual: actual_value,
                        ..
                    },
                ) => expected_field == actual_field && expected_actual == actual_value,
                _ => false,
            }
        }
    }

    const VALID: &str = r#"
targets = ["reddit.com", "medium.com"]

[proxy]
listen = "127.0.0.1:8080"
pac = "[::1]:8081"

[dns]
upstream = "https://1.1.1.1/dns-query"
"#;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        let directory = tempdir().expect("temporary directory");
        Config::parse(text, directory.path())
    }

    #[test]
    fn targets_alone_is_a_complete_configuration() {
        let config = parse("targets = [\"reddit.com\"]\n").expect("minimal configuration");

        assert_eq!(
            config.proxy.listen,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.proxy.pac_listen,
            Some("127.0.0.1:8081".parse().unwrap())
        );
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
        assert!(config.update.check);
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].host().as_str(), "reddit.com");
        assert!(config.targets[0].includes_subdomains());
        assert_eq!(config.targets[0].strategy(), Strategy::TlsRecordSplit);
    }

    #[test]
    fn update_check_can_be_turned_off() {
        let config = parse("targets = [\"reddit.com\"]\n\n[update]\ncheck = false\n")
            .expect("update section");
        assert!(!config.update().check());
    }

    #[test]
    fn default_dns_config_matches_parsed_defaults() {
        let parsed = parse("targets = [\"reddit.com\"]\n").unwrap();
        let default = DnsConfig::default();
        assert_eq!(default.upstream, parsed.dns.upstream);
        assert!(default.ca_certificate_pem.is_none());
        assert_eq!(default.timeout, parsed.dns.timeout);
        assert_eq!(default.cache_max_ttl, parsed.dns.cache_max_ttl);
        assert_eq!(default.cache_capacity, parsed.dns.cache_capacity);
        assert_eq!(default.max_response_bytes, parsed.dns.max_response_bytes);
    }

    #[test]
    fn representative_config_overrides_defaults() {
        let config = parse(VALID).expect("valid configuration");
        assert_eq!(config.proxy.pac_listen, Some("[::1]:8081".parse().unwrap()));
        assert_eq!(config.targets.len(), 2);
        assert!(config.targets.iter().all(|rule| rule.includes_subdomains()));
    }

    #[test]
    fn target_tables_and_pac_switch_shape_the_policy() {
        let config = parse(
            r#"
targets = ["reddit.com", { host = "example.com", subdomains = false }, { host = "medium.com" }]

[proxy]
pac = false
"#,
        )
        .expect("table targets");
        assert_eq!(config.proxy.pac_listen, None);
        let subdomains = |host: &str| {
            config
                .targets
                .iter()
                .find(|rule| rule.host().as_str() == host)
                .expect("configured target")
                .includes_subdomains()
        };
        assert!(subdomains("reddit.com"));
        assert!(!subdomains("example.com"));
        assert!(subdomains("medium.com"));

        let enabled = parse("targets = [\"reddit.com\"]\n[proxy]\npac = true\n").unwrap();
        assert_eq!(
            enabled.proxy.pac_listen,
            Some("127.0.0.1:8081".parse().unwrap())
        );
    }

    #[test]
    fn durations_accept_human_units() {
        let config = parse(
            r#"
targets = ["reddit.com"]

[dns]
timeout = "1500ms"
cache_ttl = "1h"

[limits]
connect_timeout = "2s"
client_hello_timeout = "750ms"
idle_timeout = "5m"
shutdown_timeout = "1m"
"#,
        )
        .expect("human durations");
        assert_eq!(config.dns.timeout, Duration::from_millis(1500));
        assert_eq!(config.dns.cache_max_ttl, Duration::from_secs(3600));
        assert_eq!(config.limits.connect_timeout, Duration::from_secs(2));
        assert_eq!(
            config.limits.client_hello_timeout,
            Duration::from_millis(750)
        );
        assert_eq!(config.limits.idle_timeout, Duration::from_secs(300));
        assert_eq!(config.limits.shutdown_timeout, Duration::from_secs(60));
    }

    #[test]
    fn rejects_unknown_missing_and_unsafe_values_with_exact_errors() {
        let cases = [
            (
                "unknown top-level field",
                format!("unknown = true\n{VALID}"),
                ExpectedConfigError::Toml("unknown field `unknown`"),
            ),
            (
                "unknown nested field",
                VALID.replace("[dns]", "[dns]\nunknown = true"),
                ExpectedConfigError::Toml("unknown field `unknown`"),
            ),
            (
                "unknown update field",
                format!("{VALID}\n[update]\nchecks = false\n"),
                ExpectedConfigError::Toml("unknown field `checks`"),
            ),
            (
                "missing targets",
                VALID.replace("targets = [\"reddit.com\", \"medium.com\"]", ""),
                ExpectedConfigError::Toml("missing field `targets`"),
            ),
            (
                "misspelled target key",
                VALID.replace(
                    "\"medium.com\"",
                    "{ host = \"medium.com\", subdomain = false }",
                ),
                ExpectedConfigError::Toml("unknown field `subdomain`"),
            ),
            (
                "target of the wrong type",
                VALID.replace("\"medium.com\"", "42"),
                ExpectedConfigError::Toml("a host name like \"example.com\""),
            ),
            (
                "integer duration",
                VALID.replace("[dns]", "[dns]\ntimeout = 10"),
                ExpectedConfigError::Toml("a duration string like \"500ms\""),
            ),
            (
                "unparseable duration",
                VALID.replace("[dns]", "[dns]\ntimeout = \"ten seconds\""),
                ExpectedConfigError::Toml("invalid duration `ten seconds`"),
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
                "PAC of the wrong type",
                VALID.replace("\"[::1]:8081\"", "8081"),
                ExpectedConfigError::Toml("a loopback address like \"127.0.0.1:8081\", or false"),
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
                VALID.replace("[\"reddit.com\", \"medium.com\"]", "[]"),
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
                "zero duration",
                format!("{VALID}\n[limits]\nidle_timeout = \"0s\"\n"),
                ExpectedConfigError::DurationOutOfBounds {
                    field: "limits.idle_timeout",
                    actual: Duration::ZERO,
                },
            ),
            (
                "excessive duration",
                format!("{VALID}\n[limits]\nshutdown_timeout = \"2m\"\n"),
                ExpectedConfigError::DurationOutOfBounds {
                    field: "limits.shutdown_timeout",
                    actual: Duration::from_secs(120),
                },
            ),
            (
                "disallowed port",
                VALID.replace("[proxy]", "[proxy]\nports = [80]"),
                ExpectedConfigError::UnsafePort(80),
            ),
            (
                "duplicate port",
                VALID.replace("[proxy]", "[proxy]\nports = [443, 443]"),
                ExpectedConfigError::DuplicatePort(443),
            ),
        ];

        for (case, invalid, expected) in cases {
            let error = parse(&invalid).expect_err("invalid configuration must be rejected");
            assert!(
                expected.matches(&error),
                "{case}: expected {expected:?}, got {error}"
            );
        }
    }

    #[test]
    fn duration_bound_errors_read_in_human_units() {
        let error = parse(&format!("{VALID}\n[limits]\nshutdown_timeout = \"2m\"\n")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "limits.shutdown_timeout must be greater than 0 and at most 1m, got 2m"
        );
    }

    #[test]
    fn missing_file_names_its_path() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("absent.toml");
        let error = Config::load(&path).unwrap_err();
        assert!(matches!(error, ConfigError::NotFound { .. }));
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn ephemeral_loopback_listener_mode_is_scoped_and_keeps_production_ports_safe() {
        let ephemeral = VALID
            .replace("127.0.0.1:8080", "127.0.0.1:0")
            .replace("[::1]:8081", "127.0.0.1:0")
            .replace("[proxy]", "[proxy]\nports = [49199]");
        let config = parse(&ephemeral).expect("ephemeral test config");
        assert_eq!(config.proxy.listen.port(), 0);
        assert_eq!(config.proxy.pac_listen.unwrap().port(), 0);
        assert_eq!(config.proxy.allowed_ports, vec![49199]);

        let production = VALID.replace("[proxy]", "[proxy]\nports = [49199]");
        assert!(matches!(
            parse(&production),
            Err(ConfigError::UnsafePort { port: 49199 })
        ));
    }

    #[test]
    fn repository_example_and_readme_define_the_same_operating_contract() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // The example is the starter file: it ships with no targets so `start` stops until
        // the user picks sites, and its commented example line is the suggested list.
        let example_path = root.join("config.example.toml");
        assert!(matches!(
            Config::load(&example_path),
            Err(ConfigError::EmptyTargets)
        ));
        let starter = fs::read_to_string(&example_path).expect("read repository example");
        let suggested = "# targets = [\"reddit.com\", \"medium.com\"]";
        assert!(
            starter.contains(suggested),
            "example is missing `{suggested}`"
        );
        let config = Config::parse(&starter.replace("targets = []", &suggested[2..]), root)
            .expect("repository example with targets must parse");
        let defaults = parse("targets = [\"reddit.com\", \"medium.com\"]\n").unwrap();

        assert_eq!(config.proxy.listen, defaults.proxy.listen);
        assert_eq!(config.proxy.pac_listen, defaults.proxy.pac_listen);
        assert_eq!(config.proxy.allowed_ports, defaults.proxy.allowed_ports);
        assert_eq!(config.dns.upstream, defaults.dns.upstream);
        assert!(config.dns.ca_certificate_pem.is_none());
        assert_eq!(config.dns.timeout, defaults.dns.timeout);
        assert_eq!(config.dns.cache_max_ttl, defaults.dns.cache_max_ttl);
        assert_eq!(config.dns.cache_capacity, defaults.dns.cache_capacity);
        assert_eq!(
            config.limits.max_connections,
            defaults.limits.max_connections
        );
        assert_eq!(config.limits.idle_timeout, defaults.limits.idle_timeout);
        assert_eq!(config.update.check, defaults.update.check);
        assert!(
            starter.contains("# check = true"),
            "example is missing the commented `[update]` default"
        );
        assert_eq!(config.targets.len(), 2);
        for (rule, host) in config.targets.iter().zip(["reddit.com", "medium.com"]) {
            assert_eq!(rule.host().as_str(), host);
            assert!(rule.includes_subdomains());
            assert_eq!(rule.strategy(), Strategy::TlsRecordSplit);
        }

        let readme = fs::read_to_string(root.join("README.md")).expect("read repository README");
        for required in [
            "cargo build --release",
            "splithello start",
            "--config <PATH>",
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
            "press `Ctrl-C`",
            "remove the browser's proxy or PAC setting",
            "splithello status",
            "splithello stop",
            "SplitHello is already running (pid",
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
