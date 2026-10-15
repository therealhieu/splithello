# SplitHello

SplitHello is a local HTTPS CONNECT proxy for selected websites. It uses authenticated DNS-over-HTTPS (DoH) and splits the TLS ClientHello across valid TLS records. Other HTTPS traffic passes through unchanged.

Inspired by [tuananh/dpi-bypass](https://github.com/tuananh/dpi-bypass).

## Run

Build, create a config, check it, and start:

```sh
cargo build --release
./target/release/splithello init    # writes ~/.config/splithello/config.toml
./target/release/splithello check   # validates it, checks listeners, resolves each target over DoH
./target/release/splithello start
```

`init` writes [`config.example.toml`](config.example.toml); the only required key is the target list:

```toml
targets = ["reddit.com", "medium.com"]   # a plain host also covers its subdomains
```

Every command accepts `--config <PATH>` to use another file, for example
`./target/release/splithello start --config config.example.toml`. Without it, SplitHello reads
`$XDG_CONFIG_HOME/splithello/config.toml`, falling back to `~/.config/splithello/config.toml`.

Configure your browser or operating system with one option:

- HTTPS proxy: `127.0.0.1:8080`
- PAC URL: `http://127.0.0.1:8081/proxy.pac`

Then open a configured target such as `https://www.reddit.com/`.

A successful start prints a ready banner (`outcome=ready` when logs are piped). Keep the process running while browsing.

### Logs

In a terminal, logs are short aligned lines. Each split target tunnel gets one line; failures get a hint; a summary line appears every minute that saw traffic:

```text
SplitHello 0.1.0 ready
  proxy  127.0.0.1:8080   browser HTTPS proxy
  pac    http://127.0.0.1:8081/proxy.pac   or this PAC URL
  dns    1.1.1.1 (DoH)   2 target(s)
21:22:10 ✓ split    www.reddit.com                 397ms  ↑596B ↓13K
21:22:11 ✗ dns      old.reddit.com                 DoH request timed out → check that [dns].upstream is reachable (`splithello check`)
21:23:10 · last 1m: 37 tunnels (12 split, 24 direct, 1 failed) · 6 active
```

| Flag | Effect |
|---|---|
| `-v` | Also show every direct (non-target) tunnel |
| `-vv` | Trace output, including dependency debug logs |
| `-q` | Only warnings and errors |
| `--log-format human\|logfmt\|auto` | `auto` (default) is human on a terminal, timestamped `key=value` logfmt when piped |

`RUST_LOG` overrides the level flags.

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

The DoH endpoint host must be bootstrap-reachable without resolving any configured selected or blocked domain. Prefer a literal-IP HTTPS endpoint whose certificate is valid for that IP, or an independently resolvable non-target hostname. HTTPS certificate authentication is mandatory. `dns.ca_certificate` may add trusted CA roots but never disables verification.

### Outcomes

| Outcome | Terminal label | Meaning |
|---|---|---|
| `outcome=target-transformed` | `✓ split` | A target used DoH and ClientHello splitting. |
| `outcome=direct-relay` | `· direct` (with `-v`) | A non-target used normal system resolution and unchanged relay. |
| `outcome=dns-error` | `✗ dns` | DoH failed or returned no valid IPv4 address. |
| `outcome=connect-error` | `✗ connect` | The proxy could not connect to the listener or origin. |
| `outcome=tls-error` | `✗ tls` | The ClientHello was incomplete, invalid, or did not match the CONNECT host. |
| `outcome=timeout` | `✗ timeout` | A configured deadline expired. |
| `outcome=unsupported-request` | `✗ request` | The request was not a supported hostname-form HTTP/1.1 CONNECT request. |

### Limits

- SplitHello is not a VPN, does not provide anonymity, and does not hide your client IP.
- It cannot guarantee access when destination IPs are blocked or when the network uses another unsupported filtering method.
- IPv6, QUIC, and HTTP/3 are not covered for selected targets.
- It does not require root, firewall rules, raw sockets, or system changes.
- It does not install certificates or terminate browser-to-origin TLS.
- Successful startup does not prove bypass success. Test the target and check its tunnel outcome.

### Stop

Stop the process with `Ctrl-C`, then remove the HTTPS proxy or PAC setting. SplitHello leaves no persistent network, firewall, resolver, or certificate changes.

### Development checks

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
