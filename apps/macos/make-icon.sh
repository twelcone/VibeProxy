#!/usr/bin/env bash
# Regenerate AppIcon.icns from AppIcon.svg. Requires librsvg (rsvg-convert) + iconutil (macOS).
# The .icns is committed so build.sh needs no image tooling; run this only when the SVG changes.
#   brew install librsvg   # if rsvg-convert is missing
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SVG="$HERE/AppIcon.svg"
SET="$(mktemp -d)/AppIcon.iconset"; mkdir -p "$SET"
gen() { rsvg-convert -w "$1" -h "$1" "$SVG" -o "$SET/$2"; }
gen 16 icon_16x16.png;      gen 32 icon_16x16@2x.png
gen 32 icon_32x32.png;      gen 64 icon_32x32@2x.png
gen 128 icon_128x128.png;   gen 256 icon_128x128@2x.png
gen 256 icon_256x256.png;   gen 512 icon_256x256@2x.png
gen 512 icon_512x512.png;   gen 1024 icon_512x512@2x.png
iconutil -c icns "$SET" -o "$HERE/AppIcon.icns"
echo "wrote $HERE/AppIcon.icns"
