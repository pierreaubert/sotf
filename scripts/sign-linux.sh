#!/bin/bash
#
# Sign Linux artifacts in dist/ using cosign (keyless or key-based)
#
# Creates a cosign bundle (.bundle) for each artifact.
# Users verify with: cosign verify-blob --bundle file.bundle file
#
# Usage:
#   ./sign-linux.sh                          # Sign current-version Linux artifacts in dist/ (keyless via OIDC)
#   ./sign-linux.sh --key cosign.key         # Sign with a local key
#   ./sign-linux.sh --key hashivault://sotf  # Sign with a KMS-backed key (Vault/AWS/GCP/Azure)
#   ./sign-linux.sh dist/sotf-desktop-*.tar.gz  # Sign specific files
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

# Extract version from root Cargo.toml
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

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
            echo "Only signs artifacts matching the current version ($VERSION) from Cargo.toml."
            echo ""
            echo "Options:"
            echo "  --key <path>  Sign with a cosign private key or KMS URI"
            echo "                Supports: local files, hashivault://, awskms://, gcpkms://, azurekms://"
            echo "                Without --key, uses keyless OIDC signing (requires browser)"
            echo ""
            echo "Examples:"
            echo "  $0                              # Keyless sign current-version Linux artifacts in dist/"
            echo "  $0 --key cosign.key             # Key-based sign current-version Linux artifacts"
            echo "  $0 dist/sotf-desktop-${VERSION}-linux-arm64.tar.gz  # Sign a specific file"
            echo ""
            echo "Verification:"
            echo "  # Keyless:"
            echo "  cosign verify-blob --bundle file.bundle \\"
            echo "    --certificate-identity=<email> --certificate-oidc-issuer=https://accounts.google.com file"
            echo ""
            echo "  # Key-based:"
            echo "  cosign verify-blob --bundle file.bundle --key cosign.pub file"
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

ensure_cosign() {
    # Check PATH and ~/go/bin
    if command -v cosign &> /dev/null; then
        return 0
    fi
    if [ -x "$HOME/go/bin/cosign" ]; then
        export PATH="$HOME/go/bin:$PATH"
        return 0
    fi

    # Not found — try to install via go
    log_warning "cosign not found in PATH or ~/go/bin"

    if command -v go &> /dev/null; then
        log_info "Installing cosign via: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
        go install github.com/sigstore/cosign/v2/cmd/cosign@latest
        if [ -x "$HOME/go/bin/cosign" ]; then
            export PATH="$HOME/go/bin:$PATH"
            log_success "cosign installed to ~/go/bin/cosign"
            return 0
        fi
    fi

    log_error "cosign is not installed and could not be auto-installed"
    log_info "Install with: brew install cosign"
    log_info "Or: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
    log_info "Docs: https://docs.sigstore.dev/cosign/system_config/installation/"
    exit 1
}

check_prerequisites() {
    ensure_cosign

    if [ -n "$COSIGN_KEY_PATH" ]; then
        case "$COSIGN_KEY_PATH" in
            hashivault://*|awskms://*|gcpkms://*|azurekms://*|k8s://*|pkcs11:*)
                # KMS URI — nothing to check locally
                ;;
            *)
                if [ ! -f "$COSIGN_KEY_PATH" ]; then
                    log_error "Key file not found: $COSIGN_KEY_PATH"
                    exit 1
                fi
                ;;
        esac
    fi
}

# Sign a single file
sign_file() {
    local file_path="$1"
    local bundle_path="${file_path}.bundle"

    if [ ! -f "$file_path" ]; then
        log_warning "File not found: $file_path"
        return
    fi

    log_info "Signing: $(basename "$file_path")"

    if [ -n "$COSIGN_KEY_PATH" ]; then
        # Key-based signing
        cosign sign-blob \
            --key "$COSIGN_KEY_PATH" \
            --bundle "$bundle_path" \
            "$file_path"
        log_success "Signed (key): $(basename "$file_path")"
    else
        # Keyless signing via OIDC (Sigstore)
        cosign sign-blob \
            --bundle "$bundle_path" \
            "$file_path"
        log_success "Signed (keyless): $(basename "$file_path")"
    fi
    log_info "  Bundle: $(basename "$bundle_path")"
}

# Check if a file is a Linux artifact for the current version
is_current_linux_artifact() {
    local f="$1"
    local name
    name=$(basename "$f")

    # Skip signature bundles
    case "$name" in
        *.bundle|*.sig|*.cert) return 1 ;;
    esac

    # Must contain the current version AND be a Linux artifact
    case "$name" in
        *"$VERSION"*linux*|*"$VERSION"*.AppImage|*"$VERSION"*.deb)
            return 0
            ;;
    esac
    return 1
}

main() {
    log_info "=========================================="
    log_info "Linux Artifact Signing (cosign) v${VERSION}"
    log_info "=========================================="

    if [ -n "$COSIGN_KEY_PATH" ]; then
        case "$COSIGN_KEY_PATH" in
            hashivault://*) log_info "Mode: KMS (HashiCorp Vault) — $COSIGN_KEY_PATH" ;;
            awskms://*)     log_info "Mode: KMS (AWS) — $COSIGN_KEY_PATH" ;;
            gcpkms://*)     log_info "Mode: KMS (GCP) — $COSIGN_KEY_PATH" ;;
            azurekms://*)   log_info "Mode: KMS (Azure) — $COSIGN_KEY_PATH" ;;
            *)              log_info "Mode: key-based ($COSIGN_KEY_PATH)" ;;
        esac
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
            if [ -f "$f" ] && is_current_linux_artifact "$f"; then
                sign_file "$f"
                found=true
            fi
        done

        if ! $found; then
            log_warning "No Linux artifacts for v${VERSION} found in $DIST_DIR"
            log_info "Run 'just docker-linux-arm64' or 'just docker-linux-x86' first"
            exit 1
        fi
    fi

    log_info "=========================================="
    log_success "Signing complete!"
    log_info "=========================================="
}

main "$@"
