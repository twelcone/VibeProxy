#!/usr/bin/env bash
# Generate Swift bindings for vibeproxy-core and (optionally) compile a smoke test.
# Requires: Rust, Swift toolchain (Command Line Tools is enough for bindings + swiftc;
# building the actual SwiftUI .app bundle needs full Xcode). Run from the repo root.
set -euo pipefail

OUT="${1:-crates/vibeproxy-ffi/generated}"
cargo build -p vibeproxy-ffi
DYLIB="$(pwd)/target/debug/libvibeproxy_ffi.dylib"

mkdir -p "$OUT"
cargo run -q -p vibeproxy-ffi --bin uniffi-bindgen -- \
  generate --library "$DYLIB" --language swift --out-dir "$OUT"

echo "Bindings in $OUT:"
ls "$OUT"
echo
echo "To use from a SwiftUI app: add $OUT/vibeproxy_ffi.swift to the target, link libvibeproxy_ffi,"
echo "and expose the FFI header via the generated modulemap. The Swift API mirrors src/lib.rs:"
echo "  coreVersion(), listProfilesJson(), activeProfileId(), usageJson(range:), switchProfile(target:), shellSnippet()"
