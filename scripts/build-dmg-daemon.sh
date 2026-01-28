#!/bin/bash
#
# Build script for SotF macOS distribution
#
# Creates a signed and notarized installer package (.pkg) containing:
#   - SotF Toolbar.app (menu bar app) -> /Applications/
#   - SotFHAL.driver (HAL audio driver) -> /Library/Audio/Plug-Ins/HAL/
#   - sotf-daemon (embedded in app)
#
# Bundle identifiers:
#   - org.spinorama.sotf-toolbar  (menu bar app)
#   - org.spinorama.sotf-hal      (HAL driver)
#   - org.spinorama.sotf-daemon   (background daemon)
#
# Usage:
#   ./build-dmg-daemon.sh                    # Build unsigned pkg (for local testing)
#   ./build-dmg-daemon.sh --sign             # Build signed pkg (requires Developer ID)
#   ./build-dmg-daemon.sh --sign --notarize  # Build, sign, and notarize (for distribution)
#   ./build-dmg-daemon.sh --dmg              # Build DMG instead of pkg (legacy)
#
# Environment variables:
#   DEVELOPER_ID             - Developer ID Application certificate name
#                              Example: "Developer ID Application: Your Name (TEAMID)"
#   INSTALLER_DEVELOPER_ID   - Developer ID Installer certificate name (for pkg signing)
#                              Example: "Developer ID Installer: Your Name (TEAMID)"
#   APPLE_ID                 - Apple ID email for notarization
#   APPLE_TEAM_ID            - Apple Developer Team ID
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
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
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
BUILD_DMG=false  # Default to pkg, use --dmg for legacy DMG output

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
        --dmg)
            BUILD_DMG=true
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
            echo "  --dmg         Build DMG instead of pkg installer (legacy)"
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

# Build the HAL driver bundle (Swift)
build_hal_driver() {
    if ! $BUILD_HAL; then
        log_warning "Skipping HAL driver build (--no-hal specified)"
        return 0
    fi

    log_info "Building Swift HAL driver..."

    local HAL_SWIFT_DIR="$HAL_DRIVER_DIR/swift"
    local HAL_SOURCES_DIR="$HAL_SWIFT_DIR/Sources"

    # Check for Swift sources
    if [ ! -d "$HAL_SOURCES_DIR" ]; then
        log_warning "Swift HAL driver sources not found at $HAL_SOURCES_DIR"
        BUILD_HAL=false
        return 0
    fi

    # Create driver bundle structure
    mkdir -p "$DRIVER_BUNDLE/Contents/MacOS"
    mkdir -p "$DRIVER_BUNDLE/Contents/Resources"

    # Find all Swift source files
    local SWIFT_FILES=(
        "$HAL_SOURCES_DIR/Timing.swift"
        "$HAL_SOURCES_DIR/RingBuffer.swift"
        "$HAL_SOURCES_DIR/SharedMemory.swift"
        "$HAL_SOURCES_DIR/SotFHALDriver.swift"
    )

    log_info "Compiling Swift HAL driver..."

    # Compile Swift to a bundle
    swiftc \
        -emit-library \
        -o "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL" \
        -module-name SotFHAL \
        -import-objc-header "$HAL_SOURCES_DIR/BridgingHeader.h" \
        -Xlinker -bundle \
        -Xlinker -rpath -Xlinker @loader_path/../Frameworks \
        -framework CoreAudio \
        -framework CoreFoundation \
        -framework Foundation \
        -O \
        "${SWIFT_FILES[@]}"

    # Verify it's a bundle
    local FILETYPE=$(otool -hv "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL" | grep -A1 filetype | tail -1 | awk '{print $5}')
    if [ "$FILETYPE" != "BUNDLE" ]; then
        log_warning "Binary is $FILETYPE instead of BUNDLE, trying alternative linking..."

        # Compile to object files first
        local BUILD_TMP="$DMG_DIR/hal_build"
        mkdir -p "$BUILD_TMP"

        for f in "${SWIFT_FILES[@]}"; do
            local BASENAME=$(basename "$f" .swift)
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
            -o "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL"

        rm -rf "$BUILD_TMP"
        FILETYPE=$(otool -hv "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL" | grep -A1 filetype | tail -1 | awk '{print $5}')
    fi

    log_info "HAL driver binary type: $FILETYPE"

    # Copy Info.plist from Swift sources (already configured)
    cp "$HAL_SWIFT_DIR/Info.plist" "$DRIVER_BUNDLE/Contents/Info.plist"

    # Set permissions
    chmod 755 "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL"
    chmod 644 "$DRIVER_BUNDLE/Contents/Info.plist"

    log_success "Swift HAL driver bundle created"
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

    # Copy menubar icon assets to Resources
    local ICON_ASSETS_DIR="$CONFIGBAR_DIR/assets"
    if [ -f "$ICON_ASSETS_DIR/icon_16.png" ]; then
        log_info "Copying menubar icon assets..."
        cp "$ICON_ASSETS_DIR/icon_16.png" "$APP_BUNDLE/Contents/Resources/"
        cp "$ICON_ASSETS_DIR/icon_16@2x.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        cp "$ICON_ASSETS_DIR/icon_18.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        cp "$ICON_ASSETS_DIR/icon_18@2x.png" "$APP_BUNDLE/Contents/Resources/" 2>/dev/null || true
        log_success "Menubar icon assets copied"
    else
        log_warning "Menubar icon assets not found at $ICON_ASSETS_DIR"
    fi

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

# Create installer package (.pkg)
create_pkg() {
    log_info "Creating installer package..."

    local pkg_path="$DMG_DIR/SotF-Toolbar-$VERSION.pkg"
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

exit 0
PREINSTALL
    chmod +x "$pkg_scripts/preinstall"

    # Build component packages
    log_info "Building component packages..."

    # App component
    pkgbuild \
        --root "$pkg_root/Applications" \
        --install-location "/Applications" \
        --identifier "$TOOLBAR_BUNDLE_ID" \
        --version "$VERSION" \
        --scripts "$pkg_scripts" \
        "$pkg_components/SotFToolbar.pkg"

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
    <options customize="never" require-scripts="true" rootVolumeOnly="true"/>

    <welcome file="welcome.html"/>
    <conclusion file="conclusion.html"/>

    <pkg-ref id="$TOOLBAR_BUNDLE_ID"/>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <pkg-ref id="$HAL_BUNDLE_ID"/>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML

    <options hostArchitectures="arm64,x86_64"/>

    <choices-outline>
        <line choice="default">
            <line choice="$TOOLBAR_BUNDLE_ID"/>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
            <line choice="$HAL_BUNDLE_ID"/>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML
        </line>
    </choices-outline>

    <choice id="default"/>
    <choice id="$TOOLBAR_BUNDLE_ID" visible="false">
        <pkg-ref id="$TOOLBAR_BUNDLE_ID"/>
    </choice>
DISTXML

    if $BUILD_HAL; then
        cat >> "$DMG_DIR/distribution.xml" << DISTXML
    <choice id="$HAL_BUNDLE_ID" visible="false">
        <pkg-ref id="$HAL_BUNDLE_ID"/>
    </choice>
DISTXML
    fi

    cat >> "$DMG_DIR/distribution.xml" << DISTXML

    <pkg-ref id="$TOOLBAR_BUNDLE_ID" version="$VERSION" onConclusion="none">SotFToolbar.pkg</pkg-ref>
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
        <li><strong>SotF Toolbar</strong> - Menu bar application for controlling the audio engine</li>
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
            <li>Launch <strong>SotF Toolbar</strong> from your Applications folder</li>
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

    # Build the distribution package
    log_info "Building distribution package..."

    if $SIGN && [ -n "${INSTALLER_DEVELOPER_ID:-}" ]; then
        productbuild \
            --distribution "$DMG_DIR/distribution.xml" \
            --package-path "$pkg_components" \
            --resources "$DMG_DIR" \
            --sign "$INSTALLER_DEVELOPER_ID" \
            "$pkg_path"
        log_success "Signed installer package created"
    else
        productbuild \
            --distribution "$DMG_DIR/distribution.xml" \
            --package-path "$pkg_components" \
            --resources "$DMG_DIR" \
            "$pkg_path"

        if $SIGN; then
            log_warning "INSTALLER_DEVELOPER_ID not set, package is unsigned"
            log_info "Set INSTALLER_DEVELOPER_ID='Developer ID Installer: Your Name (TEAMID)' to sign packages"
        fi
        log_success "Installer package created (unsigned)"
    fi

    # Cleanup
    rm -rf "$pkg_root" "$pkg_scripts" "$pkg_components"
    rm -f "$DMG_DIR/distribution.xml" "$DMG_DIR/welcome.html" "$DMG_DIR/conclusion.html"

    log_success "Package created at $pkg_path"
    echo "$pkg_path"
}

# Notarize the package
notarize_pkg() {
    if ! $NOTARIZE; then
        log_warning "Skipping notarization (use --notarize to enable)"
        return
    fi

    local pkg_path="$DMG_DIR/SotF-Toolbar-$VERSION.pkg"

    if [ ! -f "$pkg_path" ]; then
        log_error "Package not found at $pkg_path"
        exit 1
    fi

    log_info "Submitting package for notarization..."

    # Submit for notarization
    local submission_output
    submission_output=$(xcrun notarytool submit "$pkg_path" \
        --apple-id "$APPLE_ID" \
        --keychain-profile "autoeq-notarization" \
        --wait 2>&1)

    echo "$submission_output"

    if echo "$submission_output" | grep -q "status: Accepted"; then
        log_success "Notarization accepted"

        # Staple the notarization ticket
        log_info "Stapling notarization ticket..."
        xcrun stapler staple "$pkg_path"
        log_success "Notarization ticket stapled"

        # Verify
        xcrun stapler validate "$pkg_path"
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

    if $BUILD_DMG; then
        # Legacy DMG build
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

            if $SIGN; then
                log_info "Signed: Yes"
            else
                log_warning "Signed: No (use --sign for distribution)"
            fi

            if $NOTARIZE; then
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
        log_info "  /Applications/SotF\\ Toolbar.app/Contents/Resources/install-hal.sh"
    else
        # Package installer build (default)
        sign_app
        create_pkg
        notarize_pkg

        log_info "=========================================="
        log_success "Build complete!"
        log_info "=========================================="

        local pkg_path="$DMG_DIR/SotF-Toolbar-$VERSION.pkg"
        if [ -f "$pkg_path" ]; then
            log_info "Package: $pkg_path"
            log_info "Size: $(du -h "$pkg_path" | cut -f1)"

            if $SIGN && [ -n "${INSTALLER_DEVELOPER_ID:-}" ]; then
                log_info "Signed: Yes"
            else
                log_warning "Signed: No (set INSTALLER_DEVELOPER_ID for distribution)"
            fi

            if $NOTARIZE; then
                log_info "Notarized: Yes"
            else
                log_warning "Notarized: No (use --notarize for Gatekeeper)"
            fi
        fi

        log_info ""
        log_info "To install:"
        log_info "  open $pkg_path"
        log_info ""
        log_info "The installer will:"
        log_info "  - Install SotF Toolbar to /Applications/"
        log_info "  - Install HAL driver to /Library/Audio/Plug-Ins/HAL/"
        log_info "  - Restart CoreAudio automatically"
    fi
}

main "$@"