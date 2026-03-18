#!/bin/bash
#
# Sign Linux artifacts in dist/ using cosign (keyless or key-based)
#
# Creates detached signatures (.sig) and certificates (.cert) for each artifact.
# Users verify with: cosign verify-blob --signature file.sig --certificate file.cert file
#
# Usage:
#   ./sign-linux.sh                          # Sign all Linux artifacts in dist/ (keyless via OIDC)
#   ./sign-linux.sh --key cosign.key         # Sign with a local key
#   ./sign-linux.sh dist/SotF-*.tar.gz       # Sign specific files
#
# Prerequisites:
#   - cosign: https://docs.sigstore.dev/cosign/system_config/installation/
#     brew install cosign  OR  go install github.com/sigstore/cosign/v2/cmd/cosign@latest
#
# Keyless signing (default):
#   Uses Sigstore OIDC — opens a browser for identity verification.
#   No keys to manage. Signatures are logged in Rekor transparency log.
#
# Key-based signing:
#   COSIGN_KEY          - Path to cosign private key
#   COSIGN_PASSWORD     - Password for the private key (or set empty for no password)
#
# To generate a key pair:
#   cosign generate-key-pair
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"

COSIGN_KEY_PATH=""
SPECIFIC_FILES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --key)
            COSIGN_KEY_PATH="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--key <cosign.key>] [file ...]"
            echo ""
            echo "Signs Linux artifacts using cosign (Sigstore)."
            echo ""
            echo "Options:"
            echo "  --key <path>  Sign with a local cosign private key"
            echo "                Without --key, uses keyless OIDC signing"
            echo ""
            echo "Examples:"
            echo "  $0                              # Keyless sign all Linux artifacts in dist/"
            echo "  $0 --key cosign.key             # Key-based sign all Linux artifacts"
            echo "  $0 dist/SotF-0.5.11-linux-arm64.tar.gz  # Sign a specific file"
            echo ""
            echo "Verification:"
            echo "  # Keyless:"
            echo "  cosign verify-blob --signature file.sig --certificate file.cert \\"
            echo "    --certificate-identity=<email> --certificate-oidc-issuer=https://accounts.google.com file"
            echo ""
            echo "  # Key-based:"
            echo "  cosign verify-blob --signature file.sig --key cosign.pub file"
            exit 0
            ;;
        *)
            SPECIFIC_FILES+=("$1")
            shift
            ;;
    esac
done

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

check_prerequisites() {
    if ! command -v cosign &> /dev/null; then
        log_error "cosign is not installed"
        log_info "Install with: brew install cosign"
        log_info "Or: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
        log_info "Docs: https://docs.sigstore.dev/cosign/system_config/installation/"
        exit 1
    fi

    if [ -n "$COSIGN_KEY_PATH" ] && [ ! -f "$COSIGN_KEY_PATH" ]; then
        log_error "Key file not found: $COSIGN_KEY_PATH"
        exit 1
    fi
}

# Sign a single file
sign_file() {
    local file_path="$1"
    local sig_path="${file_path}.sig"
    local cert_path="${file_path}.cert"

    if [ ! -f "$file_path" ]; then
        log_warning "File not found: $file_path"
        return
    fi

    log_info "Signing: $(basename "$file_path")"

    if [ -n "$COSIGN_KEY_PATH" ]; then
        # Key-based signing
        cosign sign-blob \
            --key "$COSIGN_KEY_PATH" \
            --output-signature "$sig_path" \
            "$file_path"
        log_success "Signed (key): $(basename "$file_path")"
        log_info "  Signature: $(basename "$sig_path")"
    else
        # Keyless signing via OIDC (Sigstore)
        cosign sign-blob \
            --output-signature "$sig_path" \
            --output-certificate "$cert_path" \
            "$file_path"
        log_success "Signed (keyless): $(basename "$file_path")"
        log_info "  Signature:   $(basename "$sig_path")"
        log_info "  Certificate: $(basename "$cert_path")"
    fi
}

# Check if a file is a Linux artifact
is_linux_artifact() {
    local f="$1"
    local name
    name=$(basename "$f")
    case "$name" in
        *linux*|*.AppImage|*.deb)
            return 0
            ;;
    esac
    return 1
}

main() {
    log_info "=========================================="
    log_info "Linux Artifact Signing (cosign)"
    log_info "=========================================="

    if [ -n "$COSIGN_KEY_PATH" ]; then
        log_info "Mode: key-based ($COSIGN_KEY_PATH)"
    else
        log_info "Mode: keyless (Sigstore OIDC)"
    fi

    check_prerequisites

    if [ ${#SPECIFIC_FILES[@]} -gt 0 ]; then
        for f in "${SPECIFIC_FILES[@]}"; do
            sign_file "$f"
        done
    else
        local found=false
        for f in "$DIST_DIR"/*; do
            if [ -f "$f" ] && is_linux_artifact "$f"; then
                # Skip existing signatures
                case "$f" in
                    *.sig|*.cert) continue ;;
                esac
                sign_file "$f"
                found=true
            fi
        done

        if ! $found; then
            log_warning "No Linux artifacts found in $DIST_DIR"
            log_info "Run 'just cross-linux-arm64' or 'just cross-linux-x86' first"
            exit 1
        fi
    fi

    log_info "=========================================="
    log_success "Signing complete!"
    log_info "=========================================="
}

main "$@"
