# P1-G1 Verification Report

## Scope

- **Group:** `P1-G1`
- **Tasks:** `P1-G1-T1`, `P1-G1-T2`, `P1-G1-T3`
- **Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g1-verification-report.md`
- **Implementation Report:** `/Users/hieunguyen/git/hieu/projects/splithello/.s1mple/work/active/w260914ruzu-splithello-start/reports/p1-g1-implementation-report.md`

## Status

`Failed`

## Assigned Traceability

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES7`
- **Success Criteria:** `S1`, `S2`, `S5`, `S12`, `S17`
- **Verification Cases:** `V1`, `V2`, `V3`, `V4`, `V20`, `V21`

## Specification Verification

| Requirement Revisions | Design IDs | Success Criteria | Verification Cases | Planned outcome | Implementation evidence | Result |
|---|---|---|---|---|---|---|
| `W260914RUZU-REQ1@R1` | `W260914RUZU-DES2` | `S1`, `S2` | `V1`, `V2`, `V20` | Require `splithello start --config <PATH>` and convert strict TOML into validated values before any network/runtime side effect | `src/cli.rs:Cli`, `src/cli.rs:Command`, `src/cli.rs:StartArgs`; `src/main.rs:run`; `src/config.rs:Config::load` | `Mismatch` — implementation behavior and ordering are present, but `V2` does not assert the required exact error categories for its invalid-input matrix |
| `W260914RUZU-REQ3@R1` | `W260914RUZU-DES2` | `S5` | `V3`, `V4` | Canonical IDNA-aware, case-insensitive, one-trailing-dot, complete-label target matching with explicit subdomain policy and canonical duplicate rejection | `src/domain.rs:DomainName::parse`; `src/domain.rs:DomainName::is_subdomain_of`; `src/domain.rs:TargetMatcher::new`; `src/domain.rs:TargetMatcher::find`; `src/config.rs:validate_targets` | `Satisfied` |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | `S2`, `S12` | `V1`, `V2` | Validate HTTPS DoH configuration, bounded DNS settings, and optional additive PEM trust material before later resolver construction | `src/config.rs:validate_doh_url`; `src/config.rs:load_ca_certificate`; `src/config.rs:DnsConfig`; `src/config.rs:Config::validate` | `Satisfied` |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES2`, `W260914RUZU-DES3` | `S2`, `S12` | `V2` | Reject non-loopback listeners, disallowed/duplicate CONNECT ports, conflicting listeners, zero or excessive bounds, and any invalid-certificate bypass surface | `src/config.rs:parse_loopback_listener`; `src/config.rs:validate_allowed_ports`; `src/config.rs:bounded_usize`; `src/config.rs:duration`; focused source/dependency scan | `Satisfied` for implemented behavior; verification coverage mismatch is recorded under `P1-G1-VER1` |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | `S17` | `V1`, `V2`, `V3`, `V4`, `V20`, `V21` | Deterministic focused tests and standard Rust checks protect the foundation contracts | `src/config.rs` tests; `src/domain.rs` tests; `src/cli.rs` tests; Cargo quality commands | `Mismatch` — all commands pass, but the required exact-error assertions in `V2` are missing |

## Technical Verification

| Verification Case or Check | Success Criteria | Design IDs | Command or method | Actual evidence | Result |
|---|---|---|---|---|---|
| `V1` | `S1`, `S12` | `W260914RUZU-DES2`, `W260914RUZU-DES3` | `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml config` plus inspection of `representative_config_loads_and_materializes_defaults` | Exit 0; 5 passed, 0 failed. The test materialized proxy/PAC listeners, default port, DoH URL, CA absence, DNS limits, runtime limits, and two target entries | `Passed` |
| `V2` | `S2`, `S12` | `W260914RUZU-DES2`, `W260914RUZU-DES3` | Same focused config command plus inspection of `rejects_unknown_missing_and_unsafe_values` and `validates_additional_ca_pem_during_loading` | Exit 0 and every invalid fixture is rejected. However, 15 matrix cases assert only `is_err()` rather than the required exact `ConfigError`; only the CA cases assert variants. `src/main.rs:17-24` loads config before any runtime construction, and no listener module is wired in this group | `Insufficient` |
| `V3` | `S5` | `W260914RUZU-DES2` | `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml domain` | Exit 0; 5 passed, 0 failed. Exact Reddit and enabled subdomain matches pass; `notreddit.com` and `reddit.com.example` do not match | `Passed` |
| `V4` | `S5` | `W260914RUZU-DES2` | Same focused domain command plus config duplicate inspection | Case, trailing-dot, and IDNA equivalents canonicalize identically; empty/invalid labels, IP literals, and canonical duplicates are rejected; most-specific and exact-only policies pass | `Passed` |
| `V20` — P1-G1-owned portion | `S1`, `S2`, `S17` | `W260914RUZU-DES2`, `W260914RUZU-DES7` | Inspect production `Config::load`, CLI help, and the approved cross-part schedule | Production parser is the path used by `src/main.rs:21`; both help commands expose the exact required syntax. `README.md` and `config.example.toml` are absent because their creation and full consistency proof are explicitly assigned to Part 4, not this group's file scope | `Passed` |
| `V21` | `S17` | `W260914RUZU-DES7` | `cargo fmt --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -- --check`; `cargo clippy --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features -- -D warnings`; `cargo build --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml`; `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --workspace` | Every command exited 0; clippy emitted no warnings; build succeeded; workspace suite passed 10 tests with 0 failures | `Passed` |
| CLI focused tests | `S1` | `W260914RUZU-DES2` | `cargo test --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml cli` | Exit 0; 2 passed, 0 failed; missing `--config` yields Clap `MissingRequiredArgument`, and the exact accepted argument shape materializes the path | `Passed` |
| CLI help | `S1` | `W260914RUZU-DES2` | `cargo run --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -- --help`; `cargo run --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -- start --help` | Both exited 0; root help exposes only `start` and standard help; start usage is exactly `splithello start --config <PATH>` with no packet-tuning flags | `Passed` |
| Dependency, feature, and license review | `S12`, `S17` | `W260914RUZU-DES3`, `W260914RUZU-DES7` | `cargo tree --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml -e features`; `cargo metadata --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --format-version 1`; inspect `Cargo.toml` and active tree | Reqwest defaults are disabled and active features are `http2`, `rustls`, and `stream`; active tree contains rustls/hyper-rustls and no native-tls, hyper-tls, system-proxy, client-proxy-system, or Quinn/QUIC path. Every direct runtime/dev dependency reports MIT or MIT/Apache-2.0 licensing | `Passed` |
| All-target group build | `S17` | `W260914RUZU-DES7` | `cargo check --manifest-path /Users/hieunguyen/git/hieu/projects/splithello/Cargo.toml --all-targets --all-features` | Exit 0 | `Passed` |
| Safety and excluded-surface inspection | `S2`, `S12` | `W260914RUZU-DES2`, `W260914RUZU-DES3` | Focused source/manifest scan and manual inspection | No Rust `unsafe`, invalid-certificate acceptance API, native-TLS/system-proxy feature, transparent interception, raw-socket, firewall, NFQUEUE/eBPF, TUN/TAP, or system-setting mutation surface exists. `dns.ca_certificate` only reads and validates a non-empty PEM certificate bundle for later additive trust use | `Passed` |
| Strict pre-side-effect ordering | `S2` | `W260914RUZU-DES2` | Inspect `src/main.rs:17-24` and full implementation range | `Cli::parse` → `Config::load` → return; no listener/upstream/runtime construction exists in this group, so invalid config cannot reach a network side effect | `Passed` |
| rust-analyzer unlinked-file diagnostic | `S17` | `W260914RUZU-DES7` | Inspect module wiring and build evidence | `src/main.rs:1` declares `mod cli;`; Git tracks `src/cli.rs`; all-target check, clippy, build, and CLI tests compile and execute `cli.rs`. The harness diagnostic is not a real group issue | `Passed` |
| Change-range and working-tree isolation | `None` | `None` | Inspect `cb62cefc4ddca23d01fd521050d3ef0a5f3e87a5..c8676aeb60337545a419fea9470251752b09cc2a`, task commits, `git diff --check`, and `git status --short` | Implementation range contains only `Cargo.toml`, Cargo-generated `Cargo.lock`, `src/main.rs`, `src/cli.rs`, `src/config.rs`, and `src/domain.rs`; whitespace check passed. The only pre-existing working-tree change is the orchestrator-owned unstaged goal update | `Passed` |

## Findings

### P1-G1-VER1 — Major — Test Coverage — V2 does not assert exact configuration error outcomes

- **Requirement Revisions:** `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ9@R1`
- **Design IDs:** `W260914RUZU-DES2`, `W260914RUZU-DES7`
- **Success Criteria:** `S2`, `S12`, `S17`
- **Verification Cases:** `V2`
- **Evidence:** `src/config.rs:487-515` builds a 15-case invalid-input matrix but checks only `Config::parse(&invalid, directory.path()).is_err()`. The implementation defines distinct observable categories at `src/config.rs:73-120`, and the governing `V2` contract requires unknown/missing/unsafe fields, listeners, CA, strategy, duplicates, and bounds to return exact errors. Only the separate CA assertions at `src/config.rs:518-542` verify concrete variants.
- **Impact:** A regression that maps a malformed or unsafe configuration to the wrong error category would still pass the suite. This weakens the required stable, actionable startup boundary and leaves material validation branches unprotected even though the command-level checks are green.
- **Required resolution:** Convert the invalid-input matrix to pair each fixture with its expected `ConfigError` variant or another precise stable discriminator, covering unknown/missing fields, malformed and non-loopback/conflicting listeners, insecure/credential-bearing DoH URLs, unsupported strategy, canonical duplicates, empty targets, disallowed/duplicate ports, and zero/excessive bounds.
- **Required proof:** Run the focused config tests and `cargo test --workspace`; both must pass with assertions that fail when any covered fixture returns the wrong error category.

**Completion Checklist:**

- [ ] Required resolution is implemented.
- [ ] Required proof passes.
- [ ] Change evidence is recorded.
- [ ] Verification evidence is recorded.

## Test Coverage

- **Covered:** Required CLI parsing; representative config defaults and validated values; invalid-config rejection; additive CA file read/PEM rejection; domain case/trailing-dot/IDNA normalization; IP/invalid-label rejection; canonical duplicate rejection; exact/subdomain/adversarial suffix behavior; most-specific and exact-only matching; standard format/lint/build/workspace checks.
- **Missing:** Exact `ConfigError` assertions for the full `V2` invalid-input matrix, as detailed in `P1-G1-VER1`.

## Scope Changes

None. The implementation range matches the approved P1-G1 files. `README.md`, `config.example.toml`, listeners, DoH transport, TLS framing, relay, and process integration remain in their approved later groups.

## Unverified Items

None. Full ready-state and repository README/example consistency are later cross-part obligations; the P1-G1-owned CLI and production-parser portion of `V20` was verified here.

## Blockers

None.
