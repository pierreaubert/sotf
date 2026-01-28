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
DRIVER_SOURCE="$DMG_DIR/SotFHAL.driver"

# Target locations
APP_TARGET="/Applications/SotF Toolbar.app"
HAL_TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
HAL_TARGET="$HAL_TARGET_DIR/SotFHAL.driver"
LAUNCHAGENTS_DIR="$HOME/Library/LaunchAgents"

# Options
INSTALL_HAL=false
INSTALL_AUTOSTART=false
DEV_MODE=false

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
        --dev)
            DEV_MODE=true
            INSTALL_HAL=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --hal        Install HAL driver (requires sudo)"
            echo "  --autostart  Install LaunchAgent for auto-start"
            echo "  --dev        Dev mode: build and install HAL driver only (quick iteration)"
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

# Dev mode: quick Swift HAL driver build and install only
if $DEV_MODE; then
    log_info "Dev mode: Building and installing Swift HAL driver only..."

    cd "$PROJECT_ROOT"

    HAL_DRIVER_DIR="$PROJECT_ROOT/crates/driver-hal"
    HAL_SWIFT_DIR="$HAL_DRIVER_DIR/swift"
    HAL_SOURCES_DIR="$HAL_SWIFT_DIR/Sources"

    # Check for Swift sources
    if [ ! -d "$HAL_SOURCES_DIR" ]; then
        log_error "Swift HAL driver sources not found at $HAL_SOURCES_DIR"
        exit 1
    fi

    # Create temp build directory
    BUILD_TMP="$PROJECT_ROOT/target/hal-dev-build"
    mkdir -p "$BUILD_TMP"

    # Find all Swift source files
    SWIFT_FILES=(
        "$HAL_SOURCES_DIR/Timing.swift"
        "$HAL_SOURCES_DIR/RingBuffer.swift"
        "$HAL_SOURCES_DIR/SharedMemory.swift"
        "$HAL_SOURCES_DIR/SotFHALDriver.swift"
    )

    log_info "Compiling Swift HAL driver..."

    # Compile Swift to a bundle
    swiftc \
        -emit-library \
        -o "$BUILD_TMP/SotFHAL" \
        -module-name SotFHAL \
        -import-objc-header "$HAL_SOURCES_DIR/BridgingHeader.h" \
        -Xlinker -bundle \
        -Xlinker -rpath -Xlinker @loader_path/../Frameworks \
        -framework CoreAudio \
        -framework CoreFoundation \
        -framework Foundation \
        -O \
        "${SWIFT_FILES[@]}"

    if [ ! -f "$BUILD_TMP/SotFHAL" ]; then
        log_error "Swift compilation failed"
        exit 1
    fi

    # Verify it's a bundle
    FILETYPE=$(otool -hv "$BUILD_TMP/SotFHAL" | grep -A1 filetype | tail -1 | awk '{print $5}')
    if [ "$FILETYPE" != "BUNDLE" ]; then
        log_warning "Binary is $FILETYPE instead of BUNDLE, trying alternative linking..."

        # Compile to object files first
        for f in "${SWIFT_FILES[@]}"; do
            BASENAME=$(basename "$f" .swift)
            swiftc \
                -c \
                -o "$BUILD_TMP/$BASENAME.o" \
                -module-name SotFHAL \
                -import-objc-header "$HAL_SOURCES_DIR/BridgingHeader.h" \
                -framework CoreAudio \
                -framework CoreFoundation \
                -framework Foundation \
                -O \
                "$f"
        done

        # Link all object files as bundle
        ld -bundle \
            -arch arm64 \
            -platform_version macos 14.0.0 15.0.0 \
            -syslibroot /Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk \
            -L/usr/lib/swift \
            -lSystem \
            -lswiftCore \
            -lswiftFoundation \
            -lswiftCoreFoundation \
            -lswiftDarwin \
            -lswiftDispatch \
            -lswiftObjectiveC \
            -framework CoreAudio \
            -framework CoreFoundation \
            -framework Foundation \
            "$BUILD_TMP"/*.o \
            -o "$BUILD_TMP/SotFHAL"

        FILETYPE=$(otool -hv "$BUILD_TMP/SotFHAL" | grep -A1 filetype | tail -1 | awk '{print $5}')
    fi

    log_info "HAL driver binary type: $FILETYPE"
    log_success "Swift HAL driver compiled successfully"

    # Stop coreaudiod
    log_info "Stopping CoreAudio..."
    sudo killall coreaudiod 2>/dev/null || true
    sleep 1

    # Create driver bundle structure
    log_info "Creating driver bundle structure..."
    sudo mkdir -p "$HAL_TARGET/Contents/MacOS"
    sudo mkdir -p "$HAL_TARGET/Contents/Resources"

    # Copy the bundle binary
    log_info "Copying driver binary..."
    sudo cp "$BUILD_TMP/SotFHAL" "$HAL_TARGET/Contents/MacOS/SotFHAL"
    sudo chmod 755 "$HAL_TARGET/Contents/MacOS/SotFHAL"

    # Copy Info.plist from Swift sources
    log_info "Copying Info.plist..."
    sudo cp "$HAL_SWIFT_DIR/Info.plist" "$HAL_TARGET/Contents/Info.plist"
    sudo chmod 644 "$HAL_TARGET/Contents/Info.plist"

    # Sign the bundle
    log_info "Signing driver bundle..."
    sudo codesign --force --deep --sign - "$HAL_TARGET"

    # Clean up build temp
    rm -rf "$BUILD_TMP"

    # CoreAudio will restart automatically
    log_success "HAL driver installed!"

    # Wait for coreaudiod to restart and load driver
    sleep 2

    # Check if driver loaded
    log_info "Checking driver status..."
    if log show --predicate 'eventMessage contains "SotFHAL"' --last 30s 2>/dev/null | grep -q "Attempting to load"; then
        if log show --predicate 'eventMessage contains "SotFHAL"' --last 30s 2>/dev/null | grep -q "unable to load"; then
            log_error "Driver failed to load. Check: log show --predicate 'eventMessage contains \"SotF\"' --last 1m"
        else
            log_success "Driver appears to be loading"
        fi
    else
        log_info "Check driver status with: log show --predicate 'eventMessage contains \"SotF\"' --last 1m"
    fi

    echo ""
    echo -e "${GREEN}Dev install complete!${NC}"
    exit 0
fi

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

        # Remove old driver (both names)
        if [ -d "$HAL_TARGET" ]; then
            sudo rm -rf "$HAL_TARGET"
        fi
        if [ -d "$HAL_TARGET_DIR/sotf.driver" ]; then
            sudo rm -rf "$HAL_TARGET_DIR/sotf.driver"
        fi

        # Copy driver
        sudo cp -R "$DRIVER_SOURCE" "$HAL_TARGET_DIR/"

        # Set ownership and permissions
        sudo chown -R root:wheel "$HAL_TARGET"
        sudo chmod -R 755 "$HAL_TARGET"
        sudo chmod 644 "$HAL_TARGET/Contents/Info.plist"

        # Sign with simple ad-hoc signature
        # Use runtime option to be compatible with hardened runtime
        sudo codesign --force --deep --sign - --options runtime "$HAL_TARGET"

        log_success "HAL driver installed to $HAL_TARGET"

        # Restart CoreAudio
        log_info "Restarting CoreAudio..."
        if sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod >/dev/null 2>&1; then
             log_success "CoreAudio restarted (via launchctl)"
        else
             sudo killall coreaudiod 2>/dev/null || true
             log_success "CoreAudio restarted (via killall)"
        fi
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
