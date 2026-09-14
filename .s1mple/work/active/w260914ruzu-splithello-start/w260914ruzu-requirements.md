# W260914RUZU: splithello-start — Requirements

**Document Status:** Ready for System Design

**Context Source:** `w260914ruzu-discuss-context.md`

**Research Source:** None

**Unresolved Items:** None

**Ready for System Design Checklist:**

- [x] Every active topic is covered.
- [x] Every current requirement has a stable, unique ID and exact current revision.
- [x] Every current requirement includes its question, answer, rationale, acceptance criteria, and dependencies.
- [x] Scope, constraints, assumptions, risks, and compatibility boundaries are explicit.
- [x] For roadmap-linked work, the accepted parent baseline has been checked, assigned scope is covered by current requirements, and gates due before requirements approval have evidence; unresolved parent impacts prevent readiness. Not applicable — standalone work.
- [x] Revised and replaced requirements preserve append-only lineage and complete prior revisions. None exist.
- [x] When research exists, every RF-to-evaluated-revision relationship has one reconciliation outcome; a finding that evaluated no requirement has one `None` relationship row. Not applicable — formal research was skipped.
- [x] Deferred decisions are outside the current-requirements ledger.
- [x] `Unresolved Items` is `None`.

# Table of Contents

1. [Problem and Desired Outcome](#1-problem-and-desired-outcome)
2. [Current and Expected State](#2-current-and-expected-state)
3. [Scope](#3-scope)
4. [Standards and Constraints](#4-standards-and-constraints)
5. [Current Requirements](#5-current-requirements)
   - [W260914RUZU-REQ1@R1: Stable start command and strict configuration](#w260914ruzu-req1r1-stable-start-command-and-strict-configuration)
   - [W260914RUZU-REQ2@R1: Local explicit proxy boundary](#w260914ruzu-req2r1-local-explicit-proxy-boundary)
   - [W260914RUZU-REQ3@R1: Exact configured-domain policy](#w260914ruzu-req3r1-exact-configured-domain-policy)
   - [W260914RUZU-REQ4@R1: Encrypted target-domain resolution](#w260914ruzu-req4r1-encrypted-target-domain-resolution)
   - [W260914RUZU-REQ5@R1: TLS ClientHello record fragmentation](#w260914ruzu-req5r1-tls-clienthello-record-fragmentation)
   - [W260914RUZU-REQ6@R1: End-to-end TLS and local security](#w260914ruzu-req6r1-end-to-end-tls-and-local-security)
   - [W260914RUZU-REQ7@R1: Fail-open runtime and owned cleanup](#w260914ruzu-req7r1-fail-open-runtime-and-owned-cleanup)
   - [W260914RUZU-REQ8@R1: Actionable diagnostics and honest scope](#w260914ruzu-req8r1-actionable-diagnostics-and-honest-scope)
   - [W260914RUZU-REQ9@R1: Focused verification](#w260914ruzu-req9r1-focused-verification)
6. [Research Impact Review](#6-research-impact-review)
7. [Historical Requirement Revisions](#7-historical-requirement-revisions)

# 1. Problem and Desired Outcome

## Problem Statement

The user's ISP blocks browser access to `reddit.com` and `medium.com`, but the exact mechanism is not yet measured. Version 1 needs a small, maintainable local tool that covers two common hostname-level mechanisms—poisoned DNS answers and IPv4 TCP TLS-SNI inspection—without requiring transparent packet interception, TLS certificate installation, or a broad collection of fragile packet strategies.

## Desired Outcome

After one-time browser or operating-system proxy configuration, running `splithello start --config config.example.toml` starts a loopback-only HTTPS CONNECT proxy. Requests for configured Reddit and Medium hostnames use encrypted DNS resolution and a standards-valid fragmented TLS ClientHello; unrelated destinations pass through without TLS modification. The tool reports whether a failure is configuration, DNS, proxy setup, unsupported transport, upstream connectivity, or a likely unsupported blocking method.

# 2. Current and Expected State

## Current State

```text
`SplitHello`
    → print "Hello, world!"
    → exit

Browser → ISP DNS / direct TLS connection → blocked Reddit or Medium
```

The repository contains only a placeholder Rust binary and no network, configuration, proxy, or diagnostic behavior.

## Expected State

```text
Browser configured with local proxy or generated PAC route
    → CONNECT reddit.com:443 / medium.com:443
    → `SplitHello` validates configured target policy
    → resolve target through configured DoH upstream
    → open IPv4 TCP connection to resolved address
    → preserve browser TLS end-to-end
    → fragment the selected target's ClientHello across valid TLS records
    → relay the remaining byte streams unchanged

Unselected destination → local proxy → normal resolution → unchanged relay
```

The proxy design solves DNS and TLS-handshake compatibility in user space. It avoids root-only raw sockets and firewall ownership, while preserving browser-to-origin TLS authentication and encryption.

# 3. Scope

## In Scope

- Linux Rust CLI with the exact command shape `splithello start --config <path>`.
- Repository-root `config.example.toml` containing explicit policies for `reddit.com` and `medium.com`.
- Loopback-only HTTP CONNECT proxy suitable for browser HTTPS proxy or PAC configuration.
- Target-domain encrypted DNS resolution through a configured DNS-over-HTTPS upstream.
- IPv4 TCP connections and TLS ClientHello fragmentation at valid TLS-record boundaries for configured targets.
- Unmodified relay for unselected destinations after the CONNECT tunnel is established.
- End-to-end browser-to-origin TLS without local certificate generation or TLS termination.
- Strict configuration validation, bounded protocol parsing, graceful cancellation, safe resource cleanup, and structured secret-safe diagnostics.
- Unit and local integration tests; optional ignored live-network tests must never be required for the default suite.

## Out of Scope

- Transparent firewall, NFQUEUE, raw-socket, eBPF, TUN/TAP, or kernel packet interception in version 1.
- Automatic browser or desktop proxy-setting changes; setup is documented and a PAC response may be served locally.
- VPN behavior, anonymity, client-IP hiding, traffic-shape concealment, or protection from traffic analysis.
- Destination-IP blocking, complete network shutdown, captive portals, authenticated enterprise controls, or compromised endpoints.
- IPv6 upstream connections, QUIC/HTTP/3, UDP/443 manipulation, plain-HTTP filtering, mobile-app compatibility guarantees, or universal censorship circumvention.
- Fake packets, low-TTL decoys, TCP sequence manipulation, arbitrary packet recipes, or automatic adversarial strategy discovery.
- Copying code, structure, or documentation from the unlicensed `tuananh/dpi-bypass` repository.
- Deferred: transparent system-wide operation. Reconsider only if explicit-proxy deployment cannot satisfy representative browser access and a later requirement accepts root privileges and greater platform complexity.

## Roadmap Assignment

None — standalone work.

## Affected Interfaces and Actors

- `splithello` CLI — gains the stable `start --config <path>` interface.
- `config.example.toml` — becomes the documented configuration contract and initial Reddit/Medium policy.
- Browser or desktop proxy configuration — sends selected HTTPS CONNECT requests to the loopback listener; it remains an external one-time setup boundary.
- DNS-over-HTTPS upstream — resolves configured targets independently of the ISP resolver.
- Reddit and Medium HTTPS origins — receive normal end-to-end TLS handshakes whose ClientHello handshake bytes are unchanged but span multiple valid TLS records.

# 4. Standards and Constraints

## Binding Standards

- `.s1mple/S1MPLE.md:15` — choose the simplest robust design; version 1 therefore uses an explicit user-space proxy rather than privileged transparent interception.
- `.s1mple/S1MPLE.md:17` — deliver one complete end-to-end increment rather than speculative multi-platform or multi-strategy groundwork.
- `.s1mple/docs/languages/rust.md:65` — dependencies must be maintained, licensed, explicit, and complexity-reducing.
- `.s1mple/docs/languages/rust.md:88` — recoverable startup, configuration, DNS, handshake, and relay failures use structured `Result` errors.
- `.s1mple/docs/languages/rust.md:93` — TOML, CONNECT requests, hostnames, DNS responses, and TLS records are untrusted boundary input and require checked parsing.
- `.s1mple/docs/languages/rust.md:100` — version 1 introduces no `unsafe`; safe user-space networking is sufficient.
- RFC 9110 — HTTP CONNECT establishes the local proxy tunnel without terminating origin TLS.
- RFC 8484 — DNS-over-HTTPS carries DNS messages over HTTPS.
- RFC 6066 — SNI appears in the TLS ClientHello and is used only for validation and precise split selection.
- RFC 8446 — TLS handshake messages may span records; fragmentation must preserve the exact handshake bytes and valid record framing.
- RFC 9293 — TCP is an ordered byte stream; relay logic must not depend on application writes matching TCP segment boundaries.
- RFC 9505 — diagnostics and documentation must not claim that one hostname-level technique handles every censorship method.

## Constraints

- The stable invocation is `splithello start --config config.example.toml`.
- Version 1 is a single local process and binds only loopback addresses by default.
- Version 1 uses an explicit proxy boundary and must not require root or network-administration capabilities.
- Only IPv4 TCP upstream connections are supported for the target mitigation path.
- Configuration must be fully validated before opening listeners or upstream connections.
- The implementation must remain clean-room and original.
- The default test suite must be deterministic and must not depend on Reddit, Medium, the user's ISP, or a public DoH service.

## Accepted Assumptions

- The user can perform one-time browser or desktop HTTPS proxy/PAC configuration.
- Browser HTTPS proxy requests use hostname-form CONNECT targets, such as `CONNECT reddit.com:443`.
- Representative browser access to configured Reddit and Medium web hostnames is sufficient; every embedded third-party asset and native application is not guaranteed.
- A censor that blocks destination IP addresses or all ECH/fragmented TLS traffic is outside version 1's capability.
- TLS-record fragmentation is attempted only for configured targets because indiscriminate rewriting increases compatibility risk.
- Conservative defaults are selected without interactive discussion, as explicitly requested by the user.

## Dependencies

- A maintained async runtime and HTTP primitives — support concurrent CONNECT tunnels, cancellation, timeouts, and bounded I/O without hand-written event loops.
- A maintained TOML/Serde stack — strict typed configuration with unknown-field rejection.
- A maintained HTTP client with rustls — authenticated DoH without native TLS or custom cryptography.
- No packet-injection, firewall, raw-socket, TLS-termination, or certificate-authority dependency is required.

## Risks and Compatibility

- **Proxy setup** — if the browser is not configured to use the listener, traffic bypasses the tool; startup output and documentation must make this observable.
- **TLS compatibility** — some origins or intermediary services may reject unusual record boundaries; the selected tunnel falls back to unchanged relay on pre-forward parsing inability, but cannot undo bytes after a modified handshake has been sent.
- **HTTP/3 preference** — proxy configuration normally routes HTTPS through CONNECT/TCP, but clients that bypass the proxy with QUIC remain unsupported and must be diagnosed honestly.
- **DNS trust and availability** — DoH avoids the local resolver for selected targets but depends on the configured upstream and its reachable bootstrap endpoint.
- **Open-proxy exposure** — non-loopback listeners are rejected by default and require no version-1 override.
- **Memory exhaustion** — CONNECT headers, ClientHello buffering, DNS responses, and pending tunnel counts must have explicit limits.

# 5. Current Requirements

## W260914RUZU-REQ1@R1: Stable start command and strict configuration

**Requirement ID:** `W260914RUZU-REQ1`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** How must users start and configure the program?

**Answer:** The program must start with `splithello start --config <path>`, require a readable TOML file, reject unknown or invalid fields, validate the complete configuration before binding listeners, and ship `config.example.toml` with working Reddit and Medium examples.

**Rationale:** One explicit command and one typed configuration contract keep operation understandable and prevent misspellings or partial startup from silently changing behavior.

```toml
[proxy]
listen = "127.0.0.1:8080"
pac_listen = "127.0.0.1:8081"

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
```

**Acceptance Criteria:**

- [ ] The exact invocation loads a valid example and reaches a ready state with the configured loopback addresses.
- [ ] Missing `--config`, unreadable files, malformed TOML, unknown fields, duplicate targets, invalid domains, non-loopback listeners, unsupported strategies, or unsafe limits return a non-zero exit before listeners are opened.
- [ ] `--help` documents the command without exposing expert packet-tuning options.

**Depends On:** None

## W260914RUZU-REQ2@R1: Local explicit proxy boundary

**Requirement ID:** `W260914RUZU-REQ2`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** How should browser traffic enter the tool while keeping the system simple and unprivileged?

**Answer:** Version 1 must expose a loopback-only HTTP/1.1 CONNECT proxy and an optional loopback PAC response. It must not install firewall rules, intercept traffic transparently, require root, or alter browser/system settings automatically.

**Rationale:** An explicit proxy gives the program a hostname before DNS and TLS processing, handles retransmissions through the operating system's TCP stack, and sharply reduces platform-specific cleanup and privilege risk.

```text
Browser CONNECT hostname:443 → loopback proxy → origin TCP connection
```

**Acceptance Criteria:**

- [ ] A valid hostname-form `CONNECT <host>:443` receives `200 Connection Established` only after the upstream connection is ready.
- [ ] Unsupported methods, malformed authorities, IP-form targets selected by hostname policy, oversized headers, and disallowed ports receive a bounded HTTP error and do not panic.
- [ ] The optional PAC response selects the local proxy only for configured targets and returns `DIRECT` for other hosts.
- [ ] The process runs as an unprivileged user and binds no non-loopback address.

**Depends On:** `W260914RUZU-REQ1@R1`

## W260914RUZU-REQ3@R1: Exact configured-domain policy

**Requirement ID:** `W260914RUZU-REQ3`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** Which destinations receive DNS and TLS compatibility behavior?

**Answer:** Only normalized, label-aware matches for configured targets receive mitigation. Each target explicitly controls subdomain inclusion; suffix tricks such as `notreddit.com` must never match `reddit.com`.

**Rationale:** Precise selection minimizes compatibility impact and makes the configuration auditable.

**Acceptance Criteria:**

- [ ] Matching is case-insensitive, removes one trailing dot, converts accepted internationalized names to their canonical ASCII representation, and compares complete DNS labels.
- [ ] `reddit.com` matches itself; with `include_subdomains = true`, it matches `www.reddit.com` but not `notreddit.com` or `reddit.com.example`.
- [ ] Duplicate normalized targets and invalid names are rejected during configuration validation.
- [ ] Unselected hosts use unchanged relay and no target-specific DoH or TLS rewrite.

**Depends On:** `W260914RUZU-REQ1@R1`

## W260914RUZU-REQ4@R1: Encrypted target-domain resolution

**Requirement ID:** `W260914RUZU-REQ4`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** How should the proxy avoid poisoned local DNS for selected domains?

**Answer:** Selected CONNECT hostnames must be resolved through a configured RFC 8484 DNS-over-HTTPS upstream using authenticated HTTPS, bounded response sizes, IPv4 A records, explicit timeouts, and a small in-memory TTL cache. Unselected hosts use the operating-system resolver.

**Rationale:** The proxy already receives the hostname, so target-only encrypted resolution avoids system DNS mutation and leaves unrelated lookups unchanged.

**Acceptance Criteria:**

- [ ] Selected targets never use the operating-system resolver before their DoH result is obtained.
- [ ] Only valid A records from successful, matching DNS responses enter the candidate set; malformed, mismatched, oversized, empty, or non-success responses produce an actionable target-tunnel error.
- [ ] Cache entries never outlive the smallest accepted DNS TTL or the configured maximum and expire deterministically.
- [ ] The configured DoH endpoint is HTTPS and has a documented bootstrap rule that does not depend on resolving selected blocked domains.

**Depends On:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`

## W260914RUZU-REQ5@R1: TLS ClientHello record fragmentation

**Requirement ID:** `W260914RUZU-REQ5`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** How should selected target TLS handshakes be transformed without raw packet manipulation or TLS termination?

**Answer:** After CONNECT succeeds, the proxy must buffer one bounded initial TLS ClientHello, verify that its normalized SNI matches the selected CONNECT host, preserve every handshake byte exactly, and emit the handshake across multiple syntactically valid TLS records with a split inside the SNI hostname. It then relays both directions as unchanged byte streams.

**Rationale:** TLS-record fragmentation is implementable at the stream boundary, remains standards-valid, delegates TCP retransmission to the kernel, and is materially simpler than raw-packet desynchronization. Locating the split inside SNI avoids arbitrary fixed chunk sizes.

```text
Original: TLS record [ClientHello ... SNI=reddit.com ...]
Output:   TLS record [ClientHello prefix ... redd]
          TLS record [it.com ... remaining ClientHello bytes]
```

**Acceptance Criteria:**

- [ ] The reassembled TLS handshake bytes after output-record parsing are byte-for-byte equal to the input ClientHello handshake bytes.
- [ ] The chosen boundary falls strictly inside the configured SNI hostname and each emitted record length is valid.
- [ ] Fragmented input records and ClientHello messages spanning records are reassembled within configured byte/time limits before validation and transformation.
- [ ] If the first tunneled bytes are not a complete valid matching ClientHello within bounds, no transformed bytes are sent; the proxy either relays the buffered bytes unchanged when safe or closes only that tunnel with a precise error.
- [ ] After the initial decision, all subsequent bytes are relayed unchanged; the proxy never decrypts TLS application data.

**Depends On:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`

## W260914RUZU-REQ6@R1: End-to-end TLS and local security

**Requirement ID:** `W260914RUZU-REQ6`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** What security boundary must the proxy preserve?

**Answer:** The proxy must remain a byte-stream intermediary, never terminate origin TLS, generate or install certificates, capture application plaintext, log full DNS payloads or ClientHello bytes, or listen outside loopback. CONNECT is limited to configured safe ports, with port 443 as the version-1 default.

**Rationale:** Preserving browser-to-origin certificate verification and limiting exposure prevents a compatibility tool from becoming a local interception or open-proxy service.

**Acceptance Criteria:**

- [ ] Browser certificate validation remains directly against the origin; no local CA or server certificate exists.
- [ ] Logs contain normalized host, outcome category, durations, and bounded error context but no application data, credentials, cookies, complete DNS messages, or raw handshakes.
- [ ] Configuration rejects non-loopback listeners and disallowed CONNECT ports.
- [ ] Per-tunnel buffer sizes, header sizes, DNS sizes, idle timeouts, and concurrent connection counts are bounded.

**Depends On:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`

## W260914RUZU-REQ7@R1: Fail-open runtime and owned cleanup

**Requirement ID:** `W260914RUZU-REQ7`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** How must failures affect selected and unrelated traffic?

**Answer:** Unselected destinations must be relayed unchanged. A target-specific DNS or TLS transformation failure must affect only that CONNECT tunnel, never the listener or unrelated tunnels. Cancellation must stop accepting work, drain or cancel bounded active work, and close only process-owned sockets; no system network state is modified.

**Rationale:** Explicit proxying cannot guarantee direct access when the proxy itself is selected, but it can provide strong isolation: one malformed or blocked target must not break unrelated browser traffic or leave persistent machine changes.

**Acceptance Criteria:**

- [ ] An unselected tunnel remains operational while another target tunnel encounters malformed TLS, DoH timeout, or upstream refusal.
- [ ] Panic-free per-connection failures are logged and converted into bounded tunnel closure or HTTP errors.
- [ ] SIGINT/SIGTERM stops listeners and tasks within a configured shutdown deadline without persistent network-state cleanup commands.
- [ ] Startup is atomic from the user's perspective: if any required listener cannot bind, all listeners opened by this process are closed and startup returns non-zero.

**Depends On:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`

## W260914RUZU-REQ8@R1: Actionable diagnostics and honest scope

**Requirement ID:** `W260914RUZU-REQ8`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** What must users be able to understand from startup and runtime output?

**Answer:** Startup must report validated listeners, configured targets, DoH endpoint host, supported transport, PAC URL when enabled, and concise browser setup guidance. Runtime results must use stable categories for direct relay, target DoH resolution, transformed handshake, configuration failure, unsupported request, timeout, and upstream failure. Documentation must state the explicit non-goals.

**Rationale:** The user's ISP mechanism is unknown, so operational clarity is part of correctness; successful startup must not be confused with successful circumvention.

**Acceptance Criteria:**

- [ ] A ready log identifies the proxy address, PAC URL if configured, target count, and `IPv4 TCP only` limitation.
- [ ] Each target tunnel records whether DoH resolution and TLS fragmentation were attempted and whether the tunnel reached relay state.
- [ ] Errors distinguish proxy-not-used symptoms, DoH failure, no A records, malformed/mismatched ClientHello, IPv6/QUIC non-coverage, TCP refusal/timeout, and possible unsupported IP-level blocking.
- [ ] README guidance states that the tool is not a VPN, does not hide the client IP, and cannot guarantee Reddit or Medium access against unsupported blocking methods.

**Depends On:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ7@R1`

## W260914RUZU-REQ9@R1: Focused verification

**Requirement ID:** `W260914RUZU-REQ9`

**Revision:** `R1`

**Previous Revision:** None — initial revision

**Replaces:** None

**Research Relationship:** None — not research-informed

**Revision Summary:** None — initial revision

**Question:** What evidence is required before version 1 is considered usable?

**Answer:** Verification must cover strict configuration, label-aware domain matching, DoH validation and caching, fragmented ClientHello reassembly and byte preservation, CONNECT parsing, target and non-target relay, failure isolation, PAC generation, graceful shutdown, and the exact example command using deterministic local test servers. Standard Rust format, lint, build, and test checks must pass.

**Rationale:** Local deterministic tests prove the tool's contracts without depending on public services or a particular ISP. A manual live check may supplement but cannot replace them.

**Acceptance Criteria:**

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo build`, and `cargo test --workspace` pass.
- [ ] A local integration test starts the CLI with a temporary valid TOML file, sends target and non-target CONNECT traffic through local fake DNS/TLS endpoints, and asserts the expected transformed or unchanged bytes.
- [ ] Unit tests cover every parser boundary and invariant, including truncation, oversized values, mismatched lengths, malicious domain suffixes, and byte-for-byte handshake preservation.
- [ ] Public-network checks are optional, ignored by default, and reported separately from deterministic evidence.

**Depends On:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`

# 6. Research Impact Review

None — formal s1mple research was skipped. The preceding user-requested repository analysis and protocol references were already captured as discussion context; no new external uncertainty was needed to decide the conservative explicit-proxy scope. System design must verify concrete crate choices against current maintenance and licensing information before implementation.

# 7. Historical Requirement Revisions

None — no requirement has been revised or replaced.
