# Benchmarks

`benches/proxy.rs` drives the compiled `splithello` binary end to end over loopback:

```text
bench client ──CONNECT──► splithello ──DoH (20ms simulated RTT, keep-alive HTTPS)──► TestDoh
                              └──TCP + split ClientHello──► TLS origin (rustls) / stream origin
```

| Scenario | What one iteration measures |
|---|---|
| `target_cold_ttfb` | CONNECT → TLS handshake through the split → first response byte, for a never-seen subdomain (DoH cache miss) |
| `target_warm_ttfb` | Same, for a host already in the DoH cache |
| `direct_ttfb` | Non-target tunnel to a plain origin, first byte |
| `burst_cold_20` | 20 concurrent target tunnels to one never-seen host (a page load); also records DoH queries per burst |
| `first_request_after_start` | Fresh process, 200ms idle after `ready`, first request to the target apex |
| `throughput_down_64m` | 64 MiB origin → client through one direct tunnel |
| `throughput_bidir_32m` | 32 MiB each way concurrently through one direct tunnel |

The DoH model: a new DoH connection costs `2 × RTT` (TCP + TLS 1.3), each query costs `1 × RTT`.

## Workflow

```sh
scripts/bench.sh compare main      # gate: base vs HEAD, paired rounds on this machine
scripts/bench.sh run               # criterion run of HEAD
scripts/bench.sh snapshot <label>  # record that run in benches/results/ and below
```

- **Gate** (decides pass/fail): `compare` builds the base ref in a temporary worktree, then one
  bench process measures base and HEAD in alternating rounds (A B B A …). Each round yields a
  HEAD/base ratio, so load or thermal drift that hits both binaries cancels out. A scenario fails
  when the whole 95% bootstrap CI of the median ratio is above +5%. Two sequential runs of the
  same commit differed by up to 35% on a loaded laptop; paired rounds of identical binaries stay
  within about ±5%.
- **History** (reference only): each PR commits one snapshot. Absolute numbers move with machine
  load, so compare them only within one machine row and trust the gate for verdicts.
- Loopback tunnels pin two ephemeral ports each for the 30s TIME_WAIT window. The bench paces
  itself (≤6,000 tunnels per 32s, waits excluded from timings) and starts each scenario with an
  empty window, so a full run takes a few minutes.

## Results

Medians; throughput rows show MiB/s.

<!-- results:start -->
| Scenario | pr1-baseline | pr2-config | pr3-logs |
|---|---|---|---|
| `target_cold_ttfb` | 24.89 ms | 24.72 ms | 25.46 ms |
| `target_warm_ttfb` | 453.9 µs | 376.8 µs | 398.4 µs |
| `direct_ttfb` | 575.2 µs | 417.6 µs | 437.6 µs |
| `burst_cold_20` | 33.90 ms · 20 DoH q | 28.20 ms · 20 DoH q | 28.32 ms · 20 DoH q |
| `first_request_after_start` | 81.48 ms | 75.37 ms | 74.92 ms |
| `throughput_down_64m` | 2314 MiB/s | 2841 MiB/s | 2817 MiB/s |
| `throughput_bidir_32m` | 2168 MiB/s | 2264 MiB/s | 2359 MiB/s |

| Snapshot | Commit | Date | CPU | OS | rustc |
|---|---|---|---|---|---|
| pr1-baseline | `c88e4c9` | 2026-10-15 | Apple M1 Max | macOS 14.4 | 1.99.0 |
| pr2-config | `94c0b97` | 2026-10-15 | Apple M1 Max | macOS 14.4 | 1.99.0 |
| pr3-logs | `6635814` | 2026-10-15 | Apple M1 Max | macOS 14.4 | 1.99.0 |
<!-- results:end -->
