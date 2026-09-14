# SplitHello

- **Last Modified:** 2026-09-14

Use this as the reusable project baseline. Keep change-specific decisions and execution evidence in [.s1mple work records](../work/). Cite authoritative sources rather than duplicating them.

## Purpose and Scope

`SplitHello` is an unprivileged, loopback-only HTTP/1.1 CONNECT proxy for explicit browser or operating-system proxy configuration. Configured Reddit and Medium hostnames use authenticated DNS-over-HTTPS (DoH) and a bounded TLS ClientHello record split. Unselected destinations use normal operating-system IPv4/IPv6 resolution and unchanged bidirectional relay.

```text
Browser → loopback CONNECT proxy
          ├─ selected host → authenticated DoH A records → IPv4 origin
          │                 → split ClientHello record framing → opaque relay
          └─ other host    → OS IPv4/IPv6 resolution → unchanged relay
```

The proxy preserves browser-to-origin TLS. It does not terminate TLS, install certificates, modify firewall or resolver settings, require root, provide VPN/anonymity behavior, hide the client IP, or implement transparent interception, selected-target IPv6, QUIC, or HTTP/3.

## Technology Stack

| Technology | Role | Version or configuration source |
|---|---|---|
| Rust | Application language | Edition 2024 in [Cargo.toml](../../Cargo.toml) |
| Tokio / tokio-util | Async TCP, task ownership, cancellation, timeouts, and signals | [Cargo.toml](../../Cargo.toml) |
| clap | Exact `start --config <PATH>` CLI | [Cargo.toml](../../Cargo.toml), [`src/cli.rs`](../../src/cli.rs) |
| Serde / TOML / IDNA | Strict configuration and canonical domain values | [Cargo.toml](../../Cargo.toml), [`src/config.rs`](../../src/config.rs), [`src/domain.rs`](../../src/domain.rs) |
| reqwest + rustls / hickory-proto | Authenticated RFC 8484 DoH and DNS wire messages | [Cargo.toml](../../Cargo.toml), [`src/dns.rs`](../../src/dns.rs) |
| thiserror / tracing | Structured errors and stable secret-safe diagnostics | [Cargo.toml](../../Cargo.toml), [`src/diagnostics.rs`](../../src/diagnostics.rs) |
| rcgen / tokio-rustls | Dev-only authenticated local DoH test boundary | [Cargo.toml](../../Cargo.toml), [`tests/support/mod.rs`](../../tests/support/mod.rs) |

Reqwest default features are disabled. The active client uses rustls, disables ambient proxy discovery and redirects, enforces HTTPS and timeouts, and exposes no invalid-certificate acceptance path. `dns.ca_certificate` adds PEM trust roots; it never disables certificate verification.

## Repository and Component Map

| Path | Responsibility |
|---|---|
| [`src/main.rs`](../../src/main.rs) | Tracing initialization, CLI dispatch, validated configuration, runtime lifecycle, process exit mapping |
| [`src/cli.rs`](../../src/cli.rs) | Stable `splithello start --config <PATH>` command contract |
| [`src/config.rs`](../../src/config.rs) | Strict TOML schema, defaults, hard bounds, loopback and safe-port validation |
| [`src/domain.rs`](../../src/domain.rs) | Canonical IDNA domain names and exact/label-aware target matching |
| [`src/dns.rs`](../../src/dns.rs) | Target-only IPv4 DoH resolver, DNS validation, TTL cache, and direct OS resolver |
| [`src/tls.rs`](../../src/tls.rs) | Bounded ClientHello parser and byte-preserving SNI-internal record split |
| [`src/proxy.rs`](../../src/proxy.rs) | CONNECT parsing, upstream connection, direct/selected relay, listener and tunnel task ownership |
| [`src/pac.rs`](../../src/pac.rs) | Deterministic target-only PAC rendering and bounded PAC service |
| [`src/app.rs`](../../src/app.rs) | Atomic listener binding, shared composition, readiness, signals, cancellation, and bounded shutdown |
| [`src/diagnostics.rs`](../../src/diagnostics.rs) | Stable outcome vocabulary and secret-safe tunnel reports |
| [`config.example.toml`](../../config.example.toml) | Valid fixed-loopback Reddit/Medium operating example |
| [`README.md`](../../README.md) | Build, setup, diagnostics, limitations, DoH bootstrap rule, and rollback |
| [`tests/start_proxy.rs`](../../tests/start_proxy.rs) | Compiled-binary V14–V19 integration scenarios and port-boundary regression |
| [`tests/support/mod.rs`](../../tests/support/mod.rs) | Generated-CA DoH, origins, process harness, CONNECT/PAC clients, deadlines, and cleanup |

## Architecture and Key Flows

### Startup and Shutdown

```text
CLI parse → Config::load and validate
          → construct matcher/resolvers/semaphore
          → atomically bind proxy and optional PAC listeners
          → spawn tracked services → emit outcome=ready
          → signal/cancellation → stop accepts → drain to deadline → abort and join remainder
```

Configuration is fully validated before runtime side effects. Required listeners bind before any service task starts or readiness is emitted. Every production task belongs to a runtime or service `JoinSet`; shutdown owns only process sockets, tasks, and memory.

### Direct Tunnel

```text
CONNECT unselected-host:port
  → OS resolver returns ordered IPv4/IPv6 SocketAddr candidates
  → connect within one deadline
  → send 200 only after upstream success
  → relay both directions unchanged
```

CONNECT-header acquisition, resolution, connection, relay idle time, concurrency, and cancellation are bounded. The authority port must always be listed in validated `allowed_ports`.

### Selected Tunnel

```text
CONNECT selected-host:port
  → authenticated DoH A query (never OS fallback)
  → IPv4 origin connection
  → send 200
  → buffer one bounded ClientHello
  → require canonical SNI == CONNECT host
  → emit two valid TLS records split strictly inside SNI
  → relay all later bytes opaquely
```

The transformed records preserve the exact ClientHello handshake byte sequence. No origin prefix is written before a complete rewrite, safe pass-through, or rejection decision.

## Data and Contracts

| Contract | Required behavior |
|---|---|
| `Config` | Reject unknown fields, unsafe limits, non-loopback listeners, invalid DoH, invalid/additional CA material, duplicate targets, and unlisted CONNECT ports before startup |
| `DomainName` / `TargetMatcher` | Canonical lowercase ASCII/IDNA identity; complete-label exact and explicit subdomain matching; no suffix overmatch |
| `TargetResolver` | Selected hosts return validated IPv4 A candidates from authenticated DoH only |
| `SystemResolver` | Unselected hosts retain normal ordered IPv4/IPv6 operating-system candidates |
| `RewriteOutcome` | Explicit rewritten, safe pass-through, need-more, or rejected state; no partial transformed output |
| `Outcome` / `TunnelReport` | Stable categories and bounded metadata; no raw headers, DNS messages, ClientHello bytes, credentials, cookies, or application payloads |
| PAC | Exact configured hosts and enabled subdomains use `PROXY`; all others use `DIRECT` |

The scoped loopback port-0 mode exists for deterministic integration tests. It allows explicit nonstandard ports to be listed for test origins, but CONNECT parsing still requires exact membership in `allowed_ports`. Normal nonzero listener configurations remain limited to ports 443 and 8443.

## Operating Contract

Build and start:

```sh
cargo build --release
./target/release/splithello start --config config.example.toml
```

The shipped example uses:

- proxy `127.0.0.1:8080`;
- PAC `http://127.0.0.1:8081/proxy.pac`;
- CONNECT port 443;
- authenticated DoH at `https://1.1.1.1/dns-query` with normal trust;
- `reddit.com` and `medium.com`, including subdomains;
- conservative explicit DNS, buffer, timeout, cache, shutdown, and concurrency limits.

The DoH endpoint must be bootstrap-reachable without resolving a configured selected or blocked domain. Use a certificate-valid literal-IP HTTPS endpoint or an independently resolvable non-target hostname. See the [README](../../README.md) for client setup, outcomes, limitations, and rollback.

## Engineering Standards and Constraints

- Follow [Rust language rules](languages/rust.md) and default rustfmt.
- Keep the product as one edition-2024 binary with crate-private cohesive modules.
- Use safe Rust; no `unsafe` is present in the verified implementation.
- Validate TOML, hostnames, CONNECT requests, DNS messages, and TLS bytes at their boundaries.
- Keep listeners loopback-only and all sizes, timeouts, cache entries, ports, connections, and tasks bounded.
- Do not log traffic payloads or secrets.
- Do not add transparent interception, root/firewall/raw-socket behavior, packet recipes, selected-target IPv6, QUIC/HTTP3, TLS termination, or automatic system/browser configuration.
- Treat `Cargo.lock` as Cargo-generated; never hand-edit it.

## Development and Verification

The verification layers are:

```text
Unit/component tests (V1–V13)
  → config, domain, CONNECT, PAC, DNS/cache, TLS, relay and lifecycle contracts
Compiled loopback integration (V14–V19)
  → authenticated DoH, direct/selected tunnels, isolation, shutdown, readiness/PAC, diagnostics
Repository consistency (V20)
  → production Config::load + README/config/CLI agreement
Rust quality checks (V21)
  → format, Clippy, build, workspace tests
```

Run from the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build
cargo test --workspace
```

Verified on 2026-09-14 after overall remediation:

| Check | Evidence |
|---|---|
| Format | Passed; no formatting diff |
| Clippy | Passed with `-D warnings` |
| Build | Passed for the edition-2024 binary |
| Tests | Passed: 75 total — 66 unit/component and 9 compiled integration tests |
| Integration | Passed: authenticated negative/positive trust, direct IPv4/available IPv6, selected exact ClientHello preservation, failure isolation, shutdown, PAC/readiness, secret-safe failure, and exact dynamic-port allowlist |
| CLI | Both help commands passed with exact `start --config <PATH>` and no packet controls |
| Security/scope | No invalid-cert bypass, native TLS/OpenSSL, `unsafe`, raw sockets, firewall/system mutation, selected-target IPv6, or QUIC implementation |
| Dependencies | Rustls feature path; direct dependencies use MIT, Apache-2.0, or dual MIT/Apache-2.0 licenses |

Documentation tests are not applicable because the package has no library target. Default tests use loopback services only and require neither public Internet nor root.

## Sources and Open Questions

- **Sources:** [Cargo.toml](../../Cargo.toml), [README](../../README.md), [example configuration](../../config.example.toml), repository source/tests, [Rust rules](languages/rust.md), and [W260914RUZU execution records](../work/active/w260914ruzu-splithello-start/).
- **Open Questions:** None for the verified version-1 scope. Real ISP effectiveness remains environment-specific and is not established by deterministic acceptance.
