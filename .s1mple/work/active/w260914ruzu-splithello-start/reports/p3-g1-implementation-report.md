# P3-G1 Implementation Report

## Scope

- **Goal:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-goal.md`
- **Plan Part:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-3-proxy-runtime.md`
- **Group:** `P3-G1 — connect-and-relay-engine`
- **Tasks:** `P3-G1-T1`, `P3-G1-T2`
- **Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-implementation-report.md`

Report commit SHAs are returned to the orchestrator and recorded in the execution goal; they are not written into this file because doing so would require a self-referential follow-up commit.

## Initial Implementation

Written by the implementer after the initial implementation turn and before returning to the orchestrator.

### Status

`Completed`

### Tasks

| Task | Changes | Verification | Commit evidence | Result |
|---|---|---|---|---|
| `P3-G1-T1 — implement-address-connection-and-direct-relay` | `src/proxy.rs:ProxyState`, `connect_socket_candidates`, direct branch of `handle_client`, `relay_streams`, response/report helpers and loopback tests — preserves ordered OS `SocketAddr` candidates across IPv4/IPv6, applies one candidate-attempt deadline, acknowledges only after upstream connection, never calls `TargetResolver` on a matcher miss, and relays both directions unchanged with cancellation and idle bounds | RED/GREEN/final: `cargo test ... proxy` passed 16/16; direct available/unavailable, family-order, target-miss, bidirectional bytes/stats, cancellation, idle behavior, and acknowledgement timing tests pass; Clippy and all-target check pass | `4b45788c5ce1d4dd2b3490d9accd1fec36a66c1c`; self-review follow-up `3568b1d6c77c651e70f38d4742635aeda9abbdfc` | `Completed` |
| `P3-G1-T2 — implement-selected-target-tunnel` | `src/proxy.rs:handle_target`, `decide_client_hello_prefix` and selected-path tests — invokes only the injected IPv4 `TargetResolver`, connects candidates before `200`, makes one bounded/time-limited ClientHello decision, forwards one rewritten or explicitly safe unchanged prefix, closes on reject without prefix output, then treats later bytes as opaque relay; terminal reports remain host/category/count-only and the semaphore permit spans the full handler lifetime | RED/GREEN/final: selected resolver/rewrite, pass-through-once, reject-with-zero-origin-bytes, DNS failure-before-200, one-total-prefix-timeout, opaque suffix relay, and concurrency-permit-lifetime tests pass in the 16/16 proxy suite; DNS 10/10, TLS 9/9, workspace 50/50, Clippy and all-target check pass | `bcf23f409e23294c84565282be19bca2d627375f`; self-review follow-up `3568b1d6c77c651e70f38d4742635aeda9abbdfc` | `Completed` |

### Group Change Range

- **Starting commit:** `8cbb95722ab202eede20de63d1ec159d020cc713`
- **Final commit:** `3568b1d6c77c651e70f38d4742635aeda9abbdfc`
- **Remaining group changes:** `None`. The pre-existing orchestrator-owned unstaged goal update remains preserved and excluded.

### Group Verification

- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml proxy` → `Passed` — 16 passed, 34 filtered out.
- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml dns` → `Passed` — 10 passed, 40 filtered out.
- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml tls::tests` → `Passed` — 9 passed, 41 filtered out.
- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --workspace` → `Passed` — 50 passed, 0 failed.
- `cargo fmt --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -- --check` → `Passed` — no formatting diff.
- `cargo clippy --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features -- -D warnings` → `Passed` — no issues found.
- `cargo check --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features` → `Passed` — exit 0.
- `cargo build --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml` → `Passed` — edition-2024 binary built successfully.
- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --doc --workspace` → `Not applicable` — Cargo reported `no library targets found in package splithello`; the package remains binary-only.
- Safety/flow inspection of `src/proxy.rs` → `Passed` — selected branch has no `SystemResolver` call, direct branch has no target resolver or TLS call, no prefix is written before a terminal rewrite/pass/reject decision, no production task is spawned, and no `unsafe`, payload logging, TLS termination, invalid-certificate handling, accept loop, signal handling, PAC server, or compiled harness was added.
- `git diff --check 8cbb95722ab202eede20de63d1ec159d020cc713..3568b1d6c77c651e70f38d4742635aeda9abbdfc` plus name/status, commit-body, and complete source review → `Passed` — the implementation range contains only `src/proxy.rs`; task/follow-up commits match the exact assigned titles and no group implementation change remains uncommitted.

### Deviations and Remaining Issues

- The orchestrator-owned update to `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-goal.md` existed before implementation and remains unstaged and uncommitted as required.
- Full listener ownership, signal lifecycle, PAC serving, runtime composition, and compiled-process integration remain in their planned P3-G2 and Part 4 groups; no out-of-scope implementation was added here.
- No implementation deviation or unresolved P3-G1 issue remains.

## Remediation

Append remediation evidence here after verifier findings. This is not a separate report. Preserve the Initial Implementation section unchanged.

- **Verification reports:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p3-g1-verification-report.md` committed at `1ebd942cbf5883a7d2299151b3932d87cf123d32`
- **Status:** `Completed`

### Finding Closure

| Finding | Change and verification evidence | Commit evidence | Result |
|---|---|---|---|
| `P3-G1-VER1` | `src/proxy.rs:read_connect_request`, `RequestReadError`, `TunnelError::outcome`, and `stalled_connect_header_times_out_with_431_and_releases_waiting_permit` apply one total CONNECT-header deadline plus resettable idle deadline, preserve cancellation, return exact HTTP `431` with stable `timeout`, and prove a waiting valid direct tunnel proceeds after the timed-out handler releases the sole permit. Focused proxy tests passed 21/21 and workspace tests passed 55/55. | `f91bfa6c43ae98b98680bbbff459cbbb42aa8f3c` — `P3-G1-T1` | `Fixed` |
| `P3-G1-VER2` | `src/proxy.rs:DirectResolver`, `ProxyState::direct_resolver`, and the direct branch of `handle_client` wrap OS resolution in cancellation plus the same total resolution/connection deadline before candidate attempts. Deterministic pending-resolver tests prove timeout returns HTTP `504`/`timeout`, cancellation returns `cancelled`, neither sends `200`, and both release the permit. The injected-success test returns ordered IPv6/IPv4 candidates unchanged and completes direct relay. Focused proxy tests passed 21/21 and DNS regressions passed 10/10. | `ec3f1cbb49fdeb08de40113c4ca82af4706f3757` — `P3-G1-T1` | `Fixed` |
| `P3-G1-VER3` | `src/proxy.rs:concurrent_malformed_selected_tunnel_does_not_break_direct_relay` runs real selected and direct sockets concurrently through one shared `ProxyState`: two configured permits are simultaneously held, malformed selected SNI yields `tls-error` and zero origin prefix bytes, while the direct IPv6 origin receives `direct` byte-for-byte, returns `success`, and reports `direct-relay`; both permits return afterward. Focused proxy tests passed 21/21. | `9547716b301675244c1ac967088f57832128823a` — `P3-G1-T2` | `Fixed` |

**P3-G1-VER1 Completion Checklist:**

- [x] Required resolution is implemented — CONNECT-header acquisition has total and idle deadlines, cancellation, exact timeout mapping, and bounded permit lifetime.
- [x] Required proof passes — stalled incomplete request returns `431`/`timeout`, releases the sole permit, and a waiting valid direct tunnel completes.
- [x] Change evidence is recorded — exact production and test symbols are listed above.
- [x] Verification evidence is recorded — focused, regression, workspace, quality, range, and isolation checks are recorded below.

**P3-G1-VER2 Completion Checklist:**

- [x] Required resolution is implemented — direct resolution is cancellation-aware and shares one configured total deadline with ordered candidate connection attempts.
- [x] Required proof passes — cancellation and timeout send no `200`, report stable outcomes, release permits, and ordered IPv6/IPv4 success remains intact.
- [x] Change evidence is recorded — exact resolver seam, handler branch, and tests are listed above.
- [x] Verification evidence is recorded — focused, DNS, workspace, quality, range, and isolation checks are recorded below.

**P3-G1-VER3 Completion Checklist:**

- [x] Required resolution is implemented — one real-socket concurrent selected-failure/direct-success shared-state scenario was added without listener/process scope.
- [x] Required proof passes — target origin receives zero prefix and `tls-error`; direct relay is byte-identical with `direct-relay`; configured semaphore use and release are asserted.
- [x] Change evidence is recorded — exact concurrent test symbol is listed above.
- [x] Verification evidence is recorded — focused and full group checks are recorded below.

### Blockers and Unverified Checks

| Report item | Resolution or check | Actual evidence | Commit evidence | Result |
|---|---|---|---|---|
| Successful available-IPv6-origin direct relay through composed listener/process | Preserved as planned Part 4 `V14`; P3-G1 now proves real direct IPv6 origin relay through handler/shared state | Concurrent isolation test binds the direct origin on `::1`, transfers exact request/reply bytes, and reports `direct-relay`; compiled process remains intentionally deferred | `9547716b301675244c1ac967088f57832128823a` | `Resolved` |
| Authenticated local DoH selected flow through compiled process | Confirmed still assigned to Part 4 `V15`; no invalid-certificate or fallback change was introduced | Selected handler continues to consume only `TargetResolver::resolve_ipv4`; DNS 10/10 and TLS 9/9 pass | No remediation change required beyond P3-G1 fixes | `Resolved` |
| Listener survival/process-level concurrency observation | Confirmed still assigned to P3-G2/Part 4; P3-G1 handler-level shared-state concurrency is now proven | Concurrent selected failure/direct success and permit-bound assertions pass | `9547716b301675244c1ac967088f57832128823a` | `Resolved` |
| Captured tracing output/process secret scan | Confirmed still assigned to P3-G2/Part 4; P3-G1 report/error surfaces remain payload-free | Static inspection shows no logging additions; stable outcome/report tests and workspace suite pass | No remediation change required | `Resolved` |

- During `P3-G1-VER3` TDD, the first concurrent test attempted to bind `127.0.0.2` and failed on this host with OS error 49 (`AddrNotAvailable`). The test was corrected before commit to use the available IPv6 loopback `::1` for the direct origin, which also strengthens the direct address-family proof; all final checks pass.

## Final Status

`Completed`

### Final Evidence

- **Final report status:** `Completed — P3-G1-VER1 through P3-G1-VER3 are fixed, every original checklist item is satisfied, all affected task/group checks pass, and no group implementation change remains uncommitted`
- **Final report commit:** `Returned to the orchestrator after this finalized report-only commit`
- **Final commits:** initial implementation `4b45788c5ce1d4dd2b3490d9accd1fec36a66c1c`, `bcf23f409e23294c84565282be19bca2d627375f`, `3568b1d6c77c651e70f38d4742635aeda9abbdfc`; remediation `f91bfa6c43ae98b98680bbbff459cbbb42aa8f3c`, `ec3f1cbb49fdeb08de40113c4ca82af4706f3757`, `9547716b301675244c1ac967088f57832128823a`
- **Final checks:** `cargo test ... proxy` passed 21/21; DNS passed 10/10; TLS passed 9/9; workspace passed 55/55; format, Clippy with denied warnings, all-target/all-feature check, build, safety/flow inspection, complete initial-plus-remediation range review, commit scope, and working-tree isolation passed; documentation tests remain not applicable because the package has no library target
- **Remaining issues:** `None within P3-G1 scope; the verification report's named compiled-process/listener/logging observations remain in their explicitly planned P3-G2 and Part 4 owners`
