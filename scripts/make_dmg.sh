#!/usr/bin/env bash
# Builds the installer: dist/Nothing-X-App-<version>.dmg
# (universal app: Apple Silicon + Intel, drag-to-Applications layout, read-me included).
# Needs: rustup target add x86_64-apple-darwin aarch64-apple-darwin
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
APP="dist/Nothing X App.app"
OUT="dist/Nothing-X-App-${VERSION}.dmg"

scripts/bundle.sh --universal

STAGE="$(mktemp -d)/Nothing X App"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cp packaging/LISEZ-MOI.txt "$STAGE/Lisez-moi - Read me.txt"

hdiutil create -volname "Nothing X App ${VERSION}" -srcfolder "$STAGE" \
  -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$OUT" >/dev/null
codesign --force --sign - "$OUT"

echo "✓ $OUT ($(du -h "$OUT" | cut -f1))"
shasum -a 256 "$OUT"
