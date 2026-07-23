#!/usr/bin/env bash
# Build VibeProxy.app from Swift sources + the Rust core (via uniffi) — no Xcode required, just the
# Swift toolchain from Command Line Tools. Produces apps/macos/build/VibeProxy.app.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
FFI="$ROOT/crates/vibeproxy-ffi"
GEN="$FFI/generated"
APP="$HERE/build/VibeProxy.app"
SDK="$(xcrun --show-sdk-path)"

echo "› building core FFI dylib (release)"
cargo build --release -p vibeproxy-ffi --manifest-path "$ROOT/Cargo.toml"

echo "› generating Swift bindings"
"$FFI/generate-swift.sh" "$GEN" >/dev/null

echo "› assembling bundle skeleton"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources"
cp "$ROOT/target/release/libvibeproxy_ffi.dylib" "$APP/Contents/Frameworks/"
# The binary loads the dylib via @rpath; point the copy's install name to match.
install_name_tool -id @rpath/libvibeproxy_ffi.dylib "$APP/Contents/Frameworks/libvibeproxy_ffi.dylib"

# App icon (committed; regenerate from AppIcon.svg with make-icon.sh when it changes).
cp "$HERE/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>VibeProxy</string>
  <key>CFBundleIdentifier</key><string>dev.vibeproxy.menubar</string>
  <key>CFBundleName</key><string>VibeProxy</string>
  <key>CFBundleDisplayName</key><string>VibeProxy</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleShortVersionString</key><string>0.2.1</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHumanReadableCopyright</key><string>VibeProxy</string>
</dict></plist>
PLIST

echo "› compiling Swift → $APP/Contents/MacOS/VibeProxy"
swiftc -O -parse-as-library \
  "$HERE"/Sources/*.swift "$GEN/vibeproxy_ffi.swift" \
  -I "$GEN" -Xcc -fmodule-map-file="$GEN/vibeproxy_ffiFFI.modulemap" \
  -sdk "$SDK" \
  -L "$APP/Contents/Frameworks" -lvibeproxy_ffi \
  -framework SwiftUI -framework AppKit -framework Charts \
  -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  -o "$APP/Contents/MacOS/VibeProxy"

echo "› signing (ad-hoc)"
codesign --force --deep --sign - "$APP" >/dev/null 2>&1

echo "✓ built $APP"
