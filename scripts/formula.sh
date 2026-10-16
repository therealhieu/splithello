#!/usr/bin/env bash
# Prints the Homebrew formula for a release (used by .github/workflows/release.yml).
#
#   scripts/formula.sh <version> <base-url> <SHA256SUMS>
#
# <base-url> is where the release archives live: the GitHub release download URL, or a
# file:// directory for a dry run. Every Unix archive must have a line in <SHA256SUMS>.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <version> <base-url> <SHA256SUMS>" >&2
  exit 2
fi
version="$1"
base_url="${2%/}"
sums="$3"

# Prints `url` and `sha256` lines for one target's archive.
asset() {
  local name="splithello-v$version-$1.tar.gz" sha
  sha="$(awk -v name="$name" '$2 == name || $2 == "*" name { print $1 }' "$sums")"
  if [[ -z "$sha" ]]; then
    echo "error: $name is missing from $sums" >&2
    exit 1
  fi
  printf '      url "%s/%s"\n      sha256 "%s"\n' "$base_url" "$name" "$sha"
}

# Resolved before printing: a failure inside the heredoc's $(...) would not stop the script.
mac_arm="$(asset aarch64-apple-darwin)"
mac_intel="$(asset x86_64-apple-darwin)"
linux_arm="$(asset aarch64-unknown-linux-gnu)"
linux_intel="$(asset x86_64-unknown-linux-gnu)"

# Homebrew reads the version from the archive name, so the formula needs no `version` line.
cat <<EOF
class Splithello < Formula
  desc "Local HTTPS proxy that splits the TLS ClientHello for selected sites"
  homepage "https://github.com/therealhieu/splithello"
  license "MIT"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  on_macos do
    on_arm do
$mac_arm
    end
    on_intel do
$mac_intel
    end
  end
  on_linux do
    on_arm do
$linux_arm
    end
    on_intel do
$linux_intel
    end
  end

  def install
    bin.install "splithello"
  end

  test do
    assert_equal "splithello #{version}", shell_output("#{bin}/splithello --version").strip
  end
end
EOF
