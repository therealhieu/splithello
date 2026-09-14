#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::atomic::{AtomicU16, Ordering},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query, ResponseCode},
    rr::{DNSClass, Name, RData, RecordType},
};
use thiserror::Error;

use crate::{config::DnsConfig, diagnostics::Outcome, domain::DomainName};

const MAX_CNAME_HOPS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DnsAnswer {
    pub(crate) addrs: Vec<Ipv4Addr>,
    pub(crate) ttl: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QueryIdentity {
    id: u16,
    name: Name,
}

#[derive(Debug, Error)]
pub(crate) enum ResolveError {
    #[error("could not encode DNS query")]
    Encode(#[source] hickory_proto::ProtoError),
    #[error("could not decode DNS response")]
    Decode(#[source] hickory_proto::serialize::binary::DecodeError),
    #[error("DNS response exceeded the configured size limit")]
    ResponseTooLarge,
    #[error("DNS response identity did not match the query")]
    MismatchedResponse,
    #[error("DNS response was truncated")]
    Truncated,
    #[error("DNS response returned {0}")]
    ResponseCode(ResponseCode),
    #[error("DNS response contained an unrelated or unsupported answer")]
    InvalidAnswer,
    #[error("DNS response CNAME chain loops or exceeds its bound")]
    InvalidCnameChain,
    #[error("DNS response contained no IPv4 addresses")]
    EmptyAnswer,
    #[error("could not build authenticated DoH client")]
    Client(#[source] reqwest::Error),
    #[error("additional DoH CA certificate is not valid PEM")]
    InvalidCaCertificate,
    #[error("DoH request timed out")]
    Timeout,
    #[error("DoH request failed")]
    Request(#[source] reqwest::Error),
    #[error("DoH server returned HTTP status {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("DoH response did not use application/dns-message")]
    InvalidContentType,
    #[error("DNS cache lock was unavailable")]
    CacheUnavailable,
    #[error("operating-system DNS resolution failed")]
    System(#[source] std::io::Error),
    #[error("operating-system DNS resolution returned no addresses")]
    EmptySystemAnswer,
}

impl ResolveError {
    pub(crate) const fn outcome(&self) -> Outcome {
        match self {
            Self::Timeout => Outcome::Timeout,
            _ => Outcome::DnsError,
        }
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    addrs: Vec<Ipv4Addr>,
    expires_at: Instant,
    generation: u64,
}

#[derive(Debug)]
pub(crate) struct DnsCache {
    entries: HashMap<DomainName, CacheEntry>,
    capacity: usize,
    max_ttl: Duration,
    next_generation: u64,
}

impl DnsCache {
    pub(crate) fn new(capacity: usize, max_ttl: Duration) -> Self {
        debug_assert!(capacity > 0);
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
            max_ttl,
            next_generation: 0,
        }
    }

    pub(crate) fn get(&mut self, host: &DomainName, now: Instant) -> Option<Vec<Ipv4Addr>> {
        let expired = self
            .entries
            .get(host)
            .is_some_and(|entry| now >= entry.expires_at);
        if expired {
            self.entries.remove(host);
            return None;
        }

        let generation = self.take_generation();
        self.entries.get_mut(host).map(|entry| {
            entry.generation = generation;
            entry.addrs.clone()
        })
    }

    pub(crate) fn insert(&mut self, host: DomainName, answer: &DnsAnswer, now: Instant) {
        let ttl = answer.ttl.min(self.max_ttl);
        if ttl.is_zero() {
            self.entries.remove(&host);
            return;
        }

        self.entries.retain(|_, entry| now < entry.expires_at);
        if !self.entries.contains_key(&host) && self.entries.len() >= self.capacity {
            let victim = self
                .entries
                .iter()
                .min_by(|(left_host, left), (right_host, right)| {
                    left.generation
                        .cmp(&right.generation)
                        .then_with(|| left_host.cmp(right_host))
                })
                .map(|(host, _)| host.clone());
            if let Some(victim) = victim {
                self.entries.remove(&victim);
            }
        }

        let generation = self.take_generation();
        self.entries.insert(
            host,
            CacheEntry {
                addrs: answer.addrs.clone(),
                expires_at: now + ttl,
                generation,
            },
        );
    }

    fn take_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        generation
    }
}

pub(crate) fn build_a_query(
    host: &DomainName,
    id: u16,
) -> Result<(Vec<u8>, QueryIdentity), ResolveError> {
    let name = Name::from_ascii(format!("{}.", host.as_str())).map_err(ResolveError::Encode)?;
    let mut message = Message::new(id, MessageType::Query, OpCode::Query);
    message.metadata.recursion_desired = true;
    message.add_query(Query::query(name.clone(), RecordType::A));
    let bytes = message.to_vec().map_err(ResolveError::Encode)?;
    Ok((bytes, QueryIdentity { id, name }))
}

pub(crate) fn parse_a_response(
    bytes: &[u8],
    expected: &QueryIdentity,
    max_response_bytes: usize,
) -> Result<DnsAnswer, ResolveError> {
    if bytes.len() > max_response_bytes {
        return Err(ResolveError::ResponseTooLarge);
    }
    let message = Message::from_vec(bytes).map_err(ResolveError::Decode)?;
    validate_message_identity(&message, expected)?;

    if message.metadata.truncation {
        return Err(ResolveError::Truncated);
    }
    if message.metadata.response_code != ResponseCode::NoError {
        return Err(ResolveError::ResponseCode(message.metadata.response_code));
    }

    let mut cname_targets = HashMap::new();
    let mut addresses: HashMap<Name, Vec<(Ipv4Addr, u32)>> = HashMap::new();
    for record in &message.answers {
        if record.dns_class != DNSClass::IN {
            return Err(ResolveError::InvalidAnswer);
        }
        match &record.data {
            RData::CNAME(target) => {
                if cname_targets
                    .insert(record.name.clone(), (target.0.clone(), record.ttl))
                    .is_some()
                {
                    return Err(ResolveError::InvalidAnswer);
                }
            }
            RData::A(address) => addresses
                .entry(record.name.clone())
                .or_default()
                .push((address.0, record.ttl)),
            _ => return Err(ResolveError::InvalidAnswer),
        }
    }

    if cname_targets
        .keys()
        .any(|owner| addresses.contains_key(owner))
    {
        return Err(ResolveError::InvalidAnswer);
    }

    let mut owner = expected.name.clone();
    let mut visited = HashSet::new();
    let mut accepted_ttls = Vec::new();
    for _ in 0..=MAX_CNAME_HOPS {
        if !visited.insert(owner.clone()) {
            return Err(ResolveError::InvalidCnameChain);
        }
        if let Some(records) = addresses.get(&owner) {
            let mut unique = HashSet::new();
            let mut addrs = Vec::new();
            for (address, ttl) in records {
                accepted_ttls.push(*ttl);
                if unique.insert(*address) {
                    addrs.push(*address);
                }
            }
            if addrs.is_empty() {
                return Err(ResolveError::EmptyAnswer);
            }
            if cname_targets.keys().any(|name| !visited.contains(name))
                || addresses
                    .keys()
                    .any(|name| name != &owner && !visited.contains(name))
            {
                return Err(ResolveError::InvalidAnswer);
            }
            let ttl = accepted_ttls.into_iter().min().unwrap_or(0);
            return Ok(DnsAnswer {
                addrs,
                ttl: Duration::from_secs(u64::from(ttl)),
            });
        }
        let Some((target, ttl)) = cname_targets.get(&owner) else {
            return Err(ResolveError::EmptyAnswer);
        };
        accepted_ttls.push(*ttl);
        owner = target.clone();
    }

    Err(ResolveError::InvalidCnameChain)
}

pub(crate) trait TargetResolver: Send + Sync {
    fn resolve_ipv4<'a>(
        &'a self,
        host: &'a DomainName,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>>;
}

pub(crate) struct DohResolver {
    client: reqwest::Client,
    upstream: reqwest::Url,
    cache: Arc<Mutex<DnsCache>>,
    max_response_bytes: usize,
    next_query_id: AtomicU16,
}

impl DohResolver {
    pub(crate) fn new(config: &DnsConfig) -> Result<Self, ResolveError> {
        let client = build_authenticated_client(config.ca_certificate_pem(), config.timeout())?;
        Ok(Self {
            client,
            upstream: config.upstream().clone(),
            cache: Arc::new(Mutex::new(DnsCache::new(
                config.cache_capacity(),
                config.cache_max_ttl(),
            ))),
            max_response_bytes: config.max_response_bytes(),
            next_query_id: AtomicU16::new(0),
        })
    }

    async fn query_a(&self, host: &DomainName) -> Result<DnsAnswer, ResolveError> {
        let id = self.next_query_id.fetch_add(1, Ordering::Relaxed);
        let (query, identity) = build_a_query(host, id)?;
        let mut response = self
            .client
            .post(self.upstream.clone())
            .header(reqwest::header::ACCEPT, "application/dns-message")
            .header(reqwest::header::CONTENT_TYPE, "application/dns-message")
            .body(query)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(ResolveError::HttpStatus(response.status()));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if !content_type.is_some_and(|value| value.eq_ignore_ascii_case("application/dns-message"))
        {
            return Err(ResolveError::InvalidContentType);
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(ResolveError::ResponseTooLarge);
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_request_error)? {
            let new_len = body
                .len()
                .checked_add(chunk.len())
                .ok_or(ResolveError::ResponseTooLarge)?;
            if new_len > self.max_response_bytes {
                return Err(ResolveError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        parse_a_response(&body, &identity, self.max_response_bytes)
    }
}

impl TargetResolver for DohResolver {
    fn resolve_ipv4<'a>(
        &'a self,
        host: &'a DomainName,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(cached) = self
                .cache
                .lock()
                .map_err(|_| ResolveError::CacheUnavailable)?
                .get(host, Instant::now())
            {
                return Ok(cached);
            }

            let answer = self.query_a(host).await?;
            let addrs = answer.addrs.clone();
            self.cache
                .lock()
                .map_err(|_| ResolveError::CacheUnavailable)?
                .insert(host.clone(), &answer, Instant::now());
            Ok(addrs)
        })
    }
}

fn build_authenticated_client(
    additional_ca_pem: Option<&[u8]>,
    timeout: Duration,
) -> Result<reqwest::Client, ResolveError> {
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .connect_timeout(timeout)
        .read_timeout(timeout);
    if let Some(pem) = additional_ca_pem {
        let certificates = reqwest::tls::Certificate::from_pem_bundle(pem)
            .map_err(|_| ResolveError::InvalidCaCertificate)?;
        if certificates.is_empty() {
            return Err(ResolveError::InvalidCaCertificate);
        }
        builder = builder.tls_certs_merge(certificates);
    }
    builder.build().map_err(ResolveError::Client)
}

fn map_request_error(error: reqwest::Error) -> ResolveError {
    if error.is_timeout() {
        ResolveError::Timeout
    } else {
        ResolveError::Request(error)
    }
}

pub(crate) struct SystemResolver;

impl SystemResolver {
    pub(crate) async fn resolve(
        host: &DomainName,
        port: u16,
    ) -> Result<Vec<SocketAddr>, ResolveError> {
        collect_system_addresses(
            tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(ResolveError::System)?,
        )
    }
}

fn collect_system_addresses(
    addresses: impl IntoIterator<Item = SocketAddr>,
) -> Result<Vec<SocketAddr>, ResolveError> {
    let addresses = addresses.into_iter().collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(ResolveError::EmptySystemAnswer);
    }
    Ok(addresses)
}

fn validate_message_identity(
    message: &Message,
    expected: &QueryIdentity,
) -> Result<(), ResolveError> {
    let queries = &message.queries;
    if message.metadata.id != expected.id
        || message.metadata.message_type != MessageType::Response
        || message.metadata.op_code != OpCode::Query
        || queries.len() != 1
        || queries[0].name() != &expected.name
        || queries[0].query_type() != RecordType::A
        || queries[0].query_class() != DNSClass::IN
    {
        return Err(ResolveError::MismatchedResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, Ipv6Addr, SocketAddr},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use hickory_proto::{
        op::{Message, MessageType, Query, ResponseCode},
        rr::{
            Name, RData, Record, RecordType,
            rdata::{A, CNAME},
        },
    };

    use crate::domain::DomainName;

    use super::{
        DnsAnswer, DnsCache, DohResolver, MAX_CNAME_HOPS, ResolveError, TargetResolver,
        build_a_query, build_authenticated_client, collect_system_addresses, parse_a_response,
    };

    fn build_test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .use_rustls_tls()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_secs(2))
            .read_timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }

    fn host(value: &str) -> DomainName {
        DomainName::parse(value).expect("valid test domain")
    }

    fn response(id: u16, query_name: &str, answers: Vec<Record>) -> Vec<u8> {
        let name = Name::from_ascii(format!("{query_name}.")).unwrap();
        let mut message = Message::new(id, MessageType::Response, hickory_proto::op::OpCode::Query);
        message.add_query(Query::query(name, RecordType::A));
        for answer in answers {
            message.add_answer(answer);
        }
        message.to_vec().unwrap()
    }

    fn a(owner: &str, address: [u8; 4], ttl: u32) -> Record {
        Record::from_rdata(
            Name::from_ascii(owner).unwrap(),
            ttl,
            RData::A(A::new(address[0], address[1], address[2], address[3])),
        )
    }

    fn cname(owner: &str, target: &str, ttl: u32) -> Record {
        Record::from_rdata(
            Name::from_ascii(owner).unwrap(),
            ttl,
            RData::CNAME(CNAME(Name::from_ascii(target).unwrap())),
        )
    }

    #[test]
    fn builds_a_in_query_and_accepts_matching_a_and_bounded_cname_answers() {
        let (query, identity) = build_a_query(&host("www.reddit.com"), 42).unwrap();
        let decoded = Message::from_vec(&query).unwrap();
        assert_eq!(decoded.metadata.id, 42);
        assert_eq!(decoded.queries[0].query_type(), RecordType::A);

        let bytes = response(
            42,
            "www.reddit.com",
            vec![
                cname("www.reddit.com", "edge.reddit.com", 120),
                a("edge.reddit.com", [203, 0, 113, 7], 90),
                a("edge.reddit.com", [203, 0, 113, 8], 60),
            ],
        );
        let answer = parse_a_response(&bytes, &identity, 4096).unwrap();
        assert_eq!(
            answer.addrs,
            vec![Ipv4Addr::new(203, 0, 113, 7), Ipv4Addr::new(203, 0, 113, 8)]
        );
        assert_eq!(answer.ttl, Duration::from_secs(60));
    }

    #[test]
    fn rejects_oversized_malformed_mismatched_truncated_rcode_and_empty_responses() {
        let (_, identity) = build_a_query(&host("reddit.com"), 7).unwrap();
        assert!(matches!(
            parse_a_response(&[0; 20], &identity, 10),
            Err(ResolveError::ResponseTooLarge)
        ));
        assert!(matches!(
            parse_a_response(&[1, 2, 3], &identity, 10),
            Err(ResolveError::Decode(_))
        ));
        assert!(matches!(
            parse_a_response(&response(8, "reddit.com", vec![]), &identity, 4096),
            Err(ResolveError::MismatchedResponse)
        ));

        let mut truncated = Message::from_vec(&response(7, "reddit.com", vec![])).unwrap();
        truncated.metadata.truncation = true;
        assert!(matches!(
            parse_a_response(&truncated.to_vec().unwrap(), &identity, 4096),
            Err(ResolveError::Truncated)
        ));

        let mut refused = Message::from_vec(&response(7, "reddit.com", vec![])).unwrap();
        refused.metadata.response_code = ResponseCode::Refused;
        assert!(matches!(
            parse_a_response(&refused.to_vec().unwrap(), &identity, 4096),
            Err(ResolveError::ResponseCode(ResponseCode::Refused))
        ));
        assert!(matches!(
            parse_a_response(&response(7, "reddit.com", vec![]), &identity, 4096),
            Err(ResolveError::EmptyAnswer)
        ));
    }

    #[test]
    fn rejects_each_dns_identity_mismatch_with_exact_error() {
        let (_, identity) = build_a_query(&host("reddit.com"), 17).unwrap();
        let valid = Message::from_vec(&response(
            17,
            "reddit.com",
            vec![a("reddit.com", [203, 0, 113, 17], 30)],
        ))
        .unwrap();

        let mut cases = Vec::new();

        let mut question_name = valid.clone();
        question_name.queries[0].name = Name::from_ascii("medium.com.").unwrap();
        cases.push(("question name", question_name));

        let mut query_type = valid.clone();
        query_type.queries[0].query_type = RecordType::AAAA;
        cases.push(("query type", query_type));

        let mut query_class = valid.clone();
        query_class.queries[0].query_class = hickory_proto::rr::DNSClass::CH;
        cases.push(("query class", query_class));

        let mut message_type = valid.clone();
        message_type.metadata.message_type = MessageType::Query;
        cases.push(("message type", message_type));

        let mut opcode = valid;
        opcode.metadata.op_code = hickory_proto::op::OpCode::Update;
        cases.push(("opcode", opcode));

        for (case, message) in cases {
            assert!(
                matches!(
                    parse_a_response(&message.to_vec().unwrap(), &identity, 4096),
                    Err(ResolveError::MismatchedResponse)
                ),
                "{case} must return exactly MismatchedResponse"
            );
        }
    }

    #[test]
    fn rejects_unrelated_looping_and_excessive_cname_answers() {
        let (_, identity) = build_a_query(&host("reddit.com"), 9).unwrap();
        let unrelated = response(
            9,
            "reddit.com",
            vec![a("attacker.example", [192, 0, 2, 1], 30)],
        );
        assert!(matches!(
            parse_a_response(&unrelated, &identity, 4096),
            Err(ResolveError::EmptyAnswer | ResolveError::InvalidAnswer)
        ));

        let conflicting = response(
            9,
            "reddit.com",
            vec![
                cname("reddit.com", "edge.reddit.com", 30),
                a("reddit.com", [192, 0, 2, 2], 30),
            ],
        );
        assert!(matches!(
            parse_a_response(&conflicting, &identity, 4096),
            Err(ResolveError::InvalidAnswer)
        ));

        let looping = response(
            9,
            "reddit.com",
            vec![
                cname("reddit.com", "edge.reddit.com", 30),
                cname("edge.reddit.com", "reddit.com", 30),
            ],
        );
        assert!(matches!(
            parse_a_response(&looping, &identity, 4096),
            Err(ResolveError::InvalidCnameChain)
        ));

        let mut answers = Vec::new();
        for hop in 0..=MAX_CNAME_HOPS {
            let owner = if hop == 0 {
                "reddit.com".to_owned()
            } else {
                format!("hop{hop}.example")
            };
            let target = format!("hop{}.example", hop + 1);
            answers.push(cname(&owner, &target, 30));
        }
        let excessive = response(9, "reddit.com", answers);
        assert!(matches!(
            parse_a_response(&excessive, &identity, 16_384),
            Err(ResolveError::InvalidCnameChain)
        ));
    }

    #[tokio::test]
    async fn doh_posts_dns_message_reuses_cache_and_returns_only_ipv4() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let captured_for_server = Arc::clone(&captured);
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let header_end = loop {
                let mut chunk = [0_u8; 1024];
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
                if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    break position + 4;
                }
            };
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while request.len() < header_end + content_length {
                let mut chunk = [0_u8; 1024];
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
            }
            captured_for_server.lock().unwrap().push(request.clone());

            let query =
                Message::from_vec(&request[header_end..header_end + content_length]).unwrap();
            let query_name = query.queries[0].name().clone();
            let mut response = Message::new(
                query.metadata.id,
                MessageType::Response,
                hickory_proto::op::OpCode::Query,
            );
            response
                .add_query(query.queries[0].clone())
                .add_answer(Record::from_rdata(
                    query_name,
                    30,
                    RData::A(A::new(203, 0, 113, 20)),
                ));
            let body = response.to_vec().unwrap();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        });

        let resolver = DohResolver {
            client: build_test_client(),
            upstream: reqwest::Url::parse(&format!("http://{address}/dns-query")).unwrap(),
            cache: Arc::new(Mutex::new(DnsCache::new(4, Duration::from_secs(60)))),
            max_response_bytes: 4096,
            next_query_id: std::sync::atomic::AtomicU16::new(11),
        };
        let expected = vec![Ipv4Addr::new(203, 0, 113, 20)];
        assert_eq!(
            resolver.resolve_ipv4(&host("reddit.com")).await.unwrap(),
            expected
        );
        assert_eq!(
            resolver.resolve_ipv4(&host("reddit.com")).await.unwrap(),
            expected
        );
        server.await.unwrap();

        let requests = captured.lock().unwrap();
        assert_eq!(requests.len(), 1, "second resolution must use the cache");
        let request = std::str::from_utf8(&requests[0]).unwrap();
        assert!(request.starts_with("POST /dns-query HTTP/1.1\r\n"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("content-type: application/dns-message")
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("accept: application/dns-message")
        );
    }

    #[tokio::test]
    async fn doh_does_not_follow_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let resolver = DohResolver {
            client: build_test_client(),
            upstream: reqwest::Url::parse(&format!("http://{address}/dns-query")).unwrap(),
            cache: Arc::new(Mutex::new(DnsCache::new(1, Duration::from_secs(60)))),
            max_response_bytes: 4096,
            next_query_id: std::sync::atomic::AtomicU16::new(0),
        };
        assert!(matches!(
            resolver.resolve_ipv4(&host("reddit.com")).await,
            Err(ResolveError::HttpStatus(reqwest::StatusCode::FOUND))
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn doh_rejects_declared_oversized_body_before_buffering() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let resolver = DohResolver {
            client: build_test_client(),
            upstream: reqwest::Url::parse(&format!("http://{address}/dns-query")).unwrap(),
            cache: Arc::new(Mutex::new(DnsCache::new(1, Duration::from_secs(60)))),
            max_response_bytes: 64,
            next_query_id: std::sync::atomic::AtomicU16::new(0),
        };
        assert!(matches!(
            resolver.resolve_ipv4(&host("reddit.com")).await,
            Err(ResolveError::ResponseTooLarge)
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn doh_rejects_unknown_length_stream_when_chunks_cross_body_limit() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            stream.write_all(b"20\r\n").await.unwrap();
            stream.write_all(&[0_u8; 32]).await.unwrap();
            stream.write_all(b"\r\n28\r\n").await.unwrap();
            stream.write_all(&[0_u8; 40]).await.unwrap();
            let _ = stream.write_all(b"\r\n0\r\n\r\n").await;
        });
        let resolver = DohResolver {
            client: build_test_client(),
            upstream: reqwest::Url::parse(&format!("http://{address}/dns-query")).unwrap(),
            cache: Arc::new(Mutex::new(DnsCache::new(1, Duration::from_secs(60)))),
            max_response_bytes: 64,
            next_query_id: std::sync::atomic::AtomicU16::new(0),
        };
        assert!(matches!(
            resolver.resolve_ipv4(&host("reddit.com")).await,
            Err(ResolveError::ResponseTooLarge)
        ));
        server.await.unwrap();
    }

    #[test]
    fn client_rejects_malformed_additional_ca_and_system_results_preserve_families() {
        assert!(matches!(
            build_authenticated_client(Some(b"not a certificate"), Duration::from_secs(1)),
            Err(ResolveError::InvalidCaCertificate)
        ));
        let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
        assert!(
            build_authenticated_client(
                Some(generated.cert.pem().as_bytes()),
                Duration::from_secs(1),
            )
            .is_ok()
        );
        let ipv4 = SocketAddr::from(([192, 0, 2, 1], 443));
        let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, 443));
        assert_eq!(
            collect_system_addresses([ipv6, ipv4]).unwrap(),
            vec![ipv6, ipv4]
        );
        assert!(matches!(
            collect_system_addresses([]),
            Err(ResolveError::EmptySystemAnswer)
        ));
    }

    #[test]
    fn cache_obeys_expiry_ttl_ceiling_and_deterministic_lru_capacity() {
        let start = std::time::Instant::now();
        let answer = DnsAnswer {
            addrs: vec![Ipv4Addr::new(203, 0, 113, 1)],
            ttl: Duration::from_secs(60),
        };
        let mut cache = DnsCache::new(2, Duration::from_secs(10));
        cache.insert(host("a.example"), &answer, start);
        cache.insert(host("b.example"), &answer, start);
        assert_eq!(
            cache.get(&host("a.example"), start + Duration::from_secs(9)),
            Some(answer.addrs.clone())
        );

        cache.insert(host("c.example"), &answer, start + Duration::from_secs(9));
        assert!(
            cache
                .get(&host("b.example"), start + Duration::from_secs(9))
                .is_none()
        );
        assert!(
            cache
                .get(&host("a.example"), start + Duration::from_secs(9))
                .is_some()
        );
        assert!(
            cache
                .get(&host("a.example"), start + Duration::from_secs(10))
                .is_none()
        );
        assert!(
            cache
                .get(&host("c.example"), start + Duration::from_secs(18))
                .is_some()
        );
        assert!(
            cache
                .get(&host("c.example"), start + Duration::from_secs(19))
                .is_none()
        );
    }
}
