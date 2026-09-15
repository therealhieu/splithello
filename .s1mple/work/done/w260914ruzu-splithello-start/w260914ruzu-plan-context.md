# W260914RUZU Plan Context

## Status

`Ready`

## 1. Sources

| Source | Path | Use |
|---|---|---|
| s1mple instructions | `.s1mple/S1MPLE.md` | Plan behavior plus implementation, verification, and delivery constraints |
| Project document | `.s1mple/docs/project.md` | Minimal Rust-binary baseline, current toolchain, and standard checks |
| System design | `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-system-design.md` | Primary approved architecture, code shapes, flows, success criteria, and verification cases |
| Requirements | `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-requirements.md` | Nine current `R1` behavior, safety, scope, and verification obligations |
| Research | Not needed | Formal research was skipped; the approved design already records the protocol and current-library evidence needed for planning |
| Discussion context | `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-discuss-context.md` | Authorized local-use goal, clean-room rule, initial Reddit/Medium scope, and version-1 exclusions |
| Roadmap sources | Not applicable | Standalone work with no parent binding or cross-work assignment |
| Additional project files | `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `.gitignore` | Confirm the current one-package placeholder state and exact existing code surface |

## 2. System-Design Center

### Approved Direction

Replace the placeholder with one unprivileged Rust binary that starts through `splithello start --config <path>`, validates strict TOML before side effects, and exposes a loopback HTTP CONNECT proxy plus optional PAC endpoint. Configured targets use authenticated, target-only DNS-over-HTTPS to obtain IPv4 addresses and a bounded, byte-preserving TLS ClientHello record-framing transform; unselected hosts use normal operating-system IPv4/IPv6 resolution and unchanged relay. Tokio owns concurrency, cancellation, and graceful shutdown; the process never installs a certificate authority, terminates origin TLS, changes firewall/system resolver state, or requires root.

### Design Decisions

| Design decision | Planning meaning | Affected boundaries | Success criterion |
|---|---|---|---|
| `W260914RUZU-DES1` | Build the explicit loopback CONNECT service and optional PAC endpoint; acknowledge CONNECT only after upstream readiness | `src/app.rs`, `src/proxy.rs`, `src/pac.rs`; browser-to-proxy boundary | `S3`, `S4`, `S11`, `S12` |
| `W260914RUZU-DES2` | Implement clap command types, strict Serde/TOML raw-to-validated conversion, hard ceilings, and canonical label-aware target rules | `src/cli.rs`, `src/config.rs`, `src/domain.rs`, `config.example.toml` | `S1`, `S2`, `S5` |
| `W260914RUZU-DES3` | Implement selected-target A-record DoH through reqwest/rustls and hickory-proto, an optional additive DoH CA path, and a bounded TTL cache; keep direct OS resolution separate | `src/dns.rs`, `src/config.rs`; DoH and OS resolver boundaries | `S6`, `S7` |
| `W260914RUZU-DES4` | Implement a narrow safe parser that reassembles one bounded ClientHello, validates SNI, and serializes identical handshake bytes across two valid TLS records split inside SNI | `src/tls.rs`, selected relay in `src/proxy.rs` | `S8`, `S9`, `S10`, `S11` |
| `W260914RUZU-DES5` | Track service and tunnel tasks, bound concurrency and I/O, isolate tunnel failures, preserve direct relay, and drain then abort/join on cancellation | `src/app.rs`, `src/proxy.rs`, `src/dns.rs` | `S12`, `S13`, `S14` |
| `W260914RUZU-DES6` | Define stable secret-safe outcome/report contracts and usable setup, diagnostics, limitations, and rollback documentation | `src/diagnostics.rs`, runtime emitters, `README.md` | `S15`, `S16` |
| `W260914RUZU-DES7` | Use unit tests for pure contracts, one deterministic loopback binary integration suite for collaboration/lifecycle, and standard Rust checks | module tests, `tests/start_proxy.rs`, `tests/support/mod.rs` | `S17` |

### Runtime Flow

```text
splithello start --config <path>
    → parse required command
    → load, canonicalize, and validate complete TOML
    → construct matcher, selected DoH resolver/cache, limits, and cancellation
    → bind loopback CONNECT and optional PAC listeners atomically
    → report ready and setup/scope
    → accept bounded independent CONNECT tunnels
         ├─ unselected host
         │    → OS IPv4/IPv6 candidates
         │    → connect → 200 → unchanged bidirectional relay
         └─ selected host
              → authenticated DoH A candidates
              → IPv4 connect → 200
              → buffer one bounded ClientHello
              → validate CONNECT host equals SNI
              → rewrite TLS record framing with identical handshake bytes
              → unchanged bidirectional relay
    → SIGINT/SIGTERM
    → cancel accept/tunnel work → drain to deadline → abort and join remaining tasks
```

### Roadmap Validation

None — standalone work.

### Cross-Work Dependency Gates

None.

## 3. Binding Constraints

| Constraint | Source | Planning implication |
|---|---|---|
| Base plans on project context, Rust rules, and approved work artifacts | `.s1mple/S1MPLE.md:51-55` | Every task must name its exact requirement revisions, design IDs, success criteria, verification cases, files, and dependencies |
| Implement only an approved plan and keep changes scoped | `.s1mple/S1MPLE.md:57-62` | Execution cannot add transparent interception, packet recipes, IPv6 target mitigation, QUIC, VPN behavior, or other excluded scope |
| Do not hand-edit `Cargo.lock` | `.s1mple/S1MPLE.md:59-61` | Dependency tasks modify `Cargo.toml` and regenerate the lockfile through Cargo |
| Run applicable checks and report exact commands/results | `.s1mple/S1MPLE.md:64-68` | Each task and group includes focused tests; overall acceptance includes format, clippy, build, and workspace tests |
| Add focused tests for new behavior | `.s1mple/S1MPLE.md:68` and `.s1mple/docs/languages/rust.md:105-121` | Plan parser/state work with TDD and reserve integration tests for real collaboration/lifecycle risks |
| Summarize files, behavior, evidence, and unresolved risk | `.s1mple/S1MPLE.md:70-74` | Execution reports and final goal must preserve commit SHAs, verification evidence, failures, and non-goals |
| Choose the simplest robust design and avoid speculative abstractions | `.s1mple/S1MPLE.md:15-17` | Keep one binary, crate-private cohesive modules, concrete adapters, one selected strategy, and no plugin framework |
| Prefer safe standard APIs and no `unsafe` when avoidable | `.s1mple/docs/languages/rust.md:98-103` | TLS framing, DNS, proxy, and lifecycle code use checked safe Rust only |
| Validate untrusted boundaries and avoid logging sensitive data | `.s1mple/docs/languages/rust.md:86-96` | Bound and validate TOML, CONNECT, domain, DNS, TLS, and log fields; errors remain structured |
| Add only maintained, licensed, complexity-reducing crates with explicit features | `.s1mple/docs/languages/rust.md:63-73` | Resolve exact current versions/licenses during dependency implementation; reqwest uses rustls and disabled default/system-proxy behavior |
| Clean-room implementation | `w260914ruzu-discuss-context.md:48` and `w260914ruzu-requirements.md:99-106` | Do not copy source, distinctive structure, text, or tests from the unlicensed reference repository |
| Version 1 target path is IPv4 TCP only, while direct traffic preserves normal OS families | `w260914ruzu-requirements.md:137-145` and reviewed system design | Separate `TargetResolver::resolve_ipv4` from `SystemResolver::resolve -> Vec<SocketAddr>` and test IPv6 direct relay where available |
| DoH authentication cannot be disabled | Reviewed system design sections 6-8 | Optional `dns.ca_certificate` only adds PEM roots; integration uses a generated CA/localhost certificate rather than accepting invalid certificates |

## 4. Supporting Evidence

### Requirements

| Requirement | Design coverage | Required outcome |
|---|---|---|
| `W260914RUZU-REQ1@R1` | `W260914RUZU-DES2` | Exact command, strict pre-side-effect validation, usable Reddit/Medium example, and concise help |
| `W260914RUZU-REQ2@R1` | `W260914RUZU-DES1` | Unprivileged loopback CONNECT and optional PAC; bounded errors; no automatic system mutation |
| `W260914RUZU-REQ3@R1` | `W260914RUZU-DES2` | Canonical label-aware target selection without suffix overmatch; direct non-target behavior |
| `W260914RUZU-REQ4@R1` | `W260914RUZU-DES3` | Selected targets use validated authenticated DoH A answers and bounded TTL caching, never OS DNS |
| `W260914RUZU-REQ5@R1` | `W260914RUZU-DES4` | Bounded ClientHello reassembly, matching SNI, valid SNI-internal record split, exact handshake-byte preservation, opaque subsequent relay |
| `W260914RUZU-REQ6@R1` | `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5` | End-to-end browser-origin TLS, loopback-only exposure, safe ports, secret-safe logs, and resource bounds |
| `W260914RUZU-REQ7@R1` | `W260914RUZU-DES5` | Direct fail-open relay, per-tunnel failure isolation, atomic startup, and bounded process-owned shutdown |
| `W260914RUZU-REQ8@R1` | `W260914RUZU-DES6` | Stable actionable startup/tunnel outcomes and honest unsupported-method documentation |
| `W260914RUZU-REQ9@R1` | `W260914RUZU-DES7` | Deterministic parser and loopback integration evidence plus passing standard Rust checks |

### Research Findings

Not needed — formal research was skipped because the approved requirements and reviewed system design already resolve the architecture and protocol boundaries. The design's current-library checks are implementation inputs, not an independent research artifact or revision source.

## 5. Project Implementation Context

### Relevant Structure

```text
./
├── Cargo.toml                 # existing minimal edition-2024 package
├── Cargo.lock                 # existing generated local-only resolution
├── .gitignore                # existing /target exclusion
├── config.example.toml       # new validated user contract
├── README.md                 # new setup, diagnostics, scope, rollback
├── src/
│   ├── main.rs               # replace placeholder composition root
│   ├── cli.rs                # new command types
│   ├── config.rs             # new raw/validated config and limits
│   ├── domain.rs             # new canonical hostname/matcher
│   ├── app.rs                # new construction/lifecycle/tasks
│   ├── proxy.rs              # new CONNECT and relay orchestration
│   ├── pac.rs                # new optional PAC service
│   ├── dns.rs                # new selected DoH/direct resolution
│   ├── tls.rs                # new bounded ClientHello framing transform
│   └── diagnostics.rs        # new stable outcomes/reports
└── tests/
    ├── start_proxy.rs        # new binary-level loopback scenarios
    └── support/mod.rs        # new authenticated DoH/origin/process harness
```

### Established Patterns

| Pattern | Evidence | Planning use |
|---|---|---|
| One Cargo binary package | `Cargo.toml:1-7` | Keep one package and crate-private modules rather than introducing workspace/crate boundaries |
| Rust edition 2024 | `Cargo.toml:4` | All source and selected dependency versions must compile under edition 2024 |
| Placeholder-only entrypoint | `src/main.rs:1-3` | No compatibility-preserving application behavior exists; replace `main` with the approved composition root |
| No current dependencies | `Cargo.toml:6-7` | Add only the explicit approved runtime/protocol/CLI/diagnostic set; document purpose and features |
| Generated lockfile | `Cargo.lock:1-8` | Use Cargo to update resolution and commit the generated result |
| Build output excluded | `.gitignore:1` | No additional generated artifacts belong in commits |

### Code Surfaces

| Path | Current symbol or wiring | Intended change | Evidence |
|---|---|---|---|
| `Cargo.toml` | Empty `[dependencies]` | Add clap, Serde/TOML, Tokio/tokio-util, reqwest/rustls, hickory-proto, IDNA, errors, tracing, and focused dev dependencies with narrow features | `Cargo.toml:1-7`; system design section 7 |
| `Cargo.lock` | Local package only | Cargo-generated approved dependency graph | `Cargo.lock:1-8`; system design section 7 |
| `src/main.rs` | `main` prints `Hello, world!` | Tokio entrypoint, tracing, CLI dispatch, config load, runtime lifecycle, structured exit | `src/main.rs:1-3`; system design `src/main.rs` entry |
| `src/cli.rs` | New | `Cli`, `Command`, `StartArgs` | system design section 7 |
| `src/config.rs` | New | Raw strict schema, validated `Config`, optional additive DoH CA, limits, errors | system design section 7 |
| `src/domain.rs` | New | `DomainName`, `TargetRule`, `TargetMatcher` | system design section 7 |
| `src/app.rs` | New | `Runtime::start`, signal wait, tracked service/tunnel shutdown | system design section 7 |
| `src/proxy.rs` | New | CONNECT parser/server, direct/target mode, socket candidate connection, relay | system design section 7 |
| `src/pac.rs` | New | deterministic target-only PAC generation and local serving | system design section 7 |
| `src/dns.rs` | New | `TargetResolver`, `SystemResolver`, `DohResolver`, `DnsCache`, response validation | system design section 7 |
| `src/tls.rs` | New | checked record/handshake parser, SNI validation, record serializer, explicit outcomes | system design section 7 |
| `src/diagnostics.rs` | New | stable `Outcome` and `TunnelReport` | system design section 7 |
| `README.md`, `config.example.toml` | New | executable user workflow and strict example | system design section 7 |
| `tests/start_proxy.rs`, `tests/support/mod.rs` | New | deterministic compiled-binary collaboration/lifecycle harness | system design sections 7-8 |

### Verification Surfaces

| Suite or check | Location or command | Required verification |
|---|---|---|
| Configuration unit tests | `src/config.rs` | `V1`, `V2`: example/defaults and complete invalid-input matrix before side effects |
| Domain unit tests | `src/domain.rs` | `V3`, `V4`: exact/subdomain/IDNA/case/trailing-dot/adversarial/duplicate behavior |
| CONNECT unit tests | `src/proxy.rs` | `V5`, `V6`: valid authority and bounded method/authority/port/header errors |
| PAC unit tests | `src/pac.rs` | `V7`: exact and subdomain targets proxy; lookalikes/direct hosts remain direct |
| DNS unit tests | `src/dns.rs` | `V8`-`V10`: valid A/CNAME handling, invalid response rejection, controlled cache expiry/capacity |
| TLS unit tests | `src/tls.rs` | `V11`-`V13`: legal input fragmentation, exact output preservation, malformed/bounds/policy outcomes, no panic |
| Binary integration | `tests/start_proxy.rs`, `tests/support/mod.rs` | `V14`-`V19`: direct IPv4/IPv6 relay, authenticated local DoH selected transform, isolation, shutdown, ready/PAC, secret-safe outcomes |
| Documentation/config consistency | `V20` inspection or focused test/script | Exact README command exists, example parses, exclusions remain present |
| Rust quality checks | `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo build`; `cargo test --workspace` | `V21`: all commands pass with exact results recorded |

## 6. Additional Inspections

| Planning question | Files inspected | Finding |
|---|---|---|
| Is there existing application architecture to preserve? | `src/main.rs`, `Cargo.toml`, `.s1mple/docs/project.md` | No; only a placeholder binary exists, so approved modules can be introduced without migration logic |
| Does deterministic DoH integration preserve authentication? | reviewed system design sections 6-8 | Yes; `dns.ca_certificate` adds a test CA root, while invalid-certificate acceptance remains unavailable |
| Does direct relay preserve unrelated address-family compatibility? | reviewed system design sections 2, 5-8 | Yes; direct resolution uses normal OS IPv4/IPv6 `SocketAddr` candidates; selected DoH mitigation remains IPv4-only |
| Is persistent rollback required? | requirements and system design migration/delivery sections | No; runtime owns sockets, tasks, memory, and optional browser configuration documentation only |
| Are protocol implementation boundaries sufficiently narrow? | system design detailed `dns.rs` and `tls.rs` shapes | Yes; hickory-proto owns DNS wire format, while internal TLS code is limited to bounded plaintext record/ClientHello framing and never cryptography |

## 7. Planning Gaps

None.

## 8. Planning Handoff

- **Scope:** Implement the complete one-binary loopback CONNECT/PAC product, strict configuration/domain contracts, selected authenticated DoH, direct normal OS resolution, bounded TLS record transformation, lifecycle/diagnostics, deterministic tests, example configuration, and user documentation.
- **Primary boundaries:** command/config/domain foundation; DNS and TLS pure/protocol contracts; proxy/PAC runtime and lifecycle; binary integration and documentation acceptance.
- **Likely sequence:** establish dependencies and pure validated contracts → implement/test DNS and TLS components → wire proxy/PAC/lifecycle/diagnostics → complete authenticated loopback integration, documentation, and overall checks.
- **Verification focus:** exact handshake-byte preservation; selected DoH without OS fallback; additive CA authentication; direct IPv4/IPv6 unchanged relay; strict pre-side-effect validation; bounded parser/resource behavior; per-tunnel isolation; tracked shutdown; stable secret-safe outcomes.
- **Excluded scope:** transparent interception, root/firewall/raw sockets, fake packets or packet recipes, VPN/anonymity/IP hiding, selected IPv6 mitigation, QUIC/HTTP3, system-setting mutation, public-network test gates, and copied unlicensed code.
- **Blocking items:** None.
