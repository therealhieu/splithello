#!/usr/bin/env bash
# SplitHello benchmark workflow (see benches/BASELINE.md).
#
#   scripts/bench.sh compare [base-ref] [filter]  build <base-ref> (default: main) in a temporary
#                                                 worktree, then measure base and HEAD in
#                                                 alternating rounds; fails on a >5% regression,
#                                                 and with SPLITHELLO_BENCH_NO_SLOWER=1 on any
#                                                 significant slowdown (the CI merge gate)
#   scripts/bench.sh run [criterion args]         criterion run of HEAD (feeds `snapshot`)
#   scripts/bench.sh snapshot <label>             record the latest `run` in benches/results/
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CRITERION_HOME="$root/target/criterion"

case "${1:-}" in
  compare)
    base_ref="${2:-main}"
    base_dir="$(mktemp -d)"
    trap 'git -C "$root" worktree remove --force "$base_dir" >/dev/null 2>&1 || true' EXIT
    git -C "$root" worktree add --detach "$base_dir" "$base_ref" >/dev/null
    base_target="$root/target/bench-base"
    echo "==> base: $base_ref ($(git -C "$base_dir" rev-parse --short HEAD))" >&2
    CARGO_TARGET_DIR="$base_target" cargo build --quiet --release \
      --manifest-path "$base_dir/Cargo.toml" --bin splithello
    echo "==> head: $(git -C "$root" rev-parse --short HEAD)$(git -C "$root" diff --quiet HEAD || echo '+dirty')" >&2
    SPLITHELLO_BENCH_BASE_BIN="$base_target/release/splithello" \
      cargo bench --manifest-path "$root/Cargo.toml" --bench proxy -- ${3:+"$3"}
    ;;
  run)
    cargo bench --manifest-path "$root/Cargo.toml" --bench proxy -- --noplot "${@:2}"
    ;;
  snapshot)
    label="${2:?usage: scripts/bench.sh snapshot <label>}"
    if [[ "$(uname -s)" == "Darwin" ]]; then
      cpu="$(sysctl -n machdep.cpu.brand_string)"
      os="macOS $(sw_vers -productVersion)"
    else
      cpu="$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs || uname -m)"
      os="$(uname -sr)"
    fi
    BENCH_DATE="$(date +%F)" \
      BENCH_COMMIT="$(git -C "$root" rev-parse --short HEAD)" \
      BENCH_CPU="$cpu" \
      BENCH_OS="$os" \
      BENCH_RUSTC="$(rustc --version | cut -d' ' -f2)" \
      cargo run --quiet --release --manifest-path "$root/Cargo.toml" \
      --example bench_report -- snapshot "$label"
    ;;
  *)
    sed -n '2,10p' "$0" >&2
    exit 2
    ;;
esac
