# SplitHello

SplitHello is a local HTTPS CONNECT proxy for selected websites. It uses authenticated DNS-over-HTTPS (DoH) and splits the TLS ClientHello across valid TLS records. Other HTTPS traffic passes through unchanged.

Inspired by [tuananh/dpi-bypass](https://github.com/tuananh/dpi-bypass).

## Install

| Method | Command |
|---|---|
| Homebrew (macOS, Linux) | `brew install therealhieu/tap/splithello` |
| Cargo | `cargo install splithello --locked` |
| Release archive | Download `splithello-v<version>-<target>` from [Releases](https://github.com/therealhieu/splithello/releases), extract it, and put `splithello` on your `PATH` |
| From source (optional) | `cargo build --release`, then use `./target/release/splithello` wherever this README says `splithello` |

Archives cover macOS, Linux (glibc 2.35+) and Windows, each on x86_64 and ARM64. Each release lists their checksums in `SHA256SUMS`, and `gh attestation verify <archive> --repo therealhieu/splithello` confirms an archive was built by this repository's release workflow. On macOS, an archive downloaded with a browser is quarantined; clear it with `xattr -d com.apple.quarantine splithello`.

## Run

1. Start once. It creates `~/.config/splithello/config.toml` (`$XDG_CONFIG_HOME` if set) and stops.

   ```sh
   splithello start
   ```

2. Add your sites to that file, then run `splithello start` again.

   ```toml
   targets = ["reddit.com", "medium.com"]   # subdomains included
   ```

3. Set your browser's HTTPS proxy to `127.0.0.1:8080`, or its PAC URL to `http://127.0.0.1:8081/proxy.pac`.

Keep it running while you browse. A ready banner (`outcome=ready` when piped) means it started. `splithello check` validates the config, checks the listeners, and resolves every target over DoH without starting the proxy.

To stop, press `Ctrl-C` and remove the browser's proxy or PAC setting. SplitHello makes no persistent network, firewall, resolver, or certificate changes.

### Use another config file

Pass `--config <PATH>` to `start`, `check`, or `update`:

```sh
splithello start --config ~/work.toml
```

| File at `<PATH>` | Result |
|---|---|
| has targets | starts |
| missing | creates it, then stops |
| `targets = []` | stops with a warning |

An existing file is never overwritten.

### Logs

In a terminal, logs are short aligned lines. Each split target tunnel gets one line; failures get a hint; a summary line appears every minute that saw traffic:

```text
SplitHello 0.1.0 ready
  proxy  127.0.0.1:8080   browser HTTPS proxy
  pac    http://127.0.0.1:8081/proxy.pac   or this PAC URL
  dns    1.1.1.1 (DoH)   2 target(s)
21:22:10 ✓ split    www.reddit.com                 397ms  ↑596B ↓13K
21:22:11 ✗ dns      old.reddit.com                 DoH request timed out → check that [dns].upstream is reachable (`splithello check`)
21:22:15 ↑ update   0.2.0 available (you have 0.1.0) → splithello update
21:23:10 · last 1m: 37 tunnels (12 split, 24 direct, 1 failed) · 6 active
```

| Flag | Effect |
|---|---|
| `-v` | Also show every direct (non-target) tunnel |
| `-vv` | Trace output, including dependency debug logs |
| `-q` | Only warnings and errors |
| `--log-format human\|logfmt\|auto` | `auto` (default) is human on a terminal, timestamped `key=value` logfmt when piped |

`RUST_LOG` overrides the level flags.

## Update

`splithello start` and `splithello check` say when a newer release exists. Install it with:

```sh
splithello update                     # asks first; --yes skips the question
splithello update --check             # only report
splithello update --version v0.2.0    # a specific release, also an older one or a prerelease
```

| Installed with | `splithello update` |
|---|---|
| Release archive | downloads the release for this platform, verifies its SHA-256 digest, runs it once, then replaces the binary in place; restart `start` afterwards |
| Homebrew | prints `brew upgrade splithello` |
| Cargo | prints `cargo install splithello --locked` |

The check asks GitHub's API for the latest release at most once a day, resolving GitHub over the configured DoH upstream, and remembers the answer in `~/.cache/splithello/update-check.toml` (`$XDG_CACHE_HOME` if set; safe to delete). It never delays startup. Turn it off with `[update] check = false` in the config or `SPLITHELLO_NO_UPDATE_CHECK=1`. Prereleases are only installed by `--version`.

## Explanations

### How it works

```text
Browser                 SplitHello              DoH Server       Website
   │                         │                       │                │
   │  CONNECT host:443       │                       │                │
   ├────────────────────────►│                       │                │
   │                         │                       │                │
   │                         │  DNS query over HTTPS │                │
   │                         ├──────────────────────►│                │
   │                         │  IPv4 address         │                │
   │                         │◄──────────────────────┤                │
   │                         │                       │                │
   │                         │  TCP connect to IPv4                   │
   │                         ├───────────────────────────────────────►│
   │                         │                                        │
   │  ClientHello            │                                        │
   ├────────────────────────►│                                        │
   │                         │  TLS record 1                          │
   │                         ├───────────────────────────────────────►│
   │                         │  TLS record 2                          │
   │                         ├───────────────────────────────────────►│
   │                         │  ServerHello                           │
   │                         │◄───────────────────────────────────────┤
   │  ServerHello            │                                        │
   │◄────────────────────────┤                                        │
   │                                                                  │
   │◄════════════ encrypted HTTPS through SplitHello ════════════════►│
```

Targets are defined by `targets` in the config. The example includes `reddit.com`, `medium.com`, and their subdomains; `{ host = "example.com", subdomains = false }` matches one exact host. The PAC file sends only those targets through SplitHello.

The DoH endpoint host must be bootstrap-reachable without resolving any configured selected or blocked domain. Prefer a literal-IP HTTPS endpoint whose certificate is valid for that IP, or an independently resolvable non-target hostname. HTTPS certificate authentication is mandatory. `dns.ca_certificate` may add trusted CA roots but never disables verification. The DoH connection uses X25519 key exchange only: post-quantum key shares make the TLS ClientHello larger than one TCP segment, and some networks reset those to public DoH resolvers.

### Outcomes

| Outcome | Terminal label | Meaning |
|---|---|---|
| `outcome=target-transformed` | `✓ split` | A target used DoH and ClientHello splitting. |
| `outcome=direct-relay` | `· direct` (with `-v`) | A non-target used normal system resolution and unchanged relay. |
| `outcome=dns-error` | `✗ dns` | DoH failed or returned no valid IPv4 address, and no answer from the last 10 minutes could stand in. The line shows the underlying error. |
| `outcome=connect-error` | `✗ connect` | The proxy could not connect to the origin, or the connection broke before the site replied (as an injected reset does). The line shows the underlying error. |
| `outcome=tls-error` | `✗ tls` | The ClientHello was incomplete, invalid, or did not match the CONNECT host. |
| `outcome=timeout` | `✗ timeout` | A configured deadline expired. |
| `outcome=peer-reset` | `· reset` (non-targets with `-v`) | The browser or site hung up abruptly after the site had replied, e.g. a spare parallel connection or a WebSocket reconnect. The line names the side and the OS error. |
| `outcome=idle-timeout` | `· idle` (non-targets with `-v`) | A tunnel carried no traffic for `limits.idle_timeout` and was closed. Normal for browser keep-alive connections. |
| `outcome=unsupported-request` | `✗ request` | The request was not a supported hostname-form HTTP/1.1 CONNECT request. |

### Limits

- SplitHello is not a VPN, does not provide anonymity, and does not hide your client IP.
- It cannot guarantee access when destination IPs are blocked or when the network uses another unsupported filtering method.
- IPv6, QUIC, and HTTP/3 are not covered for selected targets.
- It does not require root, firewall rules, raw sockets, or system changes.
- It does not install certificates or terminate browser-to-origin TLS.
- Successful startup does not prove bypass success. Test the target and check its tunnel outcome.

## Development

### Checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build
cargo test --workspace
```

### Benchmarks

End-to-end benchmarks drive the release binary over loopback with a simulated DoH round trip:

```sh
scripts/bench.sh compare main      # base vs HEAD in alternating rounds; fails on a >5% regression
scripts/bench.sh run               # criterion run of HEAD
scripts/bench.sh snapshot <label>  # record that run in benches/results/
```

Scenarios, method, and recorded results live in [`benches/BASELINE.md`](benches/BASELINE.md).
