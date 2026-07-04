#!/bin/bash
# Build the Rust static library for iOS simulator or device.
#
# This script is called from Xcode's pre-build phase and also works standalone.
# It detects the target architecture from PLATFORM_NAME/ARCHS (set by Xcode)
# or defaults to the iOS simulator.
#
# Output: $SOTF_IOS_RUST_LIB_DIR/libsotf_ios.a, or Xcode's DERIVED_FILE_DIR/rust.

set -euo pipefail

# Navigate to the workspace root (3 levels up from ios/)
WORKSPACE_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
IOS_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Building SotF iOS static library ==="
echo "  Workspace: $WORKSPACE_ROOT"
echo "  iOS dir:   $IOS_DIR"

CARGO_BIN="${CARGO:-}"
if [ -z "$CARGO_BIN" ]; then
    if command -v cargo >/dev/null 2>&1; then
        CARGO_BIN="$(command -v cargo)"
    elif [ -x "$HOME/.cargo/bin/cargo" ]; then
        CARGO_BIN="$HOME/.cargo/bin/cargo"
    else
        echo "error: cargo not found on PATH or at \$HOME/.cargo/bin/cargo" >&2
        echo "       Install Rust or set CARGO=/path/to/cargo before running this script." >&2
        exit 127
    fi
fi

# Determine Rust target from Xcode environment or default to simulator
if [ "${PLATFORM_NAME:-}" = "iphoneos" ]; then
    RUST_TARGET="aarch64-apple-ios"
    PROFILE="release"
elif [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
    RUST_TARGET="aarch64-apple-ios-sim"
    PROFILE="release"
else
    # Default: simulator (for standalone builds)
    RUST_TARGET="aarch64-apple-ios-sim"
    PROFILE="release"
fi

echo "  Target:    $RUST_TARGET"
echo "  Profile:   $PROFILE"
echo "  Cargo:     $CARGO_BIN"

# Build the static library
cd "$WORKSPACE_ROOT"

if [ "$PROFILE" = "release" ]; then
    "$CARGO_BIN" build -p sotf-ios --target "$RUST_TARGET" --release
    LIB_DIR="target/$RUST_TARGET/release"
else
    "$CARGO_BIN" build -p sotf-ios --target "$RUST_TARGET"
    LIB_DIR="target/$RUST_TARGET/debug"
fi

# Copy the .a file to Xcode's derived-file area so build outputs never land in
# the source tree. Standalone invocations fall back to ignored ios/build/rust/.
OUTPUT_LIB_DIR="${SOTF_IOS_RUST_LIB_DIR:-${DERIVED_FILE_DIR:-$IOS_DIR/build}/rust}"
mkdir -p "$OUTPUT_LIB_DIR"
cp "$LIB_DIR/libsotf_ios.a" "$OUTPUT_LIB_DIR/"

echo "=== SotF iOS static library built: $OUTPUT_LIB_DIR/libsotf_ios.a ==="
ls -lh "$OUTPUT_LIB_DIR/libsotf_ios.a"
