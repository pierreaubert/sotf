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
#   ./build-msix.sh --arch x86_64            # Specify architecture (x86_64 or arm64)
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

# When invoked over SSH on Windows (Git Bash), tools installed via MSYS2's
# pacman live under /c/msys64/usr/bin and are NOT on Git Bash's default PATH.
# Prepend known MSYS2 / Cygwin / Git-for-Windows-SDK bin dirs so
# `command -v zip` etc. succeed.
for msys_bin in \
    /c/msys64/usr/bin \
    /c/msys64/mingw64/bin \
    /c/msys64/ucrt64/bin \
    /c/cygwin64/bin \
    /c/git-sdk-64/usr/bin \
    /c/git-sdk-64/mingw64/bin \
    "/c/Program Files/Git/usr/bin"; do
    if [ -d "$msys_bin" ] && [[ ":$PATH:" != *":$msys_bin:"* ]]; then
        PATH="$msys_bin:$PATH"
    fi
done
export PATH

# Locate MakeAppx.exe on a Windows machine — it lives under the Windows SDK
# tree at ".../Windows Kits/10/bin/<sdkver>/<arch>/makeappx.exe". We try (in
# order): $MAKEAPPX env override, anything on PATH, then the latest sub-version
# under the standard Win10 SDK install. Echoes the path on stdout (empty if
# nothing was found).
find_makeappx() {
    if [ -n "${MAKEAPPX:-}" ] && [ -x "$MAKEAPPX" ]; then
        printf '%s\n' "$MAKEAPPX"; return 0
    fi
    if command -v makeappx.exe &> /dev/null; then
        command -v makeappx.exe; return 0
    fi
    if command -v MakeAppx.exe &> /dev/null; then
        command -v MakeAppx.exe; return 0
    fi
    local sdk_root
    for sdk_root in \
        "/c/Program Files (x86)/Windows Kits/10/bin" \
        "/c/Program Files/Windows Kits/10/bin"; do
        if [ -d "$sdk_root" ]; then
            # Pick the highest sdk version dir with a working x64/makeappx.exe.
            local candidate
            candidate=$(find "$sdk_root" -maxdepth 3 -type f -iname 'makeappx.exe' \
                -path '*/x64/*' 2>/dev/null | sort -V | tail -1)
            if [ -n "$candidate" ] && [ -x "$candidate" ]; then
                printf '%s\n' "$candidate"; return 0
            fi
        fi
    done
    return 1
}

# Pre-flight check on the rendered AppxManifest.xml. Catches the common
# breakages that otherwise only surface inside MakeAppx (slow round-trip when
# the manifest sits on a remote Windows machine):
#
#   1. XML well-formedness (xmllint --noout).
#   2. Capabilities child ordering — the schema for <Capabilities> is an
#      xs:sequence, NOT xs:choice, so the children must appear in this order:
#          <Capability>             (foundation)
#          <uap*:Capability>        (uap, uap2, uap3, …)
#          <DeviceCapability>       (foundation)
#          <rescap*:Capability>     (restricted)
#      MakeAppx fails with "Element ... is unexpected ... Expecting:
#      DeviceCapability" when this is wrong.
#   3. Sanity checks on the elements we know break things if missing/wrong
#      (Identity Version is 4-part, ProcessorArchitecture is one of the
#      accepted values, runFullTrust is declared when any Application uses
#      Windows.FullTrustApplication).
#
# Returns 0 on success, non-zero on the first failure (logged).
validate_appx_manifest() {
    local manifest="$1"
    local issues=0

    if [ ! -f "$manifest" ]; then
        log_error "validate_appx_manifest: no such file: $manifest"
        return 1
    fi

    # 1. XML well-formedness
    if command -v xmllint &> /dev/null; then
        if ! xmllint --noout "$manifest" 2>&1; then
            log_error "AppxManifest.xml is not well-formed XML"
            return 1
        fi
        log_info "AppxManifest.xml: well-formed XML ✓"
    else
        log_warning "xmllint not found; skipping XML well-formedness check"
        log_info "  Install with: $(install_hint libxml2)"
    fi

    # 2. Capabilities child ordering. Strip XML comments first so that the
    # example elements in the schema-documentation comment don't show up as
    # real entries.
    local cap_order
    cap_order=$(awk '
        BEGIN { in_c = 0 }
        {
            s = $0
            while (1) {
                if (in_c) {
                    e = index(s, "-->")
                    if (e == 0) { s = ""; break }
                    s = substr(s, e + 3); in_c = 0
                }
                b = index(s, "<!--")
                if (b == 0) break
                rest = substr(s, b + 4)
                e = index(rest, "-->")
                if (e == 0) { s = substr(s, 1, b - 1); in_c = 1; break }
                s = substr(s, 1, b - 1) substr(rest, e + 3)
            }
            print s
        }' "$manifest" \
        | awk '/<Capabilities>/{f=1;next} /<\/Capabilities>/{f=0} f' \
        | grep -oE '<[A-Za-z0-9]+:?[A-Za-z0-9]*Capability\b' \
        | sed 's/^<//')
    local last_class=0 last_elem=""
    while IFS= read -r elem; do
        [ -n "$elem" ] || continue
        local class
        case "$elem" in
            Capability)            class=1 ;;
            uap*:Capability)       class=2 ;;
            DeviceCapability)      class=3 ;;
            rescap*:Capability)    class=4 ;;
            *)                     class=99 ;;
        esac
        if [ "$class" -lt "$last_class" ]; then
            log_error "AppxManifest.xml: <Capabilities> children out of order"
            log_error "  Found <$elem> after <$last_elem>"
            log_error "  Required order (xs:sequence): Capability → uap*:Capability → DeviceCapability → rescap*:Capability"
            issues=$((issues + 1))
        fi
        last_class="$class"
        last_elem="$elem"
    done <<< "$cap_order"
    if [ "$issues" -eq 0 ]; then
        log_info "AppxManifest.xml: <Capabilities> ordering ✓"
    fi

    # 3a. Identity Version must be a 4-part dotted number (M.m.b.r)
    local version_attr
    version_attr=$(grep -oE '[[:space:]]Version="[^"]+"' "$manifest" | head -1 \
        | sed 's/.*Version="\([^"]*\)".*/\1/')
    if [ -z "$version_attr" ]; then
        log_error "AppxManifest.xml: Identity has no Version attribute"
        issues=$((issues + 1))
    elif ! echo "$version_attr" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$'; then
        log_error "AppxManifest.xml: Identity Version must be Major.Minor.Build.Revision (got '$version_attr')"
        issues=$((issues + 1))
    else
        log_info "AppxManifest.xml: Identity Version=$version_attr ✓"
    fi

    # 3b. ProcessorArchitecture must be one of x64 | x86 | arm64 | arm | neutral
    local proc_arch
    proc_arch=$(grep -oE 'ProcessorArchitecture="[^"]+"' "$manifest" | head -1 \
        | sed 's/.*="\([^"]*\)".*/\1/')
    case "$proc_arch" in
        x64|x86|arm64|arm|neutral)
            log_info "AppxManifest.xml: ProcessorArchitecture=$proc_arch ✓"
            ;;
        "")
            log_error "AppxManifest.xml: Identity has no ProcessorArchitecture"
            issues=$((issues + 1))
            ;;
        *)
            log_error "AppxManifest.xml: ProcessorArchitecture must be x64|x86|arm64|arm|neutral (got '$proc_arch')"
            issues=$((issues + 1))
            ;;
    esac

    # 3c. runFullTrust required when any Application uses
    # EntryPoint="Windows.FullTrustApplication"
    if grep -qE 'EntryPoint="Windows\.FullTrustApplication"' "$manifest" \
       && ! grep -qE '<rescap:Capability[[:space:]]+Name="runFullTrust"' "$manifest"; then
        log_error "AppxManifest.xml: an Application uses EntryPoint=\"Windows.FullTrustApplication\""
        log_error "  but the package does not declare <rescap:Capability Name=\"runFullTrust\"/>"
        issues=$((issues + 1))
    fi

    if [ "$issues" -gt 0 ]; then
        log_error "AppxManifest.xml: $issues validation issue(s) — see above"
        return 1
    fi
    return 0
}

# Suggest the right install command for whichever environment we're running in.
install_hint() {
    local pkg="$1"
    if command -v pacman &> /dev/null; then
        echo "pacman -S $pkg"
    elif command -v apt &> /dev/null; then
        echo "apt install $pkg"
    elif command -v dnf &> /dev/null; then
        echo "dnf install $pkg"
    elif command -v brew &> /dev/null; then
        echo "brew install $pkg"
    else
        echo "(install $pkg via your OS package manager)"
    fi
}

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

# Defaults — ARCH is the dist filename suffix (matches the
# `name-version-os-arch.format` convention); MSIX_ARCH is the value the
# AppxManifest needs for ProcessorArchitecture.
ARCH="x86_64"
MSIX_ARCH="x64"
BUILD_DIR=""
DIST_DIR="$PROJECT_ROOT/dist"
SIGN=false
TIMESTAMP_URL="${WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}"

# Map an input arch label (the dist filename convention OR Microsoft's manifest
# convention) to (ARCH, MSIX_ARCH).
set_arch() {
    case "$1" in
        x86_64|x64)
            ARCH="x86_64"
            MSIX_ARCH="x64"
            ;;
        arm64|aarch64)
            ARCH="arm64"
            MSIX_ARCH="arm64"
            ;;
        *)
            echo "ERROR: --arch must be x86_64, x64, arm64 or aarch64; got '$1'"
            exit 1
            ;;
    esac
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build-dir)
            BUILD_DIR="$2"
            shift 2
            ;;
        --arch)
            set_arch "$2"
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
            echo "  --arch <x86_64|arm64>  Target architecture (default: x86_64;"
            echo "                       'x64' is accepted as an alias for x86_64)"
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

# If no build dir specified, try to find binaries. Release cuts now build
# under the `dist` profile (target/<triple>/dist/). The `release` paths
# remain as a fallback so manual `cargo build --release` invocations from a
# dev shell still package — feel free to drop the legacy paths once the dist
# profile is the only thing anyone uses.
if [ -z "$BUILD_DIR" ]; then
    TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
    for triple in x86_64-pc-windows-gnullvm x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm aarch64-pc-windows-gnu; do
        for profile in dist release; do
            candidate="$TARGET_DIR/$triple/$profile"
            if [ -f "$candidate/sotf-tui.exe" ]; then
                BUILD_DIR="$candidate"
                break 2
            fi
        done
    done
    # Fallback to plain dist / release dir
    if [ -z "$BUILD_DIR" ]; then
        if [ -f "$TARGET_DIR/dist/sotf-tui.exe" ]; then
            BUILD_DIR="$TARGET_DIR/dist"
        else
            BUILD_DIR="$TARGET_DIR/release"
        fi
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
        log_info "Install with: $(install_hint openssl)"
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
    for bin in sotf-desktop.exe sotf-tui.exe; do
        if [ -f "$BUILD_DIR/$bin" ]; then
            log_info "Found $bin"
            found_any=true
        fi
    done

    if ! $found_any; then
        log_error "No Windows binaries found in $BUILD_DIR"
        log_info "Build them first with: just docker-windows-x86"
        exit 1
    fi

    if ! command -v zip &> /dev/null; then
        log_error "zip is required but not found"
        log_info "Searched PATH: $PATH"
        log_info "Install with: $(install_hint zip)"
        exit 1
    fi

    if $SIGN; then
        if ! command -v osslsigncode &> /dev/null; then
            log_error "osslsigncode is required for signing but not found"
            log_info "Install with: $(install_hint osslsigncode)"
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

xml_escape() {
    local value="$1"
    value=${value//&/&amp;}
    value=${value//</&lt;}
    value=${value//>/&gt;}
    value=${value//\"/&quot;}
    printf '%s' "$value"
}

file_size_bytes() {
    stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1"
}

string_size_bytes() {
    LC_ALL=C printf '%s' "$1" | wc -c | tr -d ' '
}

content_type_for_extension() {
    case "$1" in
        csv)  printf 'text/csv' ;;
        dll)  printf 'application/x-msdownload' ;;
        exe)  printf 'application/x-msdownload' ;;
        ico)  printf 'image/x-icon' ;;
        jpg|jpeg) printf 'image/jpeg' ;;
        json) printf 'application/json' ;;
        otf)  printf 'font/otf' ;;
        png)  printf 'image/png' ;;
        svg)  printf 'image/svg+xml' ;;
        ttf)  printf 'font/ttf' ;;
        txt)  printf 'text/plain' ;;
        webp) printf 'image/webp' ;;
        woff) printf 'font/woff' ;;
        woff2) printf 'font/woff2' ;;
        xml)  printf 'application/xml' ;;
        *)    printf 'application/octet-stream' ;;
    esac
}

generate_content_types() {
    local staging="$1"
    local ext ext_lc content_type

    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'

    find "$staging" -type f ! -name 'AppxBlockMap.xml' -print0 \
        | while IFS= read -r -d '' file; do
            local rel="${file#$staging/}"
            local name="${rel##*/}"
            [[ "$name" == *.* ]] || continue
            ext="${name##*.}"
            ext_lc=$(printf '%s' "$ext" | tr '[:upper:]' '[:lower:]')
            printf '%s\n' "$ext_lc"
        done \
        | sort -u \
        | while IFS= read -r ext; do
        content_type=$(content_type_for_extension "$ext")
        printf '  <Default Extension="%s" ContentType="%s"/>\n' \
            "$(xml_escape "$ext")" "$(xml_escape "$content_type")"
    done

    printf '%s\n' '  <Override PartName="/AppxManifest.xml" ContentType="application/vnd.ms-appx.manifest+xml"/>'
    printf '%s\n' '  <Override PartName="/AppxBlockMap.xml" ContentType="application/vnd.ms-appx.blockmap+xml"/>'
    printf '%s\n' '</Types>'
}

generate_block_map() {
    local staging="$1"
    local block_size=65536

    printf '%s\n' '<?xml version="1.0" encoding="UTF-8" standalone="no"?>'
    printf '%s\n' '<BlockMap xmlns="http://schemas.microsoft.com/appx/2010/blockmap" HashMethod="http://www.w3.org/2001/04/xmlenc#sha256">'

    while IFS= read -r -d '' file; do
        local rel="${file#$staging/}"
        local size lfh_size block_index hash
        size=$(file_size_bytes "$file")
        lfh_size=$((30 + $(string_size_bytes "$rel")))

        printf '  <File Name="%s" Size="%s" LfhSize="%s">\n' \
            "$(xml_escape "$rel")" "$size" "$lfh_size"

        block_index=0
        while [ $((block_index * block_size)) -lt "$size" ]; do
            hash=$(dd if="$file" bs="$block_size" skip="$block_index" count=1 2>/dev/null \
                | openssl dgst -sha256 -binary \
                | openssl base64 -A)
            printf '    <Block Hash="%s"/>\n' "$hash"
            block_index=$((block_index + 1))
        done

        printf '%s\n' '  </File>'
    done < <(find "$staging" -type f ! -name 'AppxBlockMap.xml' -print0 | sort -z)

    printf '%s\n' '</BlockMap>'
}

pack_msix_with_zip() {
    local staging="$1"
    local output="$2"

    if ! command -v openssl &> /dev/null; then
        log_error "openssl is required for built-in MSIX metadata generation"
        log_info "Install with: $(install_hint openssl)"
        exit 1
    fi

    generate_content_types "$staging" > "$staging/[Content_Types].xml"
    generate_block_map "$staging" > "$staging/AppxBlockMap.xml"

    (
        cd "$staging"
        find . -type f -print0 \
            | sort -z \
            | while IFS= read -r -d '' file; do
                printf '%s\n' "${file#./}"
            done \
            | zip -X -0 -q "$output" -@
    )
}

build_msix() {
    log_info "Building MSIX package v${VERSION} (${ARCH})..."

    local staging="$DIST_DIR/msix-staging"
    local output="$DIST_DIR/sotf-desktop-${VERSION}-windows-${ARCH}.msix"

    rm -rf "$staging"
    mkdir -p "$staging/assets"

    # Copy binaries
    for bin in sotf-desktop.exe sotf-tui.exe; do
        if [ -f "$BUILD_DIR/$bin" ]; then
            cp "$BUILD_DIR/$bin" "$staging/"
            log_info "Added $bin"
        fi
    done

    # No native runtime DLLs are bundled. cobyla is pure Rust now (nlopt.dll
    # removed) and the MSVC C/C++ runtime is satisfied via the
    # Microsoft.VCLibs.140.00.UWPDesktop framework dependency declared in
    # AppxManifest.xml.

    # Sign executables before packaging into MSIX
    if $SIGN; then
        for bin in sotf-desktop.exe sotf-tui.exe; do
            if [ -f "$staging/$bin" ]; then
                sign_file "$staging/$bin" "SotF Player"
            fi
        done
    fi

    # Copy app assets (fonts, icons, headphone-targets — demo-audio now lives in data_tests/audio)
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

    # Generate AppxManifest.xml with correct version and architecture.
    # The leading [[:space:]] in the Version pattern is critical: without it
    # sed also matches the substring `Version="..."` inside `MinVersion="..."`
    # and `MaxVersionTested="..."` on the <TargetDeviceFamily> line and
    # corrupts those attributes (a non-Win10-class MinVersion makes the package
    # fail install with "error in parsing the app package").
    sed -e "s/\\([[:space:]]\\)Version=\"[^\"]*\"/\\1Version=\"${MSIX_VERSION}\"/" \
        -e "s/ProcessorArchitecture=\"[^\"]*\"/ProcessorArchitecture=\"${MSIX_ARCH}\"/" \
        "$PROJECT_ROOT/builds/windows/AppxManifest.xml" > "$staging/AppxManifest.xml"

    # Pre-flight validation on the rendered manifest. Catches XML
    # well-formedness errors, Capabilities mis-ordering, broken Version /
    # ProcessorArchitecture, and missing runFullTrust *before* invoking
    # MakeAppx (which is slow when it lives on a remote Windows machine).
    if ! validate_appx_manifest "$staging/AppxManifest.xml"; then
        log_error "Aborting MSIX build — manifest validation failed."
        rm -rf "$staging"
        exit 1
    fi

    # Build the .msix.
    #
    # A real MSIX is NOT just a ZIP — it must contain `[Content_Types].xml`
    # (MIME registry) and `AppxBlockMap.xml` (SHA256 hashes of every file
    # block) generated by the packaging tool. Without those Windows refuses
    # the install with "error in parsing the app package".
    #
    # We use Microsoft's MakeAppx.exe (Windows SDK) when available; that's
    # the only tool that emits a fully-spec-compliant package. Plain `zip`
    # is left as a last-resort fallback that prints a loud warning.
    rm -f "$output"

    local makeappx
    makeappx=$(find_makeappx || true)

    if [ -n "$makeappx" ]; then
        log_info "Packing with MakeAppx.exe: $makeappx"
        # /o overwrites the output. /v makes it verbose. We deliberately do
        # NOT pass /nv: leaving manifest validation enabled surfaces broken
        # capabilities, missing assets, etc. at pack time rather than at
        # install time.
        #
        # Two MSYS/Git Bash gotchas we have to defend against:
        #   1. Flags like `/o`, `/v`, `/d`, `/p` look like Unix absolute paths
        #      to MSYS, which rewrites them to `O:/`, `V:/`, … before makeappx
        #      ever sees them ("Unknown command line option: O:/").
        #   2. The two REAL paths (staging dir, output file) come in as
        #      `/c/Users/...` MSYS form, which makeappx (a native Win32 binary)
        #      can't open.
        # Fix: convert the paths ourselves with cygpath, then run the command
        # with MSYS_NO_PATHCONV=1 so MSYS leaves the flags alone.
        local staging_win output_win
        staging_win=$(cygpath -w "$staging" 2>/dev/null || printf '%s' "$staging")
        output_win=$(cygpath -w  "$output"  2>/dev/null || printf '%s' "$output")
        MSYS_NO_PATHCONV=1 "$makeappx" pack /o /v /d "$staging_win" /p "$output_win"
    elif command -v makemsix &> /dev/null; then
        # Microsoft's open-source MSIX SDK build (https://github.com/microsoft/msix-packaging)
        log_info "Packing with makemsix (msix-packaging-tool)"
        makemsix pack -d "$staging" -p "$output"
    else
        log_warning "Neither MakeAppx.exe nor makemsix is available."
        log_warning "Packing with the built-in uncompressed ZIP fallback."
        pack_msix_with_zip "$staging" "$output"
    fi

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
