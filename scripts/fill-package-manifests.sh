#!/usr/bin/env bash
# Fills the winget and Chocolatey templates in packaging/ for one release.
#
#   scripts/fill-package-manifests.sh <tag, e.g. v1.0.0> <SHA256SUMS.txt> <out dir>
#
# The installer hash is read from SHA256SUMS.txt (the file the release job writes over the files
# it publishes), never typed. Fails when the installer has no line there or the line is not a
# 64-digit hash, so a manifest is never written with a guessed or missing hash.
set -euo pipefail
tag="$1"; sums="$2"; out="$3"
version="${tag#v}"
installer="ledgerly-${version}-windows-x86_64-setup.exe"
line="$(grep -E "^[0-9a-f]{64}  ${installer//./\\.}\$" "$sums" || true)"
if [ -z "$line" ]; then
  echo "fill-package-manifests: no SHA-256 line for $installer in $sums" >&2
  exit 1
fi
sha="${line%% *}"
root="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$out/winget" "$out/chocolatey/tools"
fill() { sed -e "s/@VERSION@/${version}/g" -e "s/@SHA256@/${sha}/g" "$1" > "$2"; }
for f in "$root"/packaging/winget/*.yaml; do fill "$f" "$out/winget/$(basename "$f")"; done
fill "$root/packaging/chocolatey/ledgerly.nuspec" "$out/chocolatey/ledgerly.nuspec"
fill "$root/packaging/chocolatey/tools/chocolateyinstall.ps1" "$out/chocolatey/tools/chocolateyinstall.ps1"
if grep -rq '@VERSION@\|@SHA256@' "$out"; then
  echo "fill-package-manifests: a placeholder was left in $out" >&2
  exit 1
fi
echo "fill-package-manifests: $installer $sha -> $out"
