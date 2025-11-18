#!/bin/bash
#
# Install Audio HAL Driver
#
# This script installs the driver bundle into the system HAL plugins directory
# and restarts the Core Audio daemon to load the new driver.

set -e

echo "📦 Installing Audio HAL Driver..."

# Check if running as root
#if [ "$EUID" -ne 0 ]; then
#    echo "❌ Please run as root (use sudo)"
#    exit 1
#fi

# Configuration
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)/.."
BUNDLE_NAME="sotf.driver"
WORKSPACE_DIR="${PROJECT_DIR}/../target"
SOURCE_BUNDLE="${WORKSPACE_DIR}/${BUNDLE_NAME}"
TARGET_DIR="$HOME/Library/Audio/Plug-Ins/HAL"
TARGET_BUNDLE="${TARGET_DIR}/${BUNDLE_NAME}"

# Verify source bundle exists
if [ ! -d "${SOURCE_BUNDLE}" ]; then
    echo "❌ Source bundle not found: ${SOURCE_BUNDLE}"
    echo "   Please run ./build_driver.sh first"
    exit 1
fi

# Verify the bundle has the binary
if [ ! -f "${SOURCE_BUNDLE}/Contents/MacOS/sotf_driver" ]; then
    echo "❌ Bundle binary not found"
    echo "   Please run ./build_driver.sh first"
    exit 1
fi

# Create target directory if it doesn't exist
echo "📁 Creating HAL plugins directory..."
mkdir -p "${TARGET_DIR}"

# Remove old version if it exists
if [ -d "${TARGET_BUNDLE}" ]; then
    echo "🗑️  Removing old driver..."
    rm -rf "${TARGET_BUNDLE}"
fi

# Copy the bundle
echo "📋 Copying driver bundle..."
cp -R "${SOURCE_BUNDLE}" "${TARGET_DIR}/"

# Set ownership and permissions
echo "🔐 Setting permissions..."
# /usr/sbin/chown -R root:wheel "${TARGET_BUNDLE}"
/bin/chmod -R 755 "${TARGET_BUNDLE}"
/bin/chmod 644 "${TARGET_BUNDLE}/Contents/Info.plist"

# Verify installation
if [ -d "${TARGET_BUNDLE}" ]; then
    echo "✅ Driver installed successfully to ${TARGET_BUNDLE}"
else
    echo "❌ Installation failed"
    exit 1
fi

# Sign the driver with ad-hoc signature
echo "🔐 Signing driver bundle..."
codesign --force --deep --sign - "${TARGET_BUNDLE}"

echo "✅ Driver signed"
echo ""

# Verify signature
echo "🔍 Verifying signature..."
codesign -dv "${TARGET_BUNDLE}" 2>&1 | head -10

echo ""
echo "✅ Driver fixed and signed successfully!"
echo ""
echo "Next steps:"


# Try to restart coreaudiod (may fail with SIP enabled)
if killall coreaudiod 2>/dev/null; then
    echo "✅ Core Audio daemon restarted successfully"
    sleep 2
else
    echo "⚠️  Could not restart Core Audio automatically"
    echo "   This is normal with SIP enabled. Please manually restart:"
    echo ""
    echo "   Option 1: Run this command:"
    echo "   sudo killall coreaudiod"
    echo ""
    echo "   Option 2: Reboot your Mac"
    echo ""
fi

echo ""
echo "✅ Driver files installed successfully!"
echo ""
echo "After Core Audio restarts, the driver should be available in:"
echo "  - Audio MIDI Setup.app"
echo "  - System Settings > Sound"
echo "  - system_profiler SPAudioDataType"
echo ""
echo "To check if driver is loaded:"
echo "  system_profiler SPAudioDataType | grep -A 20 -i 'sotf\|SotF'"
echo ""
echo "To check logs:"
echo "  log stream --predicate 'subsystem == \"com.apple.audio\"' --level debug"
echo "  Console.app (search for AutoEQ or AudioHAL)"
echo ""
echo "To uninstall:"
echo "  sudo ./uninstall_driver.sh"
echo ""
