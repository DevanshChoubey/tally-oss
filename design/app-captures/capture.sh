#!/usr/bin/env bash
# Captures the app's screens, converts them to lossless WebP and compares them with the mock.
#
#   design/app-captures/capture.sh                 every entry in the registry
#   design/app-captures/capture.sh --only home     entries whose <theme>/<state-id> contains "home"
#   design/app-captures/capture.sh --audit         the audit set (design/REAL-PATH-AUDIT.md) into audit/
#   design/app-captures/capture.sh --audit --only cold.rail
#
# 1. Runs crates/ledgerly-app/examples/ui_snapshots.rs, which writes <theme>/<state-id>.ppm files.
# 2. Converts each one to design/app-captures/<theme>/<state-id>.webp (lossless).
# 3. Runs design/compare.mjs, which writes design/COMPARE.md.
#
# Environment:
#   CAPTURE_RAW=<dir>   convert the .ppm files already in <dir> and skip step 1.
#   CAPTURE_ROOT=<dir>  the tree the paths resolve in (for tests); default is this repository.

set -uo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
if command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER="${RUSTC_WRAPPER:-sccache}"; fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
export TMPDIR="${TMPDIR:-/tmp}"

usage() {
  echo "usage: capture.sh [--audit] [--only <substring>]" >&2
  exit 2
}

only=()
audit=()
if [[ "${1:-}" == "--audit" ]]; then audit=(--audit); shift; fi
case "${1:-}" in
  "") ;;
  --only) [[ $# -eq 2 ]] || usage; only=(--only "$2") ;;
  *) usage ;;
esac

ROOT="${CAPTURE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
OUT="$ROOT/design/app-captures"
RAW="${CAPTURE_RAW:-$TMPDIR/app-snapshots}"

if [[ -z "${CAPTURE_RAW:-}" ]]; then
  rm -rf "$RAW"
  mkdir -p "$RAW"
  cd "$ROOT" || exit 1
  # The filter drops the xkbcommon warnings that the headless window prints on every run.
  cargo run -p ledgerly-app --example ui_snapshots -- "$RAW" "${only[@]}" "${audit[@]}" 2>&1 | grep -v xkbcommon
  status=${PIPESTATUS[0]}
  if [[ $status -ne 0 ]]; then
    echo "ui_snapshots failed with status $status" >&2
    exit "$status"
  fi
fi

if [[ ! -d "$RAW" ]]; then
  echo "no captures in $RAW" >&2
  exit 1
fi

# The flat captures from before the <theme>/<state-id> layout.
find "$OUT" -maxdepth 1 -name '*.webp' -delete
# A full run replaces every capture, so a renamed or removed registry entry leaves nothing behind.
if (( ${#only[@]} == 0 && ${#audit[@]} == 0 )); then
  rm -rf "$OUT/light" "$OUT/dark" "$OUT/hc"
fi

mapfile -d '' ppms < <(find "$RAW" -name '*.ppm' -print0 | sort -z)
if (( ${#ppms[@]} == 0 )); then
  echo "no .ppm files in $RAW" >&2
  exit 1
fi

for ppm in "${ppms[@]}"; do
  rel="${ppm#"$RAW"/}"
  dest="$OUT/${rel%.ppm}.webp"
  mkdir -p "$(dirname "$dest")"
  if ! ffmpeg -loglevel error -y -i "$ppm" \
      -c:v libwebp -lossless 1 -compression_level 6 -map_metadata -1 "$dest"; then
    echo "ffmpeg failed on $ppm" >&2
    exit 1
  fi
done
echo "converted ${#ppms[@]} captures into ${OUT#"$ROOT"/}"

node "$ROOT/design/compare.mjs" "$ROOT/design" || exit $?
