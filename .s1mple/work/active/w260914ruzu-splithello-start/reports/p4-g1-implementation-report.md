# P4-G1 Implementation Report

## Scope

- **Goal:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-goal.md`
- **Plan Part:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-4-integration-documentation.md`
- **Group:** `P4-G1 — deterministic-binary-integration`
- **Tasks:** `P4-G1-T1`, `P4-G1-T2`, `P4-G1-T3`
- **Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-implementation-report.md`

Report commit SHAs are returned to the orchestrator and recorded in the execution goal; they are not written into this file because doing so would require a self-referential follow-up commit.

## Initial Implementation

Written by the implementer after the initial implementation turn and before returning to the orchestrator.

### Status

`Completed`

### Tasks

| Task | Changes | Verification | Commit evidence | Result |
|---|---|---|---|---|
| `P4-G1-T1 — build-authenticated-loopback-test-harness` | `tests/support/mod.rs`: generated CA and CA-signed localhost certificate, authenticated HTTPS DoH with configurable valid/raw/HTTP outcomes and query recording, negative untrusted-client proof, IPv4/conditional-IPv6 origins, compiled child with exact `start --config` arguments and temporary validated config, readiness/output capture, CONNECT/PAC clients, signal control, hard timeouts, and Drop cleanup; `tests/start_proxy.rs`: integration target registration | RED: `cargo test --test start_proxy` first failed because the target/harness did not exist; GREEN: authenticated harness self-test passed 1/1 and Clippy with warnings denied passed | `dc846084eb76edc0b3cae53f8b37436d71b1b099` | `Completed` |
| `P4-G1-T2 — prove-direct-selected-and-failure-isolation` | `tests/start_proxy.rs:v14_*`, `v15_*`, `v16_*`; `tests/support/mod.rs`: deterministic approved-port origin serialization — compiled binary proves unchanged direct IPv4 and conditional IPv6 relay with unused DoH, selected authenticated A-query plus two-record exact ClientHello reassembly and opaque suffix relay, and malformed selected failure isolated from queued direct success, listener reuse, and configured one-handler bound | `cargo test --test start_proxy` passed the harness plus V14–V16; `cargo test --workspace` passed 67 tests; Clippy passed with warnings denied | `600a73f31dc11c97ebca68a495640299393ea0b2` | `Completed` |
| `P4-G1-T3 — prove-shutdown-readiness-pac-and-secret-safe-outcomes` | `tests/start_proxy.rs:v17_*`, `v18_*`, `v19_*`; `tests/support/mod.rs`: raw authenticated DoH failure response, output/temp-dir accessors, connection-count observation — active-tunnel SIGTERM exits before deadline and closes proxy/PAC/active stream, exact child command reaches stable ready metadata and serves target-only PAC, and malformed authenticated DoH response emits `dns-error` plus canonical host without injected config/header secrets, mixed-case raw host, raw DNS bytes, ClientHello bytes, or origin connection | `cargo test --test start_proxy` passed 7/7; `cargo test --workspace` passed 70/70; Clippy passed with warnings denied | `c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7` | `Completed` |

### Group Change Range

- **Starting commit:** `85db54ec63ad8db354c48a676d5e47a852c1b8ce`
- **Final commit:** `c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7`
- **Remaining group changes:** `None`; the pre-existing orchestrator-owned unstaged goal update remains preserved and excluded.

### Group Verification

- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --test start_proxy` → `Passed` — 7 tests passed: authenticated trust self-test plus V14–V19.
- `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --workspace` → `Passed` — 70 tests passed across unit and compiled-binary integration suites.
- `cargo fmt --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -- --check` → `Passed` — no formatting diff.
- `cargo clippy --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features -- -D warnings` → `Passed` — no warnings.
- `cargo build --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml` → `Passed` — binary built successfully.
- `cargo check --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features` → `Passed` — all targets and features compiled.
- Authentication inspection of `tests/support/mod.rs`, `src/dns.rs`, and `Cargo.toml` plus focused search → `Passed` — the generated CA is added through `dns.ca_certificate`; the untrusted reqwest client fails; the trusted client succeeds; production uses rustls, `https_only`, `no_proxy`, no redirects, and additive `tls_certs_merge`; no invalid-certificate bypass or native-TLS surface exists.
- Direct/selected boundary inspection → `Passed` — V14 records no DoH query and conditionally proves `::1`; V15 records one A query and validates two output TLS records with the exact input handshake and opaque suffix; selected production resolution remains IPv4-only while direct production lookup retains OS `SocketAddr` families.
- Cleanup/isolation inspection → `Passed` — child Drop kills and waits as fallback, socket/process waits have deadlines, local server tasks abort on Drop, signals are explicit, no public Internet/root/system settings are used, and `git status --short` contains only the preserved unstaged goal file.
- Complete group diff `git diff --check 85db54ec63ad8db354c48a676d5e47a852c1b8ce..c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7` and commit inspection → `Passed` — only `tests/start_proxy.rs` and `tests/support/mod.rs` were added; task bodies use the exact planned titles.

### Deviations and Remaining Issues

- No Cargo dependency change was needed: the planned `rcgen` and `tokio-rustls` dev dependencies were already present and sufficient, so `Cargo.toml` and Cargo-generated `Cargo.lock` remained unchanged.
- Config permits only ports 443 and 8443, so deterministic origins use approved port 8443 and integration cases serialize origin binding through a Tokio mutex. IPv6 remains conditional as planned. No runtime contract was changed.
- Intermediate RED/fixture failures included the initially absent integration target, rcgen API/compiler corrections, rustls provider installation, use of unapproved ephemeral origin ports, and a missing final CONNECT header terminator in V19. Each was corrected before its task commit; final checks above pass.
- No implementation deviation or unresolved P4-G1 issue remains.

## Remediation

Append remediation evidence here after verifier findings. This is not a separate report. Preserve the Initial Implementation section unchanged.

- **Verification reports:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-verification-report.md` committed at `6926ad120347689a6602e3ccce032d318063bcff`
- **Status:** `Completed`

### Finding Closure

| Finding | Change and verification evidence | Commit evidence | Result |
|---|---|---|---|
| `P4-G1-VER1` | `src/config.rs:Config::validate` permits two loopback `:0` listeners to request distinct kernel-assigned addresses and scopes nonstandard CONNECT ports to that explicit ephemeral-listener mode; normal nonzero production listeners still reject ports outside 443/8443. `src/app.rs:Runtime::start` carries the validated mode into `ProxyState`; `src/proxy.rs:parse_connect_with_ephemeral_ports` preserves the normal parser contract while allowing the exact dynamically assigned test origin port only in ephemeral mode. `tests/support/mod.rs:RunningSplitHello::start` now writes loopback `:0`, waits for ready, parses actual bound proxy/PAC addresses, and has no reserve/drop/rebind window. `TestOrigin` binds IPv4/conditional IPv6 port 0 without a global 8443 dependency or serialization lock. `tests/start_proxy.rs:kernel_assigned_proxy_pac_and_origin_ports_are_distinct_and_held` repeats startup eight times, asserts distinct actual proxy/PAC addresses and that all three listeners remain kernel-held; the complete integration suite was then repeated five times. Focused config passed 6/6, integration passed 8/8, workspace passed 72/72, and all quality/security/scope checks passed. | `f54e841b538e304786784d3528426a2530f7d330` — `P4-G1-T2`, `P4-G1-T3` | `Fixed` |

**P4-G1-VER1 Completion Checklist:**

- [x] Required resolution is implemented — proxy/PAC bind directly to kernel-assigned port 0 and actual ready addresses are consumed; origins use independent kernel-assigned ports.
- [x] Required proof passes — the focused allocation regression repeats eight starts, five complete integration-suite stress runs pass, and integration/workspace checks pass.
- [x] Change evidence is recorded — exact production and harness symbols plus contract-preserving scope are listed above.
- [x] Verification evidence is recorded — focused, repeated, full, quality, authentication, address-family, cleanup, and scope checks are listed below.

## Final Status

`Completed`

### Final Evidence

- **Final report status:** `Completed — P4-G1-VER1 is fixed, its checklist is satisfied, all affected task/group checks pass, and no group implementation change remains uncommitted`
- **Final report commit:** `Returned to the orchestrator after this finalized report-only commit`
- **Final commits:** initial implementation `dc846084eb76edc0b3cae53f8b37436d71b1b099`, `600a73f31dc11c97ebca68a495640299393ea0b2`, `c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7`; remediation `f54e841b538e304786784d3528426a2530f7d330`
- **Final checks:** `cargo test --test start_proxy` passed 8/8; five repeated complete integration runs passed 8/8 each; focused config passed 6/6; workspace passed 72/72; format, Clippy with denied warnings, build, all-target/all-feature check, additive-CA authentication, selected/direct address-family boundaries, cleanup/process isolation, production safe-port preservation, complete group/remediation diff, and commit scope passed
- **Remaining issues:** `None`

## Overall Remediation — OVR-VER1

- **Overall verification report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/overall-verification-report.md` committed at `8d05e8e36796529af117fbc7991dfd55a8bed33e`
- **Status:** `Completed`

### Finding Closure

| Finding | Change and verification evidence | Commit evidence | Result |
|---|---|---|---|
| `OVR-VER1 — Port-0 mode bypasses the configured CONNECT-port allowlist` | `src/proxy.rs:parse_connect` now has one invariant for every listener mode: the CONNECT authority port must be a member of the validated `allowed_ports` set. The blanket `parse_connect_with_ephemeral_ports` bypass and runtime flag propagation were removed from `src/proxy.rs:ProxyState` and `src/app.rs:Runtime::start`. `src/config.rs:validate_allowed_ports` still permits a strictly explicit nonstandard origin port only when the loopback proxy listener requests kernel assignment with `:0`; normal nonzero listener configs remain limited to 443/8443. `src/config.rs:ephemeral_loopback_listener_mode_is_scoped_and_keeps_production_ports_safe` proves the exact dynamic port materializes while the same production port is rejected. `src/proxy.rs:explicitly_configured_dynamic_port_does_not_allow_other_ports` proves that exact dynamic port parses and another unconfigured port returns exact `ProxyError::PortNotAllowed` with status 403. `tests/start_proxy.rs:ephemeral_listener_runtime_enforces_exact_dynamic_allowed_port` starts the compiled binary with proxy `:0`, receives exact HTTP 403 for an unconfigured authority port, and proves neither DoH nor origin is reached. | `7ee9ddb9adae488af633be5dcbb175223ae051f4`, lint follow-up `193ec9575da47ebae17c3e23413ea6860cd342d5` | `Fixed` |

**OVR-VER1 Completion Checklist:**

- [x] CONNECT authority membership in validated `allowed_ports` is unconditional in parser and runtime paths.
- [x] Loopback listener `:0` can still validate one explicitly listed kernel-assigned nonstandard origin port.
- [x] Normal nonzero listener configuration remains limited to approved 443/8443 values.
- [x] Exact parser proof accepts the configured dynamic port and rejects a different port as `ProxyError::PortNotAllowed`/403.
- [x] Exact compiled-runtime proof returns HTTP 403 and performs no DoH/origin work for the unconfigured port.
- [x] Affected P4-G1 and overall checks pass.

### Overall Remediation Verification

- Focused config scope: `cargo test config::tests::ephemeral_loopback_listener_mode_is_scoped_and_keeps_production_ports_safe -- --exact` → `Passed` — 1/1.
- Focused parser: `cargo test proxy::tests::explicitly_configured_dynamic_port_does_not_allow_other_ports -- --exact` → `Passed` — 1/1 with exact `PortNotAllowed`/403 assertion.
- Focused compiled runtime: `cargo test --test start_proxy ephemeral_listener_runtime_enforces_exact_dynamic_allowed_port -- --exact` → `Passed` — 1/1 with exact HTTP 403 and no DoH/origin access.
- P4-G1 integration: `cargo test --test start_proxy` → `Passed` — 9/9.
- Workspace regression: `cargo test --workspace` → `Passed` — 66 unit/component plus 9 integration tests, 75 total.
- Quality: `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo build` → `Passed` — no formatting diff, no warnings, build exit 0.
- CLI: `cargo run -- --help`; `cargo run -- start --help` → `Passed` — exact `start --config <PATH>` surface and no packet controls.
- Security/scope inspection: no invalid-certificate bypass, native TLS/OpenSSL, `unsafe`, raw socket, firewall/system mutation, selected-target IPv6, or QUIC implementation; the only QUIC/HTTP3 and `iptables` text is the required README non-goal/V20 assertion.
- Isolation: `git diff --check`, remediation range diff check, commit path inspection, and `git status --short` → `Passed` — only the preserved orchestrator-owned unstaged goal remains.

### Overall Remediation Final Evidence

- **Status:** `Completed — OVR-VER1 is fixed and all checklist items pass`
- **Implementation commits:** `7ee9ddb9adae488af633be5dcbb175223ae051f4`, `193ec9575da47ebae17c3e23413ea6860cd342d5`
- **Remaining issues:** `None`
