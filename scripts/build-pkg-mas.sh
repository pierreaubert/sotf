#!/bin/bash
#
# Build a Mac App Store .pkg for SotF.
#
# This is the App-Store-only path. For the direct-distribution DMG (signed
# with Developer ID + notarized), use scripts/build-dmg-sotf.sh instead.
#
# Required prerequisites (one-time, manual):
#   1. Apple Distribution cert in keychain
#        e.g. "Apple Distribution: Pierre Aubert (RTH7ZJXLT6)"
#   2. 3rd Party Mac Developer Installer cert in keychain
#        e.g. "3rd Party Mac Developer Installer: Pierre Aubert (RTH7ZJXLT6)"
#   3. Mac App Store Distribution provisioning profile downloaded from
#      https://developer.apple.com/account/resources/profiles  bound to
#      bundle id `org.spinorama.sotf` and the Apple Distribution cert.
#      Save it at builds/macos/sotf-mas.provisionprofile (or set
#      MAS_PROVISIONING_PROFILE in ~/.sotf-release.conf).
#   4. App record created in App Store Connect for `org.spinorama.sotf`.
#
# Usage:
#   ./scripts/build-pkg-mas.sh                         # arm64, build number = git commit count
#   ./scripts/build-pkg-mas.sh --binary <path>
#   ./scripts/build-pkg-mas.sh --arch x86_64 --binary <path>
#   ./scripts/build-pkg-mas.sh --build-number 17       # manual override
#
# Output:
#   dist/sotf-desktop-<version>-macos-<arch>-mas.pkg
#

set -euo pipefail

APP_NAME="sotf-desktop"
BUNDLE_ID="org.spinorama.sotf"
BINARY_NAME="sotf-desktop"
EXTERNAL_PLUGIN_WORKER_NAME="sotf-external-plugin-worker"
MACOS_SANDBOX_HELPER_NAME="sotf-macos-sandbox-helper"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---- Version (from workspace Cargo.toml) ----
VERSION=$(grep -m1 '^version = ' "$PROJECT_ROOT/Cargo.toml" \
    | sed 's/version = "\(.*\)"/\1/')
[ -n "$VERSION" ] || { echo "ERROR: Could not extract version from Cargo.toml" >&2; exit 1; }

# ---- Defaults ----
ARCH="arm64"
SOURCE_BINARY=""
BUILD_NUMBER=""

# ---- Load config (cert names, profile path, optional overrides) ----
CONFIG_FILE="${HOME}/.sotf-release.conf"
# shellcheck source=/dev/null
[ -f "$CONFIG_FILE" ] && source "$CONFIG_FILE"

ENTITLEMENTS="$PROJECT_ROOT/builds/macos/entitlements-mas.plist"
INHERIT_ENTITLEMENTS="$PROJECT_ROOT/builds/macos/entitlements-mas-inherit.plist"
INFO_PLIST_TEMPLATE="$PROJECT_ROOT/builds/macos/org.spinorama.sotf.plist"
PB=/usr/libexec/PlistBuddy

MAS_DISTRIBUTION_CERT="${MAS_DISTRIBUTION_CERT:-Apple Distribution: Pierre Aubert (RTH7ZJXLT6)}"
MAS_INSTALLER_CERT="${MAS_INSTALLER_CERT:-3rd Party Mac Developer Installer: Pierre Aubert (RTH7ZJXLT6)}"
MAS_PROVISIONING_PROFILE="${MAS_PROVISIONING_PROFILE:-$PROJECT_ROOT/builds/macos/sotf-mas.provisionprofile}"
BUILD_NUMBER_SOURCE="git commit count"
[ -n "${BUILD_NUMBER:-}" ] && BUILD_NUMBER_SOURCE="configuration"

# ---- Logging ----
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'
log_info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
log_step()  { echo -e "${BOLD}→${NC} $1"; }
log_ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

usage() {
    sed -n '2,/^$/p' "$0" | sed 's/^#//' | sed 's/^ //'
}

derive_build_number_from_git() {
    local count
    count=$(git -C "$PROJECT_ROOT" rev-list --count HEAD 2>/dev/null) || return 1
    case "$count" in
        ''|*[!0-9]*) return 1 ;;
    esac
    printf '%s' "$count"
}

# ---- Args ----
while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)        SOURCE_BINARY="$2"; shift 2 ;;
        --arch)          ARCH="$2"; shift 2 ;;
        --build-number)  BUILD_NUMBER="$2"; BUILD_NUMBER_SOURCE="command line"; shift 2 ;;
        --help|-h)       usage; exit 0 ;;
        *) log_error "Unknown option: $1"; usage; exit 1 ;;
    esac
done

if [ -z "${BUILD_NUMBER:-}" ]; then
    if ! BUILD_NUMBER=$(derive_build_number_from_git); then
        log_error "Could not derive BUILD_NUMBER from git commit count."
        log_error "Run from a git checkout, or pass --build-number <integer>."
        exit 1
    fi
fi
case "$BUILD_NUMBER" in
    ''|*[!0-9]*)
        log_error "Build number must be a positive integer, got '$BUILD_NUMBER'"
        exit 1
        ;;
esac
if [ "$BUILD_NUMBER" -le 0 ]; then
    log_error "Build number must be greater than zero, got '$BUILD_NUMBER'"
    exit 1
fi
if [ "$BUILD_NUMBER_SOURCE" = "git commit count" ] \
    && [ "$(git -C "$PROJECT_ROOT" rev-parse --is-shallow-repository 2>/dev/null)" = "true" ]; then
    log_warn "Repository is shallow; build number $BUILD_NUMBER may be lower than the full-history commit count."
    log_warn "Use --build-number <integer> if App Store Connect needs a higher value."
fi

# Default binary paths follow --arch.
case "$ARCH" in
    arm64)  TARGET_TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TARGET_TRIPLE="x86_64-apple-darwin" ;;
    *) log_error "Unsupported --arch: $ARCH (expected arm64 or x86_64)"; exit 1 ;;
esac
DEFAULT_BINARY_DIR="$PROJECT_ROOT/target/$TARGET_TRIPLE/release"
if [ -z "$SOURCE_BINARY" ]; then
    SOURCE_BINARY="$DEFAULT_BINARY_DIR/$BINARY_NAME"
fi
EXTERNAL_PLUGIN_WORKER_BINARY="${EXTERNAL_PLUGIN_WORKER_BINARY:-$DEFAULT_BINARY_DIR/$EXTERNAL_PLUGIN_WORKER_NAME}"
MACOS_SANDBOX_HELPER_BINARY="${MACOS_SANDBOX_HELPER_BINARY:-$DEFAULT_BINARY_DIR/$MACOS_SANDBOX_HELPER_NAME}"

# ---- Preflight ----
log_step "Preflight checks"
[ -f "$SOURCE_BINARY" ]    || { log_error "App binary not found: $SOURCE_BINARY"; exit 1; }
[ -f "$EXTERNAL_PLUGIN_WORKER_BINARY" ] || { log_error "External plugin worker not found: $EXTERNAL_PLUGIN_WORKER_BINARY"; exit 1; }
[ -f "$MACOS_SANDBOX_HELPER_BINARY" ] || { log_error "macOS sandbox helper not found: $MACOS_SANDBOX_HELPER_BINARY"; exit 1; }
[ -f "$ENTITLEMENTS" ]     || { log_error "App entitlements not found: $ENTITLEMENTS"; exit 1; }
[ -f "$INHERIT_ENTITLEMENTS" ] || { log_error "Inherited sandbox entitlements not found: $INHERIT_ENTITLEMENTS"; exit 1; }
[ -f "$INFO_PLIST_TEMPLATE" ] || { log_error "Info.plist template not found: $INFO_PLIST_TEMPLATE"; exit 1; }

# Private-API scan: aborts the build if the binary references SPI symbols
# (e.g. CGSSetWindowBackgroundBlurRadius) or links non-public frameworks.
# Catches MAS-rejection-class issues before we sign + upload.
log_step "Scanning binaries for private Apple APIs..."
for api_binary in "$SOURCE_BINARY" "$EXTERNAL_PLUGIN_WORKER_BINARY" "$MACOS_SANDBOX_HELPER_BINARY"; do
    if ! "$SCRIPT_DIR/check-mas-private-api.sh" "$api_binary"; then
        log_error "Refusing to build a .pkg that would be rejected by App Review."
        log_error "Address the findings above (vendor-and-patch the offending"
        log_error "dependency, or extend the ALLOWLIST if the symbol is in fact public)."
        exit 1
    fi
done
if [ ! -f "$MAS_PROVISIONING_PROFILE" ]; then
    log_error "MAS provisioning profile not found: $MAS_PROVISIONING_PROFILE"
    log_error "Download a 'Mac App Store Distribution' profile for $BUNDLE_ID from"
    log_error "  https://developer.apple.com/account/resources/profiles"
    log_error "and place it at the path above (or set MAS_PROVISIONING_PROFILE in $CONFIG_FILE)."
    exit 1
fi

# Cert lookups: codesign certs show in -p codesigning, installer cert does not.
if ! security find-identity -v -p codesigning 2>/dev/null | grep -qF "$MAS_DISTRIBUTION_CERT"; then
    log_error "Code-signing identity not found in keychain: $MAS_DISTRIBUTION_CERT"
    log_error "Run: security find-identity -v -p codesigning"
    exit 1
fi
if ! security find-identity -v 2>/dev/null | grep -qF "$MAS_INSTALLER_CERT"; then
    log_error "Installer identity not found in keychain: $MAS_INSTALLER_CERT"
    log_error "Run: security find-identity -v"
    exit 1
fi

# Cross-check the provisioning profile actually lists the distribution cert
# we're about to sign with. App Store Connect rejects uploads with the cryptic
# "Invalid Provisioning Profile. Missing code-signing certificate." otherwise,
# and the typical cause is picking the wrong cert when generating the profile
# in developer.apple.com (e.g. picking "Developer ID Application" by mistake
# for a "Mac App Store Distribution" profile type).
log_step "Cross-checking provisioning profile vs distribution cert"
PROFILE_PLIST=$(mktemp)
trap 'rm -f "$PROFILE_PLIST"' EXIT
security cms -D -i "$MAS_PROVISIONING_PROFILE" 2>/dev/null > "$PROFILE_PLIST" || {
    log_error "Could not parse provisioning profile (CMS-decode failed): $MAS_PROVISIONING_PROFILE"
    exit 1
}

DIST_CERT_SHA1=$(security find-certificate -c "$MAS_DISTRIBUTION_CERT" -p 2>/dev/null \
    | openssl x509 -noout -fingerprint -sha1 2>/dev/null \
    | sed 's/^.*=//; s/://g' | tr 'a-f' 'A-F')
[ -n "$DIST_CERT_SHA1" ] || {
    log_error "Could not compute SHA1 of '$MAS_DISTRIBUTION_CERT' from keychain"
    exit 1
}

PROFILE_CERT_COUNT=$(plutil -extract DeveloperCertificates raw -o - "$PROFILE_PLIST" 2>/dev/null || echo 0)
PROFILE_CERT_SHA1S=""
match_found=false
for ((i = 0; i < PROFILE_CERT_COUNT; i++)); do
    CERT_TMP=$(mktemp)
    plutil -extract "DeveloperCertificates.$i" raw -o - "$PROFILE_PLIST" 2>/dev/null \
        | base64 -D > "$CERT_TMP"
    sha1=$(openssl x509 -in "$CERT_TMP" -inform DER -noout -fingerprint -sha1 2>/dev/null \
        | sed 's/^.*=//; s/://g' | tr 'a-f' 'A-F')
    rm -f "$CERT_TMP"
    PROFILE_CERT_SHA1S="${PROFILE_CERT_SHA1S}\n    $sha1"
    [ "$sha1" = "$DIST_CERT_SHA1" ] && match_found=true
done

if ! $match_found; then
    log_error "Provisioning profile does NOT list the distribution cert."
    log_error "  Distribution cert SHA1 (from keychain): $DIST_CERT_SHA1"
    log_error "  Profile DeveloperCertificates SHA1s:$(printf '%b' "$PROFILE_CERT_SHA1S")"
    log_error ""
    log_error "Fix: regenerate the profile at https://developer.apple.com/account/resources/profiles"
    log_error "  - Pick certificate '$MAS_DISTRIBUTION_CERT'"
    log_error "  - NOT 'Developer ID Application: ...' (that's for the DMG/notarized path)"
    log_error "  - Save, download, replace $MAS_PROVISIONING_PROFILE, retry."
    exit 1
fi

PROFILE_APP_ID=$("$PB" -c "Print :Entitlements:application-identifier" "$PROFILE_PLIST" 2>/dev/null || true)
if [ -z "$PROFILE_APP_ID" ]; then
    PROFILE_APP_ID=$("$PB" -c "Print :Entitlements:com.apple.application-identifier" "$PROFILE_PLIST" 2>/dev/null || true)
fi
if [ -z "$PROFILE_APP_ID" ]; then
    log_error "Provisioning profile does not contain an application identifier entitlement."
    log_error "Regenerate the Mac App Store Distribution profile for explicit App ID $BUNDLE_ID."
    exit 1
fi
PROFILE_BUNDLE_ID="${PROFILE_APP_ID#*.}"
if [ "$PROFILE_BUNDLE_ID" = "$PROFILE_APP_ID" ] || [ -z "$PROFILE_BUNDLE_ID" ]; then
    log_error "Provisioning profile application identifier is malformed: $PROFILE_APP_ID"
    exit 1
fi
if [[ "$PROFILE_APP_ID" == *"*"* ]]; then
    log_error "Provisioning profile uses wildcard application identifier: $PROFILE_APP_ID"
    log_error "TestFlight needs an explicit App ID for $BUNDLE_ID."
    exit 1
fi
if [ "$PROFILE_BUNDLE_ID" != "$BUNDLE_ID" ]; then
    log_error "Provisioning profile App ID does not match bundle id."
    log_error "  Profile application identifier: $PROFILE_APP_ID"
    log_error "  Expected bundle id suffix: $BUNDLE_ID"
    exit 1
fi

log_ok "Version v$VERSION (build #$BUILD_NUMBER, from $BUILD_NUMBER_SOURCE)"
log_ok "Binary: $SOURCE_BINARY"
log_ok "External plugin worker: $EXTERNAL_PLUGIN_WORKER_BINARY"
log_ok "macOS sandbox helper: $MACOS_SANDBOX_HELPER_BINARY"
log_ok "Entitlements: $ENTITLEMENTS"
log_ok "Inherited sandbox entitlements: $INHERIT_ENTITLEMENTS"
log_ok "Provisioning profile: $MAS_PROVISIONING_PROFILE"
log_ok "Profile lists distribution cert ($DIST_CERT_SHA1)"
log_ok "Profile application identifier: $PROFILE_APP_ID"

# ---- Build paths ----
BUILD_DIR=$(mktemp -d -t sotf-mas-build.XXXXXX)
trap 'rm -rf "$BUILD_DIR"' EXIT
APP_BUNDLE="$BUILD_DIR/$APP_NAME.app"
DIST_DIR="$PROJECT_ROOT/dist"
PKG_FILENAME="$APP_NAME-${VERSION}-macos-${ARCH}-mas.pkg"
PKG_PATH="$DIST_DIR/$PKG_FILENAME"
mkdir -p "$DIST_DIR"

# ---- Bundle layout ----
log_step "Creating .app bundle at $APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"
cp "$SOURCE_BINARY" "$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"
chmod +x "$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"
cp "$EXTERNAL_PLUGIN_WORKER_BINARY" "$APP_BUNDLE/Contents/MacOS/$EXTERNAL_PLUGIN_WORKER_NAME"
chmod +x "$APP_BUNDLE/Contents/MacOS/$EXTERNAL_PLUGIN_WORKER_NAME"
cp "$MACOS_SANDBOX_HELPER_BINARY" "$APP_BUNDLE/Contents/MacOS/$MACOS_SANDBOX_HELPER_NAME"
chmod +x "$APP_BUNDLE/Contents/MacOS/$MACOS_SANDBOX_HELPER_NAME"
echo -n "APPL????" > "$APP_BUNDLE/Contents/PkgInfo"

# ---- Info.plist (template + MAS-required keys) ----
log_step "Writing Info.plist"
cp "$INFO_PLIST_TEMPLATE" "$APP_BUNDLE/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" \
    "$APP_BUNDLE/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$BUILD_NUMBER" \
    "$APP_BUNDLE/Contents/Info.plist"
# App Store requires an export-compliance answer. SotF only uses Apple-provided
# HTTPS via system networking, which is exempt under standard rules.
plutil -replace ITSAppUsesNonExemptEncryption -bool NO \
    "$APP_BUNDLE/Contents/Info.plist"
plutil -lint "$APP_BUNDLE/Contents/Info.plist"

# ---- Icon ----
# App Review rejects MAS submissions whose .icns is missing the
# 512pt @2x (1024×1024 px) representation. iconutil only includes a
# variant when the iconset uses the exact `icon_<pt>x<pt>[@2x].png`
# filename convention — naming by physical pixel size silently produces
# an .icns that's missing the slot Apple checks for.
log_step "Adding app icon"
ICNS_DEST="$APP_BUNDLE/Contents/Resources/AppIcon.icns"
ICON_SOURCE=""
if [ -f "$PROJECT_ROOT/crates/app-gpui/assets/icon.icns" ]; then
    cp "$PROJECT_ROOT/crates/app-gpui/assets/icon.icns" "$ICNS_DEST"
elif [ -f "$PROJECT_ROOT/crates/app-gpui/assets/sotf.png" ]; then
    ICON_SOURCE="$PROJECT_ROOT/crates/app-gpui/assets/sotf.png"
elif [ -f "$PROJECT_ROOT/crates/app-gpui/assets/sotf.jpg" ]; then
    ICON_SOURCE="$PROJECT_ROOT/crates/app-gpui/assets/sotf.jpg"
else
    log_error "No app icon source found (assets/icon.icns, assets/sotf.png, or assets/sotf.jpg)"
    log_error "App Review will reject the bundle without a real .icns."
    exit 1
fi

if [ -n "$ICON_SOURCE" ]; then
    log_info "Generating .icns from $ICON_SOURCE"
    ICON_TMP=$(mktemp -d)
    ICONSET="$ICON_TMP/AppIcon.iconset"
    mkdir -p "$ICONSET"
    # Each row: <physical-px> <iconset-filename>. Apple's required set:
    #   16pt   = 16  / 32     (16x16, 16x16@2x)
    #   32pt   = 32  / 64     (32x32, 32x32@2x)
    #   128pt  = 128 / 256
    #   256pt  = 256 / 512
    #   512pt  = 512 / 1024   ← the 1024 entry is what App Review checks
    while IFS=' ' read -r px name; do
        [ -z "$px" ] && continue
        sips -s format png -z "$px" "$px" "$ICON_SOURCE" \
            --out "$ICONSET/$name" >/dev/null
    done <<EOF
16 icon_16x16.png
32 icon_16x16@2x.png
32 icon_32x32.png
64 icon_32x32@2x.png
128 icon_128x128.png
256 icon_128x128@2x.png
256 icon_256x256.png
512 icon_256x256@2x.png
512 icon_512x512.png
1024 icon_512x512@2x.png
EOF
    # Sanity: confirm the slot Apple complained about actually exists.
    [ -f "$ICONSET/icon_512x512@2x.png" ] || {
        log_error "Failed to generate icon_512x512@2x.png in iconset"
        rm -rf "$ICON_TMP"; exit 1
    }
    iconutil -c icns -o "$ICNS_DEST" "$ICONSET"
    rm -rf "$ICON_TMP"
fi

# Final assertion: read the .icns back and confirm it carries an entry
# whose pixel dimensions are 1024×1024 (the 512pt @2x representation).
if ! sips -g pixelHeight -g pixelWidth "$ICNS_DEST" 2>/dev/null | grep -q "1024"; then
    log_warn "Generated .icns may not include the 1024×1024 / 512pt @2x slot"
    log_warn "App Store validation will reject the upload. Check $ICNS_DEST."
fi

# ---- Embedded provisioning profile (MAS requirement) ----
log_step "Embedding provisioning profile"
cp "$MAS_PROVISIONING_PROFILE" "$APP_BUNDLE/Contents/embedded.provisionprofile"

# ---- Strip macOS quarantine / Finder xattrs ----
# Apple rejects bundles whose contents carry com.apple.quarantine,
# com.apple.metadata:kMDItemWhereFroms, com.apple.macl, etc. (error 91109).
# These attach automatically to anything downloaded via a browser — most
# commonly the .provisionprofile dragged out of developer.apple.com — and
# survive `cp`. Clear all xattrs recursively before signing.
log_step "Stripping extended attributes from bundle"
xattr -cr "$APP_BUNDLE"

# ---- Patch entitlements with MAS identity ----
# TestFlight requires the signed app entitlements to match the embedded
# provisioning profile. macOS uses `com.apple.application-identifier` in the
# signed entitlements, while profiles commonly expose the same value under
# `Entitlements.application-identifier`.
TEAM_ID=$(printf '%s' "$MAS_DISTRIBUTION_CERT" | sed -E 's/.*\(([A-Z0-9]+)\)[^)]*$/\1/')
if ! [[ "$TEAM_ID" =~ ^[A-Z0-9]{10}$ ]]; then
    log_error "Could not extract 10-char Apple Team ID from MAS_DISTRIBUTION_CERT='$MAS_DISTRIBUTION_CERT'"
    exit 1
fi
SIGN_ENTITLEMENTS=$(mktemp -t sotf-mas-ent.XXXXXX.plist)
cp "$ENTITLEMENTS" "$SIGN_ENTITLEMENTS"
# Use PlistBuddy rather than `plutil -replace`: plutil interprets dots in
# the key path as nested-dict navigation, so a literal top-level key like
# `com.apple.developer.team-identifier` is silently rejected.
"$PB" -c "Set :com.apple.developer.team-identifier $TEAM_ID" "$SIGN_ENTITLEMENTS" 2>/dev/null \
    || "$PB" -c "Add :com.apple.developer.team-identifier string $TEAM_ID" "$SIGN_ENTITLEMENTS"
"$PB" -c "Set :com.apple.application-identifier $PROFILE_APP_ID" "$SIGN_ENTITLEMENTS" 2>/dev/null \
    || "$PB" -c "Add :com.apple.application-identifier string $PROFILE_APP_ID" "$SIGN_ENTITLEMENTS"
plutil -lint "$SIGN_ENTITLEMENTS" >/dev/null

# Pre-sign sanity: confirm identity entitlements actually landed.
if ! "$PB" -c "Print :com.apple.developer.team-identifier" "$SIGN_ENTITLEMENTS" >/dev/null 2>&1; then
    log_error "Entitlements patch failed: com.apple.developer.team-identifier not found"
    cat "$SIGN_ENTITLEMENTS" >&2
    exit 1
fi
if [ "$("$PB" -c "Print :com.apple.application-identifier" "$SIGN_ENTITLEMENTS" 2>/dev/null)" != "$PROFILE_APP_ID" ]; then
    log_error "Entitlements patch failed: com.apple.application-identifier does not match $PROFILE_APP_ID"
    cat "$SIGN_ENTITLEMENTS" >&2
    exit 1
fi
# Belt-and-braces: ensure no stale iOS/Catalyst key slipped in.
if "$PB" -c "Print :application-identifier" "$SIGN_ENTITLEMENTS" >/dev/null 2>&1; then
    log_error "Refusing to sign: application-identifier present in entitlements."
    log_error "Native macOS should use com.apple.application-identifier. Remove the iOS key from $ENTITLEMENTS."
    exit 1
fi
log_info "Signing with team-identifier = $TEAM_ID and application-identifier = $PROFILE_APP_ID"

# ---- Code-sign nested command-line tools first ----
log_step "Code-signing inherited sandbox helper tools"
for helper_binary in "$APP_BUNDLE/Contents/MacOS/$EXTERNAL_PLUGIN_WORKER_NAME" \
                     "$APP_BUNDLE/Contents/MacOS/$MACOS_SANDBOX_HELPER_NAME"; do
    codesign --force \
        --options runtime \
        --timestamp \
        --entitlements "$INHERIT_ENTITLEMENTS" \
        --sign "$MAS_DISTRIBUTION_CERT" \
        "$helper_binary"
    codesign --verify --strict --verbose=2 "$helper_binary"
    HELPER_ENT=$(codesign -d --entitlements :- "$helper_binary" 2>/dev/null)
    if ! printf '%s\n' "$HELPER_ENT" | grep -Fq "<key>com.apple.security.inherit</key>"; then
        log_error "Signed helper missing com.apple.security.inherit: $helper_binary"
        exit 1
    fi
done

# ---- Code-sign with Apple Distribution cert + patched MAS entitlements ----
log_step "Code-signing .app with $MAS_DISTRIBUTION_CERT"
codesign --force \
    --options runtime \
    --timestamp \
    --entitlements "$SIGN_ENTITLEMENTS" \
    --sign "$MAS_DISTRIBUTION_CERT" \
    "$APP_BUNDLE"
rm -f "$SIGN_ENTITLEMENTS"

log_step "Verifying .app signature"
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
codesign -dv --entitlements - "$APP_BUNDLE" 2>&1 | sed 's/^/    /' | head -40

# Post-sign sanity: confirm the macOS application identifier and team
# identifier match the embedded provisioning profile.
SIGNED_ENT=$(codesign -d --entitlements :- "$APP_BUNDLE" 2>/dev/null)
if ! printf '%s\n' "$SIGNED_ENT" | grep -Fq "<key>com.apple.developer.team-identifier</key>"; then
    log_error "Signed bundle missing com.apple.developer.team-identifier."
    log_error "Check that PlistBuddy patch above succeeded."
    exit 1
fi
if ! printf '%s\n' "$SIGNED_ENT" | grep -Fq "<key>com.apple.application-identifier</key>"; then
    log_error "Signed bundle missing com.apple.application-identifier."
    log_error "TestFlight rejects bundles whose signed App ID does not match the profile."
    exit 1
fi
if ! printf '%s\n' "$SIGNED_ENT" | grep -Fq "<string>$PROFILE_APP_ID</string>"; then
    log_error "Signed bundle application identifier does not match profile: $PROFILE_APP_ID"
    exit 1
fi
if printf '%s\n' "$SIGNED_ENT" | grep -Fq "<key>application-identifier</key>"; then
    log_error "Signed bundle ended up with iOS application-identifier key."
    log_error "Native macOS apps should carry com.apple.application-identifier instead."
    exit 1
fi

# Final post-sign sanity: re-confirm zero quarantine xattrs survived signing
# (codesign sometimes adds new xattrs of its own — _CodeSignature ones are
# fine, com.apple.quarantine is not).
if xattr -lr "$APP_BUNDLE" 2>/dev/null | grep -q "com.apple.quarantine"; then
    log_error "com.apple.quarantine xattr survived signing — App Store will reject (91109)."
    xattr -lr "$APP_BUNDLE" | grep "com.apple.quarantine" | head -5 >&2
    exit 1
fi

# ---- Build .pkg with 3rd Party Mac Developer Installer cert ----
log_step "Building .pkg with $MAS_INSTALLER_CERT"
productbuild \
    --component "$APP_BUNDLE" /Applications \
    --sign "$MAS_INSTALLER_CERT" \
    "$PKG_PATH"

log_step "Verifying .pkg signature"
pkgutil --check-signature "$PKG_PATH" | sed 's/^/    /'

log_ok "Mac App Store .pkg built: $PKG_PATH"
echo
echo "Next steps:"
echo "  1. Validate before upload (catches obvious rejections):"
echo "       xcrun altool --validate-app -f \"$PKG_PATH\" -t macos \\"
echo "         -u <APPLE_ID> -p <APP_SPECIFIC_PASSWORD>"
echo "  2. Upload to App Store Connect:"
echo "       xcrun altool --upload-app -f \"$PKG_PATH\" -t macos \\"
echo "         -u <APPLE_ID> -p <APP_SPECIFIC_PASSWORD>"
echo "     OR drop the .pkg into Transporter.app and click Deliver."
echo "  3. In App Store Connect, attach the build to a version and submit"
echo "     for App Review."
