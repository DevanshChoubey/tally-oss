#!/usr/bin/env bash
# Reproducible-build check (spec 6.6: a CA can confirm the binary matches the source).
#
# Builds the release binaries twice, each from a clean target directory, and compares the SHA-256
# of every output. Both builds go under $CARGO_TARGET_DIR (or ./target) in repro-a and repro-b.
# After the comparison the second copy is deleted; pass KEEP_REPRO_B=1 to keep it.
#
# What makes the builds identical:
# - --locked: the same dependency versions as Cargo.lock.
# - SOURCE_DATE_EPOCH: the commit time, for any tool that embeds a timestamp.
# - --remap-path-prefix: the repository, cargo home and target directory are written as fixed
#   paths, so the binary does not depend on where it was built.
# - CARGO_INCREMENTAL=0 and no RUSTC_WRAPPER: no cache can hand back an earlier artifact, so each
#   build really compiles.
#
# Exit status: 0 when the hashes match, 1 when they differ, 2 when a build could not run.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
binaries=(ledgerly-app ledgerly-cli)
bins=(ledgerly-app ledgerly)

if ! command -v cargo >/dev/null 2>&1; then
  echo "repro-check: cargo is not on PATH" >&2
  exit 2
fi

export SOURCE_DATE_EPOCH="$(git -C "$repo" log -1 --format=%ct)"
export CARGO_INCREMENTAL=0
unset RUSTC_WRAPPER
export RUSTFLAGS="--remap-path-prefix=${repo}=/ledgerly --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=${target}=/target ${RUSTFLAGS:-}"

build() {
  local dir="$1"
  rm -rf "$dir"
  echo "repro-check: building into $dir"
  (cd "$repo" && cargo build --release --locked \
    $(printf -- '-p %s ' "${binaries[@]}") --target-dir "$dir")
}

dir_a="$target/repro-a"
dir_b="$target/repro-b"

build "$dir_a" || { echo "repro-check: first build failed" >&2; exit 2; }
build "$dir_b" || { echo "repro-check: second build failed" >&2; exit 2; }

status=0
for bin in "${bins[@]}"; do
  a="$dir_a/release/$bin"
  b="$dir_b/release/$bin"
  if [[ ! -f "$a" || ! -f "$b" ]]; then
    echo "repro-check: missing $bin in one of the builds" >&2
    exit 2
  fi
  ha="$(sha256sum "$a" | cut -d' ' -f1)"
  hb="$(sha256sum "$b" | cut -d' ' -f1)"
  if [[ "$ha" == "$hb" ]]; then
    echo "MATCH    $bin  $ha  ($(stat -c %s "$a") bytes)"
  else
    echo "DIFFERS  $bin"
    echo "  repro-a $ha"
    echo "  repro-b $hb"
    status=1
  fi
done

if [[ "${KEEP_REPRO_B:-0}" != "1" ]]; then
  rm -rf "$dir_b"
  echo "repro-check: deleted $dir_b"
fi

if [[ $status -eq 0 ]]; then
  echo "repro-check: the release binaries are byte-identical"
else
  echo "repro-check: the release binaries differ" >&2
fi
exit "$status"
