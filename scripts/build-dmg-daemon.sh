#!/bin/bash
#
# Build script for SotF macOS distribution
#
# Creates a signed and notarized DMG containing:
#   - SotF Toolbar.app (menu bar app)
#   - SotFHAL.driver (HAL audio driver)
#   - sotf-daemon (embedded in app)
#
# Bundle identifiers:
#   - org.spinorama.sotf-toolbar  (menu bar app)
#   - org.spinorama.sotf-hal      (HAL driver)
#   - org.spinorama.sotf-daemon   (background daemon)
#
# Usage:
#   ./build-dmg-daemon.sh                    # Build unsigned DMG (for local testing)
#   ./build-dmg-daemon.sh --sign             # Build signed DMG (requires Developer ID)
#   ./build-dmg-daemon.sh --sign --notarize  # Build, sign, and notarize (for distribution)
#
# Environment variables:
#   DEVELOPER_ID         - Developer ID Application certificate name
#                          Example: "Developer ID Application: Your Name (TEAMID)"
#   APPLE_ID             - Apple ID email for notarization
#   APPLE_TEAM_ID        - Apple Developer Team ID
#
# Prerequisites:
#   - Xcode Command Line Tools
#   - Rust toolchain
#   - create-dmg (optional, for prettier DMG): brew install create-dmg
#

set -euo pipefail

# Configuration
APP_NAME="SotF Toolbar"
DRIVER_NAME="SotFHAL.driver"
DAEMON_BINARY="sotf-daemon"

# Bundle identifiers
TOOLBAR_BUNDLE_ID="org.spinorama.sotf-toolbar"
HAL_BUNDLE_ID="org.spinorama.sotf-hal"
DAEMON_BUNDLE_ID="org.spinorama.sotf-daemon"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "(.*)"/\\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

BUILD_DIR="$PROJECT_ROOT/target/release"
DMG_DIR="$PROJECT_ROOT/target/daemon-dmg"
APP_BUNDLE="$DMG_DIR/$APP_NAME.app"
DRIVER_BUNDLE="$DMG_DIR/$DRIVER_NAME"
CONFIGBAR_DIR="$PROJECT_ROOT/crates/daemon/configbar"
HAL_DRIVER_DIR="$PROJECT_ROOT/crates/driver-hal"

# Command line options (defaults: unsigned build for local testing)
SIGN=false
NOTARIZE=false
CLEAN=false
BUILD_HAL=true
DEBUG=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --sign)
            SIGN=true
            shift
            ;;
        --notarize)
            NOTARIZE=true
            SIGN=true  # Notarization requires signing
            shift
            ;;
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
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --sign        Sign the application with Developer ID"
            echo "  --notarize    Notarize the application (implies --sign)"
            echo "  --clean       Clean build directory before building"
            echo "  --debug, -d   Build in debug mode (faster, no optimizations)"
            echo "  --no-hal      Skip building HAL driver"
            echo "  --help        Show this help message"
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

    if $SIGN && [ -z "${DEVELOPER_ID:-}" ]; then
        log_error "DEVELOPER_ID environment variable not set"
        log_info "Set it to your Developer ID certificate name, e.g.:"
        log_info "  export DEVELOPER_ID='Developer ID Application: Your Name (TEAMID)'"
        exit 1
    fi

    if $NOTARIZE; then
        if [ -z "${APPLE_ID:-}" ]; then
            log_error "APPLE_ID environment variable not set"
            exit 1
        fi
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

# Build the daemon binary
build_daemon() {
    log_info "Building daemon binary ($BUILD_TYPE)..."

    cd "$PROJECT_ROOT"

    # Build with HAL support on macOS
    cargo build $CARGO_FLAGS -p sotf-daemon --features hal

    if [ ! -f "$BUILD_DIR/$DAEMON_BINARY" ]; then
        log_error "Daemon binary not found at $BUILD_DIR/$DAEMON_BINARY"
        exit 1
    fi

    log_success "Daemon binary built successfully"
}

# Build the HAL driver bundle
build_hal_driver() {
    if ! $BUILD_HAL; then
        log_warning "Skipping HAL driver build (--no-hal specified)"
        return 0
    fi

    log_info "Building HAL driver ($BUILD_TYPE)..."

    cd "$PROJECT_ROOT"

    # Build the Rust HAL library
    cargo build $CARGO_FLAGS -p driver-hal

    # Check if the dylib was created
    if [ ! -f "$BUILD_DIR/libsotf_hal.dylib" ]; then
        log_warning "HAL driver library not found, skipping HAL driver bundle"
        BUILD_HAL=false
        return 0
    fi

    # Create driver bundle structure
    mkdir -p "$DRIVER_BUNDLE/Contents/MacOS"
    mkdir -p "$DRIVER_BUNDLE/Contents/Resources"

    # Copy the dylib as the driver binary
    cp "$BUILD_DIR/libsotf_hal.dylib" "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL"

    # Update install name
    install_name_tool -id "@rpath/SotFHAL" "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL" 2>/dev/null || true

    # Create HAL driver Info.plist
    cat > "$DRIVER_BUNDLE/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>

    <key>CFBundleExecutable</key>
    <string>SotFHAL</string>

    <key>CFBundleIdentifier</key>
    <string>${HAL_BUNDLE_ID}</string>

    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>

    <key>CFBundleName</key>
    <string>SotF HAL</string>

    <key>CFBundlePackageType</key>
    <string>BNDL</string>

    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>

    <key>CFBundleSignature</key>
    <string>????</string>

    <key>CFBundleVersion</key>
    <string>1</string>

    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>MacOSX</string>
    </array>

    <key>CFPlugInDynamicRegisterFunction</key>
    <string></string>

    <key>CFPlugInDynamicRegistration</key>
    <string>NO</string>

    <key>CFPlugInFactories</key>
    <dict>
        <!-- Factory UUID for our driver - must match exported symbol -->
        <key>5A4E28B8-93F4-4B8A-B5E2-3D9F6A8C7E01</key>
        <string>SotFHALDriverFactory</string>
    </dict>

    <key>CFPlugInTypes</key>
    <dict>
        <!-- kAudioServerPlugInTypeUUID from AudioServerPlugIn.h -->
        <key>443ABAB8-E7B3-491A-B985-BEB9187030DB</key>
        <array>
            <string>5A4E28B8-93F4-4B8A-B5E2-3D9F6A8C7E01</string>
        </array>
    </dict>

    <key>SotFHalPlugIn</key>
    <dict>
        <key>Name</key>
        <string>SotFHal</string>

        <key>Manufacturer</key>
        <string>org.spinorama</string>

        <key>Version</key>
        <string>${VERSION}</string>
    </dict>

    <key>NSHumanReadableCopyright</key>
    <string>Copyright 2025 Pierre F. Aubert pierre@spinorama.org All rights reserved.</string>

    <key>OSBundleLibraries</key>
    <dict>
        <key>com.apple.CoreAudio</key>
        <string>1.0</string>
    </dict>
</dict>
</plist>
EOF

    # Set permissions
    chmod 755 "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL"
    chmod 644 "$DRIVER_BUNDLE/Contents/Info.plist"

    log_success "HAL driver bundle created"
}

# Build the Toolbar Swift app
build_toolbar() {
    log_info "Building Toolbar Swift app..."

    # Create app bundle structure
    mkdir -p "$APP_BUNDLE/Contents/MacOS"
    mkdir -p "$APP_BUNDLE/Contents/Resources"
    mkdir -p "$APP_BUNDLE/Contents/Helpers"

    # Compile Swift source
    swiftc \
        -o "$APP_BUNDLE/Contents/MacOS/sotf-toolbar" \
        "$CONFIGBAR_DIR/src/ConfigBar.swift" \
        -framework SwiftUI \
        -framework WebKit \
        -framework UserNotifications \
        -framework CoreAudio \
        -O

    log_success "Toolbar compiled successfully"
}

# Create app bundle with embedded daemon
create_app_bundle() {
    log_info "Creating app bundle..."

    # Create Toolbar Info.plist
    cat > "$APP_BUNDLE/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>sotf-toolbar</string>
    <key>CFBundleIdentifier</key>
    <string>${TOOLBAR_BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>SotF Toolbar</string>
    <key>CFBundleDisplayName</key>
    <string>Sound of the Future - Toolbar</string>
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
            sips -z $size $size "$input_image" --out "$iconset_dir/icon_${size}x${size}.png" 2>/dev/null || true
        done

        # Create @2x versions
        sips -z 32 32 "$input_image" --out "$iconset_dir/icon_16x16@2x.png" 2>/dev/null || true
        sips -z 64 64 "$input_image" --out "$iconset_dir/icon_32x32@2x.png" 2>/dev/null || true
        sips -z 256 256 "$input_image" --out "$iconset_dir/icon_128x128@2x.png" 2>/dev/null || true
        sips -z 512 512 "$input_image" --out "$iconset_dir/icon_256x256@2x.png" 2>/dev/null || true
        sips -z 1024 1024 "$input_image" --out "$iconset_dir/icon_512x512@2x.png" 2>/dev/null || true

        # Convert to icns
        iconutil -c icns "$iconset_dir" -o "$APP_BUNDLE/Contents/Resources/AppIcon.icns" 2>/dev/null || {
            log_warning "Failed to create icns, app will use default icon"
        }
    else
        log_warning "No icon source found, using default icon"
    fi

    rm -rf "$iconset_dir"
}

# Create entitlements files
create_entitlements() {
    log_info "Creating entitlements files..."

    # Daemon entitlements
    cat > "$DMG_DIR/daemon.entitlements" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <false/>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <key>com.apple.security.device.audio-input</key>
    <true/>
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
EOF

    # Toolbar entitlements
    cat > "$DMG_DIR/toolbar.entitlements" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <false/>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <key>com.apple.security.device.audio-input</key>
    <true/>
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
EOF

    # HAL driver entitlements
    cat > "$DMG_DIR/hal.entitlements" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <false/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>
EOF

    log_success "Entitlements files created"
}

# Create README for the DMG
create_readme() {
    log_info "Creating README..."

    cat > "$DMG_DIR/README.txt" << 'EOF'
SotF Toolbar - Sound of the Future Audio Engine

INSTALLATION
============
1. Drag "SotF Toolbar.app" to your Applications folder
2. Launch the app from Applications
3. The daemon will start automatically when the app launches
4. A speaker icon will appear in your menu bar

FIRST RUN
=========
On first run, macOS may show a security warning. To allow the app:
1. Open System Preferences -> Privacy & Security
2. Scroll down and click "Open Anyway" for SotF Toolbar

HAL DRIVER (Optional)
=====================
The HAL driver provides a virtual audio device for system-wide audio capture.
To install:
1. Open Terminal
2. Run: /Applications/SotF\ Toolbar.app/Contents/Resources/install-hal.sh

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
   /Applications/SotF\ Toolbar.app/Contents/Resources/uninstall-hal.sh
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

# Sign the application
sign_app() {
    if ! $SIGN;
 then
        log_warning "Skipping code signing (use --sign to enable)"
        # Ad-hoc sign for local testing
        log_info "Ad-hoc signing for local testing..."
        codesign --force --deep --sign - "$APP_BUNDLE" 2>/dev/null || true
        if $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
            codesign --force --deep --sign - "$DRIVER_BUNDLE" 2>/dev/null || true
        fi
        return
    fi

    log_info "Signing application..."

    create_entitlements

    # Sign the daemon helper first (inside-out signing)
    log_info "Signing daemon helper..."
    codesign --force --options runtime \
        --entitlements "$DMG_DIR/daemon.entitlements" \
        --sign "$DEVELOPER_ID" \
        --timestamp \
        "$APP_BUNDLE/Contents/Helpers/$DAEMON_BINARY"

    # Sign HAL driver if bundled in Resources
    if [ -d "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME" ]; then
        log_info "Signing bundled HAL driver..."
        codesign --force --options runtime \
            --entitlements "$DMG_DIR/hal.entitlements" \
            --sign "$DEVELOPER_ID" \
            --timestamp \
            "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME"
    fi

    # Sign the main executable
    log_info "Signing main executable..."
    codesign --force --options runtime \
        --entitlements "$DMG_DIR/toolbar.entitlements" \
        --sign "$DEVELOPER_ID" \
        --timestamp \
        "$APP_BUNDLE/Contents/MacOS/sotf-toolbar"

    # Sign the entire bundle
    log_info "Signing app bundle..."
    codesign --force --options runtime \
        --entitlements "$DMG_DIR/toolbar.entitlements" \
        --sign "$DEVELOPER_ID" \
        --timestamp \
        "$APP_BUNDLE"

    # Sign standalone HAL driver
    if $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
        log_info "Signing standalone HAL driver..."
        codesign --force --options runtime \
            --entitlements "$DMG_DIR/hal.entitlements" \
            --sign "$DEVELOPER_ID" \
            --timestamp \
            "$DRIVER_BUNDLE"
    fi

    # Verify signature
    codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

    rm -f "$DMG_DIR"/*.entitlements
    log_success "Application signed successfully"
}

# Create DMG
create_dmg_file() {
    log_info "Creating DMG..."

    local dmg_path="$DMG_DIR/SotF-Toolbar-$VERSION.dmg"
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
            --volname "SotF Toolbar" \
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

    if $SIGN && [ -f "$dmg_path" ]; then
        log_info "Signing DMG..."
        codesign --force --sign "$DEVELOPER_ID" --timestamp "$dmg_path"
        log_success "DMG signed"
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
    hdiutil create -volname "SotF Toolbar" \
        -srcfolder "$staging_dir" \
        -ov -format UDZO \
        "$dmg_path"

    rm -rf "$staging_dir"
    log_success "DMG created (with hdiutil)"
}

# Notarize the DMG
notarize_dmg() {
    if ! $NOTARIZE;
 then
        log_warning "Skipping notarization (use --notarize to enable)"
        return
    fi

    local dmg_path="$DMG_DIR/SotF-Toolbar-$VERSION.dmg"

    if [ ! -f "$dmg_path" ]; then
        log_error "DMG not found at $dmg_path"
        exit 1
    fi

    log_info "Submitting for notarization..."

    # Submit for notarization
    local submission_output
    submission_output=$(xcrun notarytool submit "$dmg_path" \
        --apple-id "$APPLE_ID" \
        --keychain-profile "autoeq-notarization" \
        --wait 2>&1)

    echo "$submission_output"

    if echo "$submission_output" | grep -q "status: Accepted"; then
        log_success "Notarization accepted"

        # Staple the notarization ticket
        log_info "Stapling notarization ticket..."
        xcrun stapler staple "$dmg_path"
        log_success "Notarization ticket stapled"

        # Verify
        xcrun stapler validate "$dmg_path"
        log_success "Notarization verified"
    else
        log_error "Notarization failed"
        log_info "Check the submission output above for details"

        # Extract submission ID for log retrieval
        local submission_id
        submission_id=$(echo "$submission_output" | grep -o 'id: [a-f0-9-]*' | head -1 | cut -d' ' -f2)
        if [ -n "$submission_id" ]; then
            log_info "To get detailed logs, run:"
            log_info "  xcrun notarytool log $submission_id --apple-id $APPLE_ID --keychain-profile autoeq-notarization"
        fi
        exit 1
    fi
}

# Main build process
main() {
    log_info "=========================================="
    log_info "Building SotF Toolbar v$VERSION"
    log_info "=========================================="
    log_info "Bundle IDs:"
    log_info "  Toolbar: $TOOLBAR_BUNDLE_ID"
    log_info "  Daemon:  $DAEMON_BUNDLE_ID"
    log_info "  HAL:     $HAL_BUNDLE_ID"
    log_info "=========================================="

    # Create DMG directory
    mkdir -p "$DMG_DIR"

    check_prerequisites
    clean_build
    build_daemon
    build_hal_driver
    build_toolbar
    create_app_bundle
    create_readme
    create_hal_scripts
    sign_app
    create_dmg_file
    notarize_dmg

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="

    local dmg_path="$DMG_DIR/SotF-Toolbar-$VERSION.dmg"
    if [ -f "$dmg_path" ]; then
        log_info "DMG: $dmg_path"
        log_info "Size: $(du -h "$dmg_path" | cut -f1)"

        if $SIGN;
 then
            log_info "Signed: Yes"
        else
            log_warning "Signed: No (use --sign for distribution)"
        fi

        if $NOTARIZE;
 then
            log_info "Notarized: Yes"
        else
            log_warning "Notarized: No (use --notarize for App Store/Gatekeeper)"
        fi
    fi

    log_info ""
    log_info "To install the app:"
    log_info "  open $dmg_path"
    log_info ""
    log_info "To install HAL driver after app installation:"
    log_info "  /Applications/SotF\ Toolbar.app/Contents/Resources/install-hal.sh"
}

main "$@"