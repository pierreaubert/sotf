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
#   ./build-linux.sh --deb              # Build and create .deb package (Debian/Ubuntu)
#   ./build-linux.sh --install-tools    # Download and install appimagetool
#
# Prerequisites:
#   - Rust toolchain
#   - Linux build dependencies (see justfile: install-ubuntu-common)
#   - For cross-compilation: cross tool (cargo install cross)
#   - For AppImage: appimagetool (use --install-tools to download)
#   - For .deb: dpkg-deb (usually pre-installed on Debian/Ubuntu)
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
CREATE_DEB=false
INSTALL_TOOLS=false
CLEAN=false
TARGET=""
BUILD_DIR=""
DIST_DIR="$PROJECT_ROOT/dist"
TOOLS_DIR="$PROJECT_ROOT/tools"

# AppImage tool configuration
APPIMAGETOOL_VERSION="continuous"
APPIMAGETOOL_URL="https://github.com/AppImage/AppImageKit/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-x86_64.AppImage"
APPIMAGETOOL_BIN=""

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
        --deb)
            CREATE_DEB=true
            shift
            ;;
        --install-tools)
            INSTALL_TOOLS=true
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
            echo "  --cross          Cross-compile for Linux x86_64 from macOS"
            echo "  --cross-arm64    Cross-compile for Linux ARM64 from macOS"
            echo "  --appimage       Create AppImage (native Linux only)"
            echo "  --deb            Create .deb package (Debian/Ubuntu)"
            echo "  --install-tools  Download and install appimagetool"
            echo "  --clean          Clean build directory before building"
            echo "  --help           Show this help message"
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

# Download and install appimagetool
install_appimagetool() {
    log_info "Installing appimagetool..."

    # Detect architecture
    local arch
    arch=$(uname -m)
    local tool_url

    case "$arch" in
        x86_64)
            tool_url="https://github.com/AppImage/AppImageKit/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-x86_64.AppImage"
            ;;
        aarch64|arm64)
            tool_url="https://github.com/AppImage/AppImageKit/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-aarch64.AppImage"
            ;;
        armv7l|armhf)
            tool_url="https://github.com/AppImage/AppImageKit/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-armhf.AppImage"
            ;;
        i686|i386)
            tool_url="https://github.com/AppImage/AppImageKit/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-i686.AppImage"
            ;;
        *)
            log_error "Unsupported architecture: $arch"
            log_info "Please download appimagetool manually from:"
            log_info "  https://github.com/AppImage/AppImageKit/releases"
            exit 1
            ;;
    esac

    mkdir -p "$TOOLS_DIR"

    local tool_path="$TOOLS_DIR/appimagetool"

    log_info "Downloading appimagetool for $arch..."
    log_info "URL: $tool_url"

    if command -v curl &> /dev/null; then
        curl -L -o "$tool_path" "$tool_url"
    elif command -v wget &> /dev/null; then
        wget -O "$tool_path" "$tool_url"
    else
        log_error "Neither curl nor wget found. Please install one of them."
        exit 1
    fi

    chmod +x "$tool_path"

    # Verify download
    if [ ! -f "$tool_path" ] || [ ! -s "$tool_path" ]; then
        log_error "Download failed or file is empty"
        exit 1
    fi

    log_success "appimagetool installed to $tool_path"
    log_info ""
    log_info "To use appimagetool system-wide, you can either:"
    log_info "  1. Add $TOOLS_DIR to your PATH:"
    log_info "     export PATH=\"$TOOLS_DIR:\$PATH\""
    log_info ""
    log_info "  2. Or create a symlink:"
    log_info "     sudo ln -s $tool_path /usr/local/bin/appimagetool"
    log_info ""
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
        # Check for appimagetool in PATH or in tools directory
        if command -v appimagetool &> /dev/null; then
            APPIMAGETOOL_BIN="appimagetool"
        elif [ -x "$TOOLS_DIR/appimagetool" ]; then
            APPIMAGETOOL_BIN="$TOOLS_DIR/appimagetool"
            log_info "Using appimagetool from $TOOLS_DIR"
        else
            log_warning "appimagetool not found. Will skip AppImage creation."
            log_info "Install with: $0 --install-tools"
            log_info "Or download from: https://github.com/AppImage/AppImageKit/releases"
            CREATE_APPIMAGE=false
        fi
    fi

    if $CREATE_DEB; then
        if ! command -v dpkg-deb &> /dev/null; then
            log_warning "dpkg-deb not found. Will skip .deb creation."
            log_info "Install with: sudo apt install dpkg"
            CREATE_DEB=false
        fi
        if ! command -v fakeroot &> /dev/null; then
            log_warning "fakeroot not found (recommended for .deb creation)"
            log_info "Install with: sudo apt install fakeroot"
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
    elif $CROSS_COMPILE; then
        arch="x86_64"
    else
        # Native build - detect architecture
        case "$(uname -m)" in
            x86_64)
                arch="x86_64"
                ;;
            aarch64|arm64)
                arch="arm64"
                ;;
            *)
                arch="$(uname -m)"
                ;;
        esac
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
    ARCH="$arch" "$APPIMAGETOOL_BIN" "$appdir" "$DIST_DIR/${APP_NAME}-${VERSION}-${arch}.AppImage"

    rm -rf "$appdir"
    log_success "AppImage created: $DIST_DIR/${APP_NAME}-${VERSION}-${arch}.AppImage"
}

# Create .deb package (Debian/Ubuntu)
create_deb() {
    if ! $CREATE_DEB; then
        return
    fi

    log_info "Creating .deb package..."

    # Determine architecture
    local arch
    local deb_arch
    if $CROSS_ARM64; then
        arch="arm64"
        deb_arch="arm64"
    else
        arch=$(uname -m)
        case "$arch" in
            x86_64)
                deb_arch="amd64"
                ;;
            aarch64)
                deb_arch="arm64"
                ;;
            armv7l)
                deb_arch="armhf"
                ;;
            i686)
                deb_arch="i386"
                ;;
            *)
                deb_arch="$arch"
                ;;
        esac
    fi

    local deb_name="sotf_${VERSION}_${deb_arch}"
    local deb_dir="$DIST_DIR/${deb_name}"

    rm -rf "$deb_dir"
    mkdir -p "$deb_dir/DEBIAN"
    mkdir -p "$deb_dir/usr/bin"
    mkdir -p "$deb_dir/usr/share/applications"
    mkdir -p "$deb_dir/usr/share/icons/hicolor/256x256/apps"
    mkdir -p "$deb_dir/usr/share/doc/sotf"

    # Copy binary
    cp "$BUILD_DIR/$BINARY_NAME" "$deb_dir/usr/bin/sotf"
    chmod 755 "$deb_dir/usr/bin/sotf"

    # Create desktop file
    cat > "$deb_dir/usr/share/applications/sotf.desktop" << EOF
[Desktop Entry]
Type=Application
Name=SotF Player
GenericName=Audio Player
Comment=High-quality audio player with advanced EQ and upmixing
Exec=sotf %F
Icon=sotf
Categories=Audio;AudioVideo;Player;Music;
Terminal=false
MimeType=audio/flac;audio/mpeg;audio/ogg;audio/wav;audio/x-wav;audio/mp4;audio/aac;
Keywords=audio;music;player;eq;equalizer;
EOF

    # Copy icon if it exists
    if [ -f "$SCRIPT_DIR/../assets/sotf.png" ]; then
        cp "$SCRIPT_DIR/../assets/sotf.png" "$deb_dir/usr/share/icons/hicolor/256x256/apps/sotf.png"
    elif [ -f "$SCRIPT_DIR/../assets/sotf.jpg" ]; then
        if command -v convert &> /dev/null; then
            convert "$SCRIPT_DIR/../assets/sotf.jpg" "$deb_dir/usr/share/icons/hicolor/256x256/apps/sotf.png"
        fi
    fi

    # Create copyright file
    cat > "$deb_dir/usr/share/doc/sotf/copyright" << EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: SotF
Source: https://github.com/coderdelphit/stypes

Files: *
Copyright: $(date +%Y) SotF Authors
License: MIT or Apache-2.0
EOF

    # Create changelog (minimal)
    cat > "$deb_dir/usr/share/doc/sotf/changelog" << EOF
sotf (${VERSION}) stable; urgency=medium

  * Release ${VERSION}

 -- SotF Team <sotf@example.com>  $(date -R)
EOF
    gzip -9 -n "$deb_dir/usr/share/doc/sotf/changelog"

    # Calculate installed size (in KB)
    local installed_size
    installed_size=$(du -sk "$deb_dir" | cut -f1)

    # Create control file
    cat > "$deb_dir/DEBIAN/control" << EOF
Package: sotf
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${deb_arch}
Installed-Size: ${installed_size}
Depends: libasound2 (>= 1.0.16), libgtk-3-0 (>= 3.0), libglib2.0-0 (>= 2.12.0)
Recommends: pulseaudio | pipewire-pulse
Maintainer: SotF Team <sotf@example.com>
Homepage: https://github.com/coderdelphit/stypes
Description: High-quality audio player with advanced EQ and upmixing
 SotF (Sound of the Future) is a comprehensive audio player with:
  - Advanced parametric EQ optimization
  - Stereo to 5.0 surround upmixing
  - Support for FLAC, MP3, AAC, OGG, WAV formats
  - Real-time spectrum analysis
  - EBU R128 loudness measurement
EOF

    # Create postinst script to update icon cache
    cat > "$deb_dir/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e

if [ "$1" = "configure" ]; then
    # Update icon cache
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t /usr/share/icons/hicolor || true
    fi
    # Update desktop database
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications || true
    fi
fi

exit 0
EOF
    chmod 755 "$deb_dir/DEBIAN/postinst"

    # Create postrm script
    cat > "$deb_dir/DEBIAN/postrm" << 'EOF'
#!/bin/sh
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    # Update icon cache
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t /usr/share/icons/hicolor || true
    fi
    # Update desktop database
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications || true
    fi
fi

exit 0
EOF
    chmod 755 "$deb_dir/DEBIAN/postrm"

    # Build the .deb package
    local deb_file="$DIST_DIR/${deb_name}.deb"

    if command -v fakeroot &> /dev/null; then
        fakeroot dpkg-deb --build "$deb_dir" "$deb_file"
    else
        dpkg-deb --build "$deb_dir" "$deb_file"
    fi

    # Clean up
    rm -rf "$deb_dir"

    # Verify the package
    if command -v lintian &> /dev/null; then
        log_info "Running lintian checks..."
        lintian "$deb_file" || true
    fi

    log_success ".deb package created: $deb_file"
}

# Main build process
main() {
    # Handle --install-tools first (standalone operation)
    if $INSTALL_TOOLS; then
        log_info "=========================================="
        log_info "Installing AppImage Tools"
        log_info "=========================================="
        install_appimagetool
        log_success "Tools installation complete!"
        exit 0
    fi

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

    if $CREATE_DEB; then
        create_deb
    fi

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="

    local arch
    local deb_arch
    if $CROSS_ARM64; then
        arch="arm64"
        deb_arch="arm64"
    elif $CROSS_COMPILE; then
        arch="x86_64"
        deb_arch="amd64"
    else
        # Native build - detect architecture
        case "$(uname -m)" in
            x86_64)
                arch="x86_64"
                deb_arch="amd64"
                ;;
            aarch64|arm64)
                arch="arm64"
                deb_arch="arm64"
                ;;
            *)
                arch="$(uname -m)"
                deb_arch="$(uname -m)"
                ;;
        esac
    fi

    local tarball_name="${APP_NAME}-${VERSION}-linux-${arch}"
    if [ -f "$DIST_DIR/${tarball_name}.tar.gz" ]; then
        log_info "Tarball: $DIST_DIR/${tarball_name}.tar.gz"
        log_info "Size: $(du -h "$DIST_DIR/${tarball_name}.tar.gz" | cut -f1)"
    fi

    if $CREATE_APPIMAGE && [ -f "$DIST_DIR/${APP_NAME}-${VERSION}-$(uname -m).AppImage" ]; then
        log_info "AppImage: $DIST_DIR/${APP_NAME}-${VERSION}-$(uname -m).AppImage"
    fi

    local deb_file="$DIST_DIR/sotf_${VERSION}_${deb_arch}.deb"
    if $CREATE_DEB && [ -f "$deb_file" ]; then
        log_info ".deb: $deb_file"
        log_info "Size: $(du -h "$deb_file" | cut -f1)"
        log_info ""
        log_info "Install with: sudo dpkg -i $deb_file"
    fi
}

main "$@"
