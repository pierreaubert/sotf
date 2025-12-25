#!/bin/bash
#
# Build script for SotF Player macOS application
# Creates a signed and notarized DMG for distribution
#
# Usage:
#   ./build-dmg.sh                    # Build unsigned DMG (for local testing)
#   ./build-dmg.sh --sign             # Build signed DMG (requires Developer ID)
#   ./build-dmg.sh --sign --notarize  # Build, sign, and notarize (for distribution)
#
# Environment variables:
#   DEVELOPER_ID         - Developer ID Application certificate name
#                          Example: "Developer ID Application: Your Name (TEAMID)"
#   APPLE_ID             - Apple ID email for notarization
#   APPLE_APP_PASSWORD   - App-specific password for notarization
#   APPLE_TEAM_ID        - Apple Developer Team ID
#
# Prerequisites:
#   - Xcode Command Line Tools
#   - Rust toolchain
#   - create-dmg (optional, for prettier DMG): brew install create-dmg
#

set -euo pipefail

# Configuration
APP_NAME="SotF"
BUNDLE_ID="org.spinorama.sotf"
BINARY_NAME="sotf-gpui"
BUILD_NUMBER="1"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi
BUILD_DIR="$PROJECT_ROOT/target-static/release"
DMG_DIR="$PROJECT_ROOT/target-static/dmg"
APP_BUNDLE="$DMG_DIR/$APP_NAME.app"

# Command line options
SIGN=true
NOTARIZE=true
UNIVERSAL=false
CLEAN=false

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
        --universal)
            UNIVERSAL=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --sign        Sign the application with Developer ID"
            echo "  --notarize    Notarize the application (implies --sign)"
            echo "  --universal   Build universal binary (Intel + Apple Silicon)"
            echo "  --clean       Clean build directory before building"
            echo "  --help        Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

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
#        if [ -z "${APPLE_APP_PASSWORD:-}" ]; then
#            log_error "APPLE_APP_PASSWORD environment variable not set"
#            log_info "Create an app-specific password at https://appleid.apple.com"
#            exit 1
#        fi
#        if [ -z "${APPLE_TEAM_ID:-}" ]; then
#            log_error "APPLE_TEAM_ID environment variable not set"
#            exit 1
#        fi
    fi

    log_success "Prerequisites check passed"
}

# Clean build artifacts
clean_build() {
    if $CLEAN; then
        log_info "Cleaning build directory..."
        rm -rf "$DMG_DIR"
        cargo clean -p sotf-gpui
    fi
}

# Build the binary
build_binary() {
    log_info "Building release binary..."

    cd "$PROJECT_ROOT"

    if $UNIVERSAL; then
        log_info "Building universal binary (x86_64 + arm64)..."

        # Ensure targets are installed
        rustup target add x86_64-apple-darwin aarch64-apple-darwin

        # Build for both architectures
        RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --package sotf-gpui --target x86_64-apple-darwin --target-dir ./target-static
        RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --package sotf-gpui --target aarch64-apple-darwin --target-dir ./target-static

        # Create universal binary
        mkdir -p "$BUILD_DIR"
        lipo -create \
            "$PROJECT_ROOT/target/x86_64-apple-darwin/release/$BINARY_NAME" \
            "$PROJECT_ROOT/target/aarch64-apple-darwin/release/$BINARY_NAME" \
            -output "$BUILD_DIR/$BINARY_NAME"

        log_success "Universal binary created"
    else
        RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --package sotf-gpui --target-dir ./target-static
    fi

    if [ ! -f "$BUILD_DIR/$BINARY_NAME" ]; then
        log_error "Binary not found at $BUILD_DIR/$BINARY_NAME"
        exit 1
    fi

    log_success "Binary built successfully"
}

# Create app bundle structure
create_app_bundle() {
    log_info "Creating app bundle..."

    # Clean and create directories
    rm -rf "$APP_BUNDLE"
    mkdir -p "$APP_BUNDLE/Contents/MacOS"
    mkdir -p "$APP_BUNDLE/Contents/Resources"

    # Copy binary
    cp "$BUILD_DIR/$BINARY_NAME" "$APP_BUNDLE/Contents/MacOS/"

    # Copy Info.plist and update version
    sed -e "s/<string>SOTF_VERSION<\/string>/<string>$VERSION<\/string>/" \
        -e "s/<string>1<\/string>/<string>$BUILD_NUMBER<\/string>/" \
        "$SCRIPT_DIR/Info.plist" > "$APP_BUNDLE/Contents/Info.plist"

    # Create PkgInfo
    echo -n "APPL????" > "$APP_BUNDLE/Contents/PkgInfo"

    # Copy icon if it exists (convert from jpg/png to icns if needed)
    if [ -f "$SCRIPT_DIR/../assets/sotf.jpg" ]; then
        create_icns "$SCRIPT_DIR/../assets/sotf.jpg" "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
    elif [ -f "$SCRIPT_DIR/../assets/icon.icns" ]; then
        cp "$SCRIPT_DIR/../assets/icon.icns" "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
    else
        log_warning "No icon found, app will use default icon"
    fi

    log_success "App bundle created at $APP_BUNDLE"
}

# Bundle dynamic libraries from Homebrew and other non-system locations
bundle_dylibs() {
    log_info "Bundling dynamic libraries..."

    local frameworks_dir="$APP_BUNDLE/Contents/Frameworks"
    mkdir -p "$frameworks_dir"

    local binary="$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"

    # Get list of non-system dylibs
    local dylibs
    dylibs=$(otool -L "$binary" | grep -v "^$binary" | awk '{print $1}' | grep -v "^/System" | grep -v "^/usr/lib" | grep -v "@rpath" | grep -v "@executable_path" || true)

    if [ -z "$dylibs" ]; then
        log_info "No external dylibs to bundle"
        return
    fi

    # Process each dylib
    for dylib in $dylibs; do
        if [ ! -f "$dylib" ]; then
            log_warning "Dylib not found: $dylib"
            continue
        fi

        local dylib_name
        dylib_name=$(basename "$dylib")
        local dest="$frameworks_dir/$dylib_name"

        log_info "Bundling: $dylib_name"

        # Copy the dylib
        cp "$dylib" "$dest"
        chmod 755 "$dest"

        # Update the reference in the main binary
        install_name_tool -change "$dylib" "@executable_path/../Frameworks/$dylib_name" "$binary"

        # Recursively process dependencies of this dylib
        bundle_dylib_deps "$dest" "$frameworks_dir" "$binary"
    done

    log_success "Dynamic libraries bundled"
}

# Recursively bundle dependencies of a dylib
bundle_dylib_deps() {
    local dylib="$1"
    local frameworks_dir="$2"
    local main_binary="$3"

    local deps
    deps=$(otool -L "$dylib" | grep -v "^$dylib" | awk '{print $1}' | grep -v "^/System" | grep -v "^/usr/lib" | grep -v "@rpath" | grep -v "@executable_path" || true)

    for dep in $deps; do
        if [ ! -f "$dep" ]; then
            continue
        fi

        local dep_name
        dep_name=$(basename "$dep")
        local dest="$frameworks_dir/$dep_name"

        # Skip if already bundled
        if [ -f "$dest" ]; then
            # Just update the reference
            install_name_tool -change "$dep" "@executable_path/../Frameworks/$dep_name" "$dylib"
            continue
        fi

        log_info "  Bundling dependency: $dep_name"
        cp "$dep" "$dest"
        chmod 755 "$dest"

        # Update reference in the dylib being processed
        install_name_tool -change "$dep" "@executable_path/../Frameworks/$dep_name" "$dylib"

        # Also fix the dylib's own install name
        install_name_tool -id "@executable_path/../Frameworks/$dep_name" "$dest"

        # Recurse
        bundle_dylib_deps "$dest" "$frameworks_dir" "$main_binary"
    done
}

# Create icns from image file
create_icns() {
    local input_image="$1"
    local output_icns="$2"

    log_info "Creating app icon..."

    local iconset_dir="$DMG_DIR/AppIcon.iconset"
    mkdir -p "$iconset_dir"

    # Generate all required sizes
    local sizes=(16 32 64 128 256 512 1024)
    for size in "${sizes[@]}"; do
        sips -z $size $size "$input_image" --out "$iconset_dir/icon_${size}x${size}.png" 2>/dev/null || true
    done

    # Create @2x versions
    sips -s format png -z 32 32 "$input_image" --out "$iconset_dir/icon_16x16@2x.png" 2>/dev/null || true
    sips -s format png -z 64 64 "$input_image" --out "$iconset_dir/icon_32x32@2x.png" 2>/dev/null || true
    sips -s format png -z 128 128 "$input_image" --out "$iconset_dir/icon_64x64@2x.png" 2>/dev/null || true
    sips -s format png -z 256 256 "$input_image" --out "$iconset_dir/icon_128x128@2x.png" 2>/dev/null || true
    sips -s format png -z 512 512 "$input_image" --out "$iconset_dir/icon_256x256@2x.png" 2>/dev/null || true
    sips -s format png -z 1024 1024 "$input_image" --out "$iconset_dir/icon_512x512@2x.png" 2>/dev/null || true

    # Convert to icns
    iconutil -c icns "$iconset_dir" -o "$output_icns" 2>/dev/null || {
        log_warning "Failed to create icns, app will use default icon"
        return
    }

    rm -rf "$iconset_dir"
    log_success "App icon created"
}

# Sign the application
sign_app() {
    if ! $SIGN; then
        log_warning "Skipping code signing (use --sign to enable)"
        return
    fi

    log_info "Signing application..."

    # Sign all frameworks first (must sign inside-out)
    if [ -d "$APP_BUNDLE/Contents/Frameworks" ]; then
        for lib in "$APP_BUNDLE/Contents/Frameworks"/*.dylib; do
            if [ -f "$lib" ]; then
                log_info "Signing framework: $(basename "$lib")"
                codesign --force --options runtime \
                    --sign "$DEVELOPER_ID" \
                    --timestamp \
                    "$lib"
            fi
        done
    fi

    # Sign the binary with hardened runtime
    codesign --force --options runtime \
        --entitlements "$SCRIPT_DIR/entitlements.plist" \
        --sign "$DEVELOPER_ID" \
        --timestamp \
        "$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"

    # Sign the entire bundle
    codesign --force --options runtime \
        --entitlements "$SCRIPT_DIR/entitlements.plist" \
        --sign "$DEVELOPER_ID" \
        --timestamp \
        "$APP_BUNDLE"

    # Verify signature
    codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

    log_success "Application signed successfully"
}

# Create DMG
create_dmg() {
    log_info "Creating DMG..."

    local dmg_path="$DMG_DIR/$APP_NAME-$VERSION.dmg"
    local dmg_temp="$DMG_DIR/temp.dmg"

    rm -f "$dmg_path" "$dmg_temp"

    # Check if create-dmg is available (prettier DMG)
    if command -v create-dmg &> /dev/null; then
        log_info "Using create-dmg for styled DMG..."

        create-dmg \
            --volname "$APP_NAME" \
            --volicon "$APP_BUNDLE/Contents/Resources/AppIcon.icns" \
            --window-pos 200 120 \
            --window-size 600 400 \
            --icon-size 100 \
            --icon "$APP_NAME.app" 150 190 \
            --hide-extension "$APP_NAME.app" \
            --app-drop-link 450 185 \
            --no-internet-enable \
            "$dmg_path" \
            "$APP_BUNDLE" || {
                # create-dmg returns non-zero even on success sometimes
                if [ -f "$dmg_path" ]; then
                    log_success "DMG created (with create-dmg)"
                else
                    log_warning "create-dmg failed, falling back to hdiutil"
                    create_dmg_hdiutil "$dmg_path"
                fi
            }
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

    # Create symlink to /Applications
    ln -s /Applications "$staging_dir/Applications"

    # Create DMG
    hdiutil create -volname "$APP_NAME" \
        -srcfolder "$staging_dir" \
        -ov -format UDZO \
        "$dmg_path"

    rm -rf "$staging_dir"
    log_success "DMG created (with hdiutil)"
}

# Notarize the DMG
notarize_dmg() {
    if ! $NOTARIZE; then
        log_warning "Skipping notarization (use --notarize to enable)"
        return
    fi

    local dmg_path="$DMG_DIR/$APP_NAME-$VERSION.dmg"

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
            log_info "  xcrun notarytool log $submission_id --apple-id $APPLE_ID --password <password> --team-id $APPLE_TEAM_ID"
        fi
        exit 1
    fi
}

# Main build process
main() {
    log_info "=========================================="
    log_info "Building $APP_NAME v$VERSION"
    log_info "=========================================="

    check_prerequisites
    clean_build
    build_binary
    create_app_bundle
    bundle_dylibs
    sign_app
    create_dmg
    notarize_dmg

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="

    local dmg_path="$DMG_DIR/$APP_NAME-$VERSION.dmg"
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
}

main "$@"
