# W260914RUZU Plan Part 1 — foundation-contracts

## Status

`Ready`

## Objective

Establish the approved dependency graph and all pure startup-facing contracts: exact CLI syntax, strict validated configuration, canonical domain policy, bounded CONNECT parsing, deterministic PAC rendering, and stable secret-safe diagnostic categories.

## Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S2`, `S3`, `S4`, `S5`, `S12`, `S15`, `S17`
- **Verification Cases:** `V1`–`V7`, `V20`, `V21`

## Sources

- Plan index: `w260914ruzu-plan-index.md`
- Plan context: `w260914ruzu-plan-context.md`
- System design: `w260914ruzu-system-design.md`
- Requirements: `w260914ruzu-requirements.md`

## Dependencies

- None.

## Execution Flow

```text
P1-G1: package and startup contracts → P1-G2: proxy-facing pure contracts
```

## Groups

### P1-G1 — package-and-startup-contracts

#### Goal

The crate compiles with the minimal approved dependencies, exposes the exact command, and converts strict TOML into validated canonical runtime values before any network side effect.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S2`, `S5`, `S12`, `S17`
- **Verification Cases:** `V1`–`V4`, `V20`, `V21`

#### Coherence Boundary

Dependencies, CLI shape, config validation, and hostname identity form one startup contract consumed by every later part. They must be implemented together so no downstream module invents alternate defaults or matching behavior.

#### Scope

- Included: dependency/features declaration, generated lockfile, CLI derives, raw/validated config models, hard ceilings, optional additive DoH CA path, canonical domains, target matching, focused tests.
- Excluded: listeners, DoH transport, TLS framing, relay, process integration.
- Main files: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/cli.rs`, `src/config.rs`, `src/domain.rs`.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1` (all prefixed `W260914RUZU-`) | `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES7` | `S1`, `S2`, `S5`, `S12`, `S17` | `V1`–`V4`, `V20`, `V21` | One strict, safe, canonical startup contract compiles and passes focused tests |

#### Cross-Work Gates

None.

#### Tasks

##### P1-G1-T1 — declare-the-minimal-approved-dependency-graph

- **Outcome:** Cargo resolves only the approved maintained/licensed crates with narrow features; reqwest uses rustls without system-proxy/native-TLS defaults, and the lockfile is regenerated through Cargo.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES7`
- **Success Criteria:** `S12`, `S17`
- **Verification Cases:** `V21`
- **Files:** `Cargo.toml`, `Cargo.lock`
- **Execution Approach:** Direct verification — dependency declarations are configuration, not runtime behavior.

**Steps:**

1. `Cargo.toml` — package dependency and feature declarations

   **Preview:**

   ```toml
   # Runtime: clap/serde/toml, tokio/tokio-util, reqwest rustls-only,
   # hickory-proto, idna, thiserror, tracing.
   # Dev-only: process/temp assertions and authenticated local TLS test helpers.
   # Pin compatible current versions after checking license and maintenance metadata.
   ```

   - Disable reqwest default features and do not expose an invalid-certificate option.
   - Add only features required by the reviewed design.
   - Regenerate `Cargo.lock` with Cargo; never hand-edit it.

2. **VERIFY — `V21`:**

   **Preview:**

   ```text
   cargo check --all-targets --all-features
   cargo tree -e features
   └── Expect: edition-2024 crate resolves; rustls path is present; no native-tls requirement is introduced
   ```

   - Inspect crate license metadata for every direct dependency and record any exception as a blocker.

3. **COMMIT:** Run task checks and self-review.

   **Commit message:**

   ```text
   build(deps): add approved runtime dependencies

   Task: P1-G1-T1 — declare-the-minimal-approved-dependency-graph
   ```

##### P1-G1-T2 — implement-cli-and-strict-configuration

- **Outcome:** `start --config <PATH>` is mandatory, raw TOML rejects unknown fields, and validated config enforces loopback, HTTPS DoH, additive CA trust, known strategy, safe ports, unique targets, and bounded limits.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`
- **Success Criteria:** `S1`, `S2`, `S12`
- **Verification Cases:** `V1`, `V2`, `V20`
- **Files:** `src/main.rs`, `src/cli.rs`, `src/config.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V1`, `V2`:** `src/config.rs` and CLI tests

   **Preview:**

   ```rust
   // V1: representative valid TOML materializes expected loopback listeners,
   // DoH URL, defaults, limits, and two target entries.
   // V2: reject missing/unknown fields, non-loopback binds, non-HTTPS DoH,
   // unreadable CA PEM, duplicates, unsupported strategy, unsafe bounds.
   // CLI: `start --config` required; omission is a clap error.
   ```

   - Run focused tests and confirm they fail because contracts do not exist.

2. **GREEN:** `src/cli.rs`, `src/config.rs`, `src/main.rs`

   **Preview:**

   ```rust
   struct Cli { command: Command }
   enum Command { Start(StartArgs) }
   struct StartArgs { config: PathBuf }

   struct RawConfig { /* deny unknown fields */ }
   struct Config { proxy: ProxyConfig, dns: DnsConfig, limits: RuntimeLimits, targets: Vec<TargetRule> }
   impl Config {
       fn load(path: &Path) -> Result<Self, ConfigError>;
       fn validate(raw: RawConfig) -> Result<Self, ConfigError>;
   }
   ```

   - Keep file syntax types separate from validated runtime values.
   - `dns.ca_certificate` may add PEM roots only; no verification-disable field exists.
   - Keep `main` as minimal dispatch scaffolding until Part 3 wires runtime.

3. **REFACTOR:** centralize defaults/hard ceilings and error wording without adding new user-facing fields.

   **Preview:**

   ```rust
   // Keep validation rules in config.rs; downstream modules consume validated types.
   // Do not introduce a generic settings framework.
   ```

4. **COMMIT:** Run focused config/CLI tests and self-review.

   **Commit message:**

   ```text
   feat(config): add strict start configuration

   Task: P1-G1-T2 — implement-cli-and-strict-configuration
   ```

##### P1-G1-T3 — implement-canonical-domain-policy

- **Outcome:** All components share one IDNA-aware domain value and a deterministic exact/subdomain matcher that cannot overmatch suffix lookalikes.
- **Requirement Revisions:** `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES7`
- **Success Criteria:** `S5`, `S17`
- **Verification Cases:** `V3`, `V4`
- **Files:** `src/domain.rs`, `src/config.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V3`, `V4`:** `src/domain.rs`

   **Preview:**

   ```rust
   // Matrix: reddit.com and www.reddit.com match configured rule;
   // notreddit.com and reddit.com.example do not.
   // Case/trailing-dot/IDNA canonicalize identically.
   // Empty labels, IP literals, invalid IDNA, duplicate normalized rules fail.
   ```

   - Run domain tests and confirm expected failures.

2. **GREEN:** `src/domain.rs`, config conversion

   **Preview:**

   ```rust
   struct DomainName(String);
   struct TargetRule { host: DomainName, include_subdomains: bool, strategy: Strategy }
   struct TargetMatcher { rules: Vec<TargetRule> }

   impl DomainName { fn parse(input: &str) -> Result<Self, DomainError>; }
   impl TargetMatcher { fn find(&self, host: &DomainName) -> Option<&TargetRule>; }
   ```

   - Match complete labels and prefer the most-specific rule.
   - Keep fields private and expose only behavior later modules need.

3. **REFACTOR:** remove duplicate hostname validation from config and leave canonical ownership in `domain.rs`.

   **Preview:**

   ```rust
   // Config converts strings once; all later APIs accept DomainName.
   ```

4. **COMMIT:** Run domain/config tests and self-review.

   **Commit message:**

   ```text
   feat(domain): add canonical target matching

   Task: P1-G1-T3 — implement-canonical-domain-policy
   ```

**Verification:**

- `V1`–`V4` — focused module tests → exact valid/invalid outcomes.
- `V21` — `cargo check --all-targets --all-features` → successful compile.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Foundation tests | `cargo test config domain cli` or equivalent focused filters | All V1–V4 assertions pass |
| CLI help | `cargo run -- --help` and `cargo run -- start --help` | Required `start --config <PATH>` shown; no packet-tuning flags |
| Dependency/features | `cargo tree -e features` | Approved narrow graph; rustls path; no invalid-cert setting |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** exact command, pre-side-effect validation, loopback/HTTPS/additive-CA constraints, canonical domain matching.
- **Technical:** checked bounds, useful structured errors, private validated fields, no `unsafe`, no duplicate domain rules, narrow dependency features.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

### P1-G2 — proxy-facing-pure-contracts

#### Goal

Provide bounded CONNECT parsing, target-only PAC rendering, and stable secret-safe outcome types before any network service is composed.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S4`, `S5`, `S12`, `S15`, `S17`
- **Verification Cases:** `V5`, `V6`, `V7`, `V19`

#### Coherence Boundary

These are deterministic values/functions consumed by the Part 3 servers. Implementing them first keeps socket orchestration free of parsing, policy, JavaScript-generation, and log-taxonomy invention.

#### Scope

- Included: CONNECT request parser and errors, PAC body renderer, outcome/report models, tests.
- Excluded: listener accept loops, HTTP response writes, tunnel relay, runtime logging wiring.
- Main files: `src/proxy.rs`, `src/pac.rs`, `src/diagnostics.rs`.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1` (all prefixed `W260914RUZU-`) | `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | `S3`, `S4`, `S5`, `S12`, `S15`, `S17` | `V5`–`V7`, `V19` | Pure proxy-facing contracts are bounded, deterministic, and safe to wire |

#### Cross-Work Gates

None.

#### Tasks

##### P1-G2-T1 — implement-bounded-connect-parsing

- **Outcome:** A bounded parser accepts only hostname-form HTTP/1.1 CONNECT authorities on allowed ports and returns precise client-status/error categories for malformed input.
- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S12`, `S17`
- **Verification Cases:** `V5`, `V6`
- **Files:** `src/proxy.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V5`, `V6`:** parser table tests

   **Preview:**

   ```rust
   // Accept CONNECT reddit.com:443 HTTP/1.1 with bounded headers.
   // Reject wrong methods/version, missing/invalid authority, IP-form target,
   // disallowed port, body ambiguity, and oversized/incomplete headers.
   // Assert normalized request or exact ProxyError/status; never panic.
   ```

2. **GREEN:** `ConnectRequest`, `ProxyError`, `parse_connect`

   **Preview:**

   ```rust
   struct ConnectRequest { host: DomainName, port: u16 }
   fn parse_connect(header: &[u8], allowed_ports: &BTreeSet<u16>)
       -> Result<ConnectRequest, ProxyError>;
   ```

   - Parse only the approved subset; do not add a general HTTP framework.
   - Keep maximum-header enforcement at the caller boundary and parser length checks defensive.

3. **REFACTOR:** consolidate error-to-status mapping and keep request bytes out of `Display`.

   **Preview:**

   ```rust
   // ProxyError exposes status/outcome, not raw untrusted headers.
   ```

4. **COMMIT:** Run parser tests and self-review.

   **Commit message:**

   ```text
   feat(proxy): add bounded connect parsing

   Task: P1-G2-T1 — implement-bounded-connect-parsing
   ```

##### P1-G2-T2 — implement-target-only-pac-rendering

- **Outcome:** Deterministic PAC JavaScript proxies exact configured domains and explicitly enabled subdomains while returning `DIRECT` for unrelated/lookalike hosts.
- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ8@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`
- **Success Criteria:** `S4`, `S5`, `S15`
- **Verification Cases:** `V7`
- **Files:** `src/pac.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V7`:** PAC behavior/string tests

   **Preview:**

   ```rust
   // Assert exact configured hosts and enabled subdomains select PROXY.
   // Assert notreddit.com and unrelated hosts select DIRECT.
   // Assert canonical ASCII constants and stable deterministic output.
   ```

2. **GREEN:** `render_pac`

   **Preview:**

   ```rust
   fn render_pac(proxy_addr: SocketAddr, rules: &[TargetRule]) -> String;
   // Generate exact host equality plus dnsDomainIs(host, ".example.com")
   // only for include_subdomains rules, then DIRECT fallback.
   ```

3. **REFACTOR:** isolate JavaScript string escaping for canonical values; do not add templating dependencies.

   **Preview:**

   ```rust
   // One small renderer; immutable generated body consumed by Part 3 PacServer.
   ```

4. **COMMIT:** Run PAC/domain tests and self-review.

   **Commit message:**

   ```text
   feat(pac): render target-only proxy policy

   Task: P1-G2-T2 — implement-target-only-pac-rendering
   ```

##### P1-G2-T3 — define-secret-safe-diagnostic-contracts

- **Outcome:** Runtime components can return stable kebab-case outcomes and one terminal tunnel report without storing/logging traffic payloads.
- **Requirement Revisions:** `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`
- **Design IDs:** `W260914RUZU-DES5`, `W260914RUZU-DES6`
- **Success Criteria:** `S13`, `S15`, `S16`
- **Verification Cases:** `V19`
- **Files:** `src/diagnostics.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V19`:** outcome/report formatting tests

   **Preview:**

   ```rust
   // Assert stable strings: ready, direct-relay, target-transformed,
   // unsupported-request, config-error, dns-error, connect-error,
   // tls-error, timeout, cancelled, internal-error.
   // Assert report debug/display omits test secrets and raw bytes.
   ```

2. **GREEN:** `Outcome`, `TunnelReport`, top-level error mapping hooks

   **Preview:**

   ```rust
   enum Outcome { Ready, DirectRelay, TargetTransformed, /* bounded stable set */ }
   struct TunnelReport {
       host: DomainName, port: u16, target: bool,
       outcome: Outcome, duration: Duration,
       bytes_up: u64, bytes_down: u64,
   }
   ```

3. **REFACTOR:** keep outcome conversion exhaustive and module-independent.

   **Preview:**

   ```rust
   // No dynamic arbitrary category strings and no captured request/handshake bodies.
   ```

4. **COMMIT:** Run diagnostics tests and self-review.

   **Commit message:**

   ```text
   feat(diagnostics): add stable tunnel outcomes

   Task: P1-G2-T3 — define-secret-safe-diagnostic-contracts
   ```

**Verification:**

- `V5`, `V6` — CONNECT parser table → exact accepted/rejected outcomes.
- `V7` — PAC renderer test → target-only proxy policy.
- `V19` — diagnostics formatting test → stable categories and no secret payload.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Pure proxy contracts | focused `cargo test` filters for proxy, pac, diagnostics | V5–V7 and diagnostic contract pass |
| Static safety review | inspect public fields/error displays | No raw headers/payloads or unbounded strings enter logs |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** approved CONNECT subset, target-only PAC policy, stable outcome vocabulary.
- **Technical:** no general HTTP/TLS interception framework, bounded errors, canonical hostname reuse, deterministic output, no secret-bearing formatting.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

## Coverage

| Requirement Revision | Design ID | Success Criterion | Verification Cases | Groups | Tasks |
|---|---|---|---|---|---|
| `W260914RUZU-REQ1@R1` | `W260914RUZU-DES2` | `S1`, `S2` | `V1`, `V2`, `V20` | G1 | T2 |
| `W260914RUZU-REQ2@R1` | `W260914RUZU-DES1` | `S3`, `S4` | `V5`–`V7` | G2 | T1, T2 |
| `W260914RUZU-REQ3@R1` | `W260914RUZU-DES2` | `S5` | `V3`, `V4`, `V7` | G1, G2 | G1-T3, G2-T2 |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S2`, `S12` | `V1`, `V2` | G1 | T1, T2 |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES5` | `S12` | `V2`, `V6` | G1, G2 | G1-T2, G2-T1, G2-T3 |
| `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | `S13` | `V19` | G2 | T3 |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | `S15`, `S16` | `V7`, `V19` | G2 | T2, T3 |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V1`–`V7`, `V21` | G1, G2 | all tasks |

## Completion Criteria

- Every group satisfies its acceptance criteria.
- Every dependency and cross-work gate due within this part is satisfied; none apply.
- Every source obligation assigned to Part 1 is covered.
- Validated foundation outputs are committed and ready for Parts 2–4.

## Planning Gaps

`None`
