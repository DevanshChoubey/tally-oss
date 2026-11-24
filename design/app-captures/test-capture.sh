#!/usr/bin/env bash
# Checks that capture.sh converting a partial raw folder (CAPTURE_RAW) leaves the other captures alone.
# provenance: derived from the script's own contract in its header: "--only" touches only the named states.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/capture-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/design/app-captures/light" "$work/raw/light"
echo keep > "$work/design/app-captures/light/other.webp"
printf 'P6\n1 1\n255\n\0\0\0' > "$work/raw/light/new.ppm"
CAPTURE_ROOT="$work" CAPTURE_RAW="$work/raw" bash "$here/capture.sh" >/dev/null 2>&1
fail=0
[[ -f "$work/design/app-captures/light/other.webp" ]] || { echo "FAIL: another capture was deleted"; fail=1; }
[[ -f "$work/design/app-captures/light/new.webp" ]] || { echo "FAIL: the new capture was not written"; fail=1; }
(( fail == 0 )) && echo "ok"
exit $fail
