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
# A capture run on the user's desktop session must be refused before anything is built.
# provenance: derived from the brief's rule "Captures run in a nested X display only".
out="$(cd "$work" && WAYLAND_DISPLAY=wayland-0 DISPLAY=:57 CAPTURE_ROOT="$work" CARGO_TARGET_DIR="$work/no-build" bash "$here/capture.sh" --only none 2>&1)"
status=$?
[[ $status -eq 2 ]] || { echo "FAIL: a run with WAYLAND_DISPLAY set exited $status, not 2"; fail=1; }
[[ "$out" == *"nested X display"* ]] || { echo "FAIL: the refusal does not say why: $out"; fail=1; }
[[ ! -e "$work/no-build" ]] || { echo "FAIL: cargo ran before the refusal"; fail=1; }
out="$(cd "$work" && env -u WAYLAND_DISPLAY DISPLAY=:0 CAPTURE_ROOT="$work" CARGO_TARGET_DIR="$work/no-build" bash "$here/capture.sh" --only none 2>&1)"
status=$?
[[ $status -eq 2 ]] || { echo "FAIL: a run on DISPLAY=:0 exited $status, not 2"; fail=1; }
[[ ! -e "$work/no-build" ]] || { echo "FAIL: cargo ran on DISPLAY=:0"; fail=1; }
(( fail == 0 )) && echo "ok"
exit $fail
