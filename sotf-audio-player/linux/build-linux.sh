#!/bin/bash
#
# Build script for SotF Player Linux application
# Creates a distributable tarball with the GPUI binary
#
# Usage:
#   ./build-linux.sh                    # Build release binary (native Linux)
#   ./build-linux.sh --cross            # Cross-compile from macOS ARM
#   ./build-linux.sh --cross-arm64      # Cross-compile for Linux ARM64 from macOS
#   ./build-linux.sh --appimage         # Build and create AppImage (native Linux only)
#
# Prerequisites:
#   - Rust toolchain
#   - Linux build dependencies (see justfile: install-ubuntu-common)
#   - For cross-compilation: cross tool (cargo install cross)
#   - For AppImage: appimagetool
#

set -euo pipefail

# Configuration
APP_NAME="SotF"
BINARY_NAME="SotF"
PACKAGE_NAME="sotf-gpui"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

# Build options
CROSS_COMPILE=false
CROSS_ARM64=false
CREATE_APPIMAGE=false
CLEAN=false
TARGET=""
BUILD_DIR=""
DIST_DIR="$PROJECT_ROOT/dist"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --cross)
            CROSS_COMPILE=true
            TARGET="x86_64-unknown-linux-gnu"
            shift
            ;;
        --cross-arm64)
            CROSS_COMPILE=true
            CROSS_ARM64=true
            TARGET="aarch64-unknown-linux-gnu"
            shift
            ;;
        --appimage)
            CREATE_APPIMAGE=true
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
            echo "  --cross        Cross-compile for Linux x86_64 from macOS"
            echo "  --cross-arm64  Cross-compile for Linux ARM64 from macOS"
            echo "  --appimage     Create AppImage (native Linux only)"
            echo "  --clean        Clean build directory before building"
            echo "  --help         Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Set build directory based on target
if $CROSS_COMPILE; then
    BUILD_DIR="$PROJECT_ROOT/target/$TARGET/release"
else
    BUILD_DIR="$PROJECT_ROOT/target/release"
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

    if $CROSS_COMPILE; then
        if ! command -v cross &> /dev/null; then
            log_error "cross is not installed. Install with: cargo install cross"
            exit 1
        fi
    fi

    if $CREATE_APPIMAGE; then
        if $CROSS_COMPILE; then
            log_error "AppImage creation is only supported for native Linux builds"
            exit 1
        fi
        if ! command -v appimagetool &> /dev/null; then
            log_warning "appimagetool not found. Will skip AppImage creation."
            log_info "Install from: https://github.com/AppImage/AppImageKit/releases"
            CREATE_APPIMAGE=false
        fi
    fi

    log_success "Prerequisites check passed"
}

# Clean build artifacts
clean_build() {
    if $CLEAN; then
        log_info "Cleaning build directory..."
        cargo clean -p "$PACKAGE_NAME"
    fi
}

# Build the binary
build_binary() {
    log_info "Building release binary..."

    cd "$PROJECT_ROOT"

    if $CROSS_COMPILE; then
        log_info "Cross-compiling for $TARGET..."
        CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --package "$PACKAGE_NAME" --target "$TARGET"
    else
        log_info "Building native Linux binary..."
        cargo build --release --package "$PACKAGE_NAME"
    fi

    if [ ! -f "$BUILD_DIR/$BINARY_NAME" ]; then
        log_error "Binary not found at $BUILD_DIR/$BINARY_NAME"
        exit 1
    fi

    log_success "Binary built successfully"
}

# Create distribution tarball
create_tarball() {
    log_info "Creating distribution tarball..."

    local arch
    if $CROSS_ARM64; then
        arch="arm64"
    else
        arch="x86_64"
    fi

    local tarball_name="${APP_NAME}-${VERSION}-linux-${arch}"
    local staging_dir="$DIST_DIR/$tarball_name"

    mkdir -p "$DIST_DIR"
    rm -rf "$staging_dir"
    mkdir -p "$staging_dir"

    # Copy binary
    cp "$BUILD_DIR/$BINARY_NAME" "$staging_dir/"
    chmod +x "$staging_dir/$BINARY_NAME"

    # Copy assets if they exist
    if [ -d "$SCRIPT_DIR/../assets" ]; then
        cp -r "$SCRIPT_DIR/../assets" "$staging_dir/"
    fi

    # Create a simple README
    cat > "$staging_dir/README.txt" << EOF
SotF Player v${VERSION}
======================

A high-quality audio player with advanced EQ and upmixing capabilities.

Running
-------
./SotF

Requirements
------------
- Linux x86_64 or ARM64
- X11 or Wayland display server
- ALSA or PulseAudio for audio output

For more information, visit: https://github.com/coderdelphit/stypes
EOF

    # Create tarball
    cd "$DIST_DIR"
    tar -czvf "${tarball_name}.tar.gz" "$tarball_name"
    rm -rf "$staging_dir"

    log_success "Tarball created: $DIST_DIR/${tarball_name}.tar.gz"
}

# Create AppImage (native Linux only)
create_appimage() {
    if ! $CREATE_APPIMAGE; then
        return
    fi

    log_info "Creating AppImage..."

    local appdir="$DIST_DIR/${APP_NAME}.AppDir"
    rm -rf "$appdir"
    mkdir -p "$appdir/usr/bin"
    mkdir -p "$appdir/usr/share/applications"
    mkdir -p "$appdir/usr/share/icons/hicolor/256x256/apps"

    # Copy binary
    cp "$BUILD_DIR/$BINARY_NAME" "$appdir/usr/bin/"
    chmod +x "$appdir/usr/bin/$BINARY_NAME"

    # Create desktop file
    cat > "$appdir/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Type=Application
Name=SotF Player
Comment=High-quality audio player with advanced EQ
Exec=SotF
Icon=sotf
Categories=Audio;AudioVideo;Player;
Terminal=false
EOF

    # Copy icon if it exists
    if [ -f "$SCRIPT_DIR/../assets/sotf.png" ]; then
        cp "$SCRIPT_DIR/../assets/sotf.png" "$appdir/usr/share/icons/hicolor/256x256/apps/sotf.png"
    elif [ -f "$SCRIPT_DIR/../assets/sotf.jpg" ]; then
        # Convert jpg to png if needed
        if command -v convert &> /dev/null; then
            convert "$SCRIPT_DIR/../assets/sotf.jpg" "$appdir/usr/share/icons/hicolor/256x256/apps/sotf.png"
        else
            log_warning "ImageMagick not found, skipping icon conversion"
        fi
    fi

    # Create AppRun script
    cat > "$appdir/AppRun" << 'EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
exec "${HERE}/usr/bin/SotF" "$@"
EOF
    chmod +x "$appdir/AppRun"

    # Symlinks for AppImage
    ln -sf usr/share/applications/${APP_NAME}.desktop "$appdir/${APP_NAME}.desktop"
    if [ -f "$appdir/usr/share/icons/hicolor/256x256/apps/sotf.png" ]; then
        ln -sf usr/share/icons/hicolor/256x256/apps/sotf.png "$appdir/sotf.png"
    fi

    # Build AppImage
    local arch
    arch=$(uname -m)
    ARCH="$arch" appimagetool "$appdir" "$DIST_DIR/${APP_NAME}-${VERSION}-${arch}.AppImage"

    rm -rf "$appdir"
    log_success "AppImage created: $DIST_DIR/${APP_NAME}-${VERSION}-${arch}.AppImage"
}

# Main build process
main() {
    log_info "=========================================="
    log_info "Building $APP_NAME v$VERSION for Linux"
    log_info "=========================================="

    check_prerequisites
    clean_build
    build_binary
    create_tarball

    if $CREATE_APPIMAGE; then
        create_appimage
    fi

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="

    local arch
    if $CROSS_ARM64; then
        arch="arm64"
    else
        arch="x86_64"
    fi

    local tarball_name="${APP_NAME}-${VERSION}-linux-${arch}"
    if [ -f "$DIST_DIR/${tarball_name}.tar.gz" ]; then
        log_info "Tarball: $DIST_DIR/${tarball_name}.tar.gz"
        log_info "Size: $(du -h "$DIST_DIR/${tarball_name}.tar.gz" | cut -f1)"
    fi

    if $CREATE_APPIMAGE && [ -f "$DIST_DIR/${APP_NAME}-${VERSION}-$(uname -m).AppImage" ]; then
        log_info "AppImage: $DIST_DIR/${APP_NAME}-${VERSION}-$(uname -m).AppImage"
    fi
}

main "$@"
