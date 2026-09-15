# W260914RUZU: splithello-start — System Design

**Design Status:** Ready for Planning

**Context Source:** `w260914ruzu-discuss-context.md`

**Requirements Source:** `w260914ruzu-requirements.md`

**Research Source:** None

**Current Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`

**Research Reconciliation:** None — formal research was skipped; current library documentation was checked only to select maintainable interfaces.

**Unresolved Items:** None

**Ready for Planning Checklist:**

- [x] Every current qualified requirement revision has a design decision.
- [x] Roadmap checks are not applicable — standalone work.
- [x] No historical requirement revision is used.
- [x] Formal research reconciliation is not applicable.
- [x] The design progresses from architecture to concrete files and symbols.
- [x] Every focal component has a proposed location and ownership.
- [x] The project tree accounts for source, configuration, documentation, and tests.
- [x] Every logical contract and state item has a canonical section 7 shape.
- [x] Every changed boundary has a contract and failure rule.
- [x] The only existing symbol, `main`, has an exact current source and final delta.
- [x] Detailed design covers types, functions, dependencies, invariants, wiring, flows, and verification.
- [x] Runtime flows name participating symbols and failure policies.
- [x] Cross-cutting, migration, and delivery concerns are resolved.
- [x] Every requirement, decision, and invariant maps to a success criterion and verification case.
- [x] Tests form one verification tree and use the lowest sufficient level.
- [x] Traceability is complete from current requirement through exact assertion.
- [x] Omitted structures and checks state why they do not apply.

# Table of Contents

1. [Design Goals, Constraints, and Requirement Coverage](#1-design-goals-constraints-and-requirement-coverage)
2. [System Context and Architecture](#2-system-context-and-architecture)
3. [Components and Boundaries](#3-components-and-boundaries)
4. [Project Structure Changes](#4-project-structure-changes)
5. [Runtime Flows and Error Policies](#5-runtime-flows-and-error-policies)
6. [Logical Contracts, Data Models, and State](#6-logical-contracts-data-models-and-state)
7. [Detailed Code Design](#7-detailed-code-design)
8. [Testing and Verification](#8-testing-and-verification)
9. [Finalized Design Decisions](#9-finalized-design-decisions)
   - [W260914RUZU-DES1: Explicit loopback CONNECT proxy](#w260914ruzu-des1-explicit-loopback-connect-proxy)
   - [W260914RUZU-DES2: Strict typed configuration and domain policy](#w260914ruzu-des2-strict-typed-configuration-and-domain-policy)
   - [W260914RUZU-DES3: Target-only DNS-over-HTTPS resolver](#w260914ruzu-des3-target-only-dns-over-https-resolver)
   - [W260914RUZU-DES4: Minimal TLS record rewriter](#w260914ruzu-des4-minimal-tls-record-rewriter)
   - [W260914RUZU-DES5: Isolated fail-open runtime](#w260914ruzu-des5-isolated-fail-open-runtime)
   - [W260914RUZU-DES6: Stable diagnostics and documentation](#w260914ruzu-des6-stable-diagnostics-and-documentation)
   - [W260914RUZU-DES7: Deterministic verification architecture](#w260914ruzu-des7-deterministic-verification-architecture)

# 1. Design Goals, Constraints, and Requirement Coverage

## Goals

- Provide the exact `splithello start --config <path>` operation with one readable configuration file.
- Keep version 1 unprivileged and user-space-only.
- Route only configured target hostnames through encrypted DNS and a narrow TLS ClientHello record transformation.
- Preserve browser-to-origin TLS, unrelated traffic, bounded resource use, and per-tunnel failure isolation.
- Make unsupported blocking mechanisms and deployment mistakes observable without claiming universal bypass.

## Requirement Coverage

- `W260914RUZU-REQ1@R1` — stable command and strict configuration
  - **Coverage:** `W260914RUZU-DES2`
  - **How Addressed:** Clap derives implement the exact subcommand; Serde/TOML with `deny_unknown_fields` validates before `Runtime::start`.
  - **Affected Files and Symbols:** `src/cli.rs` — `Cli`, `Command`, `StartArgs`; `src/config.rs` — `Config`, `Config::load`; `config.example.toml`.
  - **Callers and Wiring:** `src/main.rs` — `run`.
  - **Runtime Flow:** Configuration and startup flow.
  - **Success Criteria:** `S1`, `S2`.
  - **Verification:** `V1`, `V2`, `V18`, `V20`.
  - **Research Support:** None.
- `W260914RUZU-REQ2@R1` — loopback CONNECT/PAC boundary
  - **Coverage:** `W260914RUZU-DES1`
  - **How Addressed:** Tokio listeners expose one HTTP/1.1 CONNECT service and optional PAC endpoint without firewall or raw-socket integration.
  - **Affected Files and Symbols:** `src/proxy.rs` — `ProxyServer`, `handle_client`; `src/pac.rs` — `PacServer`, `render_pac`.
  - **Callers and Wiring:** `src/app.rs` — `Runtime::start`.
  - **Runtime Flow:** Selected and direct CONNECT flows; construction flow.
  - **Success Criteria:** `S3`, `S4`.
  - **Verification:** `V5`, `V6`, `V7`, `V14`.
  - **Research Support:** None.
- `W260914RUZU-REQ3@R1` — exact domain policy
  - **Coverage:** `W260914RUZU-DES2`
  - **How Addressed:** `DomainName` canonicalizes IDNA and `TargetMatcher` compares complete labels with explicit subdomain policy.
  - **Affected Files and Symbols:** `src/domain.rs` — `DomainName`, `TargetMatcher`; `src/config.rs` — `TargetConfig`.
  - **Callers and Wiring:** `proxy::handle_connect`, `pac::render_pac`.
  - **Runtime Flow:** Selected and direct CONNECT flows.
  - **Success Criteria:** `S5`.
  - **Verification:** `V3`, `V4`, `V7`.
  - **Research Support:** None.
- `W260914RUZU-REQ4@R1` — encrypted selected-domain resolution
  - **Coverage:** `W260914RUZU-DES3`
  - **How Addressed:** `DohResolver` constructs DNS messages with hickory-proto, sends bounded RFC 8484 POST requests with reqwest/rustls, validates response identity, and caches IPv4 answers by bounded TTL.
  - **Affected Files and Symbols:** `src/dns.rs` — `DohResolver`, `DnsCache`, `TargetResolver`, `SystemResolver`.
  - **Callers and Wiring:** `proxy::connect_upstream`.
  - **Runtime Flow:** Selected CONNECT flow.
  - **Success Criteria:** `S6`, `S7`.
  - **Verification:** `V8`, `V9`, `V10`, `V15`.
  - **Research Support:** Current reqwest and hickory-proto documentation checked for API suitability; not a formal RF finding.
- `W260914RUZU-REQ5@R1` — standards-valid ClientHello fragmentation
  - **Coverage:** `W260914RUZU-DES4`
  - **How Addressed:** A small internal parser buffers bounded TLS records, reassembles one ClientHello handshake, locates SNI, verifies it against CONNECT authority, and serializes two valid handshake records while preserving handshake bytes.
  - **Affected Files and Symbols:** `src/tls.rs` — `ClientHelloBuffer`, `ParsedClientHello`, `rewrite_client_hello`.
  - **Callers and Wiring:** `proxy::relay_target`.
  - **Runtime Flow:** Selected CONNECT flow.
  - **Success Criteria:** `S8`, `S9`, `S10`.
  - **Verification:** `V11`, `V12`, `V13`, `V15`.
  - **Research Support:** None; RFC contracts govern the design.
- `W260914RUZU-REQ6@R1` — end-to-end TLS and local security
  - **Coverage:** `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5`
  - **How Addressed:** The proxy never instantiates a TLS server for tunneled traffic; listener validation is loopback-only; buffers, ports, durations, and concurrency are bounded.
  - **Affected Files and Symbols:** `src/config.rs` — validation; `src/proxy.rs` — tunnel relay; `src/tls.rs` — handshake-only rewrite.
  - **Callers and Wiring:** `app::Runtime`, `proxy::handle_connect`.
  - **Runtime Flow:** All CONNECT flows and shutdown.
  - **Success Criteria:** `S11`, `S12`.
  - **Verification:** `V2`, `V6`, `V13`, `V16`.
  - **Research Support:** None.
- `W260914RUZU-REQ7@R1` — fail-open and owned cleanup
  - **Coverage:** `W260914RUZU-DES5`
  - **How Addressed:** Tunnel tasks are independent, unselected requests use direct relay, startup owns only Tokio listeners, and cancellation is coordinated by `CancellationToken` plus `JoinSet` timeout/abort.
  - **Affected Files and Symbols:** `src/app.rs` — `Runtime`; `src/proxy.rs` — task handling and relay.
  - **Callers and Wiring:** `main::run`.
  - **Runtime Flow:** Direct CONNECT, selected failure, startup rollback, and shutdown flows.
  - **Success Criteria:** `S13`, `S14`.
  - **Verification:** `V14`, `V15`, `V16`, `V17`.
  - **Research Support:** Current Tokio documentation checked for `TcpListener`, `JoinSet`, and abort/drain behavior.
- `W260914RUZU-REQ8@R1` — diagnostics and scope
  - **Coverage:** `W260914RUZU-DES6`
  - **How Addressed:** Tracing events use a stable `Outcome` category; startup prints setup and limitations; README documents proxy activation, checks, and non-goals.
  - **Affected Files and Symbols:** `src/diagnostics.rs` — `Outcome`, `TunnelReport`; `README.md`; `src/app.rs` startup logs.
  - **Callers and Wiring:** all runtime components emit through tracing.
  - **Runtime Flow:** Startup and every tunnel completion.
  - **Success Criteria:** `S15`, `S16`.
  - **Verification:** `V18`, `V19`.
  - **Research Support:** None.
- `W260914RUZU-REQ9@R1` — focused verification
  - **Coverage:** `W260914RUZU-DES7`
  - **How Addressed:** Unit tests own parsers and invariants; one local integration suite owns real TCP/HTTP collaboration; build checks own compile/lint/format evidence.
  - **Affected Files and Symbols:** inline module tests; `tests/start_proxy.rs`; `tests/support/mod.rs`.
  - **Callers and Wiring:** Cargo test targets and binary invocation.
  - **Runtime Flow:** all flows.
  - **Success Criteria:** `S17`.
  - **Verification:** `V1`–`V21`.
  - **Research Support:** None.

## Assigned Parent Obligation Coverage

None — standalone work.

## Binding Constraints

- `.s1mple/S1MPLE.md:15` — use the simplest robust design; avoid transparent packet interception.
- `.s1mple/docs/languages/rust.md:65` — dependencies must be maintained, licensed, explicit, and complexity-reducing.
- `.s1mple/docs/languages/rust.md:88` — recoverable failures use structured errors.
- `.s1mple/docs/languages/rust.md:93` — configuration, proxy input, DNS messages, and TLS bytes are validated boundaries.
- `.s1mple/docs/languages/rust.md:100` — no `unsafe`; safe user-space APIs satisfy version 1.
- `src/main.rs:1-3` — the placeholder composition root is the only current code and will be replaced.

## Cross-Cutting Applicability

| Concern | Applicability | Design Location or Reason |
|---|---|---|
| Security and privacy | Applicable | `config::Config::validate`, `proxy::parse_connect`, `tls::rewrite_client_hello`, loopback-only listeners, bounded input, and `V2`, `V6`, `V13`, `V19` |
| Configuration | Applicable | `src/config.rs`, `config.example.toml`, `V1`, `V2`, `V18` |
| Observability | Applicable | `src/diagnostics.rs`, startup/tunnel tracing, `V18`, `V19` |
| Performance and scalability | Applicable | `RuntimeLimits`, semaphore, bounded buffers/cache, streaming relay, `V16` |
| Concurrency | Applicable | Tokio accept loops, `JoinSet`, `CancellationToken`, per-tunnel tasks, `V14`–`V17` |
| Retry and idempotency | Applicable | Multiple A-record candidates are attempted once each within one deadline; no application request is replayed; cache lookup is idempotent; `V10`, `V15` |
| Migration and rollout | Applicable | New binary behavior and opt-in proxy setup; no persistent data or automatic system mutation; README rollback is stop process and remove proxy setting; `V18`, `V20` |

## Focused Code Surface

- **Existing:** `src/main.rs` — `main` — placeholder composition root to replace.
- **Proposed:** `src/cli.rs` — CLI contracts — `W260914RUZU-DES2`.
- **Proposed:** `src/app.rs` — runtime construction, cancellation, and listeners — `W260914RUZU-DES1`, `W260914RUZU-DES5`.
- **Proposed:** `src/config.rs`, `src/domain.rs` — validated configuration and matching — `W260914RUZU-DES2`.
- **Proposed:** `src/proxy.rs`, `src/pac.rs` — CONNECT/PAC services — `W260914RUZU-DES1`, `W260914RUZU-DES5`.
- **Proposed:** `src/dns.rs` — target DoH and cache — `W260914RUZU-DES3`.
- **Proposed:** `src/tls.rs` — bounded ClientHello transformation — `W260914RUZU-DES4`.
- **Proposed:** `src/diagnostics.rs` — stable outcome vocabulary — `W260914RUZU-DES6`.

# 2. System Context and Architecture

## System Context

```text
User
 ├─ starts `splithello start --config config.example.toml`
 └─ configures browser HTTPS proxy or PAC URL

Browser ──HTTP CONNECT──> SplitHello loopback boundary
                              ├─ unselected host → OS resolver → IPv4/IPv6 TCP origin
                              └─ selected host   → DoH resolver → IPv4 TCP origin
                                                       │
Browser TLS ClientHello ────────────────────────────────┤
                              selected: rewrite TLS record framing only
                              unselected: relay unchanged
                                                       ↓
                                            Reddit / Medium origin

ISP observes destination IP and traffic metadata; version 1 makes no anonymity claim.
```

## Proposed Architecture

```text
main::run
  → cli::Cli
  → config::Config::load + validate
  → app::Runtime::start
       ├─ dns::DohResolver + DnsCache
       ├─ proxy::ProxyServer
       │    ├─ proxy::parse_connect
       │    ├─ domain::TargetMatcher
       │    ├─ dns::TargetResolver / dns::SystemResolver
       │    ├─ tls::rewrite_client_hello [selected]
       │    └─ tokio::io::copy_bidirectional
       ├─ pac::PacServer [optional]
       └─ diagnostics::TunnelReport
  → app::Runtime::run_until_signal
  → app::Runtime::shutdown
```

The architecture uses one binary and cohesive modules, not separate crates or plugin abstractions. The accepted tradeoff is explicit browser proxy setup in exchange for no root privileges, no firewall lifecycle, kernel-managed TCP retransmission, and sharply reduced implementation surface.

# 3. Components and Boundaries

## CLI and Configuration

- **Target:** `src/cli.rs`, `src/config.rs`, `src/domain.rs`
- **Kind:** command and validated data-model modules
- **Responsibilities:**
  - Define `start --config <path>`.
  - Deserialize strict TOML and validate all limits before side effects.
  - Normalize domains and compile target matching policy.
- **Collaborators:**

  ```text
  cli::StartArgs → config::Config::load → domain::TargetMatcher → app::Runtime::start
  ```

- **Boundary Contract:** `Config` and `TargetMatcher` in section 6.
- **Detailed Design:** section 7 entries for `src/cli.rs`, `src/config.rs`, and `src/domain.rs`.
- **Must Not Own:** listeners, DNS transport, TLS parsing, or runtime tasks.

## Application Runtime

- **Target:** `src/app.rs`
- **Kind:** composition root and lifecycle coordinator
- **Responsibilities:**
  - Construct shared dependencies.
  - Bind all required listeners before reporting ready.
  - Own cancellation and task draining.
- **Collaborators:**

  ```text
  Runtime → ProxyServer
          → PacServer
          → DohResolver
          → CancellationToken + JoinSet
  ```

- **Boundary Contract:** `Runtime` lifecycle in section 6.
- **Detailed Design:** section 7 `src/app.rs`.
- **Must Not Own:** CONNECT parsing, domain matching rules, DNS wire parsing, or TLS record rewriting.

## CONNECT Proxy

- **Target:** `src/proxy.rs`
- **Kind:** local network service and tunnel orchestrator
- **Responsibilities:**
  - Parse bounded HTTP/1.1 CONNECT headers.
  - Select direct or target path.
  - Connect selected targets to IPv4 candidates and unselected hosts through the operating system's normal IPv4/IPv6 address candidates.
  - Transform only the first selected ClientHello and stream thereafter.
- **Collaborators:**

  ```text
  ProxyServer → TargetMatcher → TargetResolver/SystemResolver → ClientHelloRewriter → copy_bidirectional
  ```

- **Boundary Contract:** `ConnectRequest`, `TunnelMode`, `TargetResolver`, and `SystemResolver` in section 6.
- **Detailed Design:** section 7 `src/proxy.rs`.
- **Must Not Own:** TOML loading, DNS protocol implementation, PAC generation, or TLS application plaintext.

## DNS-over-HTTPS Resolver

- **Target:** `src/dns.rs`
- **Kind:** external-service adapter with in-memory cache
- **Responsibilities:**
  - Build and validate A-record DNS messages.
  - Send authenticated RFC 8484 requests.
  - Return bounded IPv4 candidates and cache them by TTL.
- **Collaborators:**

  ```text
  proxy::connect_upstream → TargetResolver::resolve_ipv4 → DohResolver → reqwest → DoH upstream
  ```

- **Boundary Contract:** `TargetResolver`, `SystemResolver`, `ResolveError`, and `DnsCache` in section 6.
- **Detailed Design:** section 7 `src/dns.rs`.
- **Must Not Own:** target selection, CONNECT responses, or tunnel relay.

## TLS ClientHello Rewriter

- **Target:** `src/tls.rs`
- **Kind:** deterministic protocol transformation module
- **Responsibilities:**
  - Accumulate bounded TLS handshake records.
  - Parse only enough ClientHello structure to locate and verify SNI.
  - Emit two valid handshake records around an SNI-internal boundary.
- **Collaborators:**

  ```text
  proxy::relay_target → ClientHelloBuffer → ParsedClientHello → rewrite_client_hello
  ```

- **Boundary Contract:** `RewriteOutcome` and byte-preservation invariant in section 6.
- **Detailed Design:** section 7 `src/tls.rs`.
- **Must Not Own:** TCP packet construction, TLS cryptography, certificates, post-handshake data, or general TLS analysis.

## PAC and Diagnostics

- **Target:** `src/pac.rs`, `src/diagnostics.rs`, `README.md`
- **Kind:** local setup adapter and observability contract
- **Responsibilities:**
  - Produce deterministic PAC JavaScript for the configured matcher.
  - Expose stable outcome categories and secret-safe tunnel reports.
  - Document setup, verification, and limitations.
- **Collaborators:**

  ```text
  Config + TargetMatcher → render_pac
  Proxy/DNS/TLS/App outcomes → TunnelReport → tracing subscriber
  ```

- **Boundary Contract:** `Outcome` and PAC response rules in section 6.
- **Detailed Design:** section 7 entries for `src/pac.rs`, `src/diagnostics.rs`, and `README.md`.
- **Must Not Own:** proxy mutation, DNS resolution, or TLS parsing.

# 4. Project Structure Changes

**Proposed Structure — not an actual Git diff.**

**Legend:** `[A] Added` · `[M] Modified`

```text
./
├── [M] Cargo.toml
│       Contains: package metadata and explicit maintained dependencies
│       Symbols: dependency/features declarations
│       Decisions: W260914RUZU-DES1–W260914RUZU-DES7
├── [M] Cargo.lock
│       Contains: Cargo-generated dependency resolution
│       Symbols: None — generated data
│       Decisions: W260914RUZU-DES1–W260914RUZU-DES7
├── [A] README.md
│       Contains: setup, command, PAC/browser configuration, diagnostics, scope, rollback
│       Symbols: None — documentation
│       Decisions: W260914RUZU-DES6
├── [A] config.example.toml
│       Contains: loopback proxy/PAC, DoH, limits, Reddit and Medium targets
│       Symbols: Config example
│       Decisions: W260914RUZU-DES2, W260914RUZU-DES3
├── src/
│   ├── [M] main.rs
│   │       Contains: tracing initialization, CLI dispatch, process exit mapping
│   │       Symbols: main, run
│   │       Decisions: W260914RUZU-DES2, W260914RUZU-DES5, W260914RUZU-DES6
│   ├── [A] cli.rs
│   │       Contains: clap command surface
│   │       Symbols: Cli, Command, StartArgs
│   │       Decisions: W260914RUZU-DES2
│   ├── [A] app.rs
│   │       Contains: composition, listener binding, signals, cancellation, task drain
│   │       Symbols: Runtime, Runtime::start, run_until_signal, shutdown
│   │       Decisions: W260914RUZU-DES1, W260914RUZU-DES5
│   ├── [A] config.rs
│   │       Contains: strict TOML schema and cross-field validation
│   │       Symbols: Config, ProxyConfig, DnsConfig, TargetConfig, RuntimeLimits
│   │       Decisions: W260914RUZU-DES2, W260914RUZU-DES3, W260914RUZU-DES5
│   ├── [A] domain.rs
│   │       Contains: canonical hostname value and label-aware matching
│   │       Symbols: DomainName, TargetRule, TargetMatcher
│   │       Decisions: W260914RUZU-DES2
│   ├── [A] proxy.rs
│   │       Contains: CONNECT parser, path selection, upstream connection, relay
│   │       Symbols: ProxyServer, ConnectRequest, TunnelMode, handle_client
│   │       Decisions: W260914RUZU-DES1, W260914RUZU-DES4, W260914RUZU-DES5
│   ├── [A] pac.rs
│   │       Contains: optional PAC listener and deterministic PAC renderer
│   │       Symbols: PacServer, render_pac
│   │       Decisions: W260914RUZU-DES1, W260914RUZU-DES2
│   ├── [A] dns.rs
│   │       Contains: TargetResolver/SystemResolver contracts, DoH implementation, bounded TTL cache
│   │       Symbols: TargetResolver, SystemResolver, DohResolver, DnsCache, ResolveError
│   │       Decisions: W260914RUZU-DES3, W260914RUZU-DES5
│   ├── [A] tls.rs
│   │       Contains: bounded record/handshake parser and record serializer
│   │       Symbols: ClientHelloBuffer, ParsedClientHello, RewriteOutcome, rewrite_client_hello
│   │       Decisions: W260914RUZU-DES4
│   └── [A] diagnostics.rs
│           Contains: stable operation outcomes and secret-safe report fields
│           Symbols: Outcome, TunnelReport
│           Decisions: W260914RUZU-DES6
└── tests/
    ├── [A] start_proxy.rs
    │       Contains: CLI/runtime integration scenarios through real loopback TCP
    │       Symbols: integration test cases V14–V19
    │       Decisions: W260914RUZU-DES1, W260914RUZU-DES3–W260914RUZU-DES7
    └── [A] support/mod.rs
            Contains: local CONNECT client, fake DoH endpoint, fake TLS origin, process harness
            Symbols: TestDoh, TestOrigin, RunningSplitHello, build_client_hello
            Decisions: W260914RUZU-DES7
```

# 5. Runtime Flows and Error Policies

## Configuration and Startup

**Participating Symbols:** `main::run`, `Cli`, `Config::load`, `Runtime::start`

**Illustrative Flow:**

```text
run → Cli::parse → Config::load → Config::validate
                              ├─ error → non-zero exit, no listeners
                              └─ valid → Runtime::start
                                           ├─ bind proxy
                                           ├─ bind PAC if enabled
                                           └─ ready event
```

1. `run` parses a required `StartArgs.config` path.
2. `Config::load` reads, deserializes, canonicalizes, and validates all fields.
3. `Runtime::start` constructs the resolver and binds required loopback listeners before spawning accept loops.
4. If a bind fails, already-created listener values are dropped before returning an error.

**Detailed Control Flow:** section 7 `src/main.rs`, `src/config.rs`, and `src/app.rs`.

## Selected CONNECT Tunnel

**Participating Symbols:** `handle_client`, `parse_connect`, `TargetMatcher::find`, `DohResolver::resolve_ipv4`, `connect_socket_candidates`, `rewrite_client_hello`, `copy_bidirectional`

**Illustrative Flow:**

```text
client → parse CONNECT → target match → DoH A records → IPv4 TCP connect
       ← 200 established ←────────────── upstream ready
client → bounded ClientHello buffer → verify SNI → rewrite record framing → origin
client ⇄ copy_bidirectional ⇄ origin
```

1. `handle_client` reads at most `max_connect_header_bytes` until the HTTP header terminator.
2. `parse_connect` accepts only configured ports and hostname authorities.
3. `TargetMatcher::find` selects the exact target rule.
4. `DohResolver::resolve_ipv4` returns cached or validated A records.
5. `connect_socket_candidates` tries candidates in response order within one total connect deadline.
6. Only after upstream success does the proxy send `200 Connection Established`.
7. `rewrite_client_hello` buffers within `client_hello_bytes` and `client_hello_timeout`, verifies SNI, and emits valid fragmented records.
8. `copy_bidirectional` streams the rest until EOF, cancellation, idle timeout, or error.

**Detailed Control Flow:** section 7 `src/proxy.rs`, `src/dns.rs`, and `src/tls.rs`.

## Unselected CONNECT Tunnel

**Participating Symbols:** `handle_client`, `TargetMatcher::find`, `SystemResolver::resolve`, `connect_socket_candidates`, `copy_bidirectional`

**Illustrative Flow:**

```text
client → CONNECT → no target → normal OS IPv4/IPv6 resolution → TCP connect → 200
client ⇄ unchanged streaming relay ⇄ origin
```

No TLS bytes are parsed or changed. This is the fail-open path for unrelated destinations when the browser uses the proxy globally.

**Detailed Control Flow:** section 7 `src/proxy.rs`.

## Shutdown

**Participating Symbols:** `run_until_signal`, `CancellationToken::cancel`, `JoinSet::join_next`, `JoinSet::abort_all`

**Illustrative Flow:**

```text
SIGINT/SIGTERM → cancel token → accept loops stop → active tasks drain until deadline
                                             └─ deadline → abort remaining → join all
```

**Detailed Control Flow:** section 7 `src/app.rs`.

## Failure and Recovery Rules

- **Invalid configuration** — detected by `Config::load`; represented as `ConfigError`; process exits non-zero before listener creation.
- **Malformed/unsupported CONNECT** — detected by `parse_connect`; represented as `ProxyError`; client receives 400, 403, 405, or 431 and that connection closes.
- **DoH timeout or invalid response** — detected by `DohResolver`; represented as `ResolveError`; selected tunnel receives 502/504 before CONNECT success and other tunnels continue.
- **No reachable candidate** — detected by `connect_socket_candidates`; represented as `ConnectError`; client receives 502/504 and diagnostics identify refusal versus timeout.
- **Invalid or mismatched ClientHello** — detected before transformed bytes are sent by `rewrite_client_hello`; buffered bytes are relayed unchanged only for the explicit `PassThrough` outcome; unsafe ambiguity closes only that tunnel.
- **Post-establishment relay error** — detected by `copy_bidirectional`; represented in `TunnelReport`; closes only that tunnel.
- **Task panic** — `JoinSet` join handling records an internal outcome; the listener remains active and shutdown still drains/aborts all tasks.
- **Cancellation** — shared token stops accept loops and tunnel waits; no retries or persistent rollback are needed because the process owns only sockets and memory.

## Construction and Wiring Flow

```text
main::run → Config::load
          → Runtime::start
               ├→ TargetMatcher::new
               ├→ reqwest::Client + DohResolver + DnsCache
               ├→ ProxyServer::bind
               └→ PacServer::bind [optional]
          → Runtime::run_until_signal
          → Runtime::shutdown
```

- **Construction:** `src/app.rs` — `Runtime::start` creates shared `Arc<Config>`, matcher, resolver, semaphore, cancellation token, and listeners.
- **Registration:** `Runtime::start` registers accept loops as `JoinSet` tasks; no framework route registry applies.
- **Exports:** internal modules remain crate-private; no library API is introduced.
- **Configuration:** `config.example.toml` maps directly to `Config`; all defaults and bounds are materialized during validation.

# 6. Logical Contracts, Data Models, and State

| Item | Kind | Purpose | Used By |
|---|---|---|---|
| `Config` | Validated configuration model | Canonical startup contract | `main::run`, `Runtime::start`, all services |
| `DomainName` / `TargetMatcher` | Value object and policy | Safe label-aware destination selection | config, proxy, PAC |
| `ConnectRequest` / `TunnelMode` | Boundary contract | Parsed authority and selected relay behavior | proxy handler |
| `TargetResolver` / `SystemResolver` / `DnsCache` | Async contracts and state | Selected IPv4 DoH resolution, normal direct-host resolution, and TTL reuse | proxy upstream connector |
| `RewriteOutcome` | Transformation result | Makes transformed, pass-through, and rejected states explicit | target relay |
| `Runtime` | Lifecycle state | Owns listeners, tasks, cancellation, and shutdown | main |
| `Outcome` / `TunnelReport` | Observability contract | Stable secret-safe completion categories | every runtime component |

## `Config` — Validated Configuration Model

**Purpose:** Represents the only accepted startup configuration after defaults, normalization, and cross-field validation.

**Includes:**

- `proxy` — loopback listener addresses and allowed CONNECT ports.
- `dns` — HTTPS DoH URL, optional additional CA-certificate path scoped only to the DoH client, timeouts, cache ceiling, and response limit.
- `limits` — header, ClientHello, concurrency, idle, and shutdown bounds.
- `targets` — normalized domain policies and selected strategy.

**Usage:**

```text
Config::load → validated Config → Runtime::start and component constructors
```

**Rules:**

- **Compatibility:** version 1 rejects unknown fields; adding optional fields with defaults is backward-compatible, while renamed/removed fields require a documented config-version change.
- **Invariants:** listeners are loopback; targets are unique; at least one target exists; DoH is HTTPS; an optional DoH CA file is readable PEM and augments rather than disables certificate verification; all sizes/durations/counts remain within hard safety ceilings.
- **Ownership and Lifecycle:** immutable `Arc<Config>` created once at startup and dropped at shutdown.

**Location:** `src/config.rs` — proposed item.

**Detailed Shape:** section 7 `src/config.rs`.

## `DomainName` / `TargetMatcher` — Value Object and Policy

**Purpose:** Ensures all components use one normalized hostname representation and exact DNS-label matching.

**Includes:**

- `DomainName` — canonical ASCII lowercase labels without a trailing dot.
- `TargetRule` — host, include-subdomains flag, and strategy.
- `TargetMatcher` — immutable rules in deterministic most-specific-first order.

**Usage:**

```text
Config::validate → TargetMatcher → proxy selection and PAC generation
```

**Rules:**

- **Compatibility:** matching semantics are part of the configuration contract.
- **Invariants:** no empty labels, IP literals, invalid IDNA, or suffix-only comparisons; exact rule beats parent subdomain rule.
- **Ownership and Lifecycle:** immutable, shared for process lifetime.

**Location:** `src/domain.rs` — proposed item.

**Detailed Shape:** section 7 `src/domain.rs`.

## `ConnectRequest` / `TunnelMode` — Boundary Contract

**Purpose:** Separates validated HTTP CONNECT input from target-policy decisions.

**Includes:**

- `ConnectRequest { host, port }` — normalized hostname and validated allowed port.
- `TunnelMode::Direct` — normal OS IPv4/IPv6 resolver behavior and unchanged relay.
- `TunnelMode::Target(TargetRule)` — DoH and ClientHello transformation.

**Usage:**

```text
parse_connect → ConnectRequest → TargetMatcher → TunnelMode → tunnel orchestration
```

**Rules:**

- **Compatibility:** only HTTP/1.1 CONNECT with hostname authority is accepted in version 1.
- **Invariants:** headers and authority are bounded; target ports are allowed; no proxy authentication or request-body semantics exist.
- **Ownership and Lifecycle:** per-client values dropped when the tunnel ends.

**Location:** `src/proxy.rs` — proposed item.

**Detailed Shape:** section 7 `src/proxy.rs`.

## `TargetResolver` / `SystemResolver` / `DnsCache` — Async Contracts and State

**Purpose:** Supplies upstream socket candidates while keeping selected IPv4 DoH and unselected normal operating-system resolution policies explicit and testable.

**Includes:**

- `TargetResolver::resolve_ipv4` — async method returning non-empty IPv4 candidates from DoH or `ResolveError`.
- `SystemResolver::resolve` — async method returning the operating system's IPv4 and IPv6 socket candidates for unselected hosts.
- `DohResolver` — reqwest client, upstream URL, DNS codec, and cache.
- `DnsCache` — map from `DomainName` to IPv4 addresses and monotonic expiry.

**Usage:**

```text
proxy::connect_upstream → selected: DohResolver
                        → direct: SystemResolver
```

**Rules:**

- **Compatibility:** resolver behavior is internal; configuration fields are stable as defined by `Config`.
- **Invariants:** selected resolution never calls system DNS and returns IPv4 only; direct resolution preserves normal OS IPv4/IPv6 candidates; response ID/question/type/class match; only A records for the queried owner or a bounded validated CNAME chain rooted at it are accepted; cache expiry is bounded by response and configured TTL.
- **Ownership and Lifecycle:** one shared resolver/cache per process; entries live only in memory.

**Location:** `src/dns.rs` — proposed item.

**Detailed Shape:** section 7 `src/dns.rs`.

## `RewriteOutcome` — Transformation Result

**Purpose:** Makes the one-time target-handshake decision explicit before the proxy enters streaming relay.

**Includes:**

- `Rewritten(Vec<u8>)` — complete buffered prefix with ClientHello encoded across valid records.
- `PassThrough(Vec<u8>, reason)` — complete buffered prefix safe to forward unchanged.
- `Reject(TlsError)` — malformed, oversized, timed-out, or policy-mismatched input that must close the tunnel.

**Usage:**

```text
ClientHelloBuffer → rewrite_client_hello → proxy::relay_target → origin write or tunnel close
```

**Rules:**

- **Compatibility:** only TLS handshake records and ClientHello are interpreted; all post-decision bytes are opaque.
- **Invariants:** no output precedes a complete decision; rewritten output preserves the handshake byte sequence exactly; split is inside normalized matching SNI; record lengths fit `u16`; total buffer is bounded.
- **Ownership and Lifecycle:** per-target tunnel and dropped immediately after first-prefix forwarding.

**Location:** `src/tls.rs` — proposed item.

**Detailed Shape:** section 7 `src/tls.rs`.

## `Runtime` — Lifecycle State

**Purpose:** Owns process resources and enforces atomic startup and bounded shutdown.

**Includes:**

- cancellation token;
- top-level proxy/PAC service-task `JoinSet`;
- proxy-owned child `JoinSet` for every spawned tunnel task;
- shutdown deadline and concurrency semaphore.

**Usage:**

```text
main::run → Runtime::start → run_until_signal → shutdown
```

**Rules:**

- **Compatibility:** process lifetime follows CLI invocation; no daemon protocol or persistent state exists.
- **Invariants:** ready is logged only after required binds succeed; Runtime tracks every service task and each service tracks every child it spawns; shutdown drains child tasks before service tasks complete, then aborts and joins work remaining after the deadline.
- **Ownership and Lifecycle:** created once, exclusively owns listeners/tasks, dropped after shutdown.

**Location:** `src/app.rs` — proposed item.

**Detailed Shape:** section 7 `src/app.rs`.

## `Outcome` / `TunnelReport` — Observability Contract

**Purpose:** Provides stable categories without exposing traffic contents.

**Includes:**

- `Outcome` variants for ready, direct relay, target transformed, unsupported request, config, DNS, connect, TLS, timeout, cancelled, and internal errors.
- `TunnelReport` with normalized host, port, target flag, durations, byte counts, and outcome.

**Usage:**

```text
runtime component → TunnelReport → tracing structured event → terminal
```

**Rules:**

- **Compatibility:** outcome names remain stable within the major CLI version.
- **Invariants:** no raw headers, DNS messages, ClientHello bytes, application plaintext, credentials, or cookies are logged.
- **Ownership and Lifecycle:** per operation; emitted then dropped; no log database is owned by the application.

**Location:** `src/diagnostics.rs` — proposed item.

**Detailed Shape:** section 7 `src/diagnostics.rs`.

# 7. Detailed Code Design

## `Cargo.toml` and `Cargo.lock`

- **Change:** Modify
- **Decisions:** `W260914RUZU-DES1`–`W260914RUZU-DES7`
- **Requirements:** all current requirements
- **Responsibilities:** Declare a minimal maintained dependency set and generate its lockfile through Cargo.
- **Affected Callers/Consumers:** all source and test modules.
- **Construction/Wiring:** Cargo feature selection controls TLS backend and async facilities.
- **Dependencies:** `clap` derive; `serde` derive; `toml`; `tokio`; `tokio-util`; `reqwest` with `default-features = false` and rustls/http features; `hickory-proto`; `idna`; `thiserror`; `tracing`; `tracing-subscriber`; dev dependencies `assert_cmd`, `predicates`, `tempfile` when needed.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `[dependencies]` | `Cargo.toml:6` empty | Explicit versioned crates and narrow feature flags; all MIT/Apache-compatible based on crate metadata checked during planning/implementation | Entire crate |
| `Cargo.lock` | Generated lock with only local package | Cargo-generated resolution for approved dependencies | Builds and CI |

**Existing Shape:** `Cargo.toml:1-7`

```toml
[package]
name = "splithello"
version = "0.1.0"
edition = "2024"

[dependencies]
```

The package is one edition-2024 binary with no dependencies.

**Proposed Shape:**

```toml
[dependencies]
clap = { version = "...", features = ["derive"] }
serde = { version = "...", features = ["derive"] }
toml = "..."
tokio = { version = "...", features = ["macros", "rt-multi-thread", "net", "io-util", "signal", "sync", "time"] }
tokio-util = { version = "...", features = ["rt"] }
reqwest = { version = "...", default-features = false, features = ["rustls-tls", "http2"] }
hickory-proto = "..."
idna = "..."
thiserror = "..."
tracing = "..."
tracing-subscriber = { version = "...", features = ["env-filter", "fmt"] }
```

Exact compatible versions are resolved during implementation from current crates.io metadata. Reqwest uses rustls only, `no_proxy()`, no redirects, total/connect/read timeouts, and bounded streamed body reads. Hickory-proto owns DNS wire encoding. No TLS parser crate is added: available crates do not provide the exact bounded reassembly plus rewrite contract, and the required parser surface is deliberately small.

- **Invariants:** default features do not silently add native TLS; lockfile is never hand-edited; every dependency has a documented role and compatible license.
- **Verification Coverage:** `S17`; `V20`, `V21`.

## `src/main.rs`

- **Change:** Modify
- **Decisions:** `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`
- **Requirements:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`
- **Responsibilities:** Initialize tracing, parse CLI, dispatch start, map structured top-level errors to stderr and non-zero exit.
- **Affected Callers/Consumers:** OS process launcher and integration harness.
- **Construction/Wiring:** calls `app::Runtime`.
- **Dependencies:** `cli`, `config`, `app`, `diagnostics`.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `main` | `src/main.rs:1-3` prints placeholder | Tokio entrypoint invokes `run`; reports error and exits 1 | CLI users/tests |
| `run` | None — new symbol | Parse `Cli`; load config; start/run/shutdown runtime | `main` |

**Existing Shape:** `src/main.rs:1-3`

```rust
fn main() {
    println!("Hello, world!");
}
```

The existing function has no inputs, lifecycle, or errors.

**Proposed Shape:**

```rust
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        tracing::error!(outcome = %error.outcome(), error = %error);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError>;
```

`run` is the composition entry: it owns the command dispatch but delegates configuration and network behavior to modules.

- **Invariants:** no listener opens before validated config; top-level failure is emitted once; no panic-based expected error handling.
- **Verification Coverage:** `S1`, `S14`, `S15`; `V1`, `V17`, `V18`.

## `src/cli.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES2`
- **Requirements:** `W260914RUZU-REQ1@R1`
- **Responsibilities:** Stable CLI syntax and generated help.
- **Affected Callers/Consumers:** `main::run`.
- **Construction/Wiring:** parsed at process startup.
- **Dependencies:** clap derive, `PathBuf`.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `Cli`, `Command`, `StartArgs` | None — new symbols | Required `Start(StartArgs)` subcommand; required long `--config: PathBuf` | `main::run`, CLI tests |

**Existing Shape:** None — new file.

```rust
None
```

No CLI contract currently exists.

**Proposed Shape:**

```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Start(StartArgs),
}

#[derive(clap::Args)]
struct StartArgs {
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
}
```

The non-optional `PathBuf` makes `--config` mandatory in clap's generated parser and help.

- **Invariants:** no alternate start syntax or expert network flags in version 1.
- **Verification Coverage:** `S1`; `V1`, `V18`.

## `src/config.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES5`
- **Requirements:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`
- **Responsibilities:** Strict TOML schema, defaults, normalization, and cross-field validation.
- **Affected Callers/Consumers:** `main::run`, `Runtime::start`, matcher/resolver/proxy/PAC constructors.
- **Construction/Wiring:** `Config::load(path)` before any side effect.
- **Dependencies:** serde, toml, reqwest URL type or `url`, domain module, thiserror.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `Config` and nested config types | None — new symbols | `deny_unknown_fields` raw schema converted to validated runtime configuration | all services |
| `Config::load` | None — new symbol | Read, deserialize, apply defaults, validate, return `Config` | `main::run` |
| `ConfigError` | None — new symbol | Path-aware I/O, TOML, domain, URL, listener, duplicate, strategy, and bounds errors | main/tests |

**Existing Shape:** None — new file.

```rust
None
```

No configuration exists.

**Proposed Shape:**

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    proxy: RawProxyConfig,
    dns: RawDnsConfig,
    #[serde(default)]
    limits: RawRuntimeLimits,
    targets: Vec<RawTargetConfig>,
}

struct Config {
    proxy: ProxyConfig,
    dns: DnsConfig,
    limits: RuntimeLimits,
    targets: Vec<TargetRule>,
}

impl Config {
    fn load(path: &Path) -> Result<Self, ConfigError>;
    fn validate(raw: RawConfig) -> Result<Self, ConfigError>;
}
```

Raw Serde types isolate file syntax from validated runtime types. `DnsConfig.ca_certificate` is optional, defaults to system/web PKI trust only, and—when present—loads additional PEM roots for private or deterministic local DoH endpoints without allowing certificate-verification bypass. Hard ceilings remain constants in this module so configuration cannot disable memory/time safety.

- **Invariants:** loopback listeners, HTTPS DoH, allowed ports, unique normalized targets, nonzero bounded limits, known strategy, at least one target.
- **Verification Coverage:** `S1`, `S2`, `S5`, `S11`, `S12`; `V1`, `V2`, `V4`.

## `src/domain.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES2`
- **Requirements:** `W260914RUZU-REQ3@R1`
- **Responsibilities:** Canonical IDNA domain value and label-aware matching.
- **Affected Callers/Consumers:** config, proxy, PAC, DNS, TLS SNI validation.
- **Construction/Wiring:** rules compiled by `Runtime::start`.
- **Dependencies:** idna, thiserror.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `DomainName` | None — new symbol | Validated canonical ASCII hostname with comparison helpers | all hostname consumers |
| `TargetRule` / `TargetMatcher` | None — new symbols | Explicit exact/subdomain policy and deterministic lookup | proxy/PAC |

**Existing Shape:** None — new file.

```rust
None
```

Hostname comparisons are not currently implemented.

**Proposed Shape:**

```rust
#[derive(Clone, Eq, Hash, PartialEq)]
struct DomainName(String);

struct TargetRule {
    host: DomainName,
    include_subdomains: bool,
    strategy: Strategy,
}

struct TargetMatcher {
    rules: Vec<TargetRule>,
}

impl DomainName {
    fn parse(input: &str) -> Result<Self, DomainError>;
    fn is_subdomain_of(&self, parent: &Self) -> bool;
}

impl TargetMatcher {
    fn find(&self, host: &DomainName) -> Option<&TargetRule>;
}
```

`is_subdomain_of` verifies a dot label boundary, and rules are ordered by label count so the most specific match wins.

- **Invariants:** canonical lowercase ASCII, no IP literal/empty label, complete-label comparison, duplicate rules rejected.
- **Verification Coverage:** `S5`; `V3`, `V4`.

## `src/app.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES1`, `W260914RUZU-DES5`
- **Requirements:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`
- **Responsibilities:** Construction, atomic listener binding, shared state, signals, cancellation, task tracking, and bounded shutdown.
- **Affected Callers/Consumers:** `main::run`; proxy and PAC accept loops.
- **Construction/Wiring:** process composition root.
- **Dependencies:** Tokio, tokio-util cancellation, config, matcher, resolver, servers.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `Runtime` | None — new symbol | Owns shared services, listeners/tasks, and cancellation | `main::run` |
| `Runtime::start` | None — new symbol | Construct dependencies and bind all listeners before spawning/reporting ready | main/tests |
| `run_until_signal`, `shutdown` | None — new symbols | Wait for SIGINT/SIGTERM; cancel, drain, abort, and join tasks | main/tests |

**Existing Shape:** None — new file.

```rust
None
```

No lifecycle exists.

**Proposed Shape:**

```rust
struct Runtime {
    cancel: CancellationToken,
    tasks: JoinSet<()>,
    shutdown_timeout: Duration,
}

impl Runtime {
    async fn start(config: Config) -> Result<Self, AppError>;
    async fn run_until_signal(&mut self) -> Result<(), AppError>;
    async fn shutdown(self);
}
```

`Runtime::start` first binds local listener values, then spawns loops after all required binds succeed. Runtime's `JoinSet` tracks service tasks; the proxy service owns a nested `JoinSet` for tunnel tasks and drains or aborts it before returning. Cancellation uses one token shared with services and tunnels.

- **Invariants:** ready after successful binds only; every task tracked; remaining tasks aborted and joined after deadline; no persistent system state.
- **Verification Coverage:** `S3`, `S12`, `S13`, `S14`; `V14`, `V16`, `V17`.

## `src/proxy.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5`
- **Requirements:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`
- **Responsibilities:** CONNECT service, request limits, policy selection, upstream connection, one-time target rewrite, and streaming relay.
- **Affected Callers/Consumers:** app runtime and integration clients.
- **Construction/Wiring:** receives immutable shared matcher, selected/system resolvers, limits, semaphore, cancellation token.
- **Dependencies:** Tokio I/O/network/time/sync, domain, DNS, TLS, diagnostics.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `ProxyServer` | None — new symbol | Loopback listener accept loop and shared dependencies | Runtime |
| `ConnectRequest`, `TunnelMode` | None — new symbols | Validated request and selected/direct decision | handler |
| `parse_connect` | None — new symbol | Bounded HTTP/1.1 CONNECT parser | handler/tests |
| `handle_client` | None — new symbol | One independent tunnel lifecycle | accept loop |
| `connect_socket_candidates` | None — new symbol | Ordered `SocketAddr` attempts within total deadline | direct/selected modes |
| `relay_target`, `relay_direct` | None — new symbols | Transform-once or unchanged relay | handler |

**Existing Shape:** None — new file.

```rust
None
```

No proxy exists.

**Proposed Shape:**

```rust
struct ConnectRequest {
    host: DomainName,
    port: u16,
}

enum TunnelMode<'a> {
    Direct,
    Target(&'a TargetRule),
}

async fn handle_client(
    client: TcpStream,
    state: Arc<ProxyState>,
    cancel: CancellationToken,
) -> Result<TunnelReport, ProxyError>;

async fn connect_socket_candidates(
    addrs: &[SocketAddr],
    deadline: Instant,
) -> Result<TcpStream, ConnectError>;
```

The handler does not acknowledge CONNECT until upstream connection succeeds. It acquires a semaphore permit before processing and keeps it through relay. Direct mode immediately uses `copy_bidirectional`; target mode first runs the bounded rewrite.

- **Invariants:** header and authority limits, loopback client boundary, allowed ports, no target system-DNS fallback, no target bytes forwarded before rewrite decision, per-tunnel errors isolated.
- **Verification Coverage:** `S3`, `S4`, `S6`, `S8`–`S14`; `V5`, `V6`, `V14`–`V17`.

## `src/pac.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES1`, `W260914RUZU-DES2`
- **Requirements:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ8@R1`
- **Responsibilities:** Serve one bounded HTTP GET response containing deterministic PAC JavaScript.
- **Affected Callers/Consumers:** browser PAC loader and Runtime.
- **Construction/Wiring:** optional listener from config.
- **Dependencies:** Tokio, config/domain.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `PacServer` | None — new symbol | loopback accept loop for `/proxy.pac` | Runtime/browser |
| `render_pac` | None — new symbol | exact/subdomain conditions mapping to `PROXY host:port`; otherwise `DIRECT` | PacServer/tests |

**Existing Shape:** None — new file.

```rust
None
```

No setup helper exists.

**Proposed Shape:**

```rust
fn render_pac(proxy_addr: SocketAddr, rules: &[TargetRule]) -> String;

async fn serve_pac(
    listener: TcpListener,
    body: Arc<str>,
    cancel: CancellationToken,
) -> Result<(), PacError>;
```

PAC generation uses escaped canonical ASCII host constants. Each rule tests exact equality first and, when subdomains are enabled, calls `dnsDomainIs(host, ".example.com")` with a leading dot so suffix lookalikes cannot match. The HTTP server accepts only `GET /proxy.pac` and emits fixed headers/content length.

- **Invariants:** no user input is interpolated without canonicalization/escaping; non-targets return `DIRECT`; response is immutable.
- **Verification Coverage:** `S4`, `S5`, `S15`; `V7`, `V18`.

## `src/dns.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES3`, `W260914RUZU-DES5`
- **Requirements:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ7@R1`
- **Responsibilities:** Selected DoH, unselected system resolver adapter, DNS validation, TTL cache, and error categories.
- **Affected Callers/Consumers:** proxy upstream connection.
- **Construction/Wiring:** one shared DoH resolver built by Runtime.
- **Dependencies:** reqwest rustls, hickory-proto, Tokio synchronization/time, domain, thiserror.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `TargetResolver` | None — new symbol | async boxed-future trait with `resolve_ipv4` for selected targets | proxy/tests |
| `DohResolver` | None — new symbol | authenticated RFC 8484 client and cache | Runtime/proxy |
| `SystemResolver` | None — new symbol | `tokio::net::lookup_host`, preserving returned IPv4 and IPv6 socket addresses | direct relay |
| `DnsCache` | None — new symbol | bounded in-memory TTL map | DohResolver |
| `ResolveError` | None — new symbol | timeout, HTTP, size, decode, mismatch, rcode, empty categories | diagnostics |

**Existing Shape:** None — new file.

```rust
None
```

No DNS behavior exists.

**Proposed Shape:**

```rust
trait TargetResolver: Send + Sync {
    fn resolve_ipv4<'a>(
        &'a self,
        host: &'a DomainName,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Ipv4Addr>, ResolveError>> + Send + 'a>>;
}

struct SystemResolver;

impl SystemResolver {
    async fn resolve(host: &DomainName, port: u16)
        -> Result<Vec<SocketAddr>, ResolveError>;
}

struct DohResolver {
    client: reqwest::Client,
    upstream: Url,
    cache: Mutex<DnsCache>,
    limits: DnsLimits,
}

impl DohResolver {
    async fn query_a(&self, host: &DomainName) -> Result<DnsAnswer, ResolveError>;
}
```

A small object-safe target-resolver contract enables deterministic fake DoH resolution without adding async-trait. Direct resolution is a separate concrete adapter because it intentionally returns both address families. Reqwest client construction sets `no_proxy`, no redirects, rustls, explicit timeouts, and any validated additional CA certificate from `DnsConfig`; it never exposes an invalid-certificate acceptance option. Body bytes are accumulated from the response stream with a hard limit before hickory decoding.

- **Invariants:** no system resolver for targets; normal IPv4/IPv6 OS candidates for direct hosts; DNS identity validation; target A-only output; non-empty candidates; bounded cache/body/time; no lock held across network await.
- **Verification Coverage:** `S6`, `S7`, `S13`; `V8`, `V9`, `V10`, `V15`.

## `src/tls.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES4`
- **Requirements:** `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`
- **Responsibilities:** Minimal bounded TLS plaintext record and ClientHello parser plus serializer.
- **Affected Callers/Consumers:** `proxy::relay_target`, unit/integration tests.
- **Construction/Wiring:** called once per selected CONNECT tunnel.
- **Dependencies:** standard library only.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `ClientHelloBuffer` | None — new symbol | consumes records until one complete ClientHello or terminal outcome | target relay |
| `ParsedClientHello` | None — new symbol | exact handshake bytes, input prefix extent, normalized SNI range | serializer/tests |
| `rewrite_client_hello` | None — new symbol | returns rewritten/pass-through/rejected outcome | target relay |
| `TlsError` | None — new symbol | truncation, invalid lengths/types, limit, timeout, missing/mismatch SNI | proxy/diagnostics |

**Existing Shape:** None — new file.

```rust
None
```

No TLS parser exists.

**Proposed Shape:**

```rust
struct ParsedClientHello {
    record_version: [u8; 2],
    handshake: Vec<u8>,
    consumed_input: usize,
    sni: DomainName,
    sni_range: Range<usize>,
}

enum RewriteOutcome {
    Rewritten(Vec<u8>),
    PassThrough { prefix: Vec<u8>, reason: PassReason },
    Reject(TlsError),
}

fn parse_client_hello(input: &[u8], limit: usize)
    -> Result<ParseProgress<ParsedClientHello>, TlsError>;

fn encode_split_records(parsed: &ParsedClientHello) -> Result<Vec<u8>, TlsError>;
```

The parser reads checked big-endian lengths for TLS records, handshake framing, session ID, cipher suites, compression, and extensions. It accepts ClientHello handshake bytes spanning multiple handshake records, rejects interleaved unexpected content before completion, and records the exact SNI byte range plus the framing location of every buffered trailing byte. Serialization chooses the midpoint inside the SNI range and writes two content-type-22 records. If the consumed input record contains bytes after ClientHello, those bytes retain their original order and content type: trailing handshake bytes are re-framed as handshake, while a complete following record is appended with its original header and payload. Unread stream bytes follow unchanged. This is a purpose-built bounded framing transform, not a general TLS stack.

- **Invariants:** checked arithmetic; no panic on arbitrary bytes; exact handshake-byte preservation; valid `u16` record lengths; split strictly within SNI; CONNECT/SNI normalized equality; no TLS decryption or cryptographic state.
- **Verification Coverage:** `S8`, `S9`, `S10`, `S11`; `V11`, `V12`, `V13`, `V15`.

## `src/diagnostics.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES6`
- **Requirements:** `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ8@R1`
- **Responsibilities:** Stable outcome taxonomy, top-level outcome mapping, and tunnel report fields.
- **Affected Callers/Consumers:** main, app, proxy, DNS, TLS, README/tests.
- **Construction/Wiring:** tracing initialized in main; reports returned from handlers.
- **Dependencies:** tracing, standard time/net types.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| `Outcome` | None — new symbol | stable kebab-case display categories | all logs/tests |
| `TunnelReport` | None — new symbol | secret-safe normalized metadata, timing, byte counts, and outcome | proxy logs |

**Existing Shape:** None — new file.

```rust
None
```

No diagnostics contract exists.

**Proposed Shape:**

```rust
enum Outcome {
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

struct TunnelReport {
    host: DomainName,
    port: u16,
    target: bool,
    outcome: Outcome,
    duration: Duration,
    bytes_up: u64,
    bytes_down: u64,
}
```

Display strings are stable and intended for tests and operator recognition. Error chains may be logged at debug level only after ensuring they contain no captured payload.

- **Invariants:** no secrets/payloads; one terminal report per accepted tunnel; startup ready event includes addresses, target count, DoH host, PAC URL, and scope.
- **Verification Coverage:** `S15`, `S16`; `V18`, `V19`.

## `README.md` and `config.example.toml`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES6`
- **Requirements:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ8@R1`
- **Responsibilities:** Usable setup and transparent limitations; valid example configuration.
- **Affected Callers/Consumers:** users and CLI integration test.
- **Construction/Wiring:** README points browser to configured proxy/PAC; example is parsed in tests.
- **Dependencies:** runtime configuration contract.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| README/config example | None — new files | Command, browser proxy/PAC setup, verification, diagnostics, non-goals, Reddit/Medium rules | users/tests |

**Existing Shape:** None — new files.

```text
None
```

No user guidance or configuration exists.

**Proposed Shape:**

```text
1. Build/install
2. Review config.example.toml
3. Run: splithello start --config config.example.toml
4. Configure HTTPS proxy 127.0.0.1:8080 or PAC URL
5. Verify target-transformed outcome
6. Troubleshoot DNS/connect/TLS/unsupported outcomes
7. Stop process and remove proxy setting to roll back
```

The example includes Reddit and Medium with subdomains, loopback listeners, DoH, strategy, and conservative limits. Documentation does not promise access or advise changing global IPv6/QUIC settings.

- **Invariants:** exact command is copy-pasteable; configuration passes parser; limitations include no VPN/anonymity/IP-block/IPv6/QUIC guarantees.
- **Verification Coverage:** `S1`, `S2`, `S15`, `S16`; `V18`, `V19`, `V20`.

## `tests/start_proxy.rs` and `tests/support/mod.rs`

- **Change:** Add
- **Decisions:** `W260914RUZU-DES7`
- **Requirements:** `W260914RUZU-REQ9@R1` and all behavior requirements traced through test cases
- **Responsibilities:** Deterministic collaboration tests using loopback services and the compiled binary.
- **Affected Callers/Consumers:** Cargo test runner.
- **Construction/Wiring:** generates a temporary test CA and localhost certificate, starts an authenticated local HTTPS DoH boundary, writes the CA PEM path to the test configuration's `dns.ca_certificate`, and starts the local origin and SplitHello process.
- **Dependencies:** Tokio test runtime, assert_cmd/tempfile or standard process/temp APIs, hickory-proto test messages, and dev-only `rcgen` plus `tokio-rustls` for the authenticated local HTTPS DoH double.

**Symbol Changes:**

| Symbol | Current Shape or Behavior | Final Shape or Behavior | Callers/Consumers Affected |
|---|---|---|---|
| test harness/types | None — new symbols | local deterministic resolver/origin/process helpers | integration cases |

**Existing Shape:** None — new files.

```rust
None
```

No tests exist.

**Proposed Shape:**

```rust
struct RunningSplitHello { child: Child, proxy_addr: SocketAddr, pac_addr: Option<SocketAddr> }
struct TestDoh { addr: SocketAddr, ca_pem: PathBuf, queries: Arc<...> }
struct TestOrigin { addr: SocketAddr, received_prefix: Arc<...> }

fn build_client_hello(sni: &str, input_record_splits: &[usize]) -> Vec<u8>;
async fn connect_via_proxy(proxy: SocketAddr, authority: &str) -> TcpStream;
```

The harness tests real socket collaboration while substituting only the external DoH and origin boundaries. Its DoH server uses a generated localhost certificate signed by a temporary CA; the child process trusts that CA only through the explicit test configuration, so HTTPS authentication remains exercised instead of disabled. Unit suites remain inline with their modules for parser edge cases.

- **Invariants:** no public Internet dependency; deterministic time/addresses; child cleanup on drop; each integration case proves a collaboration risk not duplicated by unit tests.
- **Verification Coverage:** `S17`; `V14`–`V20`.

## Migration and Delivery

- **Database or Schema Migration:** None — no database or persistent schema.
- **Data Backfill:** None — no persisted data.
- **Mixed-Version Compatibility:** None — one local process and no external service protocol; the config contract is version-1 strict.
- **Feature Flags:** None — one supported product shape; Cargo feature flags are dependency controls, not user-facing rollout flags.
- **Rollout Constraints:** Opt-in local process; user reviews config and manually selects proxy/PAC. The process does not alter machine state.
- **Rollback:** Stop `SplitHello` and remove the browser/system proxy or PAC setting. No application-owned persistent data or firewall rule remains.

# 8. Testing and Verification

## Design Success Criteria

| ID | Observable Success Condition | Traceability |
|---|---|---|
| `S1` | Exact start syntax and valid example reach ready state | W260914RUZU-REQ1@R1, W260914RUZU-DES2 |
| `S2` | Invalid/unsafe config fails before listener side effects | W260914RUZU-REQ1@R1, W260914RUZU-REQ6@R1, W260914RUZU-DES2 |
| `S3` | Loopback CONNECT establishes only after upstream readiness without root/system mutation | W260914RUZU-REQ2@R1, W260914RUZU-DES1 |
| `S4` | PAC routes configured targets to proxy and others direct | W260914RUZU-REQ2@R1, W260914RUZU-DES1 |
| `S5` | Domain matching is canonical, label-aware, explicit, and duplicate-safe | W260914RUZU-REQ3@R1, W260914RUZU-DES2 |
| `S6` | Selected targets resolve only through validated DoH A answers | W260914RUZU-REQ4@R1, W260914RUZU-DES3 |
| `S7` | DNS cache obeys response/config TTL and limits | W260914RUZU-REQ4@R1, W260914RUZU-DES3 |
| `S8` | One bounded complete target ClientHello is recognized across input records | W260914RUZU-REQ5@R1, W260914RUZU-DES4 |
| `S9` | Output has a split inside matching SNI and valid TLS records | W260914RUZU-REQ5@R1, W260914RUZU-DES4 |
| `S10` | Reassembled output handshake bytes equal input exactly | W260914RUZU-REQ5@R1, W260914RUZU-DES4 |
| `S11` | Origin TLS remains end-to-end and post-prefix bytes are opaque | W260914RUZU-REQ6@R1, W260914RUZU-DES1, W260914RUZU-DES4 |
| `S12` | Listeners, inputs, buffers, durations, ports, and concurrency are bounded | W260914RUZU-REQ6@R1, W260914RUZU-DES1, W260914RUZU-DES5 |
| `S13` | One target failure does not break listener or unrelated tunnel | W260914RUZU-REQ7@R1, W260914RUZU-DES5 |
| `S14` | Startup/shutdown own only process sockets/tasks and finish within deadline | W260914RUZU-REQ7@R1, W260914RUZU-DES5 |
| `S15` | Startup and tunnel outcomes are stable and actionable | W260914RUZU-REQ8@R1, W260914RUZU-DES6 |
| `S16` | Documentation accurately states setup, rollback, and unsupported methods | W260914RUZU-REQ8@R1, W260914RUZU-DES6 |
| `S17` | Deterministic tests and Rust quality checks cover every design invariant | W260914RUZU-REQ9@R1, W260914RUZU-DES7 |

## Test Suites and Cases

```text
src/config.rs tests [Unit]
│ Purpose: strict schema and startup-safety invariants
│ SUT: Config::validate
│ Infra: None
│ Doubles: None
│ ├── V1 valid example parses and materializes defaults
│ │   ├── Expect: parsed listeners/DoH/targets equal documented values [S1]
│ │   └── Unique risk: shipped command example is unusable
│ └── V2 invalid fields/listeners/limits/duplicates are rejected
│     ├── Expect: each fixture returns exact ConfigError; no Runtime created [S2,S12]
│     └── Unique risk: unsafe typo or bound reaches side effects
├── src/domain.rs tests [Unit]
│ Purpose: hostname canonicalization and label policy
│ SUT: DomainName, TargetMatcher
│ Infra: None
│ Doubles: None
│ ├── V3 exact/subdomain/adversarial suffix matrix
│ │   ├── Expect: reddit.com/www.reddit.com match; notreddit.com/reddit.com.example do not [S5]
│ │   └── Unique risk: suffix overmatch modifies unrelated traffic
│ └── V4 IDNA/case/trailing-dot/invalid/duplicate matrix
│     ├── Expect: canonical values compare equally; invalid/duplicate rules fail [S5]
│     └── Unique risk: components disagree on hostname identity
├── src/proxy.rs tests [Unit]
│ Purpose: bounded CONNECT contract
│ SUT: parse_connect
│ Infra: None
│ Doubles: None
│ ├── V5 valid hostname CONNECT parses
│ │   ├── Expect: normalized host and port 443 returned [S3]
│ │   └── Unique risk: supported browser request rejected
│ └── V6 method/authority/port/header boundary matrix
│     ├── Expect: exact bounded status/error category; no panic [S3,S12]
│     └── Unique risk: parser ambiguity or memory abuse
├── src/pac.rs tests [Unit]
│ Purpose: deterministic safe PAC selection
│ SUT: render_pac
│ Infra: None
│ Doubles: None
│ └── V7 target and non-target PAC branches
│     ├── Expect: exact hosts/subdomains return PROXY and adversarial suffix returns DIRECT [S4,S5]
│     └── Unique risk: browser bypasses target or proxies unrelated host
├── src/dns.rs tests [Unit]
│ Purpose: DNS validation and cache invariants
│ SUT: response decoder, DnsCache
│ Infra: None
│ Doubles: None
│ ├── V8 valid matching A response
│ │   ├── Expect: exact IPv4 candidates and minimum TTL returned [S6]
│ │   └── Unique risk: correct DoH response is unusable
│ ├── V9 malformed/oversized/mismatched/rcode/CNAME-only response matrix
│ │   ├── Expect: exact ResolveError; no unvalidated address returned [S6]
│ │   └── Unique risk: poisoned or malformed response crosses trust boundary
│ └── V10 cache hit and expiry with controlled clock
│     ├── Expect: hit before bounded expiry; miss at/after expiry; capacity eviction deterministic [S7]
│     └── Unique risk: stale DNS persists beyond contract
├── src/tls.rs tests [Unit]
│ Purpose: parser and byte-preserving transformation invariants
│ SUT: parse_client_hello, encode_split_records
│ Infra: None
│ Doubles: None
│ ├── V11 ClientHello across one/many input records
│ │   ├── Expect: same ParsedClientHello/SNI/consumed extent for every legal split [S8]
│ │   └── Unique risk: legitimate existing fragmentation defeats parser
│ ├── V12 rewrite matching SNI
│ │   ├── Expect: two valid records; boundary strictly inside SNI; reassembled handshake exactly equals input [S9,S10]
│ │   └── Unique risk: transformation corrupts TLS or misses hostname boundary
│ └── V13 malformed/truncated/oversized/missing/mismatched SNI matrix
│     ├── Expect: PassThrough only for defined safe non-TLS/non-ClientHello cases; otherwise exact Reject; no panic [S8,S11,S12]
│     └── Unique risk: arbitrary client bytes cause corruption, leak, or crash
├── tests/start_proxy.rs [Integration]
│ Purpose: real loopback collaboration, lifecycle, and isolation
│ SUT: compiled splithello binary and TCP services
│ Real: CLI, config, runtime, CONNECT proxy, PAC, DNS codec/cache, TLS transform, relay
│ Infra: loopback TCP; temporary filesystem
│ Doubles: local DoH endpoint and local origin replace public external systems
│ ├── V14 unselected CONNECT unchanged relay over IPv4 and IPv6
│ │   ├── Expect: IPv4 and, where loopback IPv6 is available, IPv6 origins receive byte-identical streams and return responses while target path remains idle [S3,S13]
│ │   └── Unique risk: global proxy damages unrelated traffic or narrows normal address-family support
│ ├── V15 selected CONNECT uses DoH and transformed prefix
│ │   ├── Expect: DoH sees target query; OS resolver fake is unused; origin parses two records whose handshake equals sent input; relay response succeeds [S6,S8,S9,S10,S11]
│ │   └── Unique risk: modules pass unit tests but selected end-to-end wiring is wrong
│ ├── V16 concurrent target failure and direct success
│ │   ├── Expect: malformed target tunnel closes with TLS outcome while direct tunnel completes; concurrency limit never exceeded [S12,S13]
│ │   └── Unique risk: per-tunnel failure terminates listener/shared state
│ ├── V17 signal shutdown with active tunnel
│ │   ├── Expect: process exits within configured deadline, listeners refuse new connections, child cleanup finds no persistent state [S14]
│ │   └── Unique risk: tracked tasks hang or process leaks resources
│ ├── V18 exact example command and readiness/PAC
│ │   ├── Expect: binary starts with temp-adapted config.example.toml; stdout contains ready fields and PAC serves expected body [S1,S4,S15]
│ │   └── Unique risk: independently correct modules do not yield usable operator flow
│ └── V19 secret-safe stable failure outcome
│     ├── Expect: induced DoH failure emits dns-error and hostname but not request bytes/test secret [S15,S16]
│     └── Unique risk: diagnostics leak contents or use unstable categories
└── Non-test verification
    ├── V20 Documentation/config consistency check
    │   ├── Expect: README exact command exists; config.example.toml passes parser; all stated exclusions present [S1,S2,S16,S17]
    │   └── Unique risk: user-facing contract drifts from executable behavior
    └── V21 Rust quality/build checks
        ├── Expect: cargo fmt --check; cargo clippy --all-targets --all-features -- -D warnings; cargo build; cargo test --workspace all pass [S17]
        └── Unique risk: compile, lint, formatting, or suite regression
        Migration: None — no database/schema/persistent state
        Configuration: V1, V2, V18, V20
        Observability: V18, V19
```

## Coverage Balance

- Parsing and pure invariants stay in unit tests.
- Only real cross-module socket behavior, process lifecycle, and failure isolation appear in integration tests.
- Public Reddit/Medium checks are optional manual evidence, never suite gates.
- No test duplicates every parser edge case through the process boundary.

## Test Quality Rules

- Unit tests assert public module outcomes and byte/state invariants, not private call order.
- Integration tests use real loopback TCP and real application modules; only public external services are replaced.
- Every asynchronous test has a deadline and deterministic cleanup.
- No test requires root, firewall changes, public DNS, or Internet access.

# 9. Finalized Design Decisions

## W260914RUZU-DES1: Explicit loopback CONNECT proxy

**Question:** What system boundary provides selected hostnames and stream control with the least privilege and maintenance cost?

**Decision:** Use an unprivileged loopback HTTP/1.1 CONNECT proxy with an optional loopback PAC endpoint; do not implement transparent interception in version 1.

**Requirements Addressed:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`

**Research Support:** None.

**Rationale:** CONNECT exposes the hostname before local DNS, preserves end-to-end TLS, delegates TCP reliability to the OS, and eliminates raw sockets, firewall ownership, and root privileges.

**Code Impact:**

- `src/app.rs` — `Runtime` — construct and own listeners.
- `src/proxy.rs` — `ProxyServer`, `handle_client` — implement CONNECT tunnels.
- `src/pac.rs` — `PacServer`, `render_pac` — optional target routing helper.

**Detailed Design:** section 7 entries for `src/app.rs`, `src/proxy.rs`, and `src/pac.rs`.

**Tradeoffs:**

- Requires one-time client proxy/PAC configuration and does not automatically cover applications that ignore it.

**Success Criteria:** `S3`, `S4`, `S11`, `S12`

**Verification Impact:**

- `V5`/`V6` assert CONNECT contract; `V7` PAC routing; `V14` unchanged direct relay; `V18` usable startup.

## W260914RUZU-DES2: Strict typed configuration and domain policy

**Question:** How should configuration remain simple, safe, and auditable?

**Decision:** Use clap derive for the exact command and strict Serde/TOML raw types converted once into validated runtime types, including canonical label-aware targets and hard safety ceilings.

**Requirements Addressed:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`

**Research Support:** Current clap documentation confirms derive-based required subcommands and `PathBuf` arguments.

**Rationale:** One typed startup boundary prevents silent typos and gives every runtime module canonical values instead of repeated validation.

**Code Impact:**

- `src/cli.rs` — CLI contracts.
- `src/config.rs` — schema and validation.
- `src/domain.rs` — hostname identity and matching.
- `config.example.toml` — executable example.

**Detailed Design:** section 7 entries for these files.

**Tradeoffs:**

- Strict unknown-field rejection makes configuration evolution deliberate rather than permissive.

**Success Criteria:** `S1`, `S2`, `S5`

**Verification Impact:**

- `V1`–`V4` and `V20` assert command/config/matching behavior.

## W260914RUZU-DES3: Target-only DNS-over-HTTPS resolver

**Question:** How should selected targets bypass poisoned local DNS without changing system DNS?

**Decision:** Resolve selected domains through one configured RFC 8484 HTTPS upstream using reqwest with rustls and hickory-proto DNS messages; use the OS resolver only for unselected direct tunnels.

**Requirements Addressed:** `W260914RUZU-REQ4@R1`

**Research Support:** Reqwest exposes rustls-only client configuration, `no_proxy`, no-redirect policy, and total/connect/read timeouts; hickory-proto provides licensed DNS message serialization/parsing.

**Rationale:** Target-only DoH isolates the requirement and avoids a local DNS server, privileged port 53, global resolver mutation, and bespoke DNS wire code.

**Code Impact:**

- `src/dns.rs` — resolver implementations and cache.
- `src/config.rs` — DoH URL, optional additional CA certificate, and limits.

**Detailed Design:** section 7 `src/dns.rs` and `src/config.rs`.

**Tradeoffs:**

- Depends on configured DoH reachability and supports A/IPv4 only in version 1.

**Success Criteria:** `S6`, `S7`

**Verification Impact:**

- `V8`–`V10` validate DNS/cache; `V15` proves selected end-to-end routing.

## W260914RUZU-DES4: Minimal TLS record rewriter

**Question:** How should selected SNI inspection be mitigated without raw packets or TLS termination?

**Decision:** Implement a small safe Rust parser for bounded plaintext TLS handshake records and ClientHello structure, then serialize the unchanged handshake across two valid records split inside the verified SNI hostname.

**Requirements Addressed:** `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`

**Research Support:** RFC contracts govern the design. Current rustls does not expose a stable transparent mutable-record parser, and tls-parser does not own reassembly or serialization, so either dependency would add coupling without removing the core logic.

**Rationale:** The parser surface is finite and testable; record framing can change without changing TLS handshake bytes or cryptographic state.

**Code Impact:**

- `src/tls.rs` — parser, transformation, and errors.
- `src/proxy.rs` — one-time selected-prefix orchestration.

**Detailed Design:** section 7 `src/tls.rs` and `src/proxy.rs`.

**Tradeoffs:**

- Some networks/origins may reject unusual framing; this single strategy is intentionally not a universal bypass engine.

**Success Criteria:** `S8`–`S11`

**Verification Impact:**

- `V11`–`V13` assert parser/transformation invariants; `V15` proves real stream collaboration.

## W260914RUZU-DES5: Isolated fail-open runtime

**Question:** How should concurrency, failure, and shutdown remain safe and maintainable?

**Decision:** Use Tokio tasks tracked by `JoinSet`, one `CancellationToken`, bounded semaphore/buffers/timeouts, independent tunnel results, direct unchanged relay for unselected hosts, and deadline-based drain then abort/join.

**Requirements Addressed:** `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`

**Research Support:** Current Tokio APIs provide listener acceptance, stream copying, tracked task joining, and explicit abort/drain behavior.

**Rationale:** One runtime owner and per-tunnel isolation prevent shared failure while avoiding bespoke thread/task management.

**Code Impact:**

- `src/app.rs` — lifecycle and task ownership.
- `src/proxy.rs` — independent bounded handlers.
- `src/dns.rs` — cache synchronization without locks across await.

**Detailed Design:** section 7 entries for these files.

**Tradeoffs:**

- A selected tunnel cannot be made truly direct after CONNECT if its mitigation fails; fail-open means isolation and safe unchanged paths where the decision can be made before transformed output.

**Success Criteria:** `S12`–`S14`

**Verification Impact:**

- `V14` asserts normal unselected IPv4/IPv6 relay; `V15`–`V17` assert target behavior, failure isolation, bounds, and shutdown.

## W260914RUZU-DES6: Stable diagnostics and documentation

**Question:** How should users distinguish successful startup, successful target transformation, and unsupported blocks?

**Decision:** Define stable outcome categories and secret-safe tunnel reports, emit an explicit ready event with setup/scope, and provide README setup, rollback, troubleshooting, and non-goals.

**Requirements Addressed:** `W260914RUZU-REQ8@R1`

**Research Support:** None.

**Rationale:** The ISP mechanism is unmeasured; honest observability prevents startup from being misreported as circumvention success.

**Code Impact:**

- `src/diagnostics.rs` — outcome/report contracts.
- `src/app.rs`, `src/proxy.rs`, `src/dns.rs`, `src/tls.rs` — categorized events.
- `README.md` — user workflow and limits.

**Detailed Design:** section 7 diagnostics and documentation entries.

**Tradeoffs:**

- Diagnostics can identify the local stage that failed but cannot prove the ISP's internal filtering policy.

**Success Criteria:** `S15`, `S16`

**Verification Impact:**

- `V18` asserts readiness/setup output; `V19` asserts stable secret-safe failure output; `V20` checks documentation.

## W260914RUZU-DES7: Deterministic verification architecture

**Question:** What is the smallest sufficient evidence for a usable version 1?

**Decision:** Keep parser/config/matching/cache proofs as unit tests, add one loopback process/socket integration suite for collaboration and lifecycle, and require standard Rust quality checks; public live tests remain optional.

**Requirements Addressed:** `W260914RUZU-REQ9@R1`

**Research Support:** None.

**Rationale:** Local deterministic evidence is reproducible and distinguishes implementation correctness from ISP-specific effectiveness.

**Code Impact:**

- module-local tests — pure contracts.
- `tests/start_proxy.rs`, `tests/support/mod.rs` — cross-module behavior.
- Cargo/README — standard checks and optional live procedure.

**Detailed Design:** section 7 test entries and section 8 verification tree.

**Tradeoffs:**

- Automated tests cannot prove access through every real ISP; live evidence is supplementary.

**Success Criteria:** `S17`

**Verification Impact:**

- `V1`–`V21` collectively cover every requirement, decision, and invariant without public network dependence.
