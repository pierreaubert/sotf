#!/bin/bash
#
# Uninstall Audio HAL Driver
#
# This script removes the driver bundle from the system and restarts Core Audio.

set -e

echo "🗑️  Uninstalling Audio HAL Driver..."

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (use sudo)"
    exit 1
fi

# Configuration
BUNDLE_NAME="AutoEQ.driver"
TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
TARGET_BUNDLE="${TARGET_DIR}/${BUNDLE_NAME}"

# Check if driver is installed
if [ ! -d "${TARGET_BUNDLE}" ]; then
    echo "ℹ️  Driver is not installed at ${TARGET_BUNDLE}"
    exit 0
fi

# Remove the bundle
echo "🗑️  Removing driver bundle..."
rm -rf "${TARGET_BUNDLE}"

# Verify removal
if [ -d "${TARGET_BUNDLE}" ]; then
    echo "❌ Failed to remove driver"
    exit 1
else
    echo "✅ Driver removed successfully"
fi

# Restart Core Audio daemon
echo "🔄 Restarting Core Audio daemon..."
launchctl kickstart -k system/com.apple.audio.coreaudiod

echo ""
echo "✅ Uninstallation complete!"
echo ""
echo "The driver has been removed from the system."
echo ""
