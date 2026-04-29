#!/bin/bash
#
# Build script for SotF macOS distribution
#
# Creates an UNSIGNED installer package (.pkg) containing:
#   - SotF Systemwide.app (menu bar app) -> /Applications/
#   - SotFHAL.driver (HAL audio driver) -> /Library/Audio/Plug-Ins/HAL/
#   - sotf-daemon (embedded in app)
#
# Signing and notarization live in ./scripts/sign-macos.sh — run that after build.
#
# Bundle identifiers:
#   - org.spinorama.sotf-systemwide  (menu bar app)
#   - org.spinorama.sotf-hal      (HAL driver)
#   - org.spinorama.sotf-daemon   (background daemon)
#
# Usage:
#   ./build-dmg-daemon.sh         # Build unsigned pkg (default)
#   ./build-dmg-daemon.sh --dmg   # Build DMG instead of pkg (legacy)
#
# Prerequisites:
#   - Xcode Command Line Tools
#   - Rust toolchain
#   - create-dmg (optional, for prettier DMG): brew install create-dmg
#

set -euo pipefail

# Configuration
APP_NAME="SotF Systemwide"
DRIVER_NAME="SotFHAL.driver"
DAEMON_BINARY="sotf-daemon"
SYSTEMWIDE_BINARY="sotf-systemwide"

# Bundle identifiers
SYSTEMWIDE_BUNDLE_ID="org.spinorama.sotf-systemwide"
HAL_BUNDLE_ID="org.spinorama.sotf-hal"
DAEMON_BUNDLE_ID="org.spinorama.sotf-daemon"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

DMG_DIR="$PROJECT_ROOT/target/daemon-dmg"
APP_BUNDLE="$DMG_DIR/$APP_NAME.app"
DRIVER_BUNDLE="$DMG_DIR/$DRIVER_NAME"
CONFIGBAR_DIR="$PROJECT_ROOT/crates/systemwide/crates/daemon/configbar"
HAL_DRIVER_DIR="$PROJECT_ROOT/crates/systemwide/crates/driver-hal"

# Command line options
CLEAN=false
BUILD_HAL=true
DEBUG=false
BUILD_DMG=false  # Default to pkg, use --dmg for legacy DMG output

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean)
            CLEAN=true
            shift
            ;;
        --debug|-d)
            DEBUG=true
            shift
            ;;
        --no-hal)
            BUILD_HAL=false
            shift
            ;;
        --dmg)
            BUILD_DMG=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --clean       Clean build directory before building"
            echo "  --debug, -d   Build in debug mode (faster, no optimizations)"
            echo "  --no-hal      Skip building HAL driver"
            echo "  --dmg         Build DMG instead of pkg installer (legacy)"
            echo "  --help        Show this help message"
            echo ""
            echo "Signing: run ./sign-macos.sh after building"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Set build type
if $DEBUG; then
    BUILD_TYPE="debug"
    CARGO_FLAGS=""
else
    BUILD_TYPE="release"
    CARGO_FLAGS="--release"
fi

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo is not installed"
        exit 1
    fi

    if ! command -v swiftc &> /dev/null; then
        log_error "Swift compiler not found (install Xcode Command Line Tools)"
        exit 1
    fi

    if ! command -v codesign &> /dev/null; then
        log_error "Xcode Command Line Tools not installed"
        exit 1
    fi

    log_success "Prerequisites check passed"
}

# Clean build artifacts
clean_build() {
    if $CLEAN; then
        log_info "Cleaning build directory..."
        rm -rf "$DMG_DIR"
        cargo clean -p sotf-daemon
        cargo clean -p driver-hal
    fi
}

BUILD_DIR="$PROJECT_ROOT/target/$BUILD_TYPE"

# Build all components using just (reuses Justfile logic)
build_components() {
    log_info "Building all components via Justfile..."

    cd "$PROJECT_ROOT"

    if $DEBUG; then
        # Debug builds - call cargo directly since Justfile only has release targets
        log_info "Building daemon binary (debug)..."
        cargo build -p sotf-daemon --features hal

        if $BUILD_HAL; then
            log_info "Building HAL driver (debug)..."
            # For debug, we still use the same optimized Swift build
            just prod-hal-driver
        fi

        log_info "Building Systemwide app (debug)..."
        just prod-systemwide
    else
        # Release builds - use Justfile targets
        if $BUILD_HAL; then
            just prod-macos-daemon
        else
            just prod-daemon
            just prod-systemwide
        fi
    fi

    # Verify daemon binary exists
    if [ ! -f "$BUILD_DIR/$DAEMON_BINARY" ]; then
        log_error "Daemon binary not found at $BUILD_DIR/$DAEMON_BINARY"
        exit 1
    fi

    log_success "All components built successfully"
}

# Copy HAL driver from target to DMG directory
copy_hal_driver() {
    if ! $BUILD_HAL; then
        log_warning "Skipping HAL driver (--no-hal specified)"
        return 0
    fi

    local HAL_BUILD_DIR="$PROJECT_ROOT/target/release/SotFHAL.driver"

    if [ ! -d "$HAL_BUILD_DIR" ]; then
        log_warning "HAL driver not found at $HAL_BUILD_DIR"
        BUILD_HAL=false
        return 0
    fi

    log_info "Copying HAL driver to DMG directory..."
    # Remove any prior staged copy first — macOS `cp -R src dst` nests src
    # *inside* dst when dst already exists, leaving the stale top-level
    # bundle untouched and getting packaged into the pkg.
    rm -rf "$DRIVER_BUNDLE"
    cp -R "$HAL_BUILD_DIR" "$DRIVER_BUNDLE"
    log_success "HAL driver copied"
}

# Set up the Systemwide app bundle structure
setup_systemwide_bundle() {
    log_info "Setting up Systemwide app bundle..."

    # Create app bundle structure
    mkdir -p "$APP_BUNDLE/Contents/MacOS"
    mkdir -p "$APP_BUNDLE/Contents/Resources"
    mkdir -p "$APP_BUNDLE/Contents/Helpers"

    # Copy the compiled Systemwide binary
    cp "$BUILD_DIR/$SYSTEMWIDE_BINARY" "$APP_BUNDLE/Contents/MacOS/"
    chmod +x "$APP_BUNDLE/Contents/MacOS/$SYSTEMWIDE_BINARY"

    log_success "Systemwide bundle structure created"
}

# Regenerate PNG icons from SVG if SVG is newer
regenerate_icons_if_needed() {
    local ICON_ASSETS_DIR="$CONFIGBAR_DIR/assets"
    local SVG_FILE="$ICON_ASSETS_DIR/icon.svg"

    # Check if SVG exists
    if [ ! -f "$SVG_FILE" ]; then
        return 0
    fi

    # Check if rsvg-convert is available
    if ! command -v rsvg-convert &> /dev/null; then
        log_warning "rsvg-convert not found, skipping icon regeneration (install with: brew install librsvg)"
        return 0
    fi

    # Check if any PNG is missing or older than SVG
    local NEED_REGEN=false
    for png in "$ICON_ASSETS_DIR/icon_18.png" "$ICON_ASSETS_DIR/icon_18@2x.png" \
               "$ICON_ASSETS_DIR/icon_22.png" "$ICON_ASSETS_DIR/icon_22@2x.png"; do
        if [ ! -f "$png" ] || [ "$SVG_FILE" -nt "$png" ]; then
            NEED_REGEN=true
            break
        fi
    done

    if $NEED_REGEN; then
        log_info "Regenerating PNG icons from SVG..."
        rsvg-convert -w 18 -h 18 "$SVG_FILE" -o "$ICON_ASSETS_DIR/icon_18.png"
        rsvg-convert -w 36 -h 36 "$SVG_FILE" -o "$ICON_ASSETS_DIR/icon_18@2x.png"
        rsvg-convert -w 22 -h 22 "$SVG_FILE" -o "$ICON_ASSETS_DIR/icon_22.png"
        rsvg-convert -w 44 -h 44 "$SVG_FILE" -o "$ICON_ASSETS_DIR/icon_22@2x.png"
        log_success "PNG icons regenerated from SVG"
    fi
}

# Create app bundle with embedded daemon
create_app_bundle() {
    log_info "Creating app bundle..."

    # Regenerate icons from SVG if needed
    regenerate_icons_if_needed

    # Copy menubar icon assets to Resources
    local ICON_ASSETS_DIR="$CONFIGBAR_DIR/assets"
    if [ -f "$ICON_ASSETS_DIR/icon_22.png" ] || [ -f "$ICON_ASSETS_DIR/icon_18.png" ]; then
        log_info "Copying menubar icon assets..."
        cp "$ICON_ASSETS_DIR/icon_22.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        cp "$ICON_ASSETS_DIR/icon_22@2x.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        cp "$ICON_ASSETS_DIR/icon_18.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        cp "$ICON_ASSETS_DIR/icon_18@2x.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        log_success "Menubar icon assets copied"
    else
        log_warning "Menubar icon assets not found at $ICON_ASSETS_DIR"
    fi

    # Create Systemwide Info.plist
    cat > "$APP_BUNDLE/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${SYSTEMWIDE_BINARY}</string>
    <key>CFBundleIdentifier</key>
    <string>${SYSTEMWIDE_BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>SotF Systemwide</string>
    <key>CFBundleDisplayName</key>
    <string>Sound of the Future - Systemwide</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>SotF needs microphone access for audio calibration and room correction measurements.</string>
</dict>
</plist>
EOF

    # Create PkgInfo
    echo -n "APPL????" > "$APP_BUNDLE/Contents/PkgInfo"

    # Copy daemon binary to Helpers
    cp "$BUILD_DIR/$DAEMON_BINARY" "$APP_BUNDLE/Contents/Helpers/"
    chmod +x "$APP_BUNDLE/Contents/Helpers/$DAEMON_BINARY"

    # Copy HAL driver bundle to Resources (for user installation)
    if $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
        log_info "Bundling HAL driver..."
        cp -R "$DRIVER_BUNDLE" "$APP_BUNDLE/Contents/Resources/"
        log_success "HAL driver bundled in app"
    fi

    # Create app icon
    create_app_icon

    log_success "App bundle created at $APP_BUNDLE"
}

# Create app icon
create_app_icon() {
    log_info "Creating app icon..."

    local iconset_dir="$DMG_DIR/AppIcon.iconset"
    mkdir -p "$iconset_dir"

    # Check if there's an existing icon we can use
    if [ -f "$PROJECT_ROOT/crates/app-gpui/assets/sotf.jpg" ]; then
        local input_image="$PROJECT_ROOT/crates/app-gpui/assets/sotf.jpg"

        # Generate all required sizes
        local sizes=(16 32 64 128 256 512 1024)
        for size in "${sizes[@]}"; do
            sips -s format png -z $size $size "$input_image" --out "$iconset_dir/icon_${size}x${size}.png" 2>/dev/null || true
        done

        # Create @2x versions
        sips -s format png -z 32 32 "$input_image" --out "$iconset_dir/icon_16x16@2x.png" 2>/dev/null || true
        sips -s format png -z 64 64 "$input_image" --out "$iconset_dir/icon_32x32@2x.png" 2>/dev/null || true
        sips -s format png -z 128 128 "$input_image" --out "$iconset_dir/icon_64x64@2x.png" 2>/dev/null || true
        sips -s format png -z 256 256 "$input_image" --out "$iconset_dir/icon_128x128@2x.png" 2>/dev/null || true
        sips -s format png -z 512 512 "$input_image" --out "$iconset_dir/icon_256x256@2x.png" 2>/dev/null || true
        sips -s format png -z 1024 1024 "$input_image" --out "$iconset_dir/icon_512x512@2x.png" 2>/dev/null || true

        # Convert to icns
        iconutil -c icns "$iconset_dir" -o "$APP_BUNDLE/Contents/Resources/AppIcon.icns" 2>/dev/null || {
            log_warning "Failed to create icns, app will use default icon"
            rm -rf "$iconset_dir"
            return
        }

        rm -rf "$iconset_dir"
        log_success "App icon created"
    else
        log_warning "No icon source found, using default icon"
    fi
}

# Create README for the DMG
create_readme() {
    log_info "Creating README..."

    cat > "$DMG_DIR/README.txt" << 'EOF'
SotF Systemwide - Sound of the Future Audio Engine

INSTALLATION
============
1. Drag "SotF Systemwide.app" to your Applications folder
2. Launch the app from Applications
3. The daemon will start automatically when the app launches
4. A speaker icon will appear in your menu bar

FIRST RUN
=========
On first run, macOS may show a security warning. To allow the app:
1. Open System Preferences -> Privacy & Security
2. Scroll down and click "Open Anyway" for SotF Systemwide

HAL DRIVER (Optional)
=====================
The HAL driver provides a virtual audio device for system-wide audio capture.
To install:
1. Open Terminal
2. Run: /Applications/SotF\ Systemwide.app/Contents/Resources/install-hal.sh

Alternatively, you can use BlackHole as the audio source.

USAGE
=====
- Click the menu bar icon to open the configuration window
- Configure your audio source (HAL driver or BlackHole)
- Set input/output channels as needed
- Save/load plugin configurations

UNINSTALLATION
==============
1. Quit the app from the menu bar icon
2. Delete the app from Applications
3. To remove HAL driver:
   /Applications/SotF\ Systemwide.app/Contents/Resources/uninstall-hal.sh
4. Remove LaunchAgent (if installed):
   rm ~/Library/LaunchAgents/org.spinorama.sotf-*.plist

SUPPORT
=======
https://github.com/spinorama/sotf

EOF

    log_success "README created"
}

# Create install/uninstall scripts for HAL driver
create_hal_scripts() {
    log_info "Creating HAL driver scripts..."

    # Install script
    cat > "$DMG_DIR/install-hal.sh" << 'INSTALL_SCRIPT'
#!/bin/bash
#
# Install SotF HAL Driver
#
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRIVER_SOURCE="$SCRIPT_DIR/SotFHAL.driver"
TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
TARGET_BUNDLE="${TARGET_DIR}/SotFHAL.driver"

echo "Installing SotF HAL Driver..."

# Check for driver source
if [ ! -d "${DRIVER_SOURCE}" ]; then
    echo "Error: HAL driver not found at ${DRIVER_SOURCE}"
    exit 1
fi

# Create target directory if needed
sudo mkdir -p "${TARGET_DIR}"

# Remove old version if it exists
if [ -d "${TARGET_BUNDLE}" ]; then
    echo "Removing old driver..."
    sudo rm -rf "${TARGET_BUNDLE}"
fi

# Also remove old named version
if [ -d "${TARGET_DIR}/sotf.driver" ]; then
    echo "Removing legacy driver..."
    sudo rm -rf "${TARGET_DIR}/sotf.driver"
fi

# Copy the bundle
echo "Copying driver bundle..."
sudo cp -R "${DRIVER_SOURCE}" "${TARGET_DIR}/"

# Set permissions
sudo chmod -R 755 "${TARGET_BUNDLE}"
sudo chmod 644 "${TARGET_BUNDLE}/Contents/Info.plist"

# Sign with ad-hoc signature
echo "Signing driver bundle..."
sudo codesign --force --deep --sign - --options runtime "${TARGET_BUNDLE}"

# Restart CoreAudio
echo "Restarting CoreAudio..."
if sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod >/dev/null 2>&1; then
     echo "CoreAudio restarted (via launchctl)"
else
     sudo killall coreaudiod 2>/dev/null || true
     echo "CoreAudio restarted (via killall)"
fi

echo ""
echo "HAL driver installed successfully!"
echo ""
echo "The driver should now appear in Audio MIDI Setup and System Settings."
echo "If not visible, try logging out and back in."
INSTALL_SCRIPT
    chmod +x "$DMG_DIR/install-hal.sh"

    # Uninstall script
    cat > "$DMG_DIR/uninstall-hal.sh" << 'UNINSTALL_SCRIPT'
#!/bin/bash
#
# Uninstall SotF HAL Driver
#
set -e

TARGET_BUNDLE="/Library/Audio/Plug-Ins/HAL/SotFHAL.driver"
LEGACY_BUNDLE="/Library/Audio/Plug-Ins/HAL/sotf.driver"

echo "Uninstalling SotF HAL Driver..."

REMOVED=false

if [ -d "${TARGET_BUNDLE}" ]; then
    echo "Removing driver bundle..."
    sudo rm -rf "${TARGET_BUNDLE}"
    REMOVED=true
fi

if [ -d "${LEGACY_BUNDLE}" ]; then
    echo "Removing legacy driver bundle..."
    sudo rm -rf "${LEGACY_BUNDLE}"
    REMOVED=true
fi

if [ "$REMOVED" = false ]; then
    echo "HAL driver is not installed."
    exit 0
fi

# Restart CoreAudio
echo "Restarting CoreAudio..."
if sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod >/dev/null 2>&1; then
     echo "CoreAudio restarted (via launchctl)"
else
     sudo killall coreaudiod 2>/dev/null || true
     echo "CoreAudio restarted (via killall)"
fi

echo ""
echo "HAL driver uninstalled successfully!"
UNINSTALL_SCRIPT
    chmod +x "$DMG_DIR/uninstall-hal.sh"

    log_success "HAL driver scripts created"
}

# Create DMG
create_dmg_file() {
    log_info "Creating DMG..."

    local dmg_path="$DMG_DIR/SotF-Systemwide-$VERSION.dmg"
    local dmg_temp="$DMG_DIR/temp.dmg"

    rm -f "$dmg_path" "$dmg_temp"

    # Copy HAL scripts to app Resources
    if $BUILD_HAL;
 then
        cp "$DMG_DIR/install-hal.sh" "$APP_BUNDLE/Contents/Resources/"
        cp "$DMG_DIR/uninstall-hal.sh" "$APP_BUNDLE/Contents/Resources/"
        # Copy standalone driver to Resources if not already there
        if [ ! -d "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME" ] && [ -d "$DRIVER_BUNDLE" ]; then
            cp -R "$DRIVER_BUNDLE" "$APP_BUNDLE/Contents/Resources/"
        fi
    fi

    # Check if create-dmg is available (prettier DMG)
    if command -v create-dmg &> /dev/null; then
        log_info "Using create-dmg for styled DMG..."

        if create-dmg \
            --volname "SotF Systemwide" \
            --window-pos 200 120 \
            --window-size 600 400 \
            --icon-size 100 \
            --icon "$APP_NAME.app" 150 190 \
            --hide-extension "$APP_NAME.app" \
            --app-drop-link 450 185 \
            --no-internet-enable \
            "$dmg_path" \
            "$APP_BUNDLE" \
            "$DMG_DIR/README.txt" 2>&1; then
            log_success "DMG created (with create-dmg)"
        else
            # Clean up any temp DMG files left by create-dmg
            rm -f "$DMG_DIR"/rw.*.dmg 2>/dev/null || true

            if [ -f "$dmg_path" ]; then
                log_success "DMG created (with create-dmg)"
            else
                log_warning "create-dmg failed, falling back to hdiutil"
                create_dmg_hdiutil "$dmg_path"
            fi
        fi
    else
        create_dmg_hdiutil "$dmg_path"
    fi

    log_success "DMG created at $dmg_path"
    echo "$dmg_path"
}

# Create DMG using hdiutil (fallback)
create_dmg_hdiutil() {
    local dmg_path="$1"
    local staging_dir="$DMG_DIR/staging"

    rm -rf "$staging_dir"
    mkdir -p "$staging_dir"

    # Copy app to staging
    cp -R "$APP_BUNDLE" "$staging_dir/"

    # Copy README
    cp "$DMG_DIR/README.txt" "$staging_dir/"

    # Create symlink to /Applications
    ln -s /Applications "$staging_dir/Applications"

    # Create DMG
    hdiutil create -volname "SotF Systemwide" \
        -srcfolder "$staging_dir" \
        -ov -format UDZO \
        "$dmg_path"

    rm -rf "$staging_dir"
    log_success "DMG created (with hdiutil)"
}


# Create installer package (.pkg)
create_pkg() {
    log_info "Creating installer package..."

    local pkg_path="$DMG_DIR/SotF-Systemwide-$VERSION.pkg"
    local pkg_root="$DMG_DIR/pkg-root"
    local pkg_scripts="$DMG_DIR/pkg-scripts"
    local pkg_components="$DMG_DIR/pkg-components"

    rm -rf "$pkg_root" "$pkg_scripts" "$pkg_components"
    mkdir -p "$pkg_root/Applications"
    mkdir -p "$pkg_root/Library/Audio/Plug-Ins/HAL"
    mkdir -p "$pkg_scripts"
    mkdir -p "$pkg_components"

    # Copy app to pkg root
    cp -R "$APP_BUNDLE" "$pkg_root/Applications/"

    # Copy HAL driver to pkg root
    if $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
        cp -R "$DRIVER_BUNDLE" "$pkg_root/Library/Audio/Plug-Ins/HAL/"
    fi

    # Create postinstall script to restart CoreAudio
    cat > "$pkg_scripts/postinstall" << 'POSTINSTALL'
#!/bin/bash
# Post-installation script for SotF

# Restart CoreAudio to load the new HAL driver
echo "Restarting CoreAudio to load HAL driver..."
if launchctl kickstart -kp system/com.apple.audio.coreaudiod >/dev/null 2>&1; then
    echo "CoreAudio restarted (via launchctl)"
else
    killall coreaudiod 2>/dev/null || true
    echo "CoreAudio restarted (via killall)"
fi

# Set correct permissions on HAL driver
if [ -d "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver" ]; then
    chmod -R 755 "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver"
    chmod 644 "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver/Contents/Info.plist"
fi

echo "SotF installation complete!"
exit 0
POSTINSTALL
    chmod +x "$pkg_scripts/postinstall"

    # Create auto-launch scripts directory
    local launch_scripts="$DMG_DIR/launch-scripts"
    mkdir -p "$launch_scripts"

    # Create postinstall script for auto-launch component
    cat > "$launch_scripts/postinstall" << 'LAUNCHSCRIPT'
#!/bin/bash
# Launch SotF Systemwide after installation

# Get the user who initiated the installation
CONSOLE_USER=$(stat -f "%Su" /dev/console)

if [ -n "$CONSOLE_USER" ] && [ "$CONSOLE_USER" != "root" ]; then
    echo "Launching SotF Systemwide for user: $CONSOLE_USER"
    # Use launchctl to run as the console user
    sudo -u "$CONSOLE_USER" open -a "/Applications/SotF Systemwide.app" &
else
    echo "No console user found, skipping auto-launch"
fi

exit 0
LAUNCHSCRIPT
    chmod +x "$launch_scripts/postinstall"

    # Create empty root for auto-launch package (it just runs the script)
    local launch_root="$DMG_DIR/launch-root"
    mkdir -p "$launch_root"

    # Build auto-launch component package
    pkgbuild \
        --nopayload \
        --identifier "org.spinorama.sotf.autolaunch" \
        --version "$VERSION" \
        --scripts "$launch_scripts" \
        "$pkg_components/SotFAutoLaunch.pkg"

    # Create preinstall script to remove old versions
    cat > "$pkg_scripts/preinstall" << 'PREINSTALL'
#!/bin/bash
# Pre-installation script for SotF

# Remove legacy driver if it exists
if [ -d "/Library/Audio/Plug-Ins/HAL/sotf.driver" ]; then
    echo "Removing legacy HAL driver..."
    rm -rf "/Library/Audio/Plug-Ins/HAL/sotf.driver"
fi

# Remove old version of current driver
if [ -d "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver" ]; then
    echo "Removing old HAL driver..."
    rm -rf "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver"
fi

# Remove old menu bar app names so Systemwide replaces Toolbar/ConfigBar cleanly
for app in "/Applications/SotF Toolbar.app" "/Applications/SotF ConfigBar.app"; do
    if [ -d "$app" ]; then
        echo "Removing legacy app: $app"
        rm -rf "$app"
    fi
done

exit 0
PREINSTALL
    chmod +x "$pkg_scripts/preinstall"

    # Build component packages
    log_info "Building component packages..."

    # App component
    pkgbuild \
        --root "$pkg_root/Applications" \
        --install-location "/Applications" \
        --identifier "$SYSTEMWIDE_BUNDLE_ID" \
        --version "$VERSION" \
        --scripts "$pkg_scripts" \
        "$pkg_components/SotFSystemwide.pkg"

    # HAL driver component (if built)
    if $BUILD_HAL && [ -d "$pkg_root/Library/Audio/Plug-Ins/HAL/$DRIVER_NAME" ]; then
        pkgbuild \
            --root "$pkg_root/Library/Audio/Plug-Ins/HAL" \
            --install-location "/Library/Audio/Plug-Ins/HAL" \
            --identifier "$HAL_BUNDLE_ID" \
            --version "$VERSION" \
            "$pkg_components/SotFHAL.pkg"
    fi

    # Create distribution XML
    cat > "$DMG_DIR/distribution.xml" << DISTXML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>SotF - Sound of the Future</title>
    <organization>org.spinorama</organization>
    <domains enable_localSystem="true"/>
    <options customize="allow" require-scripts="true" rootVolumeOnly="true"/>

    <welcome file="welcome.html"/>
    <conclusion file="conclusion.html"/>

    <pkg-ref id="$SYSTEMWIDE_BUNDLE_ID"/>
    <pkg-ref id="org.spinorama.sotf.autolaunch"/>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <pkg-ref id="$HAL_BUNDLE_ID"/>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML

    <options hostArchitectures="arm64,x86_64"/>

    <choices-outline>
        <line choice="$SYSTEMWIDE_BUNDLE_ID"/>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
        <line choice="$HAL_BUNDLE_ID"/>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML
        <line choice="org.spinorama.sotf.autolaunch"/>
    </choices-outline>

    <choice id="$SYSTEMWIDE_BUNDLE_ID" title="SotF Systemwide" description="Menu bar application for controlling the systemwide audio engine" enabled="false" selected="true">
        <pkg-ref id="$SYSTEMWIDE_BUNDLE_ID"/>
    </choice>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <choice id="$HAL_BUNDLE_ID" title="HAL Audio Driver" description="Virtual audio driver for system-wide audio processing" enabled="false" selected="true">
        <pkg-ref id="$HAL_BUNDLE_ID"/>
    </choice>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <choice id="org.spinorama.sotf.autolaunch" title="Launch after installation" description="Start SotF Systemwide automatically after installation completes" selected="true">
        <pkg-ref id="org.spinorama.sotf.autolaunch"/>
    </choice>

    <pkg-ref id="$SYSTEMWIDE_BUNDLE_ID" version="$VERSION" onConclusion="none">SotFSystemwide.pkg</pkg-ref>
    <pkg-ref id="org.spinorama.sotf.autolaunch" version="$VERSION" onConclusion="none">SotFAutoLaunch.pkg</pkg-ref>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <pkg-ref id="$HAL_BUNDLE_ID" version="$VERSION" onConclusion="none">SotFHAL.pkg</pkg-ref>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML
</installer-gui-script>
DISTXML

    # Create welcome HTML
    cat > "$DMG_DIR/welcome.html" << 'WELCOME'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
        h1 { color: #333; }
        p { color: #666; line-height: 1.6; }
        .features { margin-top: 20px; }
        .features li { margin: 8px 0; }
    </style>
</head>
<body>
    <h1>Welcome to SotF</h1>
    <p><strong>Sound of the Future</strong> - Professional audio optimization and processing for macOS.</p>

    <p>This installer will install:</p>
    <ul class="features">
        <li><strong>SotF Systemwide</strong> - Menu bar application for controlling the systemwide audio engine</li>
        <li><strong>SotF HAL Driver</strong> - Virtual audio device for system-wide audio capture</li>
    </ul>

    <p>After installation, the HAL driver will appear as "SotF" in your audio devices.</p>
</body>
</html>
WELCOME

    # Create conclusion HTML
    cat > "$DMG_DIR/conclusion.html" << 'CONCLUSION'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
        h1 { color: #28a745; }
        p { color: #666; line-height: 1.6; }
        .next-steps { background: #f8f9fa; padding: 15px; border-radius: 8px; margin-top: 20px; }
        .next-steps h3 { margin-top: 0; color: #333; }
        code { background: #e9ecef; padding: 2px 6px; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>Installation Complete!</h1>
    <p>SotF has been successfully installed on your system.</p>

    <div class="next-steps">
        <h3>Getting Started</h3>
        <ol>
            <li>Launch <strong>SotF Systemwide</strong> from your Applications folder</li>
            <li>A speaker icon will appear in your menu bar</li>
            <li>Click the icon to configure audio settings</li>
            <li>Set "SotF" as your audio output device in System Settings → Sound</li>
        </ol>
    </div>

    <p style="margin-top: 20px; font-size: 12px; color: #999;">
        Note: CoreAudio has been restarted. The SotF audio device should now be visible in Audio MIDI Setup.
    </p>
</body>
</html>
CONCLUSION

    # Build the distribution package (unsigned — sign via ./scripts/sign-macos.sh)
    log_info "Building distribution package..."
    productbuild \
        --distribution "$DMG_DIR/distribution.xml" \
        --package-path "$pkg_components" \
        --resources "$DMG_DIR" \
        "$pkg_path"
    log_success "Installer package created (unsigned)"

    # Cleanup
    rm -rf "$pkg_root" "$pkg_scripts" "$pkg_components"
    rm -f "$DMG_DIR/distribution.xml" "$DMG_DIR/welcome.html" "$DMG_DIR/conclusion.html"

    log_success "Package created at $pkg_path"
    echo "$pkg_path"
}


# Main build process
main() {
    log_info "=========================================="
    log_info "Building SotF Systemwide v$VERSION"
    log_info "=========================================="
    log_info "Bundle IDs:"
    log_info "  Systemwide: $SYSTEMWIDE_BUNDLE_ID"
    log_info "  Daemon:  $DAEMON_BUNDLE_ID"
    log_info "  HAL:     $HAL_BUNDLE_ID"
    log_info "=========================================="

    # Create DMG directory
    mkdir -p "$DMG_DIR"

    check_prerequisites
    clean_build
    build_components
    copy_hal_driver
    setup_systemwide_bundle
    create_app_bundle

    if $BUILD_DMG; then
        # Legacy DMG build
        create_readme
        create_hal_scripts
        create_dmg_file

        log_info "=========================================="
        log_success "Build complete!"
        log_info "=========================================="

        local dmg_path="$DMG_DIR/SotF-Systemwide-$VERSION.dmg"
        if [ -f "$dmg_path" ]; then
            mkdir -p "$PROJECT_ROOT/dist"
            cp "$dmg_path" "$PROJECT_ROOT/dist/"
            log_info "DMG: $PROJECT_ROOT/dist/$(basename "$dmg_path")"
            log_info "Size: $(du -h "$dmg_path" | cut -f1)"
        fi

        log_info ""
        log_info "To sign: ./scripts/sign-macos.sh"
    else
        # Package installer build (default)
        create_pkg

        log_info "=========================================="
        log_success "Build complete!"
        log_info "=========================================="

        local pkg_path="$DMG_DIR/SotF-Systemwide-$VERSION.pkg"
        if [ -f "$pkg_path" ]; then
            mkdir -p "$PROJECT_ROOT/dist"
            cp "$pkg_path" "$PROJECT_ROOT/dist/"
            log_info "Package: $PROJECT_ROOT/dist/$(basename "$pkg_path")"
            log_info "Size: $(du -h "$pkg_path" | cut -f1)"
        fi

        log_info ""
        log_info "To sign: ./scripts/sign-macos.sh"
    fi
}

main "$@"
