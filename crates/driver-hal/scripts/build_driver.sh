#!/bin/bash
#
# Build script for Audio HAL Driver bundle
#
# This script compiles the C bridge code and packages it into a proper
# macOS .driver bundle that can be loaded by Core Audio.

set -e  # Exit on error

echo "🔨 Building Audio HAL Driver..."

# Configuration
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)/.."
WORKSPACE_DIR="${PROJECT_DIR}/../target"
BUNDLE_DIR="${WORKSPACE_DIR}/sotf.driver"
BUILD_DIR="${WORKSPACE_DIR}/build"
RUST_DYLIB="${WORKSPACE_DIR}/release/libsotf_hal.dylib"
OUTPUT_BINARY="${BUNDLE_DIR}/Contents/MacOS/sotf_driver"

# Create build directory
mkdir -p "${BUILD_DIR}"
mkdir -p "${BUNDLE_DIR}/Contents/MacOS"
mkdir -p "${BUNDLE_DIR}/Contents/Resources"

echo "📦 Project directory: ${PROJECT_DIR}"
echo "📂 Bundle directory: ${BUNDLE_DIR}"
echo "🏗️  Build directory: ${BUILD_DIR}"

# Check if Rust library exists
if [ ! -f "${RUST_DYLIB}" ]; then
    echo "❌ Rust library not found: ${RUST_DYLIB}"
    echo "   Please run 'cargo build --release' first"
    exit 1
fi

# Copy the Rust dynamic library
echo "📋 Copying Rust dylib to bundle..."
cp "${RUST_DYLIB}" "${OUTPUT_BINARY}"

if [ $? -eq 0 ]; then
    echo "✅ Rust library copied successfully"
else
    echo "❌ Failed to copy Rust library"
    exit 1
fi

# Update the install name for the dynamic library
echo "🔧 Updating install name..."
install_name_tool -id "@rpath/sotf_driver" "${OUTPUT_BINARY}"

# Copy Info.plist to bundle
echo "📋 Copying Info.plist..."
cp "${PROJECT_DIR}/driver_bundle/Info.plist" "${BUNDLE_DIR}/Contents/Info.plist"

# Verify the binary
echo "🔍 Verifying binary..."
file "${OUTPUT_BINARY}"
otool -L "${OUTPUT_BINARY}"

# Set proper permissions
echo "🔐 Setting permissions..."
chmod 755 "${OUTPUT_BINARY}"
chmod 644 "${BUNDLE_DIR}/Contents/Info.plist"

# Verify bundle structure
echo "📋 Bundle structure:"
ls -lR "${BUNDLE_DIR}"

echo ""
echo "✅ Build complete!"
echo ""
echo "📍 Driver bundle location: ${BUNDLE_DIR}"
echo ""
echo "Next steps:"
echo "  1. Install: sudo ./scripts/install_driver.sh"
echo "  2. Load: sudo killall coreaudiod"
echo "  3. Check: system_profiler SPAudioDataType"
echo ""
