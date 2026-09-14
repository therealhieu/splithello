# W260914RUZU Plan Part 4 — integration-documentation

## Status

`Ready`

## Objective

Prove the complete compiled-binary workflow against deterministic authenticated loopback boundaries, ship the valid Reddit/Medium example and concise operating documentation, and pass every overall Rust, contract, scope, and isolation check.

## Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1` through `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1` through `W260914RUZU-DES7`
- **Success Criteria:** `S1`–`S17`
- **Verification Cases:** `V14`–`V21` plus regression ownership of `V1`–`V13`

## Sources

- Plan index: `w260914ruzu-plan-index.md`
- Plan context: `w260914ruzu-plan-context.md`
- System design: `w260914ruzu-system-design.md`
- Requirements: `w260914ruzu-requirements.md`

## Dependencies

- Part 1 — stable CLI/config/domain/PAC/diagnostic contracts.
- Part 2 — authenticated DoH and TLS rewrite contracts.
- Part 3 — compiled proxy/PAC/runtime lifecycle.

## Execution Flow

```text
P4-G1: deterministic binary integration → P4-G2: example, docs, and release-quality checks
```

## Groups

### P4-G1 — deterministic-binary-integration

#### Goal

A compiled `splithello` child process is exercised through real loopback CONNECT/PAC sockets, an authenticated generated-CA DoH service, IPv4/conditional-IPv6 origins, target/non-target traffic, failures, and signal shutdown with exact observable assertions.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1` through `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1` through `W260914RUZU-DES7`
- **Success Criteria:** `S1`–`S15`, `S17`
- **Verification Cases:** `V14`–`V19`

#### Coherence Boundary

The child-process harness, authenticated local DoH endpoint, local origins, temporary config, readiness capture, CONNECT client, and cleanup utilities jointly prove real wiring. One group must own their lifecycle to avoid unauthenticated shortcuts or flaky external dependencies.

#### Scope

- Included: dev dependencies if still needed, generated CA and localhost certificate, local HTTPS DoH, local origin capture, child process wrapper, temporary config, six binary integration cases, deterministic deadlines/cleanup.
- Excluded: public Reddit/Medium/DoH network calls, root/firewall tests, documentation prose.
- Main files: `tests/support/mod.rs`, `tests/start_proxy.rs`, `Cargo.toml`, `Cargo.lock` only for dev dependencies/features.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| All current `W260914RUZU-REQ1@R1`–`W260914RUZU-REQ9@R1` | `W260914RUZU-DES1`–`W260914RUZU-DES7` | `S1`–`S15`, `S17` | `V14`–`V19` | Real compiled-process behavior matches every important cross-module/lifecycle contract without public network dependence |

#### Cross-Work Gates

None.

#### Tasks

##### P4-G1-T1 — build-authenticated-loopback-test-harness

- **Outcome:** Tests can start authenticated local HTTPS DoH with generated trust, IPv4 and optional IPv6 origins, the compiled child with temporary config, a CONNECT client, readiness capture, signal control, and guaranteed cleanup.
- **Requirement Revisions:** `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S6`, `S11`–`S14`, `S17`
- **Verification Cases:** `V14`–`V19`
- **Files:** `tests/support/mod.rs`, `Cargo.toml`, `Cargo.lock`
- **Execution Approach:** TDD

**Steps:**

1. **RED — harness self-tests/skeleton integration:** `tests/support/mod.rs`

   **Preview:**

   ```rust
   // Generated CA signs localhost cert; HTTPS client with that CA succeeds,
   // client without CA fails authentication.
   // TestDoh records validated queries and returns configurable DNS responses.
   // TestOrigin captures first bytes and echoes/returns fixture bytes.
   // RunningSplitHello waits for ready, captures output, signals child, kills on Drop fallback.
   ```

2. **GREEN:** harness types and helpers

   **Preview:**

   ```rust
   struct TestDoh { addr: SocketAddr, ca_pem: PathBuf, queries: Arc<...> }
   struct TestOrigin { addr: SocketAddr, received_prefix: Arc<...> }
   struct RunningSplitHello { child: Child, proxy_addr: SocketAddr, pac_addr: Option<SocketAddr>, output: Arc<...> }

   fn build_client_hello(sni: &str, splits: &[usize]) -> Vec<u8>;
   async fn connect_via_proxy(proxy: SocketAddr, authority: &str) -> TcpStream;
   ```

   - Use `rcgen` and `tokio-rustls` only in dev/test code.
   - Write generated CA PEM path into `dns.ca_certificate`; never disable verification.
   - Give every socket/process wait a deterministic timeout.

3. **REFACTOR:** centralize cleanup guards and ephemeral-address config generation; keep scenario assertions out of support helpers.

   **Preview:**

   ```rust
   // Helpers arrange boundaries; start_proxy.rs owns behavior assertions.
   ```

4. **COMMIT:** Run harness-focused tests and self-review.

   **Commit message:**

   ```text
   test(integration): add authenticated loopback harness

   Task: P4-G1-T1 — build-authenticated-loopback-test-harness
   ```

##### P4-G1-T2 — prove-direct-selected-and-failure-isolation

- **Outcome:** Binary tests prove unselected IPv4/available-IPv6 unchanged relay, selected authenticated DoH plus exact ClientHello rewrite, and simultaneous malformed-target/direct success under concurrency limits.
- **Requirement Revisions:** `W260914RUZU-REQ2@R1` through `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1` through `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S3`, `S5`–`S13`, `S17`
- **Verification Cases:** `V14`, `V15`, `V16`
- **Files:** `tests/start_proxy.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V14`, `V15`, `V16`:** compiled-binary cases

   **Preview:**

   ```rust
   // V14: unselected IPv4 and conditional ::1 origins receive byte-identical streams;
   // target DoH/TLS paths remain unused.
   // V15: selected hostname produces authenticated DoH A query; origin observes two
   // valid handshake records whose concatenated ClientHello equals browser input.
   // V16: malformed selected tunnel closes with tls-error while concurrent direct
   // tunnel completes; measured active handlers never exceed configured limit.
   ```

2. **GREEN:** only the minimal runtime/harness corrections exposed by failing cross-module tests

   **Preview:**

   ```rust
   // Fix wiring defects only; preserve approved module contracts and task ownership.
   // Do not redesign protocols or add fallback strategies.
   ```

   - If implementation corrections touch Part 1–3 modules, commit them under this task with explicit evidence and keep scope limited to integration mismatch.

3. **REFACTOR:** reduce duplicated scenario setup while retaining unique assertions.

   **Preview:**

   ```rust
   // Shared arrange helpers; each V case names one unique integration risk.
   ```

4. **COMMIT:** Run V14–V16 plus all unit tests and self-review.

   **Commit message:**

   ```text
   test(integration): verify proxy tunnel behavior

   Task: P4-G1-T2 — prove-direct-selected-and-failure-isolation
   ```

##### P4-G1-T3 — prove-shutdown-readiness-pac-and-secret-safe-outcomes

- **Outcome:** Binary tests prove signal shutdown deadline, exact ready/PAC workflow, and stable secret-safe failure diagnostics.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S4`, `S12`–`S15`, `S17`
- **Verification Cases:** `V17`, `V18`, `V19`
- **Files:** `tests/start_proxy.rs`
- **Execution Approach:** TDD

**Steps:**

1. **RED — `V17`, `V18`, `V19`:** lifecycle/operator cases

   **Preview:**

   ```rust
   // V17: signal active child; exit before deadline; listeners close; no persistent state.
   // V18: exact start command reaches ready fields; PAC endpoint serves expected policy.
   // V19: induced DoH failure emits dns-error and normalized host but no injected secret,
   // raw DNS bytes, ClientHello bytes, or request header.
   ```

2. **GREEN:** minimal runtime/log/PAC corrections exposed by cases

   **Preview:**

   ```rust
   // Preserve stable Outcome strings and ready metadata contract.
   // Do not expose debug payloads to satisfy tests.
   ```

3. **REFACTOR:** centralize output waiting/parsing in harness, not production logging.

   **Preview:**

   ```rust
   // Production emits structured tracing; tests recognize stable fields/categories.
   ```

4. **COMMIT:** Run V17–V19 and full integration suite; self-review.

   **Commit message:**

   ```text
   test(integration): verify lifecycle and diagnostics

   Task: P4-G1-T3 — prove-shutdown-readiness-pac-and-secret-safe-outcomes
   ```

**Verification:**

- `V14`–`V19` — `cargo test --test start_proxy` → all compiled-binary cases pass deterministically.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Binary integration | `cargo test --test start_proxy` | V14–V19 pass without Internet/root |
| Full tests | `cargo test --workspace` | Unit V1–V13 and integration V14–V19 all pass |
| Test authentication inspection | inspect harness/config and negative trust case | Local DoH succeeds only with generated additive CA; verification is never disabled |
| Isolation | `git status --short` and test cleanup | No test process/socket/temp artifact remains; only scoped changes exist |

#### Review and Verification Focus

- **Specification:** exact target/direct flows, IPv4/IPv6 boundary, authenticated DoH, exact handshake preservation, failure isolation, ready/PAC/shutdown/logging.
- **Technical:** deterministic deadlines, child cleanup, no public network, unique integration risks, no invalid-cert test shortcut, no flaky port allocation.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

### P4-G2 — example-documentation-and-release-checks

#### Goal

Ship a parser-valid Reddit/Medium example and concise documentation matching executable behavior, then pass all quality, contract, clean-room, scope, and working-tree checks.

#### Coverage Summary

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S2`, `S15`, `S16`, `S17`
- **Verification Cases:** `V18`, `V19`, `V20`, `V21`

#### Coherence Boundary

The example, README, CLI help, outcome names, and full verification commands are the user-facing release contract. They must be finalized after binary behavior is proven so documentation records actual behavior rather than intent.

#### Scope

- Included: `config.example.toml`, README setup/use/PAC/diagnostics/scope/rollback, documentation consistency test/inspection, full formatting/lint/build/test, prohibited-surface inspection.
- Excluded: new runtime behavior, public live test as acceptance gate, release/push/deployment.
- Main files: `config.example.toml`, `README.md`, focused consistency test if needed, all planned files only for remediation of discovered defects.

#### Traceability

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Required outcome |
|---|---|---|---|---|
| `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1` (prefixed `W260914RUZU-`) | `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | `S1`, `S2`, `S15`–`S17` | `V18`–`V21` | Copy-pasteable operation, honest scope, and all repository checks agree |

#### Cross-Work Gates

None.

#### Tasks

##### P4-G2-T1 — ship-valid-example-and-operating-guide

- **Outcome:** `config.example.toml` parses through production code and configures loopback proxy/PAC, authenticated public DoH via normal trust, conservative limits, and Reddit/Medium target/subdomain rules; README accurately explains use and limits.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES6`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S2`, `S15`, `S16`, `S17`
- **Verification Cases:** `V18`, `V19`, `V20`
- **Files:** `config.example.toml`, `README.md`, config/consistency tests
- **Execution Approach:** TDD for example consistency, then documentation direct verification within the same coherent task.

**Steps:**

1. **RED — `V20`:** add example/config/documentation consistency assertions

   **Preview:**

   ```rust
   // Load repository config.example.toml with production Config::load.
   // Assert Reddit/Medium include-subdomains and tls-record-split strategy.
   // Assert README contains exact start command, proxy and PAC setup, rollback,
   // stable outcomes, and explicit no-VPN/no-anonymity/no-IP-block/no-target-IPv6/no-QUIC scope.
   ```

2. **GREEN:** author example and README

   **Preview:**

   ```toml
   [proxy]
   listen = "127.0.0.1:8080"
   pac_listen = "127.0.0.1:8081"
   # Authenticated DoH uses normal trust; ca_certificate omitted.

   [[targets]]
   host = "reddit.com"
   include_subdomains = true
   strategy = "tls-record-split"
   # Repeat for medium.com; include conservative limits.
   ```

   ```text
   README: purpose → build → exact command → browser HTTPS proxy/PAC setup
          → verify target-transformed → troubleshooting outcomes
          → limits/non-goals → stop/remove proxy rollback
   ```

   - State that startup success is not proof of ISP bypass.
   - Do not promise every asset/app, advise disabling security, or include packet recipes.

3. **REFACTOR:** remove duplicate prose and link configuration fields to one authoritative example.

   **Preview:**

   ```text
   Keep documentation concise; preserve exact executable strings and caveats.
   ```

4. **COMMIT:** Run V20 plus CLI/example tests and self-review.

   **Commit message:**

   ```text
   docs: add example configuration and usage

   Task: P4-G2-T1 — ship-valid-example-and-operating-guide
   ```

##### P4-G2-T2 — run-release-quality-and-scope-verification

- **Outcome:** Formatting, lint, build, all tests, CLI help, documentation consistency, prohibited-surface, dependency, and working-tree checks pass; any in-scope defect is fixed and committed before final group acceptance.
- **Requirement Revisions:** `W260914RUZU-REQ1@R1` through `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1` through `W260914RUZU-DES7`
- **Success Criteria:** `S1`–`S17`
- **Verification Cases:** `V1`–`V21`
- **Files:** all planned files only as required for verification remediation
- **Execution Approach:** Direct verification — this task validates the integrated result; fixes use focused regression tests before changes.

**Steps:**

1. Run the complete check set and inspect exact evidence.

   **Preview:**

   ```text
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo build
   cargo test --workspace
   cargo run -- --help
   cargo run -- start --help
   documentation/config consistency check
   approved-scope and invalid-certificate-bypass static search
   git status --short
   ```

   - If a check exposes an in-scope defect, add/confirm the focused regression assertion, make the smallest correction, and rerun affected plus complete checks.
   - Do not weaken tests or approved constraints to obtain green results.

2. **VERIFY — `V1`–`V21`:** map actual results to every success criterion.

   **Preview:**

   ```text
   V1–V13 unit evidence + V14–V19 binary integration + V20 consistency + V21 quality
   └── Expect: every S1–S17 has exact passing evidence and no unresolved issue
   ```

   - Inspect dependency graph/license notes, no `unsafe`, no firewall/raw socket/system mutation, no invalid-cert bypass, selected IPv4/direct dual-family behavior, and clean-room scope.

3. **COMMIT:** Commit only actual verification-driven fixes, if any; record no-change evidence otherwise.

   **Commit message:**

   ```text
   fix: close release verification gaps

   Task: P4-G2-T2 — run-release-quality-and-scope-verification
   ```

   - Do not create an empty commit when all checks pass without changes.

**Verification:**

- `V20` — exact documentation/example contract.
- `V21` — format, clippy, build, workspace tests.
- Full `V1`–`V19` regression and static approved-scope review.

#### Group Verification

| Check | Command or method | Expected result |
|---|---|---|
| Format | `cargo fmt --check` | Exit 0 |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0, no warnings |
| Build | `cargo build` | Exit 0 |
| Tests | `cargo test --workspace` | All deterministic V cases pass |
| CLI | help commands | Exact required command; no prohibited tuning surface |
| Docs/config | V20 check | Example parses; README matches fields/outcomes/scope |
| Static scope | focused source search and inspection | Safe Rust explicit loopback proxy only; no copied/reference implementation surface, firewall/raw socket, invalid-cert bypass, or target IPv6/QUIC behavior |
| Isolation | `git status --short` and commit inspection | Only planned files/reports/docs; no uncommitted scoped changes |

#### Review and Verification Focus

- **Specification:** complete REQ/DES/S/V coverage, exact command/config, honest limits, rollback, no unsupported promise.
- **Technical:** all checks green, no warnings/flakes/leaks, maintained dependency graph, additive CA authentication, address-family contract, minimal maintainable code.

#### Acceptance Criteria

- Every task outcome is implemented.
- Every assigned `REQ`, `DES`, `S`, and `V` item is satisfied.
- Every task-level and group-level verification check passes.
- Protected contracts and declared scope remain intact.
- No unresolved implementation or verification issue remains.

## Coverage

| Requirement Revision | Design ID | Success Criterion | Verification Cases | Groups | Tasks |
|---|---|---|---|---|---|
| `W260914RUZU-REQ1@R1` | `W260914RUZU-DES2` | `S1`, `S2` | `V18`, `V20`, `V21` | G1, G2 | G1-T3, G2-T1, G2-T2 |
| `W260914RUZU-REQ2@R1` | `W260914RUZU-DES1` | `S3`, `S4` | `V14`, `V18`, `V21` | G1, G2 | G1-T2, G1-T3, G2-T2 |
| `W260914RUZU-REQ3@R1` | `W260914RUZU-DES2` | `S5` | `V14`, `V20`, `V21` | G1, G2 | G1-T2, G2-T1, G2-T2 |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S6`, `S7` | `V15`, `V19`, `V21` | G1, G2 | G1-T1, G1-T2, G1-T3, G2-T2 |
| `W260914RUZU-REQ5@R1` | `W260914RUZU-DES4` | `S8`–`S10` | `V15`, `V16`, `V21` | G1, G2 | G1-T2, G2-T2 |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5` | `S11`, `S12` | `V14`–`V16`, `V19`–`V21` | G1, G2 | all tasks |
| `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | `S13`, `S14` | `V14`, `V16`, `V17`, `V21` | G1, G2 | G1-T1–T3, G2-T2 |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | `S15`, `S16` | `V18`–`V21` | G1, G2 | G1-T3, G2-T1, G2-T2 |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V14`–`V21` | G1, G2 | all tasks |

## Completion Criteria

- Both groups satisfy acceptance criteria.
- Parts 1–3 contracts and behavior are integrated without redesign.
- Every success criterion and verification case has passing evidence.
- Documentation reflects verified behavior.
- No required check, scoped change, or blocker remains unresolved.

## Planning Gaps

`None`
