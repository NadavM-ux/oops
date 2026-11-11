#!/bin/bash
# Oops installer for macOS.
# Downloads the latest release from GitHub, puts Oops.app in Applications and opens it.
# Source: https://github.com/NadavM-ux/oops
set -euo pipefail

URL="https://github.com/NadavM-ux/oops/releases/latest/download/Oops-mac.zip"
DEST="/Applications"
if [ ! -w "$DEST" ]; then
  DEST="$HOME/Applications"
  mkdir -p "$DEST"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading Oops..."
curl -fL --progress-bar "$URL" -o "$TMP/Oops.zip"

echo "Installing to $DEST..."
osascript -e 'tell application "Oops" to quit' >/dev/null 2>&1 || true
pkill -x Oops >/dev/null 2>&1 || true
rm -rf "$DEST/Oops.app"
ditto -x -k "$TMP/Oops.zip" "$DEST"
xattr -dr com.apple.quarantine "$DEST/Oops.app" >/dev/null 2>&1 || true

echo "Done. Opening Oops (it lives in your menu bar)."
open "$DEST/Oops.app"
