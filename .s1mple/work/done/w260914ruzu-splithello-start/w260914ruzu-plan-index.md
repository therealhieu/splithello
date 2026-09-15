# W260914RUZU Implementation Plan Index

## Status

`Ready`

## Sources

- Plan context: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-plan-context.md`
- System design: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-system-design.md`
- Requirements: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-requirements.md`
- Research: Not needed — formal research was skipped
- Discussion context: `.s1mple/work/active/w260914ruzu-splithello-start/w260914ruzu-discuss-context.md`

## Roadmap Contract

None — standalone work.

## Plan Overview

Implement the approved product in four dependency-ordered parts. First establish the dependency, command, configuration, domain, PAC-generation, and diagnostic contracts. Next implement the two bounded protocol components—authenticated target DoH and byte-preserving ClientHello record rewriting—behind testable APIs. Then compose the CONNECT proxy and tracked runtime, first proving unchanged direct relay and then selected-target transformation, failure isolation, and shutdown. Finally add deterministic compiled-binary integration coverage, the executable example configuration, user documentation, and all repository-wide checks.

Every implementation task follows the repository's Rust standards and the approved `Part → Group → Task → Step` plan. Execution must remain clean-room, crate-private, safe Rust, and within the explicit proxy architecture. Each task uses TDD when it owns executable behavior and direct verification only for dependency declarations or documentation. Each task is checked, self-reviewed, and committed before dependent work begins. Group verification adds acceptance evidence but does not replace task commits. Delivery records exact verification and unresolved limitations; it does not push, deploy, or alter external/system settings.

## Execution Flow

```text
Plan 1: Foundation contracts
    → Plan 2: DNS and TLS protocols
    → Plan 3: Proxy runtime and lifecycle
    → Plan 4: Binary integration and documentation
    → Overall verification
```

## Plan Parts

| Part | File | Scope | Dependencies | Design decisions | Requirements | Verification focus |
|---|---|---|---|---|---|---|
| 1 | `w260914ruzu-plan-1-foundation-contracts.md` | Dependency graph, CLI, strict configuration, canonical domains, PAC rendering, and diagnostic vocabulary | None | `W260914RUZU-DES1`, `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | `W260914RUZU-REQ1@R1`, `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1` | `V1`–`V7`; config-before-side-effects, domain safety, PAC selection, CLI help, dependency/license/features |
| 2 | `w260914ruzu-plan-2-protocol-components.md` | Target-only DoH resolver/cache and bounded ClientHello parser/rewriter | Part 1 | `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES7` | `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ9@R1` | `V8`–`V13`; authenticated DoH validation/cache, checked TLS framing, exact handshake preservation |
| 3 | `w260914ruzu-plan-3-proxy-runtime.md` | CONNECT parsing/server, direct IPv4/IPv6 relay, selected-target orchestration, PAC serving, runtime construction, diagnostics, cancellation, and failure isolation | Parts 1–2 | `W260914RUZU-DES1`, `W260914RUZU-DES3`, `W260914RUZU-DES4`, `W260914RUZU-DES5`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | `W260914RUZU-REQ2@R1`, `W260914RUZU-REQ3@R1`, `W260914RUZU-REQ4@R1`, `W260914RUZU-REQ5@R1`, `W260914RUZU-REQ6@R1`, `W260914RUZU-REQ7@R1`, `W260914RUZU-REQ8@R1`, `W260914RUZU-REQ9@R1` | `V5`, `V6`, `V14`–`V19`; acknowledgement timing, unchanged direct relay, selected transform, isolation, bounded shutdown, stable logs |
| 4 | `w260914ruzu-plan-4-integration-documentation.md` | Authenticated local DoH/origin process harness, final binary scenarios, example config, README, consistency checks, and full Rust checks | Parts 1–3 | `W260914RUZU-DES2`, `W260914RUZU-DES3`, `W260914RUZU-DES6`, `W260914RUZU-DES7` | All current requirement revisions | `V14`–`V21`; real loopback collaboration, generated test CA, exact command, scope documentation, format/lint/build/test |

## Cross-Part Coordination

| Boundary | Producing part | Consuming part | Coordination requirement |
|---|---|---|---|
| Validated `Config`, `RuntimeLimits`, `TargetRule`, and `TargetMatcher` | Part 1 | Parts 2–4 | Consumers receive only validated canonical values; no repeated TOML parsing or alternate domain semantics |
| `Outcome` and `TunnelReport` vocabulary | Part 1 | Parts 2–4 | Errors map to stable categories without payloads or secrets; exact strings remain consistent with documentation/tests |
| `TargetResolver` contract and `DohResolver` | Part 2 | Parts 3–4 | Selected targets return validated IPv4 candidates only; optional CA adds trust roots and never disables authentication |
| `RewriteOutcome` and ClientHello byte-preservation contract | Part 2 | Parts 3–4 | Proxy sends no selected ClientHello bytes before one explicit rewrite/pass/reject decision |
| `ProxyServer`, `PacServer`, and `Runtime` readiness/shutdown | Part 3 | Part 4 | Integration harness waits for ready output, exercises real listeners, and observes bounded process exit |
| Configuration/test trust contract | Parts 1–2 | Part 4 | `dns.ca_certificate` accepts generated CA PEM for local authenticated DoH; `config.example.toml` omits it and uses normal web PKI |
| Address-family contract | Parts 2–3 | Part 4 | Selected DoH remains IPv4-only; unselected OS resolution preserves IPv4/IPv6 `SocketAddr` candidates and is tested where IPv6 loopback exists |

## Coverage

| Source item | Covered by parts | Expected outcome |
|---|---|---|
| `W260914RUZU-REQ1@R1` / `W260914RUZU-DES2` / `S1`, `S2` / `V1`, `V2`, `V18`, `V20` | 1, 4 | Exact command and strict example config reach ready; invalid config has no listener side effects |
| `W260914RUZU-REQ2@R1` / `W260914RUZU-DES1` / `S3`, `S4` / `V5`–`V7`, `V14`, `V18` | 1, 3, 4 | Loopback CONNECT/PAC works unprivileged; CONNECT waits for upstream; PAC scopes targets |
| `W260914RUZU-REQ3@R1` / `W260914RUZU-DES2` / `S5` / `V3`, `V4`, `V7` | 1, 3, 4 | Canonical label-aware matching never modifies suffix lookalikes |
| `W260914RUZU-REQ4@R1` / `W260914RUZU-DES3` / `S6`, `S7` / `V8`–`V10`, `V15` | 1, 2, 3, 4 | Selected targets use authenticated validated A-record DoH and bounded TTL cache without OS fallback |
| `W260914RUZU-REQ5@R1` / `W260914RUZU-DES4` / `S8`–`S10` / `V11`–`V13`, `V15` | 2, 3, 4 | One bounded ClientHello is recognized across records and rewritten with exact handshake-byte preservation |
| `W260914RUZU-REQ6@R1` / `W260914RUZU-DES1`, `W260914RUZU-DES4`, `W260914RUZU-DES5` / `S11`, `S12` / `V2`, `V6`, `V13`, `V16` | 1–4 | Origin TLS stays end-to-end; listeners/ports/buffers/time/concurrency are bounded and loopback-only |
| `W260914RUZU-REQ7@R1` / `W260914RUZU-DES5` / `S13`, `S14` / `V14`–`V17` | 1–4 | Direct traffic is unchanged, failures remain per-tunnel, startup is atomic, shutdown is bounded and process-owned |
| `W260914RUZU-REQ8@R1` / `W260914RUZU-DES6` / `S15`, `S16` / `V18`–`V20` | 1, 3, 4 | Ready/tunnel outcomes are stable, secret-safe, actionable, and limitations are documented |
| `W260914RUZU-REQ9@R1` / `W260914RUZU-DES7` / `S17` / `V1`–`V21` | 1–4 | Deterministic unit/integration evidence and all Rust quality checks pass |
| Clean-room, safe-Rust, explicit-proxy, no-root constraint | 1–4 | No copied source, `unsafe`, firewall/raw-socket/system-setting behavior, or out-of-scope strategy appears |

## Overall Verification

| Check | Command or method | Expected result |
|---|---|---|
| Format | `cargo fmt --check` | Exit 0 with no formatting diff |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0 with no warnings |
| Build | `cargo build` | Exit 0 for the edition-2024 binary |
| Unit and integration tests | `cargo test --workspace` | Exit 0; all deterministic tests pass without Internet/root |
| CLI contract | `cargo run -- --help` and `cargo run -- start --help` | Help exposes required `start --config <PATH>` and no expert packet flags |
| Example configuration | Parse repository `config.example.toml` through the same `Config::load` path in tests | Reddit/Medium rules, listeners, DoH, strategy, and limits materialize exactly |
| Documentation consistency | Inspect README exact command, proxy/PAC setup, rollback, outcomes, and explicit exclusions against config and CLI tests | No documented command/field/outcome/scope drift |
| Clean-room and excluded-surface check | Search tracked source/docs for prohibited transparent-interception/raw-socket/firewall implementation symbols and invalid-certificate bypass options | No implementation surface outside approved explicit proxy; mentions occur only as documented non-goals where applicable |
| Working tree isolation | `git status --short` plus task/report commit inspection | Only planned work and required reports/docs are committed; no unrelated changes remain |

## Completion Criteria

- Every listed plan part is implemented and verified.
- Every design decision, current requirement revision, and binding constraint has complete coverage.
- Cross-part coordination is integrated and verified.
- Roadmap obligations and gates are not applicable.
- All overall verification checks pass.
- Group implementation and verification reports plus overall verification report are separately committed with evidence.
- `.s1mple/docs/project.md` is updated from the verified implementation and committed, or a validated no-change reason is recorded.

## Planning Gaps

`None`
