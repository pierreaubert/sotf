#!/usr/bin/env bash
# Generate the Xcode project for SOTF Audio Units.
#
# The Xcode project is no longer hand-built — it's generated from project.yml
# by xcodegen. The build recipes (just build-au-all, etc.) call xcodegen
# automatically when project.yml is newer than the generated .xcodeproj, so
# you typically don't need to run this script directly.
#
# See:
#   - QUICKSTART.md — 5-minute setup
#   - README.md     — overview and per-arch flow
#   - SETUP_GUIDE.md — full architecture
#
# Run this script if you want to (re)generate the project right now without
# kicking off a build.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v xcodegen &>/dev/null; then
    echo "ERROR: xcodegen is not installed."
    echo "Install with: brew install xcodegen"
    exit 1
fi

echo "Generating SOTFAudioUnits.xcodeproj from project.yml..."
xcodegen generate
echo ""
echo "Done. Build with:"
echo "  just build-au-all              # both arches"
echo "  just build-au-all-arm64        # arm64 only"
echo "  just build-au-all-x86_64       # x86_64 only"
echo "  just install-au-all            # install host arch into ~/Applications"
