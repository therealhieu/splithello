# P4-G1 Verification Report

## Scope

- **Group:** `P4-G1 — deterministic-binary-integration`
- **Tasks:** `P4-G1-T1`, `P4-G1-T2`, `P4-G1-T3`
- **Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-verification-report.md`
- **Implementation Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p4-g1-implementation-report.md` at commit `06cc3a4372eafaa43feaf5f8b0d90cafb0db9137`

## Status

`Failed`

## Assigned Traceability

- **Requirement Revisions:** `W260914RUZU-REQ1@R1` through `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1` through `W260914RUZU-DES7`
- **Success Criteria:** `S1`–`S15`, `S17`
- **Verification Cases:** `V14`–`V19`

## Specification Verification

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Planned outcome | Implementation evidence | Result |
|---|---|---|---|---|---|---|
| `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES2` | `S1`–`S5` | `V14`, `V18` | The exact compiled-binary start command reaches ready, exposes loopback CONNECT/PAC, preserves unselected traffic, and serves target-only PAC policy using canonical target rules. | `tests/support/mod.rs:RunningSplitHello::start`; `tests/start_proxy.rs:v14_direct_ipv4_and_available_ipv6_relay_are_byte_identical`; `tests/start_proxy.rs:v18_exact_start_command_reaches_ready_and_serves_target_only_pac`; accepted Part 1/P3 reports | `Satisfied` for behavior; deterministic port ownership is mismatched under `P4-G1-VER1`. |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S6`, `S7` | `V15`, `V19` | Selected hosts use authenticated local HTTPS DoH A answers and the existing bounded cache contract, with no OS fallback or origin connection after DNS failure. | `tests/support/mod.rs:generated_tls_material`; `tests/support/mod.rs:authenticated_doh_requires_the_generated_ca`; `tests/start_proxy.rs:v15_selected_target_uses_authenticated_doh_and_preserves_client_hello`; `tests/start_proxy.rs:v19_doh_failure_emits_stable_secret_safe_diagnostic`; `src/dns.rs:DohResolver` | `Satisfied` |
| `W260914RUZU-REQ5@R1` | `W260914RUZU-DES4` | `S8`–`S10` | `V15`, `V16` | A selected ClientHello spanning input records is rewritten into two valid records split inside matching SNI while preserving the exact handshake; malformed SNI closes only its tunnel. | `tests/start_proxy.rs:108-135`; `tests/start_proxy.rs:171-180`; `tests/support/mod.rs:decode_handshake_records`; accepted P2-G2 report | `Satisfied` |
| `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5` | `S11`–`S14` | `V14`–`V17` | Origin TLS remains end-to-end and later bytes opaque; direct IPv4/available IPv6 relay is unchanged; failure, concurrency, listeners, active tunnels, and signal shutdown are bounded and process-owned. | `tests/start_proxy.rs:v14_*`; `v15_*`; `v16_*`; `v17_*`; `src/proxy.rs:ProxyServer`; `src/app.rs:Runtime`; substantive integration run | `Mismatch` — observable flows pass, but the harness releases selected proxy/PAC ports before the child binds and requires the globally shared fixed origin port 8443, so deterministic setup is not guaranteed. |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | `S15` | `V18`, `V19` | Ready output contains stable required fields; induced DoH failure reports `dns-error` with canonical host and no injected secret, request header, DNS payload, or ClientHello bytes. | `tests/start_proxy.rs:268-282`; `tests/start_proxy.rs:289-340`; `src/app.rs:ReadyMetadata::emit`; `src/diagnostics.rs:Outcome` | `Satisfied` |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V14`–`V19` | One default, no-Internet, no-root compiled-binary suite deterministically proves collaboration and lifecycle with reliable cleanup and deadlines. | `cargo test --test start_proxy` passed 7/7; `cargo test --workspace` passed 70/70; `tests/support/mod.rs` harness inspection | `Mismatch` — tests pass on this host, but port handoff and fixed-port assumptions leave a material flakiness gap; see `P4-G1-VER1`. |

## Technical Verification

| Verification Case or Check | Success Criteria | Design IDs | Command or method | Actual evidence | Result |
|---|---|---|---|---|---|
| `V14` | `S3`, `S5`, `S11`, `S13`, `S17` | `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES5`, `W260914RUZU-DES7` | `cargo test --test start_proxy`; inspect `v14_direct_ipv4_and_available_ipv6_relay_are_byte_identical` | Passed. IPv4 origin received `direct-ipv4` exactly and replied; DoH remained unused. Where `::1:8443` was available, the IPv6 origin received `direct-ipv6` exactly and DoH remained unused. | `Passed` |
| `V15` | `S6`–`S11`, `S17` | `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES7` | Same integration command; inspect generated TLS material, negative trust case, DoH query capture, and origin capture | Passed. The untrusted rustls client failed and the generated-CA client succeeded; the child used the CA path. DoH recorded exactly one `A selected.test.` query. The origin decoder consumed exactly two valid handshake records, reassembled bytes equal to the input handshake, and retained `opaque-tail` unchanged. | `Passed` |
| `V16` | `S11`–`S13`, `S17` | `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7` | Same integration command; inspect queued direct request, malformed selected input, terminal output, listener reuse, and peak observation | Passed. With `max_connections = 1`, the direct CONNECT remained pending while the selected handler held the permit; mismatched SNI produced `tls-error`, direct relay then completed, a later direct tunnel proved listener survival, and origin peak activity stayed at one. | `Passed` |
| `V17` | `S12`–`S14`, `S17` | `W260914RUZU-DES5`, `W260914RUZU-DES7` | Same integration command; inspect signal and cleanup assertions plus post-run process/listener inspection | Passed. SIGTERM during an active direct tunnel exited successfully in under three seconds for a one-second configured shutdown deadline; proxy/PAC refused new connections and the active stream closed. No child `splithello start --config` process or listener remained after the suite. | `Passed` |
| `V18` | `S1`, `S4`, `S15`, `S17` | `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | Same integration command; inspect `RunningSplitHello::start`, ready capture, and live PAC response | Passed. The compiled binary is invoked as `CARGO_BIN_EXE_splithello start --config <temp path>`. Ready output includes `outcome=ready`, actual proxy address, target count, DoH host, selected IPv4 transport, and explicit-proxy scope. Live PAC returned 200 with exact target/subdomain conditions, configured proxy address, and `DIRECT` fallback. | `Passed` |
| `V19` | `S15`, `S17` | `W260914RUZU-DES6`, `W260914RUZU-DES7` | Same integration command; induce authenticated malformed DNS response and inspect captured process output | Passed. Client received 502; output contained `outcome=dns-error` and `host=selected.test`, while excluding the injected config secret, raw injected header value, mixed-case raw host, `deadbeef`, and generated ClientHello bytes. The origin observed zero connections. | `Passed` |
| Binary integration | `S1`–`S15`, `S17` | `W260914RUZU-DES1`–`W260914RUZU-DES7` | `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --test start_proxy` | Exit 0; 7 passed, 0 failed, 0 ignored; completed in 1.03 seconds. | `Passed` |
| Workspace regression | `S17` | `W260914RUZU-DES7` | `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --workspace` | Exit 0; 63 unit tests plus 7 integration tests passed, 0 failed; no Internet or root operation was used. | `Passed` |
| DoH authentication and invalid-cert surface | `S6`, `S11`, `S12`, `S17` | `W260914RUZU-DES3`, `W260914RUZU-DES7` | Inspect `tests/support/mod.rs:514-667`, `src/dns.rs:373-394`, `Cargo.toml`; focused source/manifest search | Generated CA signs a localhost certificate; negative untrusted and positive additive-CA clients are explicit. Production uses rustls, HTTPS-only, no proxy, no redirects, explicit timeouts, and `tls_certs_merge`. No invalid-certificate acceptance or native-TLS surface was found. | `Passed` |
| Address-family and selected-resolution boundary | `S3`, `S6`, `S11`, `S13` | `W260914RUZU-DES1`, `W260914RUZU-DES3`, `W260914RUZU-DES5` | Inspect integration scenarios and `src/dns.rs:270-275,404-426`; `src/proxy.rs:583-618` | Selected resolver returns only `Ipv4Addr` and selected proxy wiring constructs IPv4 socket addresses. Direct lookup returns unfiltered OS `SocketAddr` candidates. V14 uses no DoH for direct IPv4/conditional IPv6. | `Passed` |
| Cleanup, deadlines, and port allocation quality | `S12`–`S14`, `S17` | `W260914RUZU-DES5`, `W260914RUZU-DES7` | Inspect all wait/drop/bind helpers; post-suite process/listener inspection | Socket/process waits are bounded, child Drop kills/waits, local top-level server tasks abort on Drop, and final cleanup succeeded. However, `reserve_ipv4_addr` drops its listener before child startup, proxy and PAC reservations are not held atomically, and every origin requires fixed port 8443. | `Insufficient` |
| Implementation range and isolation | `None` | `None` | `git diff --check 85db54ec63ad8db354c48a676d5e47a852c1b8ce..c14d4fc08f49d3eda78bb7403f4a7d6b4cd60df7`; range/commit/status inspection | Diff check passed. The range adds only `tests/start_proxy.rs` and `tests/support/mod.rs`; task commits match T1–T3. The only working-tree change before this report was the preserved orchestrator-owned goal update. | `Passed` |

## Findings

### P4-G1-VER1 — Major — Determinism and Test Infrastructure — Harness ports are released before child bind and require a globally shared fixed origin port

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES1`, `W260914RUZU-DES5`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S3`, `S12`–`S14`, `S17`
- **Verification Cases:** `V14`–`V19`
- **Evidence:** `tests/support/mod.rs:291-325` obtains proxy and PAC addresses from `reserve_ipv4_addr()` and then starts the child. `tests/support/mod.rs:616-620` binds an ephemeral `std::net::TcpListener`, reads its address, and drops it on function return, so neither port remains reserved until the child binds. Consecutive proxy/PAC reservations can be reused or claimed by another process in that gap. Separately, `tests/support/mod.rs:27,190-207` binds every IPv4 origin to fixed `127.0.0.1:8443`; `origin_test_lock` serializes only this test binary and cannot prevent another local process from occupying that port. The suite passed in this run, but the plan explicitly requires deterministic addresses/cleanup and review for flaky port allocation.
- **Impact:** Default tests can fail nondeterministically with child `connect-error`, proxy/PAC listener conflict, or `bind test origin` even when production behavior is correct. This weakens the required reproducible no-Infrastructure integration evidence and can produce false failures in CI or developer environments where 8443 is occupied.
- **Required resolution:** Make integration port ownership race-free. Keep proxy/PAC reservations held until the child can inherit or atomically bind them, or add a test-only startup mechanism that reports kernel-assigned listener addresses without a release/rebind window. Remove the global external dependency on fixed port 8443, or use an equally race-free scoped mechanism that preserves the production allowed-port contract. Keep all production security and configuration constraints unchanged.
- **Required proof:** Add a deterministic harness regression that proves proxy and PAC cannot receive the same reservation and cannot be stolen before child readiness; prove integration origins do not depend on an externally free global 8443 port. Run `cargo test --test start_proxy` and `cargo test --workspace` successfully, including repeated or parallel stress sufficient to exercise the corrected allocation path.

**Completion Checklist:**

- [ ] Required resolution is implemented.
- [ ] Required proof passes.
- [ ] Change evidence is recorded.
- [ ] Verification evidence is recorded.

## Test Coverage

- **Covered:** Generated CA and CA-signed localhost DoH; negative untrusted and positive additive-CA authentication; exact compiled child arguments; direct IPv4 and conditional IPv6 unchanged relay with target path unused; selected A query and exact two-record ClientHello reassembly; opaque suffix relay; malformed selected failure, queued direct success, listener survival, and configured one-handler behavior; active-tunnel SIGTERM shutdown; listener closure; ready metadata and live PAC policy; stable secret-safe DNS failure outcome; bounded waits and Drop cleanup; full workspace regression.
- **Missing:** Race-free proxy/PAC port handoff and isolation from an externally occupied fixed origin port, as detailed in `P4-G1-VER1`. The V19 ClientHello absence assertion is necessarily secondary to source/report-shape inspection because the induced DNS failure occurs before a ClientHello can enter the selected tunnel.

## Scope Changes

None. The implementation range adds only the planned binary integration and support files. No production behavior, dependency, configuration, or documentation file changed.

## Unverified Items

None beyond the deterministic port-allocation proof identified as a finding. IPv6 relay was conditional as planned and executed successfully on this host.

## Blockers

None. The substantive verification pass completed. The verdict is `Failed` because `P4-G1-VER1` leaves the deterministic, non-flaky harness requirement unresolved.
