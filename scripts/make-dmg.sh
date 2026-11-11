#!/bin/bash
# Builds Oops-mac.dmg with the custom installer window (background, icon layout, volume icon).
# usage: scripts/make-dmg.sh <path/to/Oops.app> <output.dmg>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$1"
OUT="$2"
VENV="${TMPDIR:-/tmp}/oops-dmgbuild"
if [ ! -x "$VENV/bin/dmgbuild" ]; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install -q --disable-pip-version-check "dmgbuild==1.6.5"
fi
rm -f "$OUT"
"$VENV/bin/dmgbuild" -s "$ROOT/scripts/dmg_settings.py" \
  -D app="$APP" \
  -D background="$ROOT/app/src-tauri/dmg/background.tiff" \
  -D icon="$ROOT/app/src-tauri/icons/icon.icns" \
  "Oops" "$OUT"
echo "Built $OUT"
