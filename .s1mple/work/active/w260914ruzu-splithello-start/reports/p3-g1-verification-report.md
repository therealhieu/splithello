# P3-G1 Verification Report

## Scope

- **Group:** `P3-G1 — connect-and-relay-engine`
- **Tasks:** `P3-G1-T1`, `P3-G1-T2`
- **Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-verification-report.md`
- **Implementation Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-implementation-report.md`

## Status

`Failed`

## Assigned Traceability

- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S6`, `S8`, `S9`, `S10`, `S11`, `S12`, `S13`, `S15`, `S17`
- **Verification Cases:** `V5`, `V6`, `V14`, `V15`, `V16`, `V19`

## Specification Verification

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Planned outcome | Implementation evidence | Result |
|---|---|---|---|---|---|---|
| `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1` | `W260914RUZU-DES1` | `S3` | `V5`, `V6`, `V14` | Parse bounded hostname CONNECT requests, select direct mode for unmatched hosts, connect upstream before `200`, and relay bytes unchanged | `src/proxy.rs:parse_connect`, `handle_client`; proxy tests for exact parser errors/statuses and direct available/unavailable sockets | `Satisfied` |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S6` | `V15` | Selected hosts use only target resolver IPv4 candidates with no operating-system fallback | `src/proxy.rs:399-429`; selected resolver/failure tests; `src/dns.rs:270-275` target resolver contract | `Satisfied` |
| `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1` | `W260914RUZU-DES4` | `S8`, `S9`, `S10`, `S11` | Make one bounded timed ClientHello decision, forward exactly one rewritten or safe unchanged prefix, reject with zero upstream prefix, then treat subsequent bytes as opaque | `src/proxy.rs:444-500`, `508-551`; selected rewrite/pass-through/reject/timeout/opaque-suffix socket tests; TLS regression suite | `Satisfied` |
| `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | `S12`, `S13` | Bound all per-tunnel waits, preserve cancellation, hold the concurrency permit for the handler lifetime, and isolate failures | `src/proxy.rs:323-328`, `345`, `561-590`; permit lifetime is covered, but CONNECT-header and direct-resolution waits are not fully bounded/cancellation-aware | `Mismatch` |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | `S15` | Return stable host/category/count-only tunnel reports and secret-safe outcomes | `src/diagnostics.rs:8-121`; DNS failure, target transformation, direct relay, timeout, cancellation, connect, and TLS outcomes are mapped without payload fields | `Satisfied` |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | Deterministically prove assigned handler invariants and component-level failure isolation with Rust checks passing | 50 unit/component tests pass, but no concurrent malformed-target/direct-success handler scenario executes | `Mismatch` |

## Technical Verification

| Verification Case or Check | Success Criteria | Design IDs | Command or method | Actual evidence | Result |
|---|---|---|---|---|---|
| `V5` | `S3` | `W260914RUZU-DES1` | `cargo test proxy` plus parser inspection | `parses_normalized_hostname_connect_request` passed with normalized `reddit.com:443` | `Passed` |
| `V6` | `S3`, `S12` | `W260914RUZU-DES1`, `W260914RUZU-DES5` | `cargo test proxy` plus exact assertion inspection | Method/version, authority/IP/port/body, incomplete/oversized header, malformed line/header, and arbitrary invalid-byte cases assert exact `ProxyError` and HTTP status; all passed without panic | `Passed` |
| `V14` component coverage | `S3`, `S11`, `S13` | `W260914RUZU-DES1`, `W260914RUZU-DES5` | `cargo test proxy`; inspect direct socket path | Real IPv4 loopback bidirectional relay is byte-identical; unavailable upstream never receives `200`; mixed IPv6/IPv4 candidates retain order and no family filter; target resolver remains unused. Listener/process and successful available-IPv6-origin integration remain planned for P3-G2/Part 4 | `Passed` |
| `V15` component coverage | `S6`, `S8`, `S9`, `S10`, `S11` | `W260914RUZU-DES3`, `W260914RUZU-DES4` | `cargo test proxy`; `cargo test dns`; `cargo test tls::tests`; flow inspection | Selected socket scenario invokes target resolver once, connects IPv4 before `200`, origin receives two valid records reassembling to the exact input handshake, opaque suffix/reply relay succeeds; safe pass-through is forwarded once; reject and timeout forward zero prefix. Authenticated local DoH/process integration remains planned for Part 4 | `Passed` |
| `V16` component coverage | `S12`, `S13` | `W260914RUZU-DES5` | Inspect and run proxy tests | Permit-lifetime, standalone target rejection, cancellation, timeout, and direct-success behaviors are covered, but no test runs malformed selected and successful direct handlers concurrently against shared state | `Insufficient` |
| `V19` component coverage | `S15` | `W260914RUZU-DES6` | `cargo test --workspace`; inspect `TunnelReport`, error strings, and selected DNS failure report | Stable `dns-error` and normalized host-only report contract is present; no raw CONNECT header, DNS body, ClientHello, or application bytes enter `TunnelReport`. Captured listener/process diagnostics remain planned for P3-G2/Part 4 | `Passed` |
| Proxy component tests | `S3`, `S6`, `S8`–`S13`, `S15`, `S17` | `W260914RUZU-DES1`, `W260914RUZU-DES3`–`W260914RUZU-DES7` | `cargo test proxy` | 16 passed, 0 failed, 34 filtered out | `Passed` |
| Protocol regressions | `S6`, `S8`–`S11`, `S17` | `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES7` | `cargo test dns`; `cargo test tls::tests` | DNS 10 passed; TLS 9 passed | `Passed` |
| Workspace tests | `S17` | `W260914RUZU-DES7` | `cargo test --workspace` | 50 passed, 0 failed | `Passed` |
| Format | `S17` | `W260914RUZU-DES7` | `cargo fmt --check` | Exit 0; no formatting diff | `Passed` |
| Lint | `S17` | `W260914RUZU-DES7` | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0; no warnings | `Passed` |
| All-target build check | `S17` | `W260914RUZU-DES7` | `cargo check --all-targets --all-features` | Exit 0 | `Passed` |
| Build | `S17` | `W260914RUZU-DES7` | `cargo build` | Exit 0 | `Passed` |
| Documentation tests | `S17` | `W260914RUZU-DES7` | `cargo test --doc --workspace` | Cargo returned `error: no library targets found in package splithello`; binary-only package, so not applicable | `Passed` |
| Full-range and scope inspection | `S17` | `W260914RUZU-DES7` | `git diff --check 8cbb95722ab202eede20de63d1ec159d020cc713..3568b1d6c77c651e70f38d4742635aeda9abbdfc`; name/status, commits, full `src/proxy.rs` and collaborating contracts | Range modifies only `src/proxy.rs`; whitespace check passed; no production spawn, `unsafe`, OS fallback in selected branch, TLS termination, payload logging, listener/signal/PAC/process implementation, or unrelated scope change | `Passed` |

## Findings

### P3-G1-VER1 — Major — Resource Bounds — CONNECT-header reads can hold a concurrency permit indefinitely

- **Requirement Revisions:** `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S12`, `S13`, `S17`
- **Verification Cases:** `V6`, `V16`
- **Evidence:** `src/proxy.rs:323-330` acquires the semaphore permit before reading CONNECT; `src/proxy.rs:561-590` bounds only byte count and cancellation, with no idle or request deadline around `client.read`. A client can send no terminator, or trickle bytes below the size limit, and retain one of the globally bounded permits without limit.
- **Impact:** Up to `max_connections` slow or stalled local clients can exhaust all permits and prevent unrelated direct or selected tunnels from being processed. The handler violates the planned bounded-I/O and idle-timeout contract even though buffer memory remains bounded.
- **Required resolution:** Apply a configured bounded deadline/idle timeout to CONNECT-header acquisition while preserving cancellation and exact 431/timeout response/report semantics; release the permit when that deadline expires.
- **Required proof:** Add a deterministic socket test that stalls or trickles an incomplete CONNECT request, observes bounded handler completion and permit release, then proves a waiting valid tunnel can proceed.

**Completion Checklist:**

- [ ] Required resolution is implemented.
- [ ] Required proof passes.
- [ ] Change evidence is recorded.
- [ ] Verification evidence is recorded.

### P3-G1-VER2 — Major — Cancellation and Bounds — Direct operating-system resolution ignores tunnel cancellation and configured time bounds

- **Requirement Revisions:** `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S11`, `S12`, `S13`, `S17`
- **Verification Cases:** `V14`, `V16`
- **Evidence:** `src/proxy.rs:345` directly awaits `SystemResolver::resolve`; `src/dns.rs:404-416` directly awaits `tokio::net::lookup_host`. Unlike target resolution, candidate connection, prefix acquisition, and relay, this wait has neither a `CancellationToken` select branch nor a configured timeout.
- **Impact:** A slow or wedged operating-system resolver can keep a direct handler and its semaphore permit alive after cancellation or beyond the configured tunnel time bounds, delaying shutdown and starving unrelated traffic.
- **Required resolution:** Make direct resolution cancellation-aware and bounded by an explicit configured deadline before candidate connection, while preserving all returned IPv4/IPv6 candidates and unchanged direct relay behavior.
- **Required proof:** Add deterministic resolver-seam tests showing cancellation and timeout terminate direct resolution without `200`, release the permit, and map to stable `cancelled`/`timeout` outcomes; retain a success test proving IPv4/IPv6 candidates remain unchanged and ordered.

**Completion Checklist:**

- [ ] Required resolution is implemented.
- [ ] Required proof passes.
- [ ] Change evidence is recorded.
- [ ] Verification evidence is recorded.

### P3-G1-VER3 — Major — Test Coverage — Assigned V16 handler-level concurrent failure isolation is not exercised

- **Requirement Revisions:** `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S12`, `S13`, `S17`
- **Verification Cases:** `V16`
- **Evidence:** `src/proxy.rs:1248-1292` tests one selected rejection in isolation; `src/proxy.rs:1017-1069` tests one direct success in isolation; `src/proxy.rs:1368-1425` tests permit lifetime with two direct tunnels. No test runs a malformed selected tunnel and a successful direct tunnel concurrently through shared `ProxyState`, then asserts both terminal outcomes and continued direct relay.
- **Impact:** The assigned component proof does not establish that selected TLS failure remains isolated while an unrelated direct handler succeeds concurrently. Listener/process ownership can be deferred to P3-G2/Part 4, but the P3-G1 per-handler/shared-state portion of `V16` is currently unverified.
- **Required resolution:** Add the focused concurrent component scenario without introducing accept-loop or process-harness scope.
- **Required proof:** A deterministic real-socket test must run both handlers concurrently, assert the malformed selected origin receives zero prefix and reports `tls-error`, assert the direct tunnel completes byte-identical relay with `direct-relay`, and assert the configured semaphore bound is respected.

**Completion Checklist:**

- [ ] Required resolution is implemented.
- [ ] Required proof passes.
- [ ] Change evidence is recorded.
- [ ] Verification evidence is recorded.

## Test Coverage

- **Covered:** Exact `V5`/`V6` parser outcomes; upstream-before-`200`; real IPv4 direct bidirectional relay; mixed IPv4/IPv6 candidate preservation; selected target-resolver-only path; rewritten/pass-through/reject single-prefix behavior; zero-prefix rejection/timeout; opaque suffix relay; candidate cancellation; relay idle/cancellation behavior; semaphore lifetime; stable secret-safe reports; DNS/TLS regressions; standard Rust checks.
- **Missing:** Bounded CONNECT-header acquisition and permit release; cancellation/timeout during direct OS resolution; concurrent malformed-selected plus successful-direct shared-state component scenario. Successful available-IPv6-origin relay, listener survival, authenticated local DoH collaboration, captured runtime diagnostics, and compiled-process behavior are correctly deferred to P3-G2/Part 4.

## Scope Changes

None. The implementation range `8cbb95722ab202eede20de63d1ec159d020cc713..3568b1d6c77c651e70f38d4742635aeda9abbdfc` changes only `src/proxy.rs`; listener ownership, signal lifecycle, PAC serving, runtime composition, and compiled-process integration were not added.

## Unverified Items

- Successful available-IPv6-origin direct relay through the composed listener/process path — planned `V14` Part 4 integration evidence.
- Authenticated local DoH selected flow through the compiled process — planned `V15` Part 4 integration evidence.
- Listener survival and process-level concurrency observation — planned P3-G2/Part 4 portions of `V16`.
- Captured tracing output and secret scanning at the process boundary — planned P3-G2/Part 4 portions of `V19`.

## Blockers

None. Verification completed and identified three remediable P3-G1 findings.
