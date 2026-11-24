#!/usr/bin/env bash
# Builds the macOS release artifact: Ledgerly.app inside a .dmg.
#
# Usage: packaging/macos/package.sh <version> <release-dir> <out-dir>
# Needs: hdiutil, sips, iconutil and codesign (all on macOS runners), and rsvg-convert to draw the icon
# from the SVG (the CI job installs librsvg with Homebrew). Nothing else is downloaded.
set -euo pipefail

version="${1#v}"
release="$2"
out="$3"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
name="ledgerly-${version}-macos-arm64"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

app="$work/Ledgerly.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$release/ledgerly-app" "$app/Contents/MacOS/ledgerly-app"
cp "$release/ledgerly" "$app/Contents/MacOS/ledgerly"
sed "s/__VERSION__/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"

# Icon: PNG sizes drawn from the SVG, then packed into an .icns.
iconset="$work/ledgerly.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  rsvg-convert -w "$size" -h "$size" "$root/packaging/icons/ledgerly.svg" -o "$iconset/icon_${size}x${size}.png"
  rsvg-convert -w "$((size * 2))" -h "$((size * 2))" "$root/packaging/icons/ledgerly.svg" -o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/ledgerly.icns"

# Sign the bundle ad hoc (no Apple Developer ID). Without any bundle signature, macOS reports a
# downloaded app as "damaged" and offers no way to open it; with an ad hoc one it says it cannot
# verify the developer, and System Settings > Privacy & Security > Open Anyway lets it run.
# Inner binaries first, then the bundle, so the seal covers Info.plist and the icon.
codesign --force --sign - "$app/Contents/MacOS/ledgerly"
codesign --force --sign - "$app/Contents/MacOS/ledgerly-app"
codesign --force --sign - "$app"
codesign --verify --strict --verbose=2 "$app"

mkdir -p "$out"
dmg_src="$work/dmg"
mkdir -p "$dmg_src"
cp -R "$app" "$dmg_src/"
cp "$root/LICENSE-AGPL" "$root/LICENSE-APACHE" "$root/LICENSE-MIT" "$root/NOTICE" "$dmg_src/"
cp "$root/crates/ledgerly-app/assets/fonts/OFL.txt" "$dmg_src/FONTS-OFL.txt"
cp "$root/crates/ledgerly-app/assets/fonts/Inter-LICENSE.txt" "$dmg_src/FONTS-Inter-LICENSE.txt"
ln -s /Applications "$dmg_src/Applications"
hdiutil create -volname "Ledgerly $version" -srcfolder "$dmg_src" -ov -format UDZO "$out/$name.dmg" >/dev/null

echo "package: wrote $out/$name.dmg"
