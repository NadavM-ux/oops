#!/bin/bash
# Renders an HTML/SVG design file to PNG with headless Chrome (isolated profile).
# usage: render.sh <input.html> <output.png> <width> <height> [scale]
set -euo pipefail
IN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
OUT="$2"; W="$3"; H="$4"; SCALE="${5:-1}"
PROFILE="${TMPDIR:-/tmp}/oops-render-profile"
mkdir -p "$PROFILE"
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless=new --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check \
  --user-data-dir="$PROFILE" --default-background-color=00000000 \
  --force-device-scale-factor="$SCALE" --window-size="$W,$H" \
  --virtual-time-budget=1500 --screenshot="$OUT" "file://$IN" >/dev/null 2>&1
echo "rendered $OUT"
