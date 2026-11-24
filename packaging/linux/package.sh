#!/usr/bin/env bash
# Builds the Linux release artifacts: a tar.gz and a .deb, from the release binaries.
#
# Usage: packaging/linux/package.sh <version> <release-dir> <out-dir>
#   <version>      e.g. 0.1.0 (a leading v is removed)
#   <release-dir>  the directory that holds ledgerly-app and ledgerly (target/release)
#   <out-dir>      where the artifacts are written
#
# Needs: tar, gzip, dpkg-deb (preinstalled on Ubuntu runners). Nothing is downloaded.
set -euo pipefail

version="${1#v}"
release="$2"
out="$3"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct)}"
name="ledgerly-${version}-linux-x86_64"

for bin in ledgerly-app ledgerly; do
  [[ -x "$release/$bin" ]] || { echo "package: missing $release/$bin" >&2; exit 2; }
done
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Portable tarball: fixed owner, sorted names, fixed mtime.
stage="$work/$name"
mkdir -p "$stage"
cp "$release/ledgerly-app" "$release/ledgerly" "$stage/"
cp "$here/ledgerly.desktop" "$root/packaging/icons/ledgerly.svg" "$stage/"
cp "$root/LICENSE-AGPL" "$root/LICENSE-APACHE" "$root/LICENSE-MIT" "$root/NOTICE" "$stage/"
cp "$root/crates/ledgerly-app/assets/fonts/OFL.txt" "$stage/FONTS-OFL.txt"
cp "$root/crates/ledgerly-app/assets/fonts/Inter-LICENSE.txt" "$stage/FONTS-Inter-LICENSE.txt"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" \
  -C "$work" -cf - "$name" | gzip -n -9 > "$out/$name.tar.gz"

# Debian package: the same files at their system paths.
deb_root="$work/deb"
mkdir -p "$deb_root/DEBIAN" "$deb_root/usr/bin" "$deb_root/usr/share/applications" \
  "$deb_root/usr/share/icons/hicolor/scalable/apps" "$deb_root/usr/share/doc/ledgerly"
cp "$release/ledgerly-app" "$release/ledgerly" "$deb_root/usr/bin/"
cp "$here/ledgerly.desktop" "$deb_root/usr/share/applications/ledgerly.desktop"
cp "$root/packaging/icons/ledgerly.svg" "$deb_root/usr/share/icons/hicolor/scalable/apps/ledgerly.svg"
cp "$root/LICENSE-AGPL" "$deb_root/usr/share/doc/ledgerly/copyright"
cp "$root/LICENSE-APACHE" "$root/LICENSE-MIT" "$root/NOTICE" "$deb_root/usr/share/doc/ledgerly/"
cp "$root/crates/ledgerly-app/assets/fonts/OFL.txt" "$deb_root/usr/share/doc/ledgerly/FONTS-OFL.txt"
cp "$root/crates/ledgerly-app/assets/fonts/Inter-LICENSE.txt" "$deb_root/usr/share/doc/ledgerly/FONTS-Inter-LICENSE.txt"
size_kb="$(du -sk "$deb_root" | cut -f1)"
sed -e "s/@VERSION@/$version/" -e "s/@SIZE_KB@/$size_kb/" "$here/control.in" > "$deb_root/DEBIAN/control"
find "$deb_root" -type d -exec chmod 0755 {} +
find "$deb_root" -type f -exec chmod 0644 {} +
chmod 0755 "$deb_root/usr/bin/ledgerly-app" "$deb_root/usr/bin/ledgerly"
SOURCE_DATE_EPOCH="$epoch" dpkg-deb --root-owner-group --build "$deb_root" "$out/ledgerly_${version}_amd64.deb" >/dev/null

echo "package: wrote $out/$name.tar.gz and $out/ledgerly_${version}_amd64.deb"
