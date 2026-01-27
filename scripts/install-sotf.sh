#!/bin/bash
#
# SotF Installer Script
#
# Installs all SotF components:
#   - SotF Toolbar app to /Applications
#   - HAL driver to /Library/Audio/Plug-Ins/HAL (optional)
#   - LaunchAgents for auto-start (optional)
#
# Bundle identifiers:
#   - org.spinorama.sotf-toolbar
#   - org.spinorama.sotf-hal
#   - org.spinorama.sotf-daemon
#
# Usage:
#   ./install-sotf.sh                  # Install app only
#   ./install-sotf.sh --hal            # Install app + HAL driver
#   ./install-sotf.sh --hal --autostart  # Install all + LaunchAgent
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Bundle identifiers
TOOLBAR_BUNDLE_ID="org.spinorama.sotf-toolbar"
HAL_BUNDLE_ID="org.spinorama.sotf-hal"
DAEMON_BUNDLE_ID="org.spinorama.sotf-daemon"

# Source locations (from build)
DMG_DIR="$PROJECT_ROOT/target/daemon-dmg"
APP_SOURCE="$DMG_DIR/SotF Toolbar.app"
DRIVER_SOURCE="$DMG_DIR/sotf.driver"

# Target locations
APP_TARGET="/Applications/SotF Toolbar.app"
HAL_TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
HAL_TARGET="$HAL_TARGET_DIR/sotf.driver"
LAUNCHAGENTS_DIR="$HOME/Library/LaunchAgents"

# Options
INSTALL_HAL=false
INSTALL_AUTOSTART=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --hal)
            INSTALL_HAL=true
            shift
            ;;
        --autostart)
            INSTALL_AUTOSTART=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --hal        Install HAL driver (requires sudo)"
            echo "  --autostart  Install LaunchAgent for auto-start"
            echo "  --help       Show this help message"
            echo ""
            echo "Bundle identifiers:"
            echo "  Toolbar: $TOOLBAR_BUNDLE_ID"
            echo "  HAL:     $HAL_BUNDLE_ID"
            echo "  Daemon:  $DAEMON_BUNDLE_ID"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SotF Installer${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check if build exists
if [ ! -d "$APP_SOURCE" ]; then
    log_error "App bundle not found at $APP_SOURCE"
    log_info "Please run ./scripts/build-dmg-daemon.sh first"
    exit 1
fi

# Step 1: Install app
log_info "[1/4] Installing SotF Toolbar app..."

# Stop running instances
if pgrep -x "sotf-toolbar" > /dev/null 2>&1; then
    log_info "Stopping running Toolbar..."
    killall "sotf-toolbar" 2>/dev/null || true
fi

if pgrep -x "sotf-daemon" > /dev/null 2>&1; then
    log_info "Stopping running daemon..."
    killall "sotf-daemon" 2>/dev/null || true
fi

# Remove old app if exists
if [ -d "$APP_TARGET" ]; then
    log_info "Removing old installation..."
    rm -rf "$APP_TARGET"
fi

# Copy new app
cp -R "$APP_SOURCE" "$APP_TARGET"
log_success "App installed to $APP_TARGET"

# Step 2: Install HAL driver (optional)
log_info "[2/4] HAL driver installation..."

if $INSTALL_HAL; then
    if [ ! -d "$DRIVER_SOURCE" ]; then
        # Try from app bundle
        DRIVER_SOURCE="$APP_TARGET/Contents/Resources/sotf.driver"
    fi

    if [ ! -d "$DRIVER_SOURCE" ]; then
        log_warning "HAL driver not found, skipping"
    else
        log_info "Installing HAL driver (requires sudo)..."

        # Create target directory
        sudo mkdir -p "$HAL_TARGET_DIR"

        # Remove old driver
        if [ -d "$HAL_TARGET" ]; then
            sudo rm -rf "$HAL_TARGET"
        fi

        # Copy driver
        sudo cp -R "$DRIVER_SOURCE" "$HAL_TARGET_DIR/"

        # Set permissions
        sudo chmod -R 755 "$HAL_TARGET"
        sudo chmod 644 "$HAL_TARGET/Contents/Info.plist"

        # Sign with ad-hoc signature
        sudo codesign --force --deep --sign - "$HAL_TARGET"

        log_success "HAL driver installed to $HAL_TARGET"

        # Restart CoreAudio
        log_info "Restarting CoreAudio..."
        sudo killall coreaudiod 2>/dev/null || true
        log_success "CoreAudio restarted"
    fi
else
    log_info "Skipping HAL driver (use --hal to install)"
fi

# Step 3: Install LaunchAgent (optional)
log_info "[3/4] LaunchAgent installation..."

if $INSTALL_AUTOSTART; then
    mkdir -p "$LAUNCHAGENTS_DIR"

    # Create LaunchAgent plist for toolbar
    cat > "$LAUNCHAGENTS_DIR/$TOOLBAR_BUNDLE_ID.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$TOOLBAR_BUNDLE_ID</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Applications/SotF Toolbar.app/Contents/MacOS/sotf-toolbar</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>LimitLoadToSessionType</key>
    <array>
        <string>Aqua</string>
    </array>
    <key>StandardOutPath</key>
    <string>/tmp/sotf-toolbar.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/sotf-toolbar.error.log</string>
</dict>
</plist>
EOF

    # Load the agent
    launchctl load "$LAUNCHAGENTS_DIR/$TOOLBAR_BUNDLE_ID.plist" 2>/dev/null || true

    log_success "LaunchAgent installed and loaded"
else
    log_info "Skipping LaunchAgent (use --autostart to install)"
fi

# Step 4: Start the app
log_info "[4/4] Starting SotF Toolbar..."

open "$APP_TARGET"
log_success "SotF Toolbar started"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Installation Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Installed components:"
echo "  - SotF Toolbar: $APP_TARGET"
if $INSTALL_HAL && [ -d "$HAL_TARGET" ]; then
    echo "  - HAL Driver: $HAL_TARGET"
fi
if $INSTALL_AUTOSTART; then
    echo "  - LaunchAgent: $LAUNCHAGENTS_DIR/$TOOLBAR_BUNDLE_ID.plist"
fi
echo ""
echo "The toolbar should now appear in your menu bar."
echo ""
if ! $INSTALL_HAL; then
    echo "To install HAL driver later:"
    echo "  $APP_TARGET/Contents/Resources/install-hal.sh"
    echo ""
fi
echo "To uninstall:"
echo "  $SCRIPT_DIR/uninstall-sotf.sh"
echo ""
