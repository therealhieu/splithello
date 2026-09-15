# SplitHello

SplitHello is a local HTTPS CONNECT proxy for selected websites. It uses authenticated DNS-over-HTTPS (DoH) and splits the TLS ClientHello across valid TLS records. Other HTTPS traffic passes through unchanged.

Inspired by [tuananh/dpi-bypass](https://github.com/tuananh/dpi-bypass).

## Run

Build and start:

```sh
cargo build --release
./target/release/splithello start --config config.example.toml
```

Configure your browser or operating system with one option:

- HTTPS proxy: `127.0.0.1:8080`
- PAC URL: `http://127.0.0.1:8081/proxy.pac`

Then open a configured target such as `https://www.reddit.com/`.

A successful start logs `outcome=ready`. Keep the process running while browsing.

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

Targets are defined in `config.example.toml`. The example includes `reddit.com`, `medium.com`, and their subdomains. The PAC file sends only those targets through SplitHello.

The DoH endpoint host must be bootstrap-reachable without resolving any configured selected or blocked domain. Prefer a literal-IP HTTPS endpoint whose certificate is valid for that IP, or an independently resolvable non-target hostname. HTTPS certificate authentication is mandatory. `dns.ca_certificate` may add trusted CA roots but never disables verification.

### Outcomes

| Outcome | Meaning |
|---|---|
| `outcome=target-transformed` | A target used DoH and ClientHello splitting. |
| `outcome=direct-relay` | A non-target used normal system resolution and unchanged relay. |
| `outcome=dns-error` | DoH failed or returned no valid IPv4 address. |
| `outcome=connect-error` | The proxy could not connect to the listener or origin. |
| `outcome=tls-error` | The ClientHello was incomplete, invalid, or did not match the CONNECT host. |
| `outcome=timeout` | A configured deadline expired. |
| `outcome=unsupported-request` | The request was not a supported hostname-form HTTP/1.1 CONNECT request. |

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
