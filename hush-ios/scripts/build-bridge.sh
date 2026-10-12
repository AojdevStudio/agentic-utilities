#!/usr/bin/env bash
# Builds the Rust SDK bridge as HushBridge.xcframework for arm64 iOS devices
# and arm64 simulators. Runs on macOS with Xcode and the Rust targets
# aarch64-apple-ios and aarch64-apple-ios-sim.
#
# Output (gitignored, outside tracked source):
#   hush-ios/.build/HushBridge.xcframework   static library + header + module map
#   hush-ios/.build/HushBridge-link.txt      native libraries the app must link
#
# Environment:
#   HUSH_BRIDGE_OUT    output directory (default: hush-ios/.build)
#   CARGO_TARGET_DIR   Cargo build cache (default: hush-ios/Bridge/target)
set -euo pipefail

app_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bridge_dir="$app_dir/Bridge"
out_dir="${HUSH_BRIDGE_OUT:-$app_dir/.build}"
target_dir="${CARGO_TARGET_DIR:-$bridge_dir/target}"
targets=(aarch64-apple-ios aarch64-apple-ios-sim)

fail() {
  echo "build-bridge: $*" >&2
  exit 1
}

[[ "$(uname -s)" == Darwin ]] || fail "needs macOS with Xcode"
command -v xcodebuild >/dev/null || fail "xcodebuild not found"
command -v cargo >/dev/null || fail "cargo not found"
installed="$(rustup target list --installed)"
for target in "${targets[@]}"; do
  grep -qx "$target" <<<"$installed" || fail "missing Rust target $target (rustup target add $target)"
done

# C code inside dependencies must match the app deployment target.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
export CARGO_TARGET_DIR="$target_dir"

libraries=()
for target in "${targets[@]}"; do
  sdk=iphoneos
  [[ "$target" == *-sim ]] && sdk=iphonesimulator
  sdk_root="$(xcrun --sdk "$sdk" --show-sdk-path)"
  # Use Apple's SQLite for the SDK's in-memory database. Bundled SQLite also
  # links unused filesystem probes that do not describe this app's behavior.
  export LIBSQLITE3_SYS_USE_PKG_CONFIG=1
  export SQLITE3_LIB_DIR="$sdk_root/usr/lib"
  export SQLITE3_INCLUDE_DIR="$sdk_root/usr/include"
  export SQLITE3_STATIC=0
  release_dir="$target_dir/$target/release"
  log="$(mktemp)"
  cargo rustc --locked --release --lib --target "$target" \
    --manifest-path "$bridge_dir/Cargo.toml" -- --print native-static-libs 2>&1 | tee "$log" >&2
  # rustc prints the native libraries only when it actually links. Keep the
  # note beside the library so a cached build still has it.
  note="$(sed -n 's/.*native-static-libs: //p' "$log" | tail -n 1)"
  rm -f "$log"
  [[ -n "$note" ]] && printf '%s\n' "$note" >"$release_dir/hush_bridge.link"
  [[ -f "$release_dir/hush_bridge.link" ]] ||
    fail "no native library list for $target; remove $release_dir and rebuild"
  libraries+=("$release_dir/libhush_bridge.a")
done

mkdir -p "$out_dir"
headers="$(mktemp -d)"
trap 'rm -rf "$headers"' EXIT
chmod 755 "$headers"
cp "$bridge_dir/include/hush_bridge.h" "$bridge_dir/include/module.modulemap" "$headers/"

xcframework="$out_dir/HushBridge.xcframework"
rm -rf "$xcframework"
xcodebuild -create-xcframework \
  -library "${libraries[0]}" -headers "$headers" \
  -library "${libraries[1]}" -headers "$headers" \
  -output "$xcframework"

link="$out_dir/HushBridge-link.txt"
{
  for target in "${targets[@]}"; do
    printf '%s: %s\n' "$target" "$(cat "$target_dir/$target/release/hush_bridge.link")"
  done
} >"$link"

echo "build-bridge: wrote $xcframework"
echo "build-bridge: wrote $link"
cat "$link"
