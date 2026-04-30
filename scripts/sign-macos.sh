#!/bin/bash
#
# Sign and notarize macOS artifacts in dist/
#
# Signs DMGs, app bundles inside DMGs, and pkg containers produced by the
# build scripts. Package payload code must be signed before pkgbuild; for
# Systemwide packages this is handled by build-dmg-daemon.sh when DEVELOPER_ID
# is set.
# Run this AFTER build-dmg-sotf.sh and/or build-dmg-daemon.sh.
#
# Usage:
#   ./sign-macos.sh                    # Sign all artifacts in dist/
#   ./sign-macos.sh --notarize         # Sign and notarize
#   ./sign-macos.sh dist/SotF-macos-arm64-X.Y.Z.dmg  # Sign a specific file
#
# Environment variables:
#   DEVELOPER_ID             - Developer ID Application certificate name
#                              Example: "Developer ID Application: Your Name (TEAMID)"
#   INSTALLER_DEVELOPER_ID   - Developer ID Installer certificate (for .pkg containers)
#   APPLE_ID                 - Apple ID email for notarization
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

NOTARIZE=false
SPECIFIC_FILES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --notarize)
            NOTARIZE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--notarize] [file ...]"
            echo ""
            echo "Signs macOS artifacts (DMG, pkg) in dist/ or specific files."
            echo ""
            echo "Options:"
            echo "  --notarize    Also notarize with Apple (requires APPLE_ID)"
            echo ""
            echo "Environment variables:"
            echo "  DEVELOPER_ID             Developer ID Application certificate"
            echo "  INSTALLER_DEVELOPER_ID   Developer ID Installer certificate (for .pkg)"
            echo "  APPLE_ID                 Apple ID for notarization"
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

# Check prerequisites
check_prerequisites() {
    if [ -z "${DEVELOPER_ID:-}" ]; then
        log_error "DEVELOPER_ID environment variable not set"
        log_info "Set it to your Developer ID certificate name, e.g.:"
        log_info "  export DEVELOPER_ID='Developer ID Application: Your Name (TEAMID)'"
        exit 1
    fi

    if $NOTARIZE && [ -z "${APPLE_ID:-}" ]; then
        log_error "APPLE_ID environment variable not set for notarization"
        exit 1
    fi

    if ! command -v codesign &> /dev/null; then
        log_error "codesign not found (Xcode Command Line Tools required)"
        exit 1
    fi

    if ! /usr/sbin/pkgutil --help &> /dev/null; then
        log_error "pkgutil not found (Xcode Command Line Tools required)"
        exit 1
    fi

    if [ -n "${INSTALLER_DEVELOPER_ID:-}" ] && ! command -v productsign &> /dev/null; then
        log_error "productsign not found (Xcode Command Line Tools required)"
        exit 1
    fi
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

    log_info "  Signing nested Mach-O payloads..."
    while IFS= read -r -d '' candidate; do
        if is_macho_file "$candidate"; then
            sign_macho_file "$candidate"
            log_info "    Signed Mach-O: ${candidate#$root_dir/}"
        fi
    done < <(find "$root_dir" -type f -print0)

    log_info "  Signing code bundles..."
    while IFS= read -r bundle; do
        [ -d "$bundle" ] || continue
        sign_code_bundle "$bundle"
        log_info "    Signed bundle: ${bundle#$root_dir/}"
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

# Sign a DMG file (signs the app bundle inside, then recreates the DMG)
sign_dmg() {
    local dmg_path="$1"
    local dmg_name
    dmg_name=$(basename "$dmg_path")
    log_info "Signing DMG: $dmg_name"

    # Mount the DMG to extract the app bundle
    local mount_point
    mount_point=$(mktemp -d)
    hdiutil attach "$dmg_path" -mountpoint "$mount_point" -nobrowse -quiet

    # Find the .app bundle inside
    local app_bundle=""
    for app in "$mount_point"/*.app; do
        if [ -d "$app" ]; then
            app_bundle="$app"
            break
        fi
    done

    if [ -z "$app_bundle" ]; then
        hdiutil detach "$mount_point" -quiet
        rmdir "$mount_point"
        log_error "No .app bundle found inside DMG"
        exit 1
    fi

    local app_name
    app_name=$(basename "$app_bundle")
    log_info "  Found app bundle: $app_name"

    # Copy the app bundle to a staging area (DMG is read-only)
    local staging_dir
    staging_dir=$(mktemp -d)
    cp -R "$app_bundle" "$staging_dir/"

    # Also copy any other files from the DMG (e.g., Applications symlink)
    for item in "$mount_point"/*; do
        local item_name
        item_name=$(basename "$item")
        if [ "$item_name" != "$app_name" ]; then
            cp -R "$item" "$staging_dir/" 2>/dev/null || true
        fi
    done

    hdiutil detach "$mount_point" -quiet
    rmdir "$mount_point"

    local staged_app="$staging_dir/$app_name"

    sign_payload_code "$staging_dir"

    # Verify the app bundle signature
    codesign --verify --verbose=2 --strict "$staged_app"
    log_success "App bundle signed and verified"

    # Recreate the DMG with the signed app bundle
    rm -f "$dmg_path"
    hdiutil create -volname "${app_name%.app}" \
        -srcfolder "$staging_dir" \
        -ov -format UDZO \
        "$dmg_path"

    # Sign the DMG itself
    codesign --force --sign "$DEVELOPER_ID" --timestamp "$dmg_path"

    rm -rf "$staging_dir"

    codesign --verify --verbose=2 "$dmg_path"
    log_success "DMG signed: $dmg_name"
}

# Sign a pkg file
sign_pkg() {
    local pkg_path="$1"

    if [ -z "${INSTALLER_DEVELOPER_ID:-}" ]; then
        log_warning "INSTALLER_DEVELOPER_ID not set, skipping pkg signing: $(basename "$pkg_path")"
        return
    fi

    log_info "Signing pkg: $(basename "$pkg_path")"
    log_info "  Payload code must be signed before pkgbuild; signing installer container only."
    if ! /usr/sbin/pkgutil --payload-files "$pkg_path" >/dev/null; then
        log_error "Package payload is not readable; rebuild the pkg before signing"
        exit 1
    fi

    # pkg files use productsign, not codesign
    local signed_pkg="${pkg_path%.pkg}-signed.pkg"
    productsign --sign "$INSTALLER_DEVELOPER_ID" --timestamp "$pkg_path" "$signed_pkg"
    mv "$signed_pkg" "$pkg_path"

    log_success "pkg signed: $(basename "$pkg_path")"
}

# Notarize a file
notarize_file() {
    local file_path="$1"

    if ! $NOTARIZE; then
        return
    fi

    log_info "Submitting for notarization: $(basename "$file_path")"

    local submission_output
    submission_output=$(xcrun notarytool submit "$file_path" \
        --apple-id "$APPLE_ID" \
        --keychain-profile "autoeq-notarization" \
        --wait 2>&1)

    echo "$submission_output"

    if echo "$submission_output" | grep -q "status: Accepted"; then
        log_success "Notarization accepted"

        log_info "Stapling notarization ticket..."
        xcrun stapler staple "$file_path"
        xcrun stapler validate "$file_path"
        log_success "Notarization ticket stapled: $(basename "$file_path")"
    else
        log_error "Notarization failed for $(basename "$file_path")"
        local submission_id
        submission_id=$(echo "$submission_output" | grep -o 'id: [a-f0-9-]*' | head -1 | cut -d' ' -f2)
        if [ -n "$submission_id" ]; then
            log_info "To get detailed logs, run:"
            log_info "  xcrun notarytool log $submission_id --apple-id $APPLE_ID --keychain-profile autoeq-notarization"
        fi
        exit 1
    fi
}

# Sign a single file based on extension
sign_file() {
    local file_path="$1"

    if [ ! -f "$file_path" ]; then
        log_warning "File not found: $file_path"
        return
    fi

    case "$file_path" in
        *.dmg)
            sign_dmg "$file_path"
            notarize_file "$file_path"
            ;;
        *.pkg)
            sign_pkg "$file_path"
            notarize_file "$file_path"
            ;;
        *)
            log_warning "Unknown file type, skipping: $(basename "$file_path")"
            ;;
    esac
}

main() {
    log_info "=========================================="
    log_info "macOS Code Signing v${VERSION}"
    log_info "=========================================="

    check_prerequisites

    if [ ${#SPECIFIC_FILES[@]} -gt 0 ]; then
        # Sign specific files
        for f in "${SPECIFIC_FILES[@]}"; do
            sign_file "$f"
        done
    else
        # Sign current-version macOS artifacts in dist/
        local found=false
        for f in "$DIST_DIR"/*"$VERSION"*.dmg "$DIST_DIR"/*"$VERSION"*.pkg; do
            if [ -f "$f" ]; then
                sign_file "$f"
                found=true
            fi
        done

        if ! $found; then
            log_warning "No DMG or pkg files for v${VERSION} found in $DIST_DIR"
            log_info "Run build-dmg-sotf.sh or build-dmg-daemon.sh first"
            exit 1
        fi
    fi

    log_info "=========================================="
    log_success "Signing complete!"
    log_info "=========================================="
}

main "$@"
