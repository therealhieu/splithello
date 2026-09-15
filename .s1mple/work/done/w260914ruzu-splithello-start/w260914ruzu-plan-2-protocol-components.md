# W260914RUZU Plan Part 2 — protocol-components

## Status

`Ready`

## Objective

Implement and prove the two bounded protocol components: authenticated target-only IPv4 DNS-over-HTTPS with TTL caching, and safe byte-preserving TLS ClientHello record fragmentation for matching configured SNI.

## Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`–`S13`, `S17`
- **Verification Cases:** `V8`–`V13`, `V15`, `V21`

## Sources

- Plan index: `w260914ruzu-plan-index.md`
- Plan context: `w260914ruzu-plan-context.md`
- System design: `w260914ruzu-system-design.md`
- Requirements: `w260914ruzu-requirements.md`

## Dependencies

- Part 1 — validated `DnsConfig`, `RuntimeLimits`, `DomainName`, diagnostic outcomes, and dependency graph.

## Execution Flow

```text
P2-G1: authenticated selected DNS → P2-G2: bounded TLS framing transform
```

## Groups

### P2-G1 — authenticated-target-dns

#### Goal

Selected domains resolve through an authenticated RFC 8484 client into validated IPv4 candidates with bounded caching, while normal direct resolution remains a separate OS IPv4/IPv6 adapter.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`, `S7`, `S12`, `S13`, `S17`
- **Verification Cases:** `V8`, `V9`, `V10`, `V15`, `V21`

#### Coherence Boundary

DNS message validation, cache policy, reqwest authentication, and direct-address-family separation jointly define the resolver boundary consumed by Part 3. Splitting them across contexts risks OS fallback or authentication drift.

#### Scope

- Included: `TargetResolver`, `SystemResolver`, DNS query/response codec use, A/CNAME validation, reqwest client construction, additive PEM CA trust, bounded response/time/cache, tests.
- Excluded: CONNECT handling, target matching, origin connection orchestration, process-level local TLS server.
- Main files: `src/dns.rs`, `src/config.rs`, `Cargo.toml`, `Cargo.lock` only if an approved feature correction is required.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1` (prefixed `W260914RUZU-`) | `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES7` | `S6`, `S7`, `S12`, `S13`, `S17` | `V8`–`V10`, `V15`, `V21` | Authenticated selected DoH and normal direct resolution have explicit, safe, testable contracts |

#### Cross-Work Gates

None.

#### Tasks

##### P2-G1-T1 — implement-dns-message-validation-and-cache

- **Outcome:** DNS A queries and responses use hickory-proto, reject malformed/mismatched/unsafe answers, accept only a bounded CNAME chain rooted at the question, and cache validated IPv4 results until bounded monotonic expiry.
- **Requirement Revisions:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`, `S7`, `S12`, `S17`
- **Verification Cases:** `V8`, `V9`, `V10`
- **Files:** `src/dns.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V8`, `V9`, `V10`:** DNS/cache unit tests

   **Preview:**

   ```rust
   // V8: matching successful A answer returns exact IPv4 candidates/min TTL.
   // V9: reject truncation, oversized body, ID/question/type/class mismatch,
   // failure RCODE, unrelated owner, looping/excess CNAME, empty A result.
   // V10: controlled clock proves hit-before-expiry, miss-at-expiry,
   // configured TTL ceiling, and deterministic capacity eviction.
   ```

2. **GREEN:** DNS data types, codec helpers, `DnsCache`

   **Preview:**

   ```rust
   struct DnsAnswer { addrs: Vec<Ipv4Addr>, ttl: Duration }
   struct DnsCache { entries: HashMap<DomainName, CacheEntry>, capacity: usize }

   fn build_a_query(host: &DomainName, id: u16) -> Result<Vec<u8>, ResolveError>;
   fn parse_a_response(bytes: &[u8], expected: &QueryIdentity) -> Result<DnsAnswer, ResolveError>;
   ```

   - Delegate wire encoding/decoding to hickory-proto.
   - Validate identity/ownership before extracting addresses.
   - Inject a monotonic-time provider or pass `Instant` to cache operations for deterministic tests; do not add a broad clock framework.

3. **REFACTOR:** isolate response validation from HTTP transport and keep cache lock-free from async network waits.

   **Preview:**

   ```rust
   // Pure parse/cache helpers remain deterministic; transport is next task.
   ```

4. **COMMIT:** Run DNS unit tests and self-review.

   **Commit message:**

   ```text
   feat(dns): validate and cache ipv4 answers

   Task: P2-G1-T1 — implement-dns-message-validation-and-cache
   ```

##### P2-G1-T2 — implement-authenticated-doh-and-system-resolvers

- **Outcome:** `DohResolver` uses rustls HTTPS, disabled ambient proxy discovery, disabled redirects, explicit timeouts, bounded body reads, and optional additive CA roots; `SystemResolver` preserves OS IPv4/IPv6 candidates for unselected hosts.
- **Requirement Revisions:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`, `S7`, `S12`, `S13`, `S17`
- **Verification Cases:** `V8`–`V10`, `V15`, `V21`
- **Files:** `src/dns.rs`, `src/config.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — resolver transport cases:** `src/dns.rs`

   **Preview:**

   ```rust
   // Fake HTTP boundary or connector asserts POST application/dns-message,
   // no redirect acceptance, total/body limits, cache reuse, error mapping.
   // Build client with valid additional CA succeeds; malformed CA fails.
   // Ensure no danger_accept_invalid_certs-style surface exists.
   // SystemResolver test preserves both SocketAddr families returned by OS/test seam.
   ```

2. **GREEN:** resolver contracts and implementations

   **Preview:**

   ```rust
   trait TargetResolver: Send + Sync {
       fn resolve_ipv4<'a>(&'a self, host: &'a DomainName)
           -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>>;
   }

   struct DohResolver { client: reqwest::Client, upstream: Url, cache: Mutex<DnsCache>, limits: DnsLimits }
   struct SystemResolver;
   impl SystemResolver { async fn resolve(host: &DomainName, port: u16) -> Result<Vec<SocketAddr>, ResolveError>; }
   ```

   - Construct reqwest with rustls-only features, `no_proxy`, `Policy::none`, explicit timeouts, and additive `add_root_certificate` values.
   - Stream and count response chunks; stop above the configured maximum.
   - Selected resolver returns IPv4 only; system resolver does not filter IPv6.

3. **REFACTOR:** centralize `ResolveError → Outcome` mapping and avoid holding cache mutex across `.await`.

   **Preview:**

   ```rust
   // Cache lookup → network await → validated insert; no lock spans await.
   ```

4. **COMMIT:** Run resolver/DNS tests and self-review.

   **Commit message:**

   ```text
   feat(dns): add authenticated target resolver

   Task: P2-G1-T2 — implement-authenticated-doh-and-system-resolvers
   ```

**Verification:**

- `V8`–`V10` — module tests → exact DNS/cache outcomes.
- `V15` contract preparation — resolver is injectable and additive CA authentication is available for Part 4.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| DNS unit tests | focused `cargo test` filters for dns | V8–V10 pass |
| Resolver security inspection | search client builder/config API | No invalid-cert bypass or target OS fallback; `no_proxy` and no redirects configured |
| Address-family inspection | resolver tests and signatures | Target resolver returns `Ipv4Addr`; system resolver returns unfiltered `SocketAddr` |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** RFC 8484 POST, selected-only DoH, authenticated additive CA, A/CNAME validation, cache TTL, direct IPv4/IPv6 preservation.
- **Technical:** no lock over await, no oversized body allocation, deterministic expiry, structured errors, no ambient proxy recursion.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

### P2-G2 — bounded-tls-clienthello-transform

#### Goal

A safe deterministic component recognizes one complete ClientHello across valid input records, validates matching SNI, and emits valid split records whose reassembled handshake bytes are exactly unchanged.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S8`, `S9`, `S10`, `S11`, `S12`, `S13`, `S17`
- **Verification Cases:** `V11`, `V12`, `V13`, `V15`, `V21`

#### Coherence Boundary

Record accumulation, checked ClientHello/SNI parsing, explicit outcomes, and serialization share byte offsets and invariants. One implementation context must own them to prevent a parser/serializer mismatch.

#### Scope

- Included: bounded record/handshake parser, SNI extraction/range, parse progress, explicit rewrite/pass/reject, record serializer, arbitrary-byte no-panic tests.
- Excluded: TLS cryptography, certificates, application data inspection, TCP packet operations, socket timeouts (Part 3 caller owns wall-clock timeout).
- Main files: `src/tls.rs`, `src/domain.rs` only for existing canonical comparison use.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1` (prefixed `W260914RUZU-`) | `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7` | `S8`–`S13`, `S17` | `V11`–`V13`, `V15`, `V21` | Checked framing transform never corrupts, decrypts, or partially emits selected handshakes |

#### Cross-Work Gates

None.

#### Tasks

##### P2-G2-T1 — parse-bounded-clienthello-across-records

- **Outcome:** Checked parsing returns `NeedMore`, a complete `ParsedClientHello`, defined safe pass-through, or structured rejection without panic for arbitrary bounded input.
- **Requirement Revisions:** `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES4`, `W260914RUZU-DES7`
- **Success Criteria:** `S8`, `S11`, `S12`, `S17`
- **Verification Cases:** `V11`, `V13`
- **Files:** `src/tls.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V11`, parser half of `V13`:** TLS parse matrix

   **Preview:**

   ```rust
   // Generate one ClientHello represented in one and many legal handshake records.
   // Assert same handshake, consumed extent, SNI, and SNI range.
   // Test truncation at every field, overflow/invalid lengths, wrong content/handshake,
   // interleaving, missing SNI, oversized input, malformed extension lists.
   // Catch-unwind/property loop over arbitrary short byte vectors: no panic.
   ```

2. **GREEN:** parse state/types and checked parser

   **Preview:**

   ```rust
   enum ParseProgress<T> { NeedMore, Complete(T), PassThrough(PassReason) }
   struct ParsedClientHello {
       record_version: [u8; 2], handshake: Vec<u8>, consumed_input: usize,
       sni: DomainName, sni_range: Range<usize>, trailing: Vec<BufferedRecordBytes>,
   }
   fn parse_client_hello(input: &[u8], limit: usize)
       -> Result<ParseProgress<ParsedClientHello>, TlsError>;
   ```

   - Use checked arithmetic and explicit length helpers.
   - Parse only required plaintext ClientHello framing.
   - Preserve enough record/trailing metadata to serialize without dropping or changing bytes.

3. **REFACTOR:** isolate small cursor/length helpers; prohibit a general TLS AST.

   **Preview:**

   ```rust
   // Narrow private cursor helpers with checked bounds; no protocol abstraction layer.
   ```

4. **COMMIT:** Run TLS parser tests and self-review.

   **Commit message:**

   ```text
   feat(tls): parse bounded client hello records

   Task: P2-G2-T1 — parse-bounded-clienthello-across-records
   ```

##### P2-G2-T2 — serialize-byte-preserving-sni-record-split

- **Outcome:** Matching CONNECT/SNI handshakes are encoded into two valid TLS handshake records split strictly inside SNI; reassembled handshake and trailing bytes equal input exactly, with explicit safe pass/reject behavior.
- **Requirement Revisions:** `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S9`, `S10`, `S11`, `S13`, `S17`
- **Verification Cases:** `V12`, `V13`, `V15`
- **Files:** `src/tls.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V12`, transformation half of `V13`:** rewrite tests

   **Preview:**

   ```rust
   // Matching SNI: parse output records; assert exactly two rewritten handshake records,
   // split offset strictly inside hostname; concatenated handshake equals original bytes.
   // Assert trailing bytes/records preserve order/content/type.
   // Mismatched SNI, missing/too-short hostname, invalid record length, and unsafe ambiguity
   // return exact outcome before output; safe non-TLS/non-ClientHello uses explicit pass-through.
   ```

2. **GREEN:** `RewriteOutcome`, `encode_split_records`, high-level rewrite

   **Preview:**

   ```rust
   enum RewriteOutcome {
       Rewritten(Vec<u8>),
       PassThrough { prefix: Vec<u8>, reason: PassReason },
       Reject(TlsError),
   }
   fn rewrite_client_hello(input: &[u8], expected: &DomainName, limit: usize)
       -> RewriteOutcome;
   ```

   - Compare canonical SNI to selected CONNECT host.
   - Select midpoint strictly inside the SNI byte range and validate both record sizes.
   - Preserve handshake and buffered trailing content byte-for-byte.

3. **REFACTOR:** add a test-only reassembler helper to assert output invariants without duplicating production serializer logic.

   **Preview:**

   ```rust
   // Test decoder independently concatenates record payloads and checks headers/lengths.
   ```

4. **COMMIT:** Run all TLS tests and self-review.

   **Commit message:**

   ```text
   feat(tls): split client hello at matching sni

   Task: P2-G2-T2 — serialize-byte-preserving-sni-record-split
   ```

**Verification:**

- `V11`–`V13` — TLS unit suites → complete parsing, exact rewrite, bounded failures.
- `V15` contract preparation — public crate-private API accepts buffered client prefix and expected `DomainName`.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| TLS tests | focused `cargo test` filters for tls | V11–V13 pass |
| No-panic boundary | arbitrary/truncation test loop | Every bounded input returns an outcome without panic |
| Byte preservation | test reassembler assertion | Input and output ClientHello handshake bytes match exactly |
| Static scope review | inspect `src/tls.rs` imports/APIs | No cryptography, certificate, raw socket, unsafe, or application-data parsing |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** split strictly inside matching SNI, legal input fragmentation, no transformed output before complete decision, post-prefix opacity.
- **Technical:** checked arithmetic, preserved trailing bytes, valid record lengths, precise pass/reject boundary, focused parser only.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

## Coverage

| Requirement Revision | Design ID | Success Criterion | Verification Cases | Groups | Tasks |
|---|---|---|---|---|---|
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S6`, `S7` | `V8`–`V10`, `V15` | G1 | T1, T2 |
| `W260914RUZU-REQ5@R1` | `W260914RUZU-DES4` | `S8`–`S10` | `V11`–`V13`, `V15` | G2 | T1, T2 |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES3`, `W260914RUZU-DES4` | `S11`, `S12` | `V9`, `V13` | G1, G2 | all tasks |
| `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | `S13` | `V13`, `V15` | G1, G2 | G1-T2, G2-T2 |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V8`–`V13`, `V21` | G1, G2 | all tasks |

## Completion Criteria

- Every group satisfies its acceptance criteria.
- Part 1 contracts are consumed without alternate validation or hostname semantics.
- Every protocol boundary has deterministic tests and structured errors.
- Outputs are committed and ready for Part 3 orchestration.

## Planning Gaps

`None`
