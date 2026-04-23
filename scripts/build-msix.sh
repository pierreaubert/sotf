#!/bin/bash
#
# Build MSIX package for SotF Player from Linux
# Creates an MSIX package from pre-built Windows binaries.
# Optionally signs with osslsigncode (works in Docker/Linux).
#
# Usage:
#   ./build-msix.sh                          # Build unsigned MSIX
#   ./build-msix.sh --sign                   # Sign with self-signed cert (auto-generated)
#   ./build-msix.sh --build-dir <path>       # Custom build directory
#   ./build-msix.sh --arch x64               # Specify architecture (x64 or arm64)
#
# Signing:
#   When --sign is used, the script looks for a certificate in this order:
#     1. WINDOWS_CERT_FILE env var (existing .pfx)
#     2. certs/sotf-selfsigned.pfx (previously generated)
#     3. Auto-generates a self-signed certificate
#
#   For a CA-issued certificate, set:
#     WINDOWS_CERT_FILE     - Path to .pfx/.p12 code signing certificate
#     WINDOWS_CERT_PASSWORD - Certificate password
#     WINDOWS_TIMESTAMP_URL - Timestamp server (default: http://timestamp.digicert.com)
#
# Prerequisites:
#   - Pre-built Windows binaries (SotF.exe, sotf-tui.exe)
#   - zip
#   - For signing: osslsigncode + openssl (for self-signed cert generation)
#

set -euo pipefail

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Certificate subject — MUST match Publisher in AppxManifest.xml
CERT_SUBJECT="/CN=Pierre Aubert/O=Spinorama/C=FR"
CERT_DIR="$PROJECT_ROOT/certs"
SELFSIGNED_PFX="$CERT_DIR/sotf-selfsigned.pfx"
SELFSIGNED_PASSWORD="sotf-dev"

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi
MSIX_VERSION="${VERSION}.0"

# Defaults
ARCH="x64"
BUILD_DIR=""
DIST_DIR="$PROJECT_ROOT/dist"
SIGN=false
TIMESTAMP_URL="${WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build-dir)
            BUILD_DIR="$2"
            shift 2
            ;;
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --sign)
            SIGN=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --build-dir <path>   Directory containing built Windows binaries"
            echo "  --arch <x64|arm64>   Target architecture (default: x64)"
            echo "  --sign               Sign binaries and MSIX with osslsigncode"
            echo "  --help               Show this help message"
            echo ""
            echo "Signing:"
            echo "  With --sign, a self-signed certificate is auto-generated if needed."
            echo "  To use a CA-issued certificate, set these environment variables:"
            echo "    WINDOWS_CERT_FILE       Path to .pfx/.p12 certificate"
            echo "    WINDOWS_CERT_PASSWORD   Certificate password"
            echo "    WINDOWS_TIMESTAMP_URL   Timestamp server (default: http://timestamp.digicert.com)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# If no build dir specified, try to find binaries
if [ -z "$BUILD_DIR" ]; then
    TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
    # Try gnullvm targets first (Docker cross builds), then gnu
    for triple in x86_64-pc-windows-gnullvm x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm aarch64-pc-windows-gnu; do
        candidate="$TARGET_DIR/$triple/release"
        if [ -f "$candidate/sotf-tui.exe" ]; then
            BUILD_DIR="$candidate"
            break
        fi
    done
    # Fallback to plain release dir
    if [ -z "$BUILD_DIR" ]; then
        BUILD_DIR="$TARGET_DIR/release"
    fi
fi

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

# Generate a self-signed code signing certificate
generate_selfsigned_cert() {
    log_info "Generating self-signed code signing certificate..."

    mkdir -p "$CERT_DIR"

    local key_file="$CERT_DIR/sotf-selfsigned.key"
    local cert_file="$CERT_DIR/sotf-selfsigned.crt"

    # Generate key + certificate (valid 3 years, code signing EKU)
    openssl req -x509 -newkey rsa:2048 \
        -keyout "$key_file" \
        -out "$cert_file" \
        -days 1095 \
        -nodes \
        -subj "$CERT_SUBJECT" \
        -addext "extendedKeyUsage=codeSigning" \
        -addext "keyUsage=digitalSignature" \
        2>/dev/null

    # Bundle into .pfx
    openssl pkcs12 -export \
        -out "$SELFSIGNED_PFX" \
        -inkey "$key_file" \
        -in "$cert_file" \
        -password "pass:${SELFSIGNED_PASSWORD}" \
        2>/dev/null

    # Clean up loose key/cert (pfx has everything)
    rm -f "$key_file" "$cert_file"

    log_success "Self-signed certificate created: $SELFSIGNED_PFX"
    log_warning "This certificate is for TESTING ONLY."
    log_info "Users must trust it before installing the MSIX:"
    log_info "  1. Double-click the .msix"
    log_info "  2. Or import certs/sotf-selfsigned.pfx into Trusted People store"
}

# Resolve which certificate to use for signing
resolve_certificate() {
    # Priority 1: explicit env var
    if [ -n "${WINDOWS_CERT_FILE:-}" ] && [ -f "${WINDOWS_CERT_FILE}" ]; then
        log_info "Using certificate from WINDOWS_CERT_FILE: $WINDOWS_CERT_FILE"
        return
    fi

    # Priority 2: previously generated self-signed cert
    if [ -f "$SELFSIGNED_PFX" ]; then
        log_info "Using existing self-signed certificate: $SELFSIGNED_PFX"
        export WINDOWS_CERT_FILE="$SELFSIGNED_PFX"
        export WINDOWS_CERT_PASSWORD="$SELFSIGNED_PASSWORD"
        return
    fi

    # Priority 3: generate a new self-signed cert
    if ! command -v openssl &> /dev/null; then
        log_error "openssl is required to generate a self-signed certificate"
        log_info "Install with: apt install openssl"
        exit 1
    fi

    generate_selfsigned_cert
    export WINDOWS_CERT_FILE="$SELFSIGNED_PFX"
    export WINDOWS_CERT_PASSWORD="$SELFSIGNED_PASSWORD"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking for Windows binaries in $BUILD_DIR..."

    local found_any=false
    for bin in SotF.exe sotf-tui.exe; do
        if [ -f "$BUILD_DIR/$bin" ]; then
            log_info "Found $bin"
            found_any=true
        fi
    done

    if ! $found_any; then
        log_error "No Windows binaries found in $BUILD_DIR"
        log_info "Build them first with: just cross-windows-x86"
        exit 1
    fi

    if ! command -v zip &> /dev/null; then
        log_error "zip is required but not found"
        log_info "Install with: apt install zip"
        exit 1
    fi

    if $SIGN; then
        if ! command -v osslsigncode &> /dev/null; then
            log_error "osslsigncode is required for signing but not found"
            log_info "Install with: apt install osslsigncode"
            exit 1
        fi
        resolve_certificate
        log_info "Signing enabled with certificate: $WINDOWS_CERT_FILE"
    fi
}

# Sign a single file with osslsigncode
sign_file() {
    local input="$1"
    local description="${2:-SotF Player}"
    local filename
    filename=$(basename "$input")

    log_info "Signing $filename..."

    local signed_tmp="${input}.signed"
    osslsigncode sign \
        -pkcs12 "$WINDOWS_CERT_FILE" \
        -pass "$WINDOWS_CERT_PASSWORD" \
        -n "$description" \
        -t "$TIMESTAMP_URL" \
        -in "$input" \
        -out "$signed_tmp"
    mv "$signed_tmp" "$input"

    log_success "Signed: $filename"
}

build_msix() {
    log_info "Building MSIX package v${VERSION} (${ARCH})..."

    local staging="$DIST_DIR/msix-staging"
    local output="$DIST_DIR/SotF-${VERSION}-windows-${ARCH}.msix"

    rm -rf "$staging"
    mkdir -p "$staging/assets"

    # Copy binaries
    for bin in SotF.exe sotf-tui.exe; do
        if [ -f "$BUILD_DIR/$bin" ]; then
            cp "$BUILD_DIR/$bin" "$staging/"
            log_info "Added $bin"
        fi
    done

    # Copy nlopt.dll if present
    if [ -f "$BUILD_DIR/nlopt.dll" ]; then
        cp "$BUILD_DIR/nlopt.dll" "$staging/"
        log_info "Added nlopt.dll"
    fi

    # Sign executables before packaging into MSIX
    if $SIGN; then
        for bin in SotF.exe sotf-tui.exe; do
            if [ -f "$staging/$bin" ]; then
                sign_file "$staging/$bin" "SotF Player"
            fi
        done
    fi

    # Copy app assets (fonts, icons, headphone-targets — not demo-audio)
    local assets_src="$PROJECT_ROOT/crates/app-gpui/assets"
    if [ -d "$assets_src" ]; then
        for subdir in fonts icons headphone-targets; do
            if [ -d "$assets_src/$subdir" ]; then
                cp -r "$assets_src/$subdir" "$staging/assets/"
            fi
        done
    fi

    # Generate MSIX icon assets from source PNG
    local source_png="$assets_src/sotf.png"
    if [ -f "$source_png" ]; then
        cp "$source_png" "$staging/assets/sotf-44x44.png"
        cp "$source_png" "$staging/assets/sotf-150x150.png"
        cp "$source_png" "$staging/assets/sotf-310x150.png"
        log_info "Copied icon assets"
    fi

    # Generate AppxManifest.xml with correct version and architecture
    sed -e "s/Version=\"[^\"]*\"/Version=\"${MSIX_VERSION}\"/" \
        -e "s/ProcessorArchitecture=\"[^\"]*\"/ProcessorArchitecture=\"${ARCH}\"/" \
        "$PROJECT_ROOT/builds/windows/AppxManifest.xml" > "$staging/AppxManifest.xml"

    # Create MSIX (it's a ZIP with .msix extension)
    rm -f "$output"
    (cd "$staging" && zip -r "$output" .)

    # Cleanup staging
    rm -rf "$staging"

    if [ ! -f "$output" ]; then
        log_error "Failed to create MSIX"
        exit 1
    fi

    # Note: MSIX package-level signing requires SignTool.exe (Windows only).
    # osslsigncode cannot sign .msix files. The individual .exe files inside
    # the package are already signed above. To sign the MSIX itself, use
    # sign-windows.ps1 on a Windows machine with SignTool.exe.
    if $SIGN; then
        log_warning "MSIX package signing skipped (osslsigncode does not support .msix)"
        log_info "Executables inside the MSIX are signed."
        log_info "To sign the .msix itself, use SignTool.exe on Windows."
    fi

    log_success "MSIX created: $output"
    log_info "Size: $(du -h "$output" | cut -f1)"

    if ! $SIGN; then
        log_info ""
        log_info "Package is UNSIGNED. To sign, re-run with --sign"
    fi
}

main() {
    log_info "=========================================="
    log_info "Building SotF MSIX v${VERSION} (${ARCH})"
    log_info "=========================================="

    check_prerequisites
    build_msix

    log_info "=========================================="
    log_success "MSIX build complete!"
    log_info "=========================================="
}

main "$@"
