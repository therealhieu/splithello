# W260914RUZU Plan Part 3 — proxy-runtime

## Status

`Ready`

## Objective

Compose the approved loopback CONNECT/PAC services and process lifecycle so direct traffic relays unchanged over normal OS address families, selected traffic uses authenticated DoH plus one bounded ClientHello transformation, and failures/shutdown remain isolated and bounded.

## Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S4`, `S6`, `S8`–`S15`, `S17`
- **Verification Cases:** `V5`, `V6`, `V14`–`V19`, `V21`

## Sources

- Plan index: `w260914ruzu-plan-index.md`
- Plan context: `w260914ruzu-plan-context.md`
- System design: `w260914ruzu-system-design.md`
- Requirements: `w260914ruzu-requirements.md`

## Dependencies

- Part 1 — validated config/domain/PAC/diagnostic/CONNECT contracts.
- Part 2 — selected/system resolver boundary and TLS rewrite outcome.

## Execution Flow

```text
P3-G1: connect and relay engine → P3-G2: listeners, lifecycle, and CLI composition
```

## Groups

### P3-G1 — connect-and-relay-engine

#### Goal

One independent handler validates CONNECT, waits for upstream readiness, then performs either unchanged direct relay or selected DoH/ClientHello relay with bounded I/O and precise terminal reporting.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S6`, `S8`–`S13`, `S15`, `S17`
- **Verification Cases:** `V5`, `V6`, `V14`, `V15`, `V16`, `V19`

#### Coherence Boundary

CONNECT acknowledgement, address resolution, candidate connection, rewrite-before-forward, streaming relay, cancellation, and report finalization are one tunnel state machine. One group must own their sequencing and failure semantics.

#### Scope

- Included: `ProxyState`, socket candidate attempts, direct/target mode, `200` timing, target-prefix timeout/buffer, bidirectional relay, semaphore/cancellation checks, outcome reports, focused loopback component tests.
- Excluded: accept loop ownership, OS signals, PAC listener, compiled child-process harness.
- Main files: `src/proxy.rs`, `src/dns.rs`, `src/tls.rs`, `src/diagnostics.rs` only for integration use or necessary mechanical contract correction.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ2@R1`–`W260914RUZU-REQ9@R1` applicable set (prefixed `W260914RUZU-`) | `W260914RUZU-DES1`, `W260914RUZU-DES3`–`W260914RUZU-DES7` | `S3`, `S6`, `S8`–`S13`, `S15`, `S17` | `V5`, `V6`, `V14`–`V16`, `V19` | Per-tunnel direct and selected paths are correctly sequenced, bounded, isolated, and observable |

#### Cross-Work Gates

None.

#### Tasks

##### P3-G1-T1 — implement-address-connection-and-direct-relay

- **Outcome:** Unselected tunnels resolve through normal OS IPv4/IPv6 candidates, attempt socket addresses within one deadline, send `200` only after connection success, and relay client/origin bytes unchanged.
- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S11`, `S12`, `S13`, `S17`
- **Verification Cases:** `V5`, `V6`, `V14`
- **Files:** `src/proxy.rs`, `src/dns.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — direct path component tests:** loopback tests near `src/proxy.rs`

   **Preview:**

   ```rust
   // Upstream unavailable: client receives 502/504, never 200.
   // Upstream available: 200 then byte-identical bidirectional relay.
   // Candidate list includes IPv4 and IPv6 SocketAddr; connector does not filter family.
   // TargetMatcher miss selects Direct and never invokes TargetResolver/TLS parser.
   ```

2. **GREEN:** socket connection and direct handler path

   **Preview:**

   ```rust
   struct ProxyState { matcher: TargetMatcher, target_resolver: Arc<dyn TargetResolver>, limits: RuntimeLimits, /* shared */ }
   async fn connect_socket_candidates(addrs: &[SocketAddr], deadline: Instant) -> Result<TcpStream, ConnectError>;
   async fn relay_direct(client: TcpStream, upstream: TcpStream, limits: &RuntimeLimits, cancel: &CancellationToken) -> Result<RelayStats, ProxyError>;
   ```

   - Preserve SystemResolver addresses as returned.
   - Use one total deadline, bounded idle timeout, and cancellation-aware relay.
   - Keep CONNECT response mapping precise and payload-free.

3. **REFACTOR:** extract response writer and relay-stat mapping without abstracting over unrelated protocols.

   **Preview:**

   ```rust
   // Small HTTP status writer; RelayStats converts once to TunnelReport.
   ```

4. **COMMIT:** Run direct/connect/parser tests and self-review.

   **Commit message:**

   ```text
   feat(proxy): relay direct connect tunnels

   Task: P3-G1-T1 — implement-address-connection-and-direct-relay
   ```

##### P3-G1-T2 — implement-selected-target-tunnel

- **Outcome:** A selected tunnel uses only authenticated DoH IPv4 candidates, acknowledges after upstream connection, buffers one ClientHello to a timeout/size decision, forwards rewritten or explicitly safe pass-through bytes once, then relays opaque streams.
- **Requirement Revisions:** `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`, `S8`–`S13`, `S15`, `S17`
- **Verification Cases:** `V15`, `V16`, `V19`
- **Files:** `src/proxy.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — selected orchestration tests:** local fakes and socket pairs

   **Preview:**

   ```rust
   // Selected match invokes TargetResolver, never SystemResolver.
   // DoH/connect failure occurs before 200 and affects only this handler.
   // Matching ClientHello: origin receives valid rewritten prefix, then opaque relay.
   // PassThrough forwards original buffered bytes once; Reject closes only tunnel.
   // Timeout/size/concurrency/cancellation produce stable outcomes without leaks.
   ```

2. **GREEN:** `TunnelMode`, target path, high-level `handle_client`

   **Preview:**

   ```rust
   enum TunnelMode<'a> { Direct, Target(&'a TargetRule) }
   async fn handle_client(client: TcpStream, state: Arc<ProxyState>, cancel: CancellationToken)
       -> Result<TunnelReport, ProxyError>;
   async fn relay_target(/* client, upstream, expected host, limits, cancel */)
       -> Result<RelayStats, ProxyError>;
   ```

   - Do not fall back to OS DNS for selected targets.
   - Read no more than ClientHello limit and make the decision before upstream prefix write.
   - Never terminate TLS or inspect post-decision application data.

3. **REFACTOR:** unify direct/target terminal reporting while retaining explicit mode branches.

   **Preview:**

   ```rust
   // Shared final TunnelReport; separate direct and target mechanics remain visible.
   ```

4. **COMMIT:** Run selected/direct/protocol tests and self-review.

   **Commit message:**

   ```text
   feat(proxy): transform selected connect tunnels

   Task: P3-G1-T2 — implement-selected-target-tunnel
   ```

**Verification:**

- `V14` — direct loopback component case → unchanged data and OS-family support.
- `V15` — selected component case → target resolver plus transformed handshake.
- `V16`, `V19` preparation → independent errors and stable reports.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Proxy component tests | focused `cargo test` filters for proxy | Direct/selected sequencing and response timing pass |
| Protocol regressions | focused dns/tls tests | V8–V13 remain passing |
| Safety/flow inspection | review handler branches and output order | No selected OS fallback; no ClientHello partial forward; unselected bytes unchanged |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** `200` after upstream readiness, selected-only DoH/rewrite, direct normal OS families, end-to-end TLS, per-tunnel failure.
- **Technical:** total deadlines, cancellation, exact single prefix write, semaphore lifetime, no secret payload errors, no lock across network await.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

### P3-G2 — listeners-lifecycle-and-cli-composition

#### Goal

The compiled application atomically binds loopback CONNECT/PAC listeners, emits ready state, tracks all service/tunnel tasks, handles signals, and drains then aborts/joins within the configured shutdown deadline.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S3`, `S4`, `S12`–`S15`, `S17`
- **Verification Cases:** `V14`, `V16`, `V17`, `V18`, `V19`, `V21`

#### Coherence Boundary

Listener binding, accept loops, child-task ownership, signal cancellation, ready logging, and CLI error exits define one process lifecycle and must share one runtime owner.

#### Scope

- Included: proxy listener, PAC HTTP listener, atomic startup, nested task tracking, signal wait, bounded shutdown, tracing initialization, top-level error mapping.
- Excluded: final compiled-process external-service harness and README.
- Main files: `src/app.rs`, `src/proxy.rs`, `src/pac.rs`, `src/main.rs`, `src/diagnostics.rs`.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`–`W260914RUZU-REQ9@R1` (prefixed `W260914RUZU-`) | `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`–`W260914RUZU-DES7` | `S1`, `S3`, `S4`, `S12`–`S15`, `S17` | `V14`, `V16`–`V19`, `V21` | One atomic, observable, bounded process lifecycle composes the accepted services |

#### Cross-Work Gates

None.

#### Tasks

##### P3-G2-T1 — implement-proxy-and-pac-service-loops

- **Outcome:** Loopback listener loops enforce concurrency, track every tunnel child, serve immutable PAC responses, stop accepting on cancellation, and return only after child drain/abort/join.
- **Requirement Revisions:** `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S4`, `S12`, `S13`, `S14`, `S17`
- **Verification Cases:** `V14`, `V16`, `V17`
- **Files:** `src/proxy.rs`, `src/pac.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — service-loop lifecycle tests:** loopback listeners

   **Preview:**

   ```rust
   // Accept multiple clients up to semaphore limit; per-client error does not exit loop.
   // Cancellation stops accept; child JoinSet drains then aborts/join remaining tasks.
   // PAC serves only GET /proxy.pac with fixed content length/body and bounded request.
   ```

2. **GREEN:** `ProxyServer`, `PacServer`, accept/task loops

   **Preview:**

   ```rust
   struct ProxyServer { listener: TcpListener, state: Arc<ProxyState>, cancel: CancellationToken }
   impl ProxyServer { async fn run(self) -> Result<(), AppError>; }

   struct PacServer { listener: TcpListener, body: Arc<str>, cancel: CancellationToken }
   impl PacServer { async fn run(self) -> Result<(), AppError>; }
   ```

   - Each service owns and joins every child task it spawns.
   - Keep one cancellation token lineage and bounded request/service errors.

3. **REFACTOR:** extract shared deadline drain helper only if both services need identical behavior; do not build a service framework.

   **Preview:**

   ```rust
   // Prefer direct visible loops; abstract only proven duplicate shutdown code.
   ```

4. **COMMIT:** Run service/proxy/PAC tests and self-review.

   **Commit message:**

   ```text
   feat(runtime): run proxy and pac services

   Task: P3-G2-T1 — implement-proxy-and-pac-service-loops
   ```

##### P3-G2-T2 — compose-atomic-runtime-and-graceful-shutdown

- **Outcome:** `Runtime::start` constructs dependencies and binds all required listeners before ready, while `run_until_signal` and `shutdown` coordinate tracked service completion within the configured deadline.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S3`, `S4`, `S12`–`S15`, `S17`
- **Verification Cases:** `V16`, `V17`, `V18`, `V19`
- **Files:** `src/app.rs`, `src/main.rs`, `src/diagnostics.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — runtime startup/shutdown tests:** `src/app.rs`

   **Preview:**

   ```rust
   // Second required bind failure drops first listener and returns before ready.
   // Successful start exposes actual bound addresses and ready metadata.
   // Cancellation drains cooperative tasks; deadline aborts and joins a stuck task.
   // Task panic maps to internal-error without untracked child work.
   ```

2. **GREEN:** `Runtime`, main dispatch, tracing

   **Preview:**

   ```rust
   struct Runtime { cancel: CancellationToken, tasks: JoinSet<()>, shutdown_timeout: Duration }
   impl Runtime {
       async fn start(config: Config) -> Result<Self, AppError>;
       async fn run_until_signal(&mut self) -> Result<(), AppError>;
       async fn shutdown(self);
   }
   async fn run() -> Result<(), AppError>;
   ```

   - Bind listener values before spawning and before ready event.
   - Initialize resolver/matcher/semaphore once and share immutable state.
   - SIGINT/SIGTERM cancellation must work on supported Linux; tests use direct cancellation seam.

3. **REFACTOR:** keep `main` limited to tracing, CLI, config, lifecycle, and exit mapping.

   **Preview:**

   ```rust
   // No networking policy or protocol parsing in main/app.
   ```

4. **COMMIT:** Run runtime/service tests and self-review.

   **Commit message:**

   ```text
   feat(app): compose atomic proxy runtime

   Task: P3-G2-T2 — compose-atomic-runtime-and-graceful-shutdown
   ```

**Verification:**

- `V16` — concurrent failure/direct success component scenario.
- `V17` — cancellation/drain/abort process-owner behavior.
- `V18`, `V19` contract preparation — ready and stable events emitted for Part 4 assertions.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Runtime/service tests | focused `cargo test` filters for app/proxy/pac | atomic start, accept isolation, cancellation/shutdown pass |
| CLI smoke | run binary with invalid config and temporary valid local config | Invalid exits before bind; valid reaches ready with expected addresses |
| Task ownership inspection | inspect every `spawn` site | Each task is held by a JoinSet and joined/aborted on shutdown |
| Build | `cargo check --all-targets --all-features` | Exit 0 |

#### Review and Verification Focus

- **Specification:** loopback-only, no root/system mutation, atomic start, stable ready, shutdown deadline, PAC optional.
- **Technical:** listener bind ordering, nested task ownership, panic/error isolation, no detached work, one terminal tunnel report, no payload logging.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

## Coverage

| Requirement Revision | Design ID | Success Criterion | Verification Cases | Groups | Tasks |
|---|---|---|---|---|---|
| `W260914RUZU-REQ2@R1` | `W260914RUZU-DES1` | `S3`, `S4` | `V5`, `V6`, `V14`, `V18` | G1, G2 | G1-T1, G2-T1, G2-T2 |
| `W260914RUZU-REQ3@R1` | `W260914RUZU-DES2` | `S5`, `S13` | `V14` | G1 | T1, T2 |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S6`, `S7` | `V15` | G1 | T2 |
| `W260914RUZU-REQ5@R1` | `W260914RUZU-DES4` | `S8`–`S11` | `V15`, `V16` | G1 | T2 |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5` | `S11`, `S12` | `V14`–`V16` | G1, G2 | all tasks |
| `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | `S13`, `S14` | `V14`–`V17` | G1, G2 | all tasks |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | `S15` | `V18`, `V19` | G1, G2 | G1-T2, G2-T2 |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V5`, `V6`, `V14`–`V19`, `V21` | G1, G2 | all tasks |

## Completion Criteria

- Both groups satisfy acceptance criteria.
- Part 1 and Part 2 contracts are consumed without semantic changes.
- Every service/task is owned, bounded, and testable.
- Runtime outputs are committed and ready for compiled-binary Part 4 verification.

## Planning Gaps

`None`
