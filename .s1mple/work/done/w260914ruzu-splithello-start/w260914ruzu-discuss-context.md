# W260914RUZU: splithello-start

## Sources

| Source | Use |
|---|---|
| `.s1mple/S1MPLE.md` | Discuss behavior and required context |
| `.s1mple/docs/project.md` | Reusable project baseline |
| `.s1mple/docs/languages/rust.md` | Rust engineering, safety, dependency, and verification rules |
| `Cargo.toml` | Current package name, version, edition, and dependency state |
| `src/main.rs` | Current placeholder binary behavior |
| User input and parent-provided research summary | Product goal, CLI contract, clean-room constraint, supported blocking classes, and explicit v1 exclusions |

## Roadmap Binding

None — standalone work

### Previous Accepted Bindings

None

### Parent Synchronization

- **Registration or discussion update:** None

## 1. User Input

### Original Input

Autopilot through `s1mple-discuss` and `s1mple-plan` without asking the user questions; choose conservative best options. The user's local internet connection currently blocks `reddit.com` and `medium.com`. The design must be simple and easy to maintain, with the required invocation `splithello start --config config.example.toml`.

### Consolidated Requirement or Problem

Define a clean-room Rust CLI for authorized use on the user's own computer that can locally mitigate DNS poisoning and IPv4 TCP TLS-SNI filtering for configured domains, initially `reddit.com` and `medium.com`. The application must start through `splithello start --config <path>`, use explicit configuration, operate fail-open, remain simple to maintain, expose useful diagnostics, and accurately report when a blocking method is outside its scope.

### Constraints and Assumptions

- The work is authorized local network compatibility and censorship-circumvention software for the user's own computer.
- The first configuration example includes `reddit.com` and `medium.com`, including an explicit policy for required subdomains rather than silently matching unrelated suffixes.
- Version 1 supports Linux and Rust edition 2024; other operating systems are out of scope.
- Version 1 addresses DNS poisoning and IPv4 TCP TLS-SNI filtering only.
- IP-address blocking, IPv6 filtering, QUIC/HTTP/3 filtering, traffic anonymity, IP masking, VPN behavior, and universal censorship circumvention are explicitly out of scope.
- The runtime must fail open: unsupported, malformed, unselected, or unprocessable traffic continues unchanged whenever safe operation permits.
- Elevated network privileges may be required, but privilege scope must be documented and minimized.
- Configuration is mandatory for `start`; missing, invalid, or unsafe configuration must fail before network state is changed.
- The product should prefer one maintainable default behavior over a collection of expert packet-tuning flags.
- DNS mitigation and TLS-SNI mitigation are separate concerns with independently testable outcomes.
- The implementation must be original and clean-room. The unlicensed `tuananh/dpi-bypass` repository may inform conceptual requirements but no source code, distinctive structure, or text may be copied.
- External protocol references such as RFC 6066, RFC 9293, RFC 9505, and RFC 9849 are design evidence, not a promise that the tool can bypass every compliant or adversarial network.
- No user interaction is required during discussion; unresolved choices must use conservative defaults and be recorded explicitly.

## 2. Project Structure

- `.s1mple/S1MPLE.md` — governing discuss, plan, implementation, verification, and delivery workflow.
- `.s1mple/docs/project.md` — baseline showing a minimal single-binary Rust crate with no established production architecture.
- `.s1mple/docs/languages/rust.md` — applicable Rust rules, including explicit errors, limited dependencies, boundary validation, safe APIs, and focused tests.
- `Cargo.toml` — package `splithello` version `0.1.0`, Rust edition 2024, currently without dependencies.
- `Cargo.lock` — Cargo-generated dependency resolution; changes must be produced through Cargo rather than manual edits.
- `src/main.rs` — current placeholder entrypoint that only prints `Hello, world!`.
- `config.example.toml` — proposed repository-root example configuration required by the requested invocation; the file does not exist yet.
- `tests/` — proposed integration-test location; the directory does not exist yet.

## 3. Standards and Patterns

### Applicable Standards

- `.s1mple/S1MPLE.md:15` — choose the simplest robust design and add abstractions only for demonstrated needs.
- `.s1mple/S1MPLE.md:17` — ship complete, usable, verified end-to-end increments rather than speculative groundwork.
- `.s1mple/S1MPLE.md:59` — implementation must remain scoped to an approved plan.
- `.s1mple/S1MPLE.md:66` — report exact verification commands and results.
- `.s1mple/docs/languages/rust.md:65` — add only maintained, appropriately licensed dependencies that reduce total complexity.
- `.s1mple/docs/languages/rust.md:88` — use `Result<T, E>` for recoverable failures.
- `.s1mple/docs/languages/rust.md:93` — validate and parse untrusted input at system boundaries.
- `.s1mple/docs/languages/rust.md:100` — do not introduce `unsafe` when a safe Rust alternative can satisfy the requirement.
- `.s1mple/docs/languages/rust.md:107` — add focused unit and integration tests for meaningful behavior and boundaries.
- RFC 6066 — establishes that the TLS server-name extension carries the requested hostname in the ClientHello.
- RFC 9293 — establishes TCP as an ordered byte stream without application-message alignment to segment boundaries.
- RFC 9505 — establishes that censorship includes multiple DNS, TLS, IP, and transport techniques; one mitigation cannot be represented as universal.
- RFC 9849 — establishes Encrypted ClientHello as a protocol-level hostname privacy mechanism while leaving other network metadata and blocking surfaces visible.

### Existing Patterns

- `Cargo.toml:1` — one Cargo package named `splithello`.
- `Cargo.toml:4` — Rust edition 2024.
- `Cargo.toml:6` — no third-party dependencies currently exist.
- `src/main.rs:1` — the crate currently uses a single minimal binary entrypoint.
- `.gitignore:1` — Cargo build output under `/target` is excluded.
- `.s1mple/docs/project.md:35` — the current single-function structure is an observation, not an architecture policy.

## 4. Architecture

```text
Current repository

CLI process (`src/main.rs`)
    → print placeholder message
    → exit

Proposed product boundary for later design

Explicit TOML configuration
    → validate before side effects
    → start local DNS and/or IPv4 TCP compatibility services
    → apply behavior only to configured domains
    → preserve unrelated or unsupported traffic unchanged
    → restore owned network state on shutdown
    → emit diagnostics and explicit scope limitations
```

- CLI boundary — parses `start --config <path>`, loads configuration, reports actionable errors, and coordinates clean startup and shutdown.
- Configuration boundary — owns selected domains, enabled mitigation classes, local listener settings, and conservative validated defaults.
- DNS compatibility boundary — provides an independently testable path around poisoned local DNS answers without claiming to bypass IP blocking.
- IPv4 TCP TLS compatibility boundary — handles only configured TLS-SNI filtering cases and leaves unselected or unsupported traffic unchanged.
- Network-state boundary — owns only resources created by the current process and restores them on normal termination and startup failure.
- Diagnostics boundary — distinguishes configuration, privilege, DNS, interception, unsupported-protocol, and upstream-connectivity outcomes without logging sensitive payloads.

## 5. Code Flow

```text
`splithello start --config config.example.toml`
    → parse command and configuration path
    → read and validate the complete configuration
    → verify platform, privileges, and required local facilities
    → create scoped runtime resources
    → start enabled mitigation components
    → report readiness and per-component status
    → process only configured-domain traffic
    → pass unrelated, unsupported, or safely unprocessable traffic unchanged
    → on signal or startup failure, restore resources owned by this process
```

1. The user invokes the required `start` command with an explicit TOML file.
2. The process validates configuration, domain selectors, enabled components, and local prerequisites before changing network state.
3. The process starts only the configured DNS and/or IPv4 TCP TLS compatibility components.
4. Runtime policy limits special handling to `reddit.com`, `medium.com`, and explicitly configured subdomains.
5. Unsupported traffic, including IPv6 and QUIC/HTTP/3, is not modified and is reported diagnostically when it explains an unsuccessful test.
6. Failures in optional processing do not intentionally block unrelated traffic.
7. Shutdown restores only network state created and recorded by the current process.
8. Verification checks DNS resolution and IPv4 TCP TLS reachability separately so a successful low-level connection is not misreported as a universal bypass.

## 6. Boundaries

### In Scope

- A maintainable Rust CLI with the exact command shape `splithello start --config <path>`.
- A documented `config.example.toml` configured for `reddit.com` and `medium.com`.
- Strict TOML parsing, validation before side effects, and actionable configuration errors.
- Linux-local operation on the user's own computer.
- DNS-poisoning mitigation for configured domains through a standards-based encrypted upstream resolver selected in configuration.
- IPv4 TCP TLS-SNI-filtering mitigation for configured domains through a clean-room local compatibility mechanism selected by the finalized design.
- Fail-open handling for unrelated, unsupported, malformed, or safely unprocessable traffic.
- Minimal privilege use, owned-resource cleanup, graceful shutdown, and recovery from partial startup.
- Diagnostics that identify supported versus unsupported blocking paths without promising success.
- Unit tests for configuration and protocol parsing plus integration or isolated-network tests for startup, shutdown, domain scoping, fail-open behavior, DNS outcomes, and IPv4 TCP TLS outcomes.

### Out of Scope

- Copying source, structure, documentation, or distinctive implementation details from the unlicensed `tuananh/dpi-bypass` repository.
- VPN, proxy-as-a-service, anonymity, traffic concealment, client-IP masking, or protection from traffic analysis.
- Circumventing destination-IP blocks, total network shutdowns, captive portals, authenticated enterprise controls, or endpoint compromise.
- IPv6 packet handling or filtering mitigation in version 1.
- QUIC, UDP/443, and HTTP/3 manipulation in version 1.
- Automatic adversarial strategy discovery, broad packet mutation, arbitrary user-authored packet recipes, or a large expert-tuning surface.
- Guaranteeing access to every Reddit or Medium hostname, CDN endpoint, embedded third-party service, mobile application, or future protocol.
- Changing browser settings, system-wide IPv6 policy, or unrelated firewall policy without explicit configuration and ownership.
- Operating on remote hosts, gateways, third-party networks, or traffic not owned or authorized by the user.

### Interfaces and Dependencies

- CLI contract — `splithello start --config <path>` is stable for version 1; omitted or unreadable configuration returns a non-zero exit before network changes.
- Configuration contract — TOML is human-readable, checked strictly, provides explicit domain and component settings, and rejects unknown unsafe or misspelled fields.
- Domain policy contract — exact domains and intentional subdomains are represented explicitly; matching is case-insensitive and label-aware.
- Runtime contract — each component reports startup readiness, runtime errors, and shutdown outcome through structured, secret-safe logs.
- Safety contract — the process does not delete or replace network state it did not create; partial startup is rolled back.
- Dependency policy — prefer safe, maintained, appropriately licensed Rust crates for CLI parsing, TOML deserialization, DNS protocol handling, Linux interfaces, and structured errors only when they reduce complexity compared with bespoke code.
- Protocol evidence — RFC 6066, RFC 9293, RFC 9505, and RFC 9849 constrain terminology, parsing assumptions, supported claims, and explicit limitations.

## 7. Unknowns

- The user's exact ISP blocking behavior has not been measured; the design must therefore diagnose DNS poisoning and IPv4 TCP TLS-SNI filtering independently and report unsupported IP, IPv6, QUIC, or broader failures.
- The specific maintained Rust crates and minimum supported Linux kernel/distribution remain to be selected during system design using licensing, API safety, and maintenance evidence.
- The exact clean-room TLS-SNI compatibility mechanism remains a system-design decision; it must prioritize retransmission correctness, domain scoping, fail-open behavior, and maintainability over a broad collection of packet strategies.
- The encrypted DNS upstream and trust policy are not user-selected; a conservative configurable default with explicit documentation is required.
- Mobile Reddit/Medium applications and all third-party assets are not guaranteed by the two-domain goal; version 1 acceptance should test browser access to representative configured hostnames only.
