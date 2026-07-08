#!/bin/bash
# Build the Rust static library for tvOS simulator or device.
#
# This script is called from Xcode's pre-build phase and also works standalone.
# It detects the target architecture from PLATFORM_NAME/ARCHS (set by Xcode)
# or defaults to the tvOS simulator.
#
# tvOS is a Tier 3 Rust target, so this script uses nightly + -Zbuild-std.
#
# Output: $SOTF_TVOS_RUST_LIB_DIR/libsotf_tvos.a, or Xcode's DERIVED_FILE_DIR/rust.

set -euo pipefail

# Navigate to the workspace root (3 levels up from tvos/)
WORKSPACE_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TVOS_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Building SotF tvOS static library ==="
echo "  Workspace: $WORKSPACE_ROOT"
echo "  tvOS dir:  $TVOS_DIR"

CARGO_BIN="${CARGO:-}"
RUSTUP_BIN="${RUSTUP:-}"
if [ -z "$CARGO_BIN" ] || [ -z "$RUSTUP_BIN" ]; then
    if command -v rustup >/dev/null 2>&1; then
        RUSTUP_BIN="$(command -v rustup)"
    elif [ -x "$HOME/.cargo/bin/rustup" ]; then
        RUSTUP_BIN="$HOME/.cargo/bin/rustup"
    else
        echo "error: rustup not found on PATH or at \$HOME/.cargo/bin/rustup" >&2
        echo "       Install Rust or set RUSTUP=/path/to/rustup before running this script." >&2
        exit 127
    fi
    CARGO_BIN="${CARGO:-$($RUSTUP_BIN which cargo --toolchain nightly 2>/dev/null || true)}"
fi
if [ -z "$CARGO_BIN" ] || [ ! -x "$CARGO_BIN" ]; then
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
if [ "${PLATFORM_NAME:-}" = "appletvos" ]; then
    RUST_TARGET="aarch64-apple-tvos"
    PROFILE="release"
elif [ "${PLATFORM_NAME:-}" = "appletvsimulator" ]; then
    RUST_TARGET="aarch64-apple-tvos-sim"
    PROFILE="release"
else
    # Default: simulator (for standalone builds)
    RUST_TARGET="aarch64-apple-tvos-sim"
    PROFILE="release"
fi

echo "  Target:    $RUST_TARGET"
echo "  Profile:   $PROFILE"
echo "  Cargo:     $CARGO_BIN"

# Build the static library. tvOS is Tier 3, so we need nightly + build-std.
cd "$WORKSPACE_ROOT"

if [ "$PROFILE" = "release" ]; then
    "$CARGO_BIN" +nightly build -p sotf-tvos --target "$RUST_TARGET" --release -Zbuild-std
    LIB_DIR="target/$RUST_TARGET/release"
else
    "$CARGO_BIN" +nightly build -p sotf-tvos --target "$RUST_TARGET" -Zbuild-std
    LIB_DIR="target/$RUST_TARGET/debug"
fi

# Copy the .a file to Xcode's derived-file area so build outputs never land in
# the source tree. Standalone invocations fall back to ignored tvos/build/rust/.
OUTPUT_LIB_DIR="${SOTF_TVOS_RUST_LIB_DIR:-${DERIVED_FILE_DIR:-$TVOS_DIR/build}/rust}"
mkdir -p "$OUTPUT_LIB_DIR"
cp "$LIB_DIR/libsotf_tvos.a" "$OUTPUT_LIB_DIR/"

echo "=== SotF tvOS static library built: $OUTPUT_LIB_DIR/libsotf_tvos.a ==="
ls -lh "$OUTPUT_LIB_DIR/libsotf_tvos.a"
