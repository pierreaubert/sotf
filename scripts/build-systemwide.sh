#!/bin/bash
#
# Build script for SotF macOS distribution
#
# Creates an installer package (.pkg) containing:
#   - sotf-systemwide.app (menu bar app) -> /Applications/
#   - SotFHAL.driver (HAL audio driver) -> /Library/Audio/Plug-Ins/HAL/
#   - sotf-daemon (embedded in app)
#
# If DEVELOPER_ID is set, app/daemon/HAL payload code is signed before
# packaging. Installer-container signing and notarization live in
# ./scripts/sign-macos.sh — run that after build.
#
# Bundle identifiers:
#   - org.spinorama.sotf-systemwide  (menu bar app)
#   - org.spinorama.sotf-hal      (HAL driver)
#   - org.spinorama.sotf-daemon   (background daemon)
#
# Usage:
#   ./build-systemwide.sh         # Build pkg (default; payload signed if DEVELOPER_ID is set)
#   ./build-systemwide.sh --dmg   # Build DMG instead of pkg (legacy)
#
# Prerequisites:
#   - Xcode Command Line Tools
#   - Rust toolchain
#   - create-dmg (optional, for prettier DMG): brew install create-dmg
#

set -euo pipefail

# Configuration
# APP_NAME is the .app bundle directory name and dist filename prefix.
# DRIVER_NAME is the system-extension HAL driver bundle name (kept as-is —
# changing it would invalidate user installations).
APP_NAME="sotf-systemwide"
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

# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"

# Extract version from root Cargo.toml
VERSION=$(sotf_version "$PROJECT_ROOT")
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

# Set build type. Release cuts use the `dist` profile (fat LTO +
# codegen-units=1) — see [profile.dist] in the root Cargo.toml. Cargo emits
# artifacts to `target/dist/` instead of `target/release/`, and the matching
# Swift recipes (dist-systemwide, dist-hal-driver) follow the same convention.
if $DEBUG; then
    BUILD_TYPE="debug"
    CARGO_FLAGS=""
else
    BUILD_TYPE="dist"
    CARGO_FLAGS="--profile dist"
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

    if [ ! -x /usr/sbin/pkgutil ] || [ ! -x /usr/bin/lsbom ]; then
        log_error "macOS package tools not found"
        exit 1
    fi

    log_success "Prerequisites check passed"
}

is_macho_file() {
    local file_path="$1"
    [ -f "$file_path" ] && file "$file_path" | grep -q "Mach-O"
}

sign_macho_file() {
    local file_path="$1"

    codesign --force --sign "$DEVELOPER_ID" \
        --options runtime \
        --timestamp \
        "$file_path"
}

sign_code_bundle() {
    local bundle_path="$1"

    codesign --force --deep --sign "$DEVELOPER_ID" \
        --options runtime \
        --timestamp \
        "$bundle_path"
    codesign --verify --verbose=2 --strict "$bundle_path"
}

sign_payload_code() {
    local root_dir="$1"

    log_info "Signing Mach-O payloads under ${root_dir#$PROJECT_ROOT/}..."
    while IFS= read -r -d '' candidate; do
        if is_macho_file "$candidate"; then
            sign_macho_file "$candidate"
            log_info "  Signed Mach-O: ${candidate#$root_dir/}"
        fi
    done < <(find "$root_dir" -type f -print0)

    log_info "Signing code bundles under ${root_dir#$PROJECT_ROOT/}..."
    while IFS= read -r bundle; do
        [ -d "$bundle" ] || continue
        sign_code_bundle "$bundle"
        log_info "  Signed bundle: ${bundle#$root_dir/}"
    done < <(
        find "$root_dir" -type d \( \
            -name "*.app" -o \
            -name "*.appex" -o \
            -name "*.bundle" -o \
            -name "*.driver" -o \
            -name "*.framework" -o \
            -name "*.xpc" \
        \) -print | awk '{ print length($0) "\t" $0 }' | sort -rn | cut -f2-
    )
}

sign_release_payloads() {
    if [ -z "${DEVELOPER_ID:-}" ]; then
        log_warning "DEVELOPER_ID not set; app/daemon/HAL payloads will not be Developer ID signed"
        return
    fi

    log_info "Signing release payloads with: $DEVELOPER_ID"
    if [ -d "$DRIVER_BUNDLE" ]; then
        sign_payload_code "$DRIVER_BUNDLE"
    fi
    if [ -d "$APP_BUNDLE" ]; then
        sign_payload_code "$APP_BUNDLE"
    fi
    log_success "Release payloads signed"
}

validate_pkg_payload() {
    local pkg_path="$1"
    local expanded_parent expanded_dir
    expanded_parent=$(mktemp -d)
    expanded_dir="$expanded_parent/pkg"

    /usr/sbin/pkgutil --expand "$pkg_path" "$expanded_dir"

    local app_bom="$expanded_dir/SotFSystemwide.pkg/Bom"
    if [ ! -f "$app_bom" ] ||
        ! /usr/bin/lsbom -s "$app_bom" | grep -qx "./Library/Application Support/SotF/org.spinorama.sotf-daemon.plist"; then
        rm -rf "$expanded_parent"
        log_error "Systemwide component payload is missing the daemon LaunchAgent plist"
        exit 1
    fi

    if $BUILD_HAL; then
        local hal_bom="$expanded_dir/SotFHAL.pkg/Bom"
        if [ ! -f "$hal_bom" ] ||
            ! /usr/bin/lsbom -s "$hal_bom" | grep -qx "./Library/Audio/Plug-Ins/HAL/$DRIVER_NAME/Contents/MacOS/SotFHAL"; then
            rm -rf "$expanded_parent"
            log_error "HAL component payload is missing $DRIVER_NAME/Contents/MacOS/SotFHAL"
            exit 1
        fi
    fi

    rm -rf "$expanded_parent"
    log_success "Package payload validated"
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
            # The Swift HAL driver always builds optimised; reuse the dist
            # recipe so its output ends up under target/dist/, matching the
            # BUILD_DIR computed above in non-debug mode.
            just dist-hal-driver
        fi

        log_info "Building Systemwide app (debug)..."
        just dist-systemwide
    else
        # Release cuts use the dist Justfile recipes (fat LTO + cgu=1).
        if $BUILD_HAL; then
            just dist-macos-daemon
        else
            just dist-daemon
            just dist-systemwide
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

    # Match the dist-hal-driver recipe's output dir; in debug mode the Swift
    # build is still produced via dist-hal-driver (the binary is always
    # optimised), so the path is the same regardless of BUILD_TYPE.
    local HAL_BUILD_DIR="$PROJECT_ROOT/target/dist/SotFHAL.driver"

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
    /usr/bin/ditto "$HAL_BUILD_DIR" "$DRIVER_BUNDLE"

    if [ ! -x "$DRIVER_BUNDLE/Contents/MacOS/SotFHAL" ]; then
        log_error "Staged HAL driver executable is missing or not executable"
        exit 1
    fi

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
    <key>CFBundleIconName</key>
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

    # Copy HAL driver bundle to Resources only for the legacy DMG/manual
    # installation path. The pkg installs HAL as its own component package.
    rm -rf "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME"
    if $BUILD_DMG && $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
        log_info "Bundling HAL driver..."
        /usr/bin/ditto "$DRIVER_BUNDLE" "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME"
        log_success "HAL driver bundled in app"
    fi

    # Create app icon
    create_app_icon

    # Compile sotf-systemwide.icon (Icon Composer Liquid Glass bundle) if
    # present. This produces an Assets.car alongside the legacy .icns so
    # macOS 26+ uses the modern icon while older systems fall back to icns.
    compile_modern_app_icon

    log_success "App bundle created at $APP_BUNDLE"
}

# Compile Icon Composer .icon bundle into Assets.car using actool.
# Falls through silently if the .icon bundle hasn't been authored yet —
# in that case the .icns produced by create_app_icon is the only icon.
compile_modern_app_icon() {
    local icon_bundle="$CONFIGBAR_DIR/assets/sotf-systemwide.icon"

    if [ ! -d "$icon_bundle" ]; then
        log_info "No sotf-systemwide.icon bundle found; using .icns only"
        return 0
    fi

    if ! xcrun --find actool >/dev/null 2>&1; then
        log_warning "actool not found (install Xcode); skipping .icon compilation"
        return 0
    fi

    log_info "Compiling sotf-systemwide.icon via actool..."

    # actool requires an Assets.xcassets parent. Stage one in a tmp dir
    # with the icon renamed to AppIcon.icon so CFBundleIconName=AppIcon
    # resolves it.
    local tmp_assets
    tmp_assets="$(mktemp -d)/Assets.xcassets"
    mkdir -p "$tmp_assets"
    cat > "$tmp_assets/Contents.json" << 'JSON'
{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON
    /usr/bin/ditto "$icon_bundle" "$tmp_assets/AppIcon.icon"

    local partial_plist
    partial_plist="$(mktemp)"

    if ! xcrun actool \
        --compile "$APP_BUNDLE/Contents/Resources" \
        --platform macosx \
        --minimum-deployment-target 13.0 \
        --app-icon AppIcon \
        --include-all-app-icons \
        --output-partial-info-plist "$partial_plist" \
        --target-device mac \
        "$tmp_assets" 2>&1 | sed 's/^/    /'; then
        log_warning "actool failed to compile sotf-systemwide.icon; .icns fallback will be used"
        rm -rf "$(dirname "$tmp_assets")"
        rm -f "$partial_plist"
        return 0
    fi

    rm -rf "$(dirname "$tmp_assets")"
    rm -f "$partial_plist"

    if [ -f "$APP_BUNDLE/Contents/Resources/Assets.car" ]; then
        log_success "Liquid Glass icon compiled to Assets.car"
    else
        log_warning "actool succeeded but no Assets.car produced"
    fi
}

# Create app icon
create_app_icon() {
    log_info "Creating app icon..."

    local iconset_dir="$DMG_DIR/AppIcon.iconset"
    local input_svg="$CONFIGBAR_DIR/assets/icon.svg"
    mkdir -p "$iconset_dir"

    if [ ! -f "$input_svg" ]; then
        log_warning "No Systemwide icon source found at $input_svg; using default icon"
        rm -rf "$iconset_dir"
        return
    fi

    render_systemwide_icon() {
        local size="$1"
        local output="$2"

        if command -v rsvg-convert &> /dev/null; then
            rsvg-convert -w "$size" -h "$size" "$input_svg" -o "$output"
        elif command -v magick &> /dev/null; then
            magick -background none -size "${size}x${size}" "$input_svg" "$output"
        else
            return 1
        fi
    }

    if ! render_systemwide_icon 16 "$iconset_dir/icon_16x16.png" ||
       ! render_systemwide_icon 32 "$iconset_dir/icon_16x16@2x.png" ||
       ! render_systemwide_icon 32 "$iconset_dir/icon_32x32.png" ||
       ! render_systemwide_icon 64 "$iconset_dir/icon_32x32@2x.png" ||
       ! render_systemwide_icon 128 "$iconset_dir/icon_128x128.png" ||
       ! render_systemwide_icon 256 "$iconset_dir/icon_128x128@2x.png" ||
       ! render_systemwide_icon 256 "$iconset_dir/icon_256x256.png" ||
       ! render_systemwide_icon 512 "$iconset_dir/icon_256x256@2x.png" ||
       ! render_systemwide_icon 512 "$iconset_dir/icon_512x512.png" ||
       ! render_systemwide_icon 1024 "$iconset_dir/icon_512x512@2x.png"; then
        log_warning "Failed to render Systemwide icon from SVG; install librsvg or ImageMagick"
        rm -rf "$iconset_dir"
        return
    fi

    iconutil -c icns "$iconset_dir" -o "$APP_BUNDLE/Contents/Resources/AppIcon.icns" 2>/dev/null || {
        log_warning "Failed to create icns, app will use default icon"
        rm -rf "$iconset_dir"
        return
    }

    rm -rf "$iconset_dir"
    log_success "App icon created from configbar icon.svg"
}

# Create README for the DMG
create_readme() {
    log_info "Creating README..."

    cat > "$DMG_DIR/README.txt" << 'EOF'
SotF Systemwide - Sound of the Future Audio Engine

INSTALLATION
============
1. Drag "sotf-systemwide.app" to your Applications folder
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
2. Run: /Applications/sotf-systemwide.app/Contents/Resources/install-hal.sh

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
   /Applications/sotf-systemwide.app/Contents/Resources/uninstall-hal.sh
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
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRIVER_SOURCE="$SCRIPT_DIR/SotFHAL.driver"
TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
TARGET_BUNDLE="${TARGET_DIR}/SotFHAL.driver"
EXECUTABLE="${TARGET_BUNDLE}/Contents/MacOS/SotFHAL"
HELPER_NAME="Core-Audio-Driver-Service.helper"
SYSTEMWIDE_BUNDLE_ID="org.spinorama.sotf-systemwide"
LEGACY_BUNDLES=(
    "${TARGET_DIR}/SotFHAL.driver"
    "${TARGET_DIR}/sotf.driver"
    "${TARGET_DIR}/sotf_hal.driver"
    "${TARGET_DIR}/AutoEQ.driver"
)

daemon_is_running() {
    /usr/bin/pgrep -x "sotf-daemon" >/dev/null 2>&1
}

wait_for_daemon_exit() {
    local timeout="$1"
    local elapsed=0

    while daemon_is_running && [ "$elapsed" -lt "$timeout" ]; do
        /bin/sleep 1
        elapsed=$((elapsed + 1))
    done

    ! daemon_is_running
}

console_user_and_uid() {
    local console_user
    local console_uid

    console_user="$(/usr/bin/stat -f "%Su" /dev/console 2>/dev/null || true)"
    if [ -n "$console_user" ] && [ "$console_user" != "root" ]; then
        console_uid="$(/usr/bin/id -u "$console_user" 2>/dev/null || true)"
        if [ -n "$console_uid" ]; then
            printf '%s:%s\n' "$console_user" "$console_uid"
        fi
    fi
}

daemon_socket_candidates() {
    local user_info
    local console_user
    local console_uid
    local user_tmpdir

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        console_uid="${user_info##*:}"

        user_tmpdir="$(/usr/bin/sudo -u "$console_user" /usr/bin/getconf DARWIN_USER_TEMP_DIR 2>/dev/null || true)"
        if [ -n "$user_tmpdir" ]; then
            printf '%s\n' "${user_tmpdir%/}/sotf-daemon.sock"
        fi

        printf '%s\n' "/tmp/sotf-${console_uid}/daemon.sock"
    fi

    printf '%s\n' "/tmp/autoeq_audio.sock"
}

send_daemon_shutdown() {
    local socket_path="$1"

    [ -S "$socket_path" ] || return 0

    echo "Requesting sotf-daemon shutdown via $socket_path"

    if [ -x /usr/bin/python3 ]; then
        /usr/bin/python3 -c 'import socket, sys; s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.settimeout(0.2); s.connect(sys.argv[1]); s.sendall(b"{\"command\":\"shutdown\"}\n"); s.close()' "$socket_path" >/dev/null 2>&1 && return 0
    fi

    if [ -x /usr/bin/nc ]; then
        printf '{"command":"shutdown"}\n' | /usr/bin/nc -U -w 1 "$socket_path" >/dev/null 2>&1 || true
    fi
}

cleanup_sotf_runtime_files() {
    local user_info
    local console_user
    local console_uid
    local user_tmpdir
    local runtime_dir

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        console_uid="${user_info##*:}"
        runtime_dir="/tmp/sotf-${console_uid}"

        user_tmpdir="$(/usr/bin/sudo -u "$console_user" /usr/bin/getconf DARWIN_USER_TEMP_DIR 2>/dev/null || true)"
        if [ -n "$user_tmpdir" ]; then
            /bin/rm -f "${user_tmpdir%/}/sotf-daemon.sock"
        fi

        /bin/rm -f "${runtime_dir}/daemon.sock" "${runtime_dir}/audio.shm" "${runtime_dir}/session.key"
    fi

    /bin/rm -f "/tmp/autoeq_audio.sock"
}

quiesce_sotf_daemon() {
    while IFS= read -r socket_path; do
        send_daemon_shutdown "$socket_path"
    done < <(daemon_socket_candidates)

    wait_for_daemon_exit 2 && {
        cleanup_sotf_runtime_files
        return 0
    }

    echo "sotf-daemon still running; sending TERM"
    sudo /usr/bin/pkill -TERM -x "sotf-daemon" >/dev/null 2>&1 || true
    wait_for_daemon_exit 2 && {
        cleanup_sotf_runtime_files
        return 0
    }

    echo "sotf-daemon still running; sending KILL"
    sudo /usr/bin/pkill -KILL -x "sotf-daemon" >/dev/null 2>&1 || true
    cleanup_sotf_runtime_files
}

quit_systemwide_app() {
    local user_info
    local console_user

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        echo "Requesting SotF Systemwide app quit for user: $console_user"
        /usr/bin/sudo -u "$console_user" /usr/bin/osascript -e "tell application id \"${SYSTEMWIDE_BUNDLE_ID}\" to quit" >/dev/null 2>&1 || true
    fi

    /usr/bin/pkill -TERM -x "sotf-systemwide" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF Systemwide" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF Toolbar" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF ConfigBar" >/dev/null 2>&1 || true
    /bin/sleep 1
}

restart_coreaudio() {
    echo "Restarting CoreAudio..."
    sudo /usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true

    if sudo /usr/bin/killall coreaudiod 2>/dev/null; then
        echo "CoreAudio restart requested; launchd will relaunch coreaudiod"
    else
        echo "coreaudiod was not running or could not be signalled"
    fi

    for _ in 1 2 3 4 5; do
        if /usr/bin/pgrep -x coreaudiod >/dev/null 2>&1; then
            echo "coreaudiod is running"
            return 0
        fi
        sleep 1
    done

    echo "Warning: coreaudiod has not relaunched yet"
}

remove_bundle() {
    local bundle="$1"
    if [ -d "${bundle}" ]; then
        echo "Removing existing driver: ${bundle}"
        sudo /bin/rm -rf "${bundle}"
    fi
}

DAEMON_AGENT_LABEL="org.spinorama.sotf-daemon"

bootout_daemon_launch_agent() {
    local user_info
    local console_uid

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_uid="${user_info##*:}"
        echo "Booting out $DAEMON_AGENT_LABEL for gui/$console_uid"
        /bin/launchctl bootout "gui/$console_uid/$DAEMON_AGENT_LABEL" >/dev/null 2>&1 || true
    fi
}

echo "Installing SotF HAL Driver..."

# Check for driver source
if [ ! -d "${DRIVER_SOURCE}" ]; then
    echo "Error: HAL driver not found at ${DRIVER_SOURCE}"
    exit 1
fi

# Create target directory if needed
sudo /usr/bin/install -d -o root -g wheel -m 755 "${TARGET_DIR}"

quit_systemwide_app
bootout_daemon_launch_agent
quiesce_sotf_daemon

sudo /usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true
for bundle in "${LEGACY_BUNDLES[@]}"; do
    remove_bundle "${bundle}"
done

# Copy the bundle as a clean replacement instead of merging into any stale
# directory that the Installer or cp might leave behind.
echo "Copying driver bundle..."
sudo /usr/bin/ditto "${DRIVER_SOURCE}" "${TARGET_BUNDLE}"

if [ ! -f "${EXECUTABLE}" ]; then
    echo "Error: installed HAL executable is missing: ${EXECUTABLE}"
    exit 1
fi

echo "Setting driver ownership and permissions..."
sudo /usr/sbin/chown -R root:wheel "${TARGET_BUNDLE}"
sudo /usr/bin/find "${TARGET_BUNDLE}" -type d -exec /bin/chmod 755 {} +
sudo /usr/bin/find "${TARGET_BUNDLE}" -type f -exec /bin/chmod 644 {} +
sudo /bin/chmod 755 "${EXECUTABLE}"
sudo /usr/bin/xattr -dr com.apple.quarantine "${TARGET_BUNDLE}" 2>/dev/null || true

if [ ! -x "${EXECUTABLE}" ]; then
    echo "Error: installed HAL executable is not executable after chmod: ${EXECUTABLE}"
    exit 1
fi

# Sign with ad-hoc signature
echo "Signing driver bundle..."
sudo /usr/bin/codesign --force --deep --sign - --options runtime "${TARGET_BUNDLE}"
sudo /usr/bin/codesign --verify --deep "${TARGET_BUNDLE}"

# Restart CoreAudio
restart_coreaudio

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
set -euo pipefail

TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
HELPER_NAME="Core-Audio-Driver-Service.helper"
DRIVER_BUNDLES=(
    "${TARGET_DIR}/SotFHAL.driver"
    "${TARGET_DIR}/sotf.driver"
    "${TARGET_DIR}/sotf_hal.driver"
    "${TARGET_DIR}/AutoEQ.driver"
)

restart_coreaudio() {
    echo "Restarting CoreAudio..."
    sudo /usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true

    if sudo /usr/bin/killall coreaudiod 2>/dev/null; then
        echo "CoreAudio restart requested; launchd will relaunch coreaudiod"
    else
        echo "coreaudiod was not running or could not be signalled"
    fi

    for _ in 1 2 3 4 5; do
        if /usr/bin/pgrep -x coreaudiod >/dev/null 2>&1; then
            echo "coreaudiod is running"
            return 0
        fi
        sleep 1
    done

    echo "Warning: coreaudiod has not relaunched yet"
}

echo "Uninstalling SotF HAL Driver..."

REMOVED=false
sudo /usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true

for bundle in "${DRIVER_BUNDLES[@]}"; do
    if [ -d "${bundle}" ]; then
        echo "Removing driver bundle: ${bundle}"
        sudo /bin/rm -rf "${bundle}"
        REMOVED=true
    fi
done

if [ "$REMOVED" = false ]; then
    echo "HAL driver is not installed."
    exit 0
fi

# Restart CoreAudio
restart_coreaudio

echo ""
echo "HAL driver uninstalled successfully!"
UNINSTALL_SCRIPT
    chmod +x "$DMG_DIR/uninstall-hal.sh"

    log_success "HAL driver scripts created"
}

# Create DMG
create_dmg_file() {
    log_info "Creating DMG..."

    local dmg_path="$DMG_DIR/sotf-systemwide-$VERSION-macos-universal.dmg"
    local dmg_temp="$DMG_DIR/temp.dmg"

    rm -f "$dmg_path" "$dmg_temp"

    # Copy HAL scripts to app Resources
    if $BUILD_HAL;
 then
        cp "$DMG_DIR/install-hal.sh" "$APP_BUNDLE/Contents/Resources/"
        cp "$DMG_DIR/uninstall-hal.sh" "$APP_BUNDLE/Contents/Resources/"
        # Copy standalone driver to Resources if not already there
        if [ ! -d "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME" ] && [ -d "$DRIVER_BUNDLE" ]; then
            /usr/bin/ditto "$DRIVER_BUNDLE" "$APP_BUNDLE/Contents/Resources/$DRIVER_NAME"
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

    local pkg_path="$DMG_DIR/sotf-systemwide-$VERSION-macos-universal.pkg"
    local pkg_root="$DMG_DIR/pkg-root"
    local hal_pkg_root="$DMG_DIR/pkg-root-hal"
    local pkg_scripts="$DMG_DIR/pkg-scripts"
    local hal_pkg_scripts="$DMG_DIR/pkg-scripts-hal"
    local pkg_components="$DMG_DIR/pkg-components"
    local launch_scripts="$DMG_DIR/launch-scripts"

    rm -rf "$pkg_root" "$hal_pkg_root" "$pkg_scripts" "$hal_pkg_scripts" "$pkg_components" "$launch_scripts"
    mkdir -p "$pkg_root/Applications"
    mkdir -p "$pkg_root/Library/Application Support/SotF"
    mkdir -p "$hal_pkg_root/Library/Audio/Plug-Ins/HAL"
    mkdir -p "$pkg_scripts"
    mkdir -p "$hal_pkg_scripts"
    mkdir -p "$pkg_components"
    mkdir -p "$launch_scripts"

    # Package scripts share progress reporting, durable diagnostics, and a
    # best-effort failure dialog with an Open Logs action.
    cat > "$DMG_DIR/installer-common.sh" << 'INSTALLER_COMMON'
#!/bin/bash

INSTALL_LOG="/Library/Logs/SotF/installer.log"
SOTF_INSTALL_STEP="Starting SotF installation"

/bin/mkdir -p "$(/usr/bin/dirname "$INSTALL_LOG")"
/bin/chmod 755 "$(/usr/bin/dirname "$INSTALL_LOG")"
/usr/bin/touch "$INSTALL_LOG"
/bin/chmod 644 "$INSTALL_LOG"
exec > >(/usr/bin/tee -a "$INSTALL_LOG") 2>&1

installer_step() {
    SOTF_INSTALL_STEP="$1"
    printf 'installer:PHASE:%s\n' "$SOTF_INSTALL_STEP"
    printf 'installer:STATUS:%s\n' "$SOTF_INSTALL_STEP"
    printf '[%s] %s\n' "$(/bin/date '+%Y-%m-%d %H:%M:%S')" "$SOTF_INSTALL_STEP"
}

installer_failure() {
    local line="$1"
    local status="$2"
    local console_user
    local console_uid
    local button

    trap - ERR
    set +e
    printf '[%s] FAILED during "%s" at line %s (exit %s)\n' \
        "$(/bin/date '+%Y-%m-%d %H:%M:%S')" "$SOTF_INSTALL_STEP" "$line" "$status"

    console_user="$(/usr/bin/stat -f '%Su' /dev/console 2>/dev/null || true)"
    if [ -n "$console_user" ] && [ "$console_user" != "root" ]; then
        console_uid="$(/usr/bin/id -u "$console_user" 2>/dev/null || true)"
        if [ -n "$console_uid" ]; then
            button="$(/bin/launchctl asuser "$console_uid" \
                /usr/bin/sudo -u "$console_user" /usr/bin/osascript \
                -e 'button returned of (display alert "SotF installation failed" message "The installer could not complete the current step. Open the SotF installer log for details." as critical buttons {"Close", "Open Logs"} default button "Open Logs")' \
                2>/dev/null || true)"
            if [ "$button" = "Open Logs" ]; then
                /bin/launchctl asuser "$console_uid" \
                    /usr/bin/sudo -u "$console_user" /usr/bin/open -a Console "$INSTALL_LOG" \
                    >/dev/null 2>&1 || true
            fi
        fi
    fi
    exit "$status"
}

set -E
trap 'installer_failure "$LINENO" "$?"' ERR
INSTALLER_COMMON
    chmod 755 "$DMG_DIR/installer-common.sh"
    cp "$DMG_DIR/installer-common.sh" "$pkg_scripts/installer-common.sh"
    cp "$DMG_DIR/installer-common.sh" "$hal_pkg_scripts/installer-common.sh"
    cp "$DMG_DIR/installer-common.sh" "$launch_scripts/installer-common.sh"

    # Copy app to pkg root
    cp -R "$APP_BUNDLE" "$pkg_root/Applications/"

    # Ship the daemon LaunchAgent plist so postinstall can register it
    # for the console user. launchd owns the daemon lifecycle; the menu
    # bar app only adopts it.
    cp "$PROJECT_ROOT/builds/macos/org.spinorama.sotf-daemon.plist" \
        "$pkg_root/Library/Application Support/SotF/"

    # Copy HAL driver to pkg root
    if $BUILD_HAL && [ -d "$DRIVER_BUNDLE" ]; then
        /usr/bin/ditto "$DRIVER_BUNDLE" "$hal_pkg_root/Library/Audio/Plug-Ins/HAL/$DRIVER_NAME"
    fi

    # Create app postinstall script. HAL driver lifecycle is handled by the
    # HAL component package so CoreAudio restarts after the driver payload lands.
cat > "$pkg_scripts/postinstall" << 'POSTINSTALL'
#!/bin/bash
# Post-installation script for SotF Systemwide app

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/installer-common.sh"

AGENT_LABEL="org.spinorama.sotf-daemon"
AGENT_SRC="/Library/Application Support/SotF/org.spinorama.sotf-daemon.plist"

installer_step "Registering the SotF background audio service"

# Register the daemon LaunchAgent for the console user. launchd owns the
# daemon from here on; quitting the menu bar app does not stop systemwide
# audio processing.
CONSOLE_USER="$(/usr/bin/stat -f "%Su" /dev/console 2>/dev/null || true)"
if [ -z "$CONSOLE_USER" ] || [ "$CONSOLE_USER" = "root" ]; then
    echo "No console user; skipping LaunchAgent registration"
    exit 0
fi
CONSOLE_UID="$(/usr/bin/id -u "$CONSOLE_USER")"
USER_HOME="$(/usr/bin/dscl . -read "/Users/$CONSOLE_USER" NFSHomeDirectory 2>/dev/null | awk '{print $2}')"
[ -n "$USER_HOME" ] || USER_HOME="/Users/$CONSOLE_USER"
AGENT_DST="$USER_HOME/Library/LaunchAgents/$AGENT_LABEL.plist"

/bin/mkdir -p "$USER_HOME/Library/LaunchAgents"
/bin/rm -f "$AGENT_DST"
/usr/sbin/chown "$CONSOLE_USER" "$USER_HOME/Library/LaunchAgents"
/usr/bin/ditto "$AGENT_SRC" "$AGENT_DST"
/usr/sbin/chown "$CONSOLE_USER" "$AGENT_DST"
/bin/chmod 644 "$AGENT_DST"

# Replace any live instance of the label with the freshly installed plist.
"/bin/launchctl" bootout "gui/$CONSOLE_UID/$AGENT_LABEL" >/dev/null 2>&1 || true
installer_step "Starting the SotF background audio service"
if "/bin/launchctl" bootstrap "gui/$CONSOLE_UID" "$AGENT_DST"; then
    echo "Registered $AGENT_LABEL LaunchAgent for $CONSOLE_USER (gui/$CONSOLE_UID)"
else
    echo "Warning: could not bootstrap $AGENT_LABEL; the menu bar app will retry via kickstart"
fi

exit 0
POSTINSTALL
    chmod +x "$pkg_scripts/postinstall"

    # Create postinstall script for auto-launch component
cat > "$launch_scripts/postinstall" << 'LAUNCHSCRIPT'
#!/bin/bash
# Launch SotF Systemwide after installation

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/installer-common.sh"
installer_step "Launching SotF Systemwide"

# Get the user who initiated the installation
CONSOLE_USER=$(stat -f "%Su" /dev/console)

if [ -n "$CONSOLE_USER" ] && [ "$CONSOLE_USER" != "root" ]; then
    echo "Launching SotF Systemwide for user: $CONSOLE_USER"
    # Use launchctl to run as the console user
    sudo -u "$CONSOLE_USER" open -a "/Applications/sotf-systemwide.app" &
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

    # Create preinstall script to quiesce daemon and remove old versions
cat > "$pkg_scripts/preinstall" << 'PREINSTALL'
#!/bin/bash
# Pre-installation script for SotF Systemwide app

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/installer-common.sh"
installer_step "Stopping the current SotF audio service"

DAEMON_AGENT_LABEL="org.spinorama.sotf-daemon"

daemon_is_running() {
    /usr/bin/pgrep -x "sotf-daemon" >/dev/null 2>&1
}

wait_for_daemon_exit() {
    local timeout="$1"
    local elapsed=0

    while daemon_is_running && [ "$elapsed" -lt "$timeout" ]; do
        /bin/sleep 1
        elapsed=$((elapsed + 1))
    done

    ! daemon_is_running
}

console_user_and_uid() {
    local console_user
    local console_uid

    console_user="$(/usr/bin/stat -f "%Su" /dev/console 2>/dev/null || true)"
    if [ -n "$console_user" ] && [ "$console_user" != "root" ]; then
        console_uid="$(/usr/bin/id -u "$console_user" 2>/dev/null || true)"
        if [ -n "$console_uid" ]; then
            printf '%s:%s\n' "$console_user" "$console_uid"
        fi
    fi
}

daemon_socket_candidates() {
    local user_info
    local console_user
    local console_uid
    local user_tmpdir

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        console_uid="${user_info##*:}"

        # Matches the daemon's macOS $TMPDIR path, if it was launched from the GUI session.
        user_tmpdir="$(/usr/bin/sudo -u "$console_user" /usr/bin/getconf DARWIN_USER_TEMP_DIR 2>/dev/null || true)"
        if [ -n "$user_tmpdir" ]; then
            printf '%s\n' "${user_tmpdir%/}/sotf-daemon.sock"
        fi

        # Matches the daemon's secure fallback path from get_secure_socket_path().
        printf '%s\n' "/tmp/sotf-${console_uid}/daemon.sock"
    fi

    # Legacy compatibility socket from sotf_daemon.rs.
    printf '%s\n' "/tmp/autoeq_audio.sock"
}

send_daemon_shutdown() {
    local socket_path="$1"

    [ -S "$socket_path" ] || return 0

    echo "Requesting sotf-daemon shutdown via $socket_path"

    if [ -x /usr/bin/python3 ]; then
        /usr/bin/python3 -c 'import socket, sys; s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.settimeout(0.2); s.connect(sys.argv[1]); s.sendall(b"{\"command\":\"shutdown\"}\n"); s.close()' "$socket_path" >/dev/null 2>&1 && return 0
    fi

    if [ -x /usr/bin/nc ]; then
        printf '{"command":"shutdown"}\n' | /usr/bin/nc -U -w 1 "$socket_path" >/dev/null 2>&1 || true
    fi
}

cleanup_sotf_runtime_files() {
    local user_info
    local console_user
    local console_uid
    local user_tmpdir
    local runtime_dir

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        console_uid="${user_info##*:}"
        runtime_dir="/tmp/sotf-${console_uid}"

        user_tmpdir="$(/usr/bin/sudo -u "$console_user" /usr/bin/getconf DARWIN_USER_TEMP_DIR 2>/dev/null || true)"
        if [ -n "$user_tmpdir" ]; then
            /bin/rm -f "${user_tmpdir%/}/sotf-daemon.sock"
        fi

        /bin/rm -f "${runtime_dir}/daemon.sock" "${runtime_dir}/audio.shm" "${runtime_dir}/session.key"
    fi

    /bin/rm -f "/tmp/autoeq_audio.sock"
}

quiesce_sotf_daemon() {
    while IFS= read -r socket_path; do
        send_daemon_shutdown "$socket_path"
    done < <(daemon_socket_candidates)

    wait_for_daemon_exit 2 && {
        cleanup_sotf_runtime_files
        return 0
    }

    echo "sotf-daemon still running; sending TERM"
    /usr/bin/pkill -TERM -x "sotf-daemon" >/dev/null 2>&1 || true
    wait_for_daemon_exit 2 && {
        cleanup_sotf_runtime_files
        return 0
    }

    echo "sotf-daemon still running; sending KILL"
    /usr/bin/pkill -KILL -x "sotf-daemon" >/dev/null 2>&1 || true
    cleanup_sotf_runtime_files
}

quit_systemwide_app() {
    local user_info
    local console_user

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_user="${user_info%%:*}"
        echo "Requesting SotF Systemwide app quit for user: $console_user"
        /usr/bin/sudo -u "$console_user" /usr/bin/osascript -e 'tell application id "org.spinorama.sotf-systemwide" to quit' >/dev/null 2>&1 || true
    fi

    /usr/bin/pkill -TERM -x "sotf-systemwide" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF Systemwide" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF Toolbar" >/dev/null 2>&1 || true
    /usr/bin/pkill -TERM -x "SotF ConfigBar" >/dev/null 2>&1 || true
    /bin/sleep 1
}

bootout_daemon_launch_agent() {
    local user_info
    local console_uid

    user_info="$(console_user_and_uid)"
    if [ -n "$user_info" ]; then
        console_uid="${user_info##*:}"
        echo "Booting out $DAEMON_AGENT_LABEL for gui/$console_uid"
        /bin/launchctl bootout "gui/$console_uid/$DAEMON_AGENT_LABEL" >/dev/null 2>&1 || true
    fi
}

quit_systemwide_app
bootout_daemon_launch_agent
quiesce_sotf_daemon

installer_step "Removing legacy SotF applications"

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

    # Create HAL component scripts. These must be attached to SotFHAL.pkg, not
    # the app package, so the driver is removed before the HAL payload and
    # CoreAudio is restarted only after the new payload has been installed.
    cat > "$hal_pkg_scripts/preinstall" << 'HAL_PREINSTALL'
#!/bin/bash
# Pre-installation script for SotF HAL driver
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/installer-common.sh"
installer_step "Preparing the CoreAudio driver update"

TARGET_DIR="/Library/Audio/Plug-Ins/HAL"
HELPER_NAME="Core-Audio-Driver-Service.helper"
DRIVER_BUNDLES=(
    "${TARGET_DIR}/SotFHAL.driver"
    "${TARGET_DIR}/sotf.driver"
    "${TARGET_DIR}/sotf_hal.driver"
    "${TARGET_DIR}/AutoEQ.driver"
)

echo "Preparing SotF HAL driver install..."
/usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true

for bundle in "${DRIVER_BUNDLES[@]}"; do
    if [ -d "${bundle}" ]; then
        echo "Removing existing HAL driver: ${bundle}"
        /bin/rm -rf "${bundle}"
    fi
done

exit 0
HAL_PREINSTALL
    chmod +x "$hal_pkg_scripts/preinstall"

    cat > "$hal_pkg_scripts/postinstall" << 'HAL_POSTINSTALL'
#!/bin/bash
# Post-installation script for SotF HAL driver
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/installer-common.sh"
installer_step "Securing and verifying the SotF audio driver"

TARGET_BUNDLE="/Library/Audio/Plug-Ins/HAL/SotFHAL.driver"
EXECUTABLE="${TARGET_BUNDLE}/Contents/MacOS/SotFHAL"
HELPER_NAME="Core-Audio-Driver-Service.helper"

restart_coreaudio() {
    installer_step "Restarting CoreAudio with the new SotF audio driver"
    echo "Restarting CoreAudio to load SotF HAL driver..."
    /usr/bin/killall "${HELPER_NAME}" 2>/dev/null || true

    if /usr/bin/killall coreaudiod 2>/dev/null; then
        echo "CoreAudio restart requested; launchd will relaunch coreaudiod"
    else
        echo "coreaudiod was not running or could not be signalled"
    fi

    for _ in 1 2 3 4 5; do
        if /usr/bin/pgrep -x coreaudiod >/dev/null 2>&1; then
            echo "coreaudiod is running"
            return 0
        fi
        sleep 1
    done

    echo "Warning: coreaudiod has not relaunched yet"
}

if [ ! -f "${EXECUTABLE}" ]; then
    echo "Error: installed HAL executable is missing: ${EXECUTABLE}"
    exit 1
fi

echo "Setting SotF HAL driver ownership and permissions..."
/usr/sbin/chown -R root:wheel "${TARGET_BUNDLE}"
/usr/bin/find "${TARGET_BUNDLE}" -type d -exec /bin/chmod 755 {} +
/usr/bin/find "${TARGET_BUNDLE}" -type f -exec /bin/chmod 644 {} +
/bin/chmod 755 "${EXECUTABLE}"
/usr/bin/xattr -dr com.apple.quarantine "${TARGET_BUNDLE}" 2>/dev/null || true

if [ ! -x "${EXECUTABLE}" ]; then
    echo "Error: installed HAL executable is not executable after chmod: ${EXECUTABLE}"
    exit 1
fi

if /usr/bin/codesign --verify --deep "${TARGET_BUNDLE}" >/dev/null 2>&1; then
    echo "SotF HAL driver code signature verified"
else
    echo "Signing SotF HAL driver with an ad-hoc signature..."
    /usr/bin/codesign --force --deep --sign - --options runtime "${TARGET_BUNDLE}"
    /usr/bin/codesign --verify --deep "${TARGET_BUNDLE}"
fi

restart_coreaudio
echo "SotF HAL driver installed successfully"
exit 0
HAL_POSTINSTALL
    chmod +x "$hal_pkg_scripts/postinstall"

    # Build component packages
    log_info "Building component packages..."

    # App component
    pkgbuild \
        --root "$pkg_root" \
        --install-location "/" \
        --identifier "$SYSTEMWIDE_BUNDLE_ID" \
        --version "$VERSION" \
        --scripts "$pkg_scripts" \
        "$pkg_components/SotFSystemwide.pkg"

    # HAL driver component (if built)
    if $BUILD_HAL && [ -d "$hal_pkg_root/Library/Audio/Plug-Ins/HAL/$DRIVER_NAME" ]; then
        pkgbuild \
            --root "$hal_pkg_root" \
            --install-location "/" \
            --identifier "$HAL_BUNDLE_ID" \
            --version "$VERSION" \
            --scripts "$hal_pkg_scripts" \
            "$pkg_components/SotFHAL.pkg"
    fi

    # Create distribution XML
    cat > "$DMG_DIR/distribution.xml" << DISTXML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>SotF Systemwide $VERSION</title>
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
        html { color-scheme: light; background: #fff; }
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; background: #fff; color: #333; }
        h1 { color: #333; }
        p { color: #666; line-height: 1.6; }
        .features { margin-top: 20px; }
        .features li { margin: 8px 0; color: #333; }
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
        html { color-scheme: light; background: #fff; }
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; background: #fff; color: #333; }
        h1 { color: #28a745; }
        p { color: #666; line-height: 1.6; }
        .next-steps { background: #f8f9fa; color: #333; padding: 15px; border-radius: 8px; margin-top: 20px; }
        .next-steps h3 { margin-top: 0; color: #333; }
        .next-steps li { color: #333; margin: 6px 0; }
        .next-steps strong { color: #111; }
        code { background: #e9ecef; color: #333; padding: 2px 6px; border-radius: 4px; }
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

    # Build the distribution package. The payload may already be signed; the
    # installer container remains unsigned until ./scripts/sign-macos.sh runs.
    log_info "Building distribution package..."
    productbuild \
        --distribution "$DMG_DIR/distribution.xml" \
        --package-path "$pkg_components" \
        --resources "$DMG_DIR" \
        "$pkg_path"
    log_success "Installer package created (container unsigned)"
    validate_pkg_payload "$pkg_path"

    # Cleanup
    rm -rf "$pkg_root" "$hal_pkg_root" "$pkg_scripts" "$hal_pkg_scripts" "$pkg_components"
    rm -f "$DMG_DIR/distribution.xml" "$DMG_DIR/welcome.html" "$DMG_DIR/conclusion.html" "$DMG_DIR/installer-common.sh"

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
    sign_release_payloads

    if $BUILD_DMG; then
        # Legacy DMG build
        create_readme
        create_hal_scripts
        create_dmg_file

        log_info "=========================================="
        log_success "Build complete!"
        log_info "=========================================="

        local dmg_path="$DMG_DIR/sotf-systemwide-$VERSION-macos-universal.dmg"
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

        local pkg_path="$DMG_DIR/sotf-systemwide-$VERSION-macos-universal.pkg"
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
