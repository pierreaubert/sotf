#!/bin/bash
#
# Cross-compile SotF Player for Windows x86_64 from Linux
# Creates a distributable zip with both GPUI and TUI binaries
#
# Usage:
#   ./build-windows-cross.sh                # Build unsigned
#   ./build-windows-cross.sh --sign         # Build and sign with Authenticode
#   ./build-windows-cross.sh --clean        # Clean before building
#
# Prerequisites:
#   - Rust toolchain with x86_64-pc-windows-gnu target
#   - mingw-w64 cross-compiler (apt install mingw-w64)
#   - For signing: osslsigncode (apt install osslsigncode)
#
# Environment variables:
#   WINDOWS_CERT_FILE     - Path to .pfx/.p12 code signing certificate
#   WINDOWS_CERT_PASSWORD - Certificate password
#   WINDOWS_TIMESTAMP_URL - Timestamp server (default: http://timestamp.digicert.com)
#

set -euo pipefail

# Configuration
APP_NAME="SotF"
GPUI_BINARY="SotF.exe"
GPUI_PACKAGE="sotf-gpui"
TUI_BINARY="sotf-tui.exe"
TUI_PACKAGE="sotf-tui"
TARGET="x86_64-pc-windows-gnu"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
BUILD_DIR="$TARGET_DIR/$TARGET/release"
DIST_DIR="$PROJECT_ROOT/dist"

# Build options
SIGN=false
CLEAN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --sign)
            SIGN=true
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
            echo "  --sign    Authenticode sign the executables (requires osslsigncode)"
            echo "  --clean   Clean build directory before building"
            echo "  --help    Show this help message"
            echo ""
            echo "Environment variables for signing:"
            echo "  WINDOWS_CERT_FILE       Path to .pfx/.p12 certificate"
            echo "  WINDOWS_CERT_PASSWORD   Certificate password"
            echo "  WINDOWS_TIMESTAMP_URL   Timestamp server (default: http://timestamp.digicert.com)"
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
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo is not installed"
        exit 1
    fi

    # Check for mingw-w64 cross-compiler
    if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
        log_error "mingw-w64 cross-compiler not found"
        log_info "Install with: sudo apt install mingw-w64"
        exit 1
    fi

    # Check Rust target
    if ! rustup target list --installed | grep -q "$TARGET"; then
        log_info "Adding Rust target $TARGET..."
        rustup target add "$TARGET"
    fi

    if $SIGN; then
        if ! command -v osslsigncode &> /dev/null; then
            log_error "osslsigncode not found (required for Authenticode signing)"
            log_info "Install with: sudo apt install osslsigncode"
            exit 1
        fi
        if [ -z "${WINDOWS_CERT_FILE:-}" ]; then
            log_error "WINDOWS_CERT_FILE environment variable not set"
            log_info "Set it to the path of your .pfx/.p12 code signing certificate"
            exit 1
        fi
        if [ ! -f "$WINDOWS_CERT_FILE" ]; then
            log_error "Certificate file not found: $WINDOWS_CERT_FILE"
            exit 1
        fi
        if [ -z "${WINDOWS_CERT_PASSWORD:-}" ]; then
            log_error "WINDOWS_CERT_PASSWORD environment variable not set"
            exit 1
        fi
    fi

    log_success "Prerequisites check passed"
}

clean_build() {
    if $CLEAN; then
        log_info "Cleaning build artifacts..."
        cargo clean --target "$TARGET" -p "$GPUI_PACKAGE" 2>/dev/null || true
        cargo clean --target "$TARGET" -p "$TUI_PACKAGE" 2>/dev/null || true
    fi
}

build_binaries() {
    log_info "Cross-compiling for $TARGET..."

    cd "$PROJECT_ROOT"

    # Build TUI
    log_info "Building $TUI_PACKAGE..."
    cargo build --release --target "$TARGET" -p "$TUI_PACKAGE" --bin sotf-tui
    if [ ! -f "$BUILD_DIR/$TUI_BINARY" ]; then
        log_error "TUI binary not found at $BUILD_DIR/$TUI_BINARY"
        exit 1
    fi
    log_success "TUI binary built"

    # Build GPUI
    log_info "Building $GPUI_PACKAGE..."
    cargo build --release --target "$TARGET" -p "$GPUI_PACKAGE" --bin SotF
    if [ ! -f "$BUILD_DIR/$GPUI_BINARY" ]; then
        log_error "GPUI binary not found at $BUILD_DIR/$GPUI_BINARY"
        exit 1
    fi
    log_success "GPUI binary built"
}

sign_binary() {
    local input="$1"
    local description="$2"

    if ! $SIGN; then
        return
    fi

    local timestamp_url="${WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}"

    log_info "Signing: $(basename "$input")"

    # Sign in-place using a temp file
    local signed_tmp="${input}.signed"
    osslsigncode sign \
        -pkcs12 "$WINDOWS_CERT_FILE" \
        -pass "$WINDOWS_CERT_PASSWORD" \
        -n "$description" \
        -t "$timestamp_url" \
        -in "$input" \
        -out "$signed_tmp"
    mv "$signed_tmp" "$input"

    log_success "Signed: $(basename "$input")"
}

create_distribution() {
    log_info "Creating distribution package..."

    local dist_name="sotf-${VERSION}-windows-x64"
    local staging_dir="$DIST_DIR/$dist_name"

    mkdir -p "$DIST_DIR"
    rm -rf "$staging_dir"
    mkdir -p "$staging_dir"

    # Copy binaries
    cp "$BUILD_DIR/$GPUI_BINARY" "$staging_dir/"
    cp "$BUILD_DIR/$TUI_BINARY" "$staging_dir/"

    # Sign binaries before packaging
    if $SIGN; then
        sign_binary "$staging_dir/$GPUI_BINARY" "SotF Player"
        sign_binary "$staging_dir/$TUI_BINARY" "SotF TUI Player"
    fi

    # Copy runtime DLLs if present
    for dll in openblas.dll nlopt.dll; do
        if [ -f "$BUILD_DIR/$dll" ]; then
            cp "$BUILD_DIR/$dll" "$staging_dir/"
            log_info "Added $dll"
        fi
    done

    # Copy assets if they exist
    if [ -d "$PROJECT_ROOT/crates/app-gpui/assets" ]; then
        cp -r "$PROJECT_ROOT/crates/app-gpui/assets" "$staging_dir/"
    fi

    # Create README
    cat > "$staging_dir/README.txt" << EOF
SotF Player v${VERSION}
======================

A high-quality audio player with advanced EQ and upmixing capabilities.

Included Binaries
-----------------
- SotF.exe      : GPUI-based graphical player
- sotf-tui.exe  : Terminal UI player

Running
-------
GUI: Double-click SotF.exe
TUI: Run sotf-tui.exe from command line or PowerShell

Requirements
------------
- Windows 10/11 x64

For more information, visit: https://github.com/pierreaubert/sotf
EOF

    # Create zip
    local zip_path="$DIST_DIR/${dist_name}.zip"
    rm -f "$zip_path"

    cd "$DIST_DIR"
    if command -v zip &> /dev/null; then
        zip -r "$zip_path" "$dist_name"
    else
        # Fallback to tar+gzip if zip not available
        tar -czf "${dist_name}.tar.gz" "$dist_name"
        zip_path="$DIST_DIR/${dist_name}.tar.gz"
    fi

    rm -rf "$staging_dir"

    log_success "Distribution created: $zip_path"

    # Generate checksums
    log_info "Generating SHA256 checksums..."
    (cd "$DIST_DIR" && sha256sum "$(basename "$zip_path")" > SHA256SUMS.windows-x64)

    # GPG sign the archive if signing is enabled
    if $SIGN; then
        if command -v gpg &> /dev/null; then
            local gpg_key_arg=""
            if [ -n "${GPG_KEY_ID:-}" ]; then
                gpg_key_arg="--default-key $GPG_KEY_ID"
            fi
            gpg --batch --yes $gpg_key_arg --detach-sign --armor "$zip_path"
            gpg --batch --yes $gpg_key_arg --detach-sign --armor "$DIST_DIR/SHA256SUMS.windows-x64"
            log_success "Archive and checksums GPG-signed"
        else
            log_warning "gpg not found, skipping GPG signature of archive"
        fi
    fi
}

main() {
    log_info "=========================================="
    log_info "Cross-compiling $APP_NAME v$VERSION for Windows x64"
    log_info "=========================================="

    check_prerequisites
    clean_build
    build_binaries
    create_distribution

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="

    local dist_name="sotf-${VERSION}-windows-x64"
    for ext in zip tar.gz; do
        local pkg="$DIST_DIR/${dist_name}.${ext}"
        if [ -f "$pkg" ]; then
            log_info "Package: $pkg"
            log_info "Size: $(du -h "$pkg" | cut -f1)"
        fi
    done

    if $SIGN; then
        log_info "Authenticode signed: Yes"
    else
        log_warning "Authenticode signed: No (use --sign for distribution)"
    fi
}

main "$@"
