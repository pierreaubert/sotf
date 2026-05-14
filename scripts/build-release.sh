#!/bin/bash
#
# Build a full release of SotF for all platform/architecture combinations.
#
# Builds:
#   - macOS ARM64   (DMG + TUI binary)
#   - macOS x86_64  (DMG + TUI binary)
#   - Linux ARM64   (AppImage + tarball via Docker)
#   - Linux x86_64  (AppImage + tarball via Docker)
#   - Windows ARM64 (exe via Docker)
#   - Windows x86_64 (exe via Docker)
#
# Generates:
#   - dist/release-<version>.md   — GitHub release notes with download table
#   - Updates site/src/components/Download.astro with latest version
#
# Usage:
#   ./scripts/build-release.sh                 # Build all platforms
#   ./scripts/build-release.sh --skip-build    # Only generate release notes + update site
#   ./scripts/build-release.sh --sign          # Build and sign all artifacts
#   ./scripts/build-release.sh --platform linux # Build only Linux targets
#   ./scripts/build-release.sh --arch arm      # Build only ARM64 targets
#   ./scripts/build-release.sh --platform macos --arch x86  # macOS x86 only
#
# Prerequisites:
#   - Docker (for Linux and Windows cross-compilation)
#   - just (justfile runner)
#   - Rust toolchain with x86_64-apple-darwin target (for macOS x86 cross)
#   - For signing: see sign-macos.sh, sign-linux.sh
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

# GitHub release base URL
GITHUB_RELEASE_URL="https://github.com/pierreaubert/sotf/releases/download/v${VERSION}"

# Apple App Store product URL. The Mac desktop app's stable release is
# distributed via the Store; GitHub/macOS artifacts are beta builds.
APPLE_STORE_URL="https://apps.apple.com/ch/app/sound-of-the-future/id6754237332"

# Microsoft Store product URL. The Windows MSIX is now distributed via
# the Store (it auto-routes x64 vs arm64 to the user's CPU); the .msix
# files we still upload to GitHub Releases are unsigned-by-us fallbacks
# for sideload users and aren't the primary download surface.
MS_STORE_URL="https://apps.microsoft.com/detail/9NXCMV37NXJ7"

# Options
SKIP_BUILD=false
SKIP_SITE=false
SIGN=false
PLATFORM=""  # empty = all
ARCH=""      # empty = all, "arm" or "x86"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --skip-site)
            SKIP_SITE=true
            shift
            ;;
        --sign)
            SIGN=true
            shift
            ;;
        --platform)
            PLATFORM="$2"
            shift 2
            ;;
        --arch)
            ARCH="$2"
            if [[ "$ARCH" != "arm" && "$ARCH" != "x86" ]]; then
                echo "ERROR: --arch must be 'arm' or 'x86', got '$ARCH'"
                exit 1
            fi
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-build         Only generate release notes and update site"
            echo "  --skip-site          Do not rewrite site/src/components/Download.astro"
            echo "  --sign               Sign all artifacts after building"
            echo "  --platform <name>    Build only one platform: macos, linux, windows"
            echo "  --arch <name>        Build only one architecture: arm, x86"
            echo "  --help               Show this help message"
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

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

# Artifact metadata lookups (bash 3 compatible)
get_quality() {
    case "$1" in
        macos-arm64)   echo "beta" ;;
        *)             echo "alpha" ;;
    esac
}

get_signature() {
    case "$1" in
        macos-*)    echo "Apple Developer ID" ;;
        linux-*)    echo "cosign (Sigstore)" ;;
        # Windows MSIX is re-signed by the Microsoft Store on ingestion;
        # the .exe TUI fallback we host on GitHub Releases is still
        # self-signed but the primary install path goes through the Store.
        windows-*)  echo "Microsoft Store" ;;
    esac
}

should_build() {
    local platform="$1"
    [ -z "$PLATFORM" ] || [ "$PLATFORM" = "$platform" ]
}

should_build_arch() {
    local arch="$1"
    [ -z "$ARCH" ] || [ "$ARCH" = "$arch" ]
}

# -----------------------------------------------------------------------
# BUILD
# -----------------------------------------------------------------------

build_all() {
    mkdir -p "$DIST_DIR"

    if should_build "macos"; then
        # We deliberately do not invoke `just cross-macos-{arm64,x86}` here.
        # Those recipes historically rebuilt the GPUI binary statically before
        # wrapping it in a DMG. That static binary cannot be signed/notarized
        # cleanly, so the release pipeline builds the dynamically-linked binary
        # first and then asks the DMG script to package that exact binary.
        if should_build_arch "arm"; then
            log_info "=== macOS ARM64 (dynamic) ==="
            rustup target add aarch64-apple-darwin 2>/dev/null || true
            # Release cuts use the `dist` profile (fat LTO + codegen-units=1).
            # See [profile.dist] in root Cargo.toml. Artifacts land in
            # target/<triple>/dist/ instead of target/<triple>/release/.
            cargo build --profile dist --target aarch64-apple-darwin -p sotf-tui --features hal,onnx
            cargo build --profile dist --target aarch64-apple-darwin -p sotf-gpui --features hal,onnx
            cp "target/aarch64-apple-darwin/dist/sotf-tui"     "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64"
            cp "target/aarch64-apple-darwin/dist/sotf-desktop" "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64"
            ./scripts/build-dmg-sotf.sh --arch arm64 --binary "target/aarch64-apple-darwin/dist/sotf-desktop"
        fi
        if should_build_arch "x86"; then
            log_info "=== macOS x86_64 (dynamic) ==="
            rustup target add x86_64-apple-darwin 2>/dev/null || true
            cargo build --profile dist --target x86_64-apple-darwin -p sotf-tui --features hal
            cargo build --profile dist --target x86_64-apple-darwin -p sotf-gpui --features hal
            cp "target/x86_64-apple-darwin/dist/sotf-tui"     "$DIST_DIR/sotf-tui-${VERSION}-macos-x86_64"
            cp "target/x86_64-apple-darwin/dist/sotf-desktop" "$DIST_DIR/sotf-desktop-${VERSION}-macos-x86_64"
            ./scripts/build-dmg-sotf.sh --arch x86_64 --binary "target/x86_64-apple-darwin/dist/sotf-desktop"
        fi
    fi

    if should_build "linux"; then
        if should_build_arch "arm"; then
            log_info "=== Linux ARM64 ==="
            just docker-linux-arm64
        fi
        if should_build_arch "x86"; then
            log_info "=== Linux x86_64 ==="
            just docker-linux-x86
        fi
    fi

    if should_build "windows"; then
        if should_build_arch "arm"; then
            log_info "=== Windows ARM64 ==="
            just docker-windows-arm64
        fi
        if should_build_arch "x86"; then
            log_info "=== Windows x86_64 ==="
            just docker-windows-x86
        fi
    fi

    log_success "All builds complete. Artifacts in $DIST_DIR"
}

# -----------------------------------------------------------------------
# SIGN
# -----------------------------------------------------------------------

sign_all() {
    log_info "Signing artifacts..."

    if should_build "macos"; then
        if [ -n "${DEVELOPER_ID:-}" ]; then
            "$SCRIPT_DIR/sign-macos.sh"
        else
            log_warning "DEVELOPER_ID not set, skipping macOS signing"
        fi
    fi

    if should_build "linux"; then
        if command -v cosign &> /dev/null; then
            "$SCRIPT_DIR/sign-linux.sh"
        else
            log_warning "cosign not found, skipping Linux signing"
        fi
    fi

    log_success "Signing complete"
}

# -----------------------------------------------------------------------
# GENERATE RELEASE NOTES
# -----------------------------------------------------------------------

generate_release_md() {
    local release_file="$DIST_DIR/release-${VERSION}.md"
    log_info "Generating release notes: $release_file"

    cat > "$release_file" << EOF
# SotF v${VERSION}

## Downloads

Stable desktop releases are distributed through the Apple App Store and
Microsoft Store when available. Beta releases and command-line artifacts are
published on GitHub Releases.

| OS | Architecture | Download | Quality | Signature |
|----|-------------|----------|---------|-----------|
EOF

    echo "| macOS | Universal | [App Store](${APPLE_STORE_URL}) | stable | App Store |" >> "$release_file"
    # macOS ARM64
    append_release_row "$release_file" "macOS" "ARM64 (Apple Silicon)" "macos-arm64"
    # macOS x86_64
    append_release_row "$release_file" "macOS" "x86_64 (Intel)" "macos-x86_64"
    # Linux ARM64
    append_release_row "$release_file" "Linux" "ARM64" "linux-arm64"
    # Linux x86_64
    append_release_row "$release_file" "Linux" "x86_64" "linux-x86_64"
    # Windows ARM64
    append_release_row "$release_file" "Windows" "ARM64" "windows-arm64"
    # Windows x86_64
    append_release_row "$release_file" "Windows" "x86_64" "windows-x86_64"

    cat >> "$release_file" << 'EOF'

## What's new

<!-- Add release notes here -->

## Installation

### macOS
Use the App Store link for the stable desktop app. GitHub macOS artifacts are
beta builds: download the `sotf-desktop-*.dmg` for your CPU for the desktop app
or the `sotf-tui-*` binary for the terminal UI. For bare binaries, make them
executable (`chmod +x sotf-*`) and run from Terminal. The first run may require
right-click → Open to bypass Gatekeeper if Apple notarization is unavailable.

### Linux
Download the AppImage or tarball. For AppImage: `chmod +x sotf-desktop-*.AppImage && ./sotf-desktop-*.AppImage`.
For the tarball: extract and run `./sotf-desktop` or `./sotf-tui`.

### Windows
Desktop player: download the `sotf-desktop-*.msix` matching your CPU and
double-click to install. The first run may require accepting the developer
certificate (the package is self-signed).
Terminal player: download the `sotf-tui-*.exe` and run from PowerShell or cmd.

## Verification

### macOS
Binaries are signed with an Apple Developer ID certificate and notarized by Apple.

Check the signature:
EOF

    cat >> "$release_file" << EOF
\`\`\`bash
codesign -dv --verbose=2 sotf-desktop-${VERSION}-macos-arm64
\`\`\`

Verify notarization:
\`\`\`bash
spctl -a -vv --type execute sotf-desktop-${VERSION}-macos-arm64
\`\`\`
EOF

    cat >> "$release_file" << 'EOF'

### Linux
Artifacts are signed with cosign (Sigstore). Each artifact has a `.bundle` file containing the signature and certificate.

Verify with cosign:
EOF

    cat >> "$release_file" << EOF
\`\`\`bash
cosign verify-blob --bundle sotf-desktop-${VERSION}-linux-arm64.AppImage.bundle \\
  --certificate-identity=pierre0aubert@gmail.com \\
  --certificate-oidc-issuer=https://accounts.google.com \\
  sotf-desktop-${VERSION}-linux-arm64.AppImage
\`\`\`
EOF

    cat >> "$release_file" << 'EOF'

### Windows
Windows binaries are self-signed. A SmartScreen warning may appear on first run.
EOF

    log_success "Release notes written to $release_file"
}

append_release_row() {
    local file="$1"
    local os="$2"
    local arch="$3"
    local key="$4"

    local quality
    quality=$(get_quality "$key")
    local signature
    signature=$(get_signature "$key")
    local downloads=""

    # Build download links based on what exists
    local files=""
    case "$key" in
        macos-arm64)
            files="sotf-desktop-${VERSION}-macos-arm64.dmg:DMG sotf-desktop-${VERSION}-macos-arm64:GPUI%20binary sotf-tui-${VERSION}-macos-arm64:TUI%20binary"
            ;;
        macos-x86_64)
            files="sotf-desktop-${VERSION}-macos-x86_64.dmg:DMG sotf-desktop-${VERSION}-macos-x86_64:GPUI%20binary sotf-tui-${VERSION}-macos-x86_64:TUI%20binary"
            ;;
        linux-arm64)
            files="sotf-desktop-${VERSION}-linux-arm64.tar.gz:tarball sotf-desktop-${VERSION}-linux-arm64.AppImage:AppImage"
            ;;
        linux-x86_64)
            files="sotf-desktop-${VERSION}-linux-x86_64.tar.gz:tarball sotf-desktop-${VERSION}-linux-x86_64.AppImage:AppImage"
            ;;
        windows-arm64)
            files="sotf-desktop-${VERSION}-windows-arm64.msix:MSIX sotf-tui-${VERSION}-windows-arm64.exe:TUI%20exe"
            ;;
        windows-x86_64)
            files="sotf-desktop-${VERSION}-windows-x86_64.msix:MSIX sotf-tui-${VERSION}-windows-x86_64.exe:TUI%20exe"
            ;;
    esac

    for entry in $files; do
        local filename="${entry%%:*}"
        local label="${entry##*:}"
        label=$(echo "$label" | sed 's/%20/ /g')

        # MSIX files are served by the Microsoft Store (single product
        # page auto-routes by CPU), not from GitHub Releases. Link there
        # without a presence check — Microsoft is the host of record.
        local link
        case "$filename" in
            *.msix)
                link="[Microsoft Store](${MS_STORE_URL})"
                ;;
            *)
                if [ ! -f "$DIST_DIR/$filename" ]; then
                    continue
                fi
                link="[${label}](${GITHUB_RELEASE_URL}/${filename})"
                ;;
        esac

        if [ -n "$downloads" ]; then
            downloads="${downloads}, ${link}"
        else
            downloads="$link"
        fi
    done

    if [ -z "$downloads" ]; then
        downloads="not yet available"
    fi

    echo "| $os | $arch | $downloads | $quality | $signature |" >> "$file"
}

# -----------------------------------------------------------------------
# UPDATE SITE
# -----------------------------------------------------------------------

update_site() {
    local download_file="$PROJECT_ROOT/site/src/components/Download.astro"

    if [ ! -f "$download_file" ]; then
        log_warning "Download.astro not found, skipping site update"
        return
    fi

    log_info "Updating site Download component to v${VERSION}..."

    cat > "$download_file" << 'ASTRO_HEADER'
---
ASTRO_HEADER

    cat >> "$download_file" << EOF
const version = '${VERSION}';
const releaseUrl = 'https://github.com/pierreaubert/sotf/releases';
// Each release artifact is published in TWO places: the GitHub Release
// (canonical, signed via the build pipeline) and a mirror under
// sotf.spinorama.org/downloads/ (rsynced by site/update_prod.sh). The UI
// surfaces both sources per file so users can pick whichever is faster /
// reachable from their network.
const githubBase = \`https://github.com/pierreaubert/sotf/releases/download/v\${version}\`;
const mirrorBase = 'https://sotf.spinorama.org/downloads';
// Apple App Store product URL. Stable macOS desktop releases are distributed
// via the Store; GitHub/macOS artifacts remain available as beta builds.
const appleStoreUrl = '${APPLE_STORE_URL}';
// Microsoft Store product URL. Windows MSIX is distributed exclusively
// via the Store (single page auto-routes x64 / arm64 to the user's CPU).
const msStoreUrl = '${MS_STORE_URL}';
EOF

    cat >> "$download_file" << 'ASTRO_BODY'

const builds = [
  {
    os: 'macOS',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>`,
    variants: [
      { arch: 'Universal', quality: 'stable', signature: 'App Store', files: [
        { label: 'Desktop app', url: appleStoreUrl, host: 'App Store' },
      ]},
      { arch: 'ARM64 (Apple Silicon)', quality: 'beta', signature: 'Apple Developer ID', files: [
        { label: 'DMG',         file: `sotf-desktop-${version}-macos-arm64.dmg` },
        { label: 'GPUI binary', file: `sotf-desktop-${version}-macos-arm64` },
        { label: 'TUI binary',  file: `sotf-tui-${version}-macos-arm64` },
      ]},
      { arch: 'x86_64 (Intel)', quality: 'alpha', signature: 'Apple Developer ID', files: [
        { label: 'DMG',         file: `sotf-desktop-${version}-macos-x86_64.dmg` },
        { label: 'GPUI binary', file: `sotf-desktop-${version}-macos-x86_64` },
        { label: 'TUI binary',  file: `sotf-tui-${version}-macos-x86_64` },
      ]},
    ],
  },
  {
    os: 'Linux',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2a.42.42 0 00.114.333l.003.003c.391.778 1.113 1.368 1.884 1.43.39.033.77-.396 1.164-.664.136-.468.085-.1.003-.146-.246-.86.356-.165.535-.066.12.262-.06.609-.003.79.18.18.396.27.637.27.36 0 .756-.2 1.052-.394.296-.199.526-.465.656-.73.237-.563.03-1.204.087-1.55.057-.33.153-.577.253-.868.138-.413-.1-.763-.32-.902-.248-.098-.51-.137-.536-.316-.04-.247.135-.725.272-1.1.157-.51.303-1.136-.03-1.533l-.001-.002c-.17-.232-.484-.2-.703-.034-.186.248-.343.523-.484.84-.094.194-.192.38-.3.498-.052.063-.11.092-.171.116a.46.46 0 01-.065-.096c-.198-.398-.332-.726-.465-1.03-.135-.31-.27-.606-.428-.87-.088-.15-.23-.286-.354-.446-.125-.16-.235-.323-.236-.417 0-.056.017-.074.08-.074h.004c.218 0 .47.018.674.064l.003.001c.282.067.546.2.762.372a.65.65 0 00.03-.128c.037-.27-.01-.517-.16-.606a1.357 1.357 0 00-.399-.113l-.002-.001a5.32 5.32 0 00-.754-.057c-.283 0-.559.036-.773.098a4.4 4.4 0 01-.065-.38c-.02-.156-.033-.32-.033-.467 0-.291.035-.492.118-.63l.003-.005c.076-.112.17-.163.295-.163.104 0 .225.037.344.084l.003.002c.252.114.503.257.703.387l.13.08c.093.053.157.068.214.068.152 0 .233-.148.233-.309 0-.133-.07-.334-.18-.465-.265-.363-.603-.602-.91-.724l-.006-.003c-.27-.111-.604-.17-.887-.17-.348 0-.71.063-1 .194-.01-.097-.01-.197-.01-.3 0-1.16.224-2.173.65-3.02.416-.83 1.014-1.47 1.74-1.906a.425.425 0 00.207-.368c0-.22-.18-.4-.4-.4a.404.404 0 00-.176.04c-.837.491-1.504 1.206-1.957 2.108-.455.897-.697 1.986-.697 3.247 0 .16.003.32.013.48-.377.254-.732.574-1.019.971-.35-.178-.738-.28-1.13-.3a1.975 1.975 0 00-.34-.006 2.44 2.44 0 00-.32.031c-.095-.28-.153-.478-.153-.605 0-.159.024-.295.104-.495l.004-.005c.058-.123.165-.322.304-.557.138-.234.302-.495.459-.751l.002-.002c.306-.504.606-1.02.75-1.468.144-.454.162-.887-.085-1.148l-.001-.002c-.17-.175-.465-.268-.782-.268-.318 0-.607.093-.804.268l-.001.001c-.263.214-.45.498-.555.81-.247.76-.26 1.537-.216 2.37-.157.24-.289.469-.38.686a2.58 2.58 0 00-.204.834c-.007.164-.009.33.004.5-.293.05-.556.12-.78.22l-.127.063c-.2-.47-.406-.94-.556-1.413-.22-.61-.35-1.24-.35-1.725 0-.625.124-1.114.363-1.468l.002-.003c.238-.342.572-.567.983-.697.412-.133.928-.177 1.488-.177h.012c.728 0 1.551.084 2.166.33.618.239 1.048.642 1.048 1.377 0 .237-.078.465-.196.672l-.005.008a.401.401 0 00-.018.39.399.399 0 00.527.163l.005-.003c.296-.152.5-.383.65-.648.15-.267.243-.573.243-.89 0-.58-.22-1.072-.589-1.455-.368-.382-.87-.63-1.407-.794l-.005-.002c-.677-.2-1.483-.296-2.362-.296-.629 0-1.203.049-1.712.215a2.62 2.62 0 00-1.337.928c-.356.485-.539 1.108-.539 1.877 0 .58.149 1.277.38 1.935.19.558.43 1.094.652 1.576-.075.082-.137.17-.183.265l-.005.011c-.156.348-.217.74-.23 1.14-.014.405.017.828.07 1.214-.197.323-.345.67-.424 1.033-.12.561-.097 1.152.23 1.618l.008.012.009.01c.23.25.527.377.83.412a3.252 3.252 0 001.06-.053c.126.078.259.148.392.209.165.075.333.137.497.184a2.455 2.455 0 00-.163.667c-.026.303.008.604.087.87a.423.423 0 00.532.27l.002-.001c.157-.049.29-.11.427-.19.136-.078.26-.175.34-.295l.002-.003c.211-.337.19-.708.1-1.021a2.606 2.606 0 00-.32-.645l-.007-.01a4.13 4.13 0 01-.14-.246c.14-.097.26-.208.36-.337.143-.183.237-.395.283-.638l.005.002c.184.065.37.113.55.137l.007.001c.455.065.877-.065 1.178-.359.115.248.262.485.438.705.264.335.592.607.974.773.02.008.04.014.06.023.044.29.128.576.272.838a.424.424 0 00.564.179l.002-.001c.258-.133.41-.34.497-.572.087-.232.107-.497.06-.763l-.002-.007c-.06-.341-.197-.677-.359-.975l-.002-.004a1.42 1.42 0 01.068-.07c.176-.2.314-.412.37-.638.06-.228.04-.44-.05-.627z"/></svg>`,
    variants: [
      { arch: 'ARM64', quality: 'alpha', signature: 'cosign (Sigstore)', files: [
        { label: 'AppImage', file: `sotf-desktop-${version}-linux-arm64.AppImage` },
        { label: 'tarball', file: `sotf-desktop-${version}-linux-arm64.tar.gz` },
      ]},
      { arch: 'x86_64', quality: 'alpha', signature: 'cosign (Sigstore)', files: [
        { label: 'AppImage', file: `sotf-desktop-${version}-linux-x86_64.AppImage` },
        { label: 'tarball', file: `sotf-desktop-${version}-linux-x86_64.tar.gz` },
      ]},
    ],
  },
  {
    os: 'Windows',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M3 12V6.5l8-1.1V12H3zm0 .5h8v6.6l-8-1.1V12.5zm9 0h9V3l-9 1.2V12.5zm0 .5v6.3L21 21v-8H12z"/></svg>`,
    variants: [
      { arch: 'ARM64', quality: 'alpha', signature: 'Microsoft Store', files: [
        { label: 'MSIX',    url: msStoreUrl, host: 'Microsoft Store' },
        { label: 'TUI exe', file: `sotf-tui-${version}-windows-arm64.exe` },
      ]},
      { arch: 'x86_64', quality: 'alpha', signature: 'Microsoft Store', files: [
        { label: 'MSIX',    url: msStoreUrl, host: 'Microsoft Store' },
        { label: 'TUI exe', file: `sotf-tui-${version}-windows-x86_64.exe` },
      ]},
    ],
  },
];

const qualityColors: Record<string, string> = {
  'stable': 'text-green-400 bg-green-400/10 border-green-400/20',
  'good': 'text-green-400 bg-green-400/10 border-green-400/20',
  'beta': 'text-yellow-400 bg-yellow-400/10 border-yellow-400/20',
  'alpha': 'text-orange-400 bg-orange-400/10 border-orange-400/20',
};
---

<section id="download" class="py-24 px-4 sm:px-6 fade-section">
  <div class="max-w-4xl mx-auto text-center">
    <h2 class="text-3xl sm:text-4xl font-bold text-white mb-4">Download SotF</h2>
    <span class="inline-block px-3 py-1 rounded-full text-sm font-mono bg-accent/10 text-accent border border-accent/20 mb-8">
      v{version}
    </span>
    <p class="max-w-2xl mx-auto text-sm text-gray-400 mb-8">
      Stable desktop releases are available on the platform stores. Beta releases
      and command-line artifacts are available on GitHub Releases.
    </p>

    <div class="overflow-x-auto mb-8">
      <table class="w-full text-left text-sm">
        <thead>
          <tr class="border-b border-border text-gray-400">
            <th class="py-3 px-4">OS</th>
            <th class="py-3 px-4">Architecture</th>
            <th class="py-3 px-4">Download</th>
            <th class="py-3 px-4">Quality</th>
            <th class="py-3 px-4">Signature</th>
          </tr>
        </thead>
        <tbody>
          {builds.map((platform) =>
            platform.variants.map((variant, i) => (
              <tr class="border-b border-border/50 hover:bg-card/50 transition-colors">
                {i === 0 ? (
                  <td class="py-3 px-4 font-medium text-white" rowspan={platform.variants.length}>
                    <div class="flex items-center gap-2">
                      <Fragment set:html={platform.icon} />
                      {platform.os}
                    </div>
                  </td>
                ) : null}
                <td class="py-3 px-4 text-gray-300">{variant.arch}</td>
                <td class="py-3 px-4">
                  <div class="space-y-1.5">
                    {variant.files.map((dl) => (
                      <div class="flex items-center gap-2 flex-wrap">
                        <span class="text-xs font-medium text-gray-300 min-w-20">{dl.label}</span>
                        {dl.url ? (
                          /* Store entry: hosted by the platform store, no
                             GitHub/mirror copy for the stable desktop build. */
                          <a
                            href={dl.url}
                            class="inline-flex items-center gap-1 px-2 py-0.5 text-xs bg-accent/10 text-accent border border-accent/20 rounded hover:bg-accent/20 transition-colors"
                            title={`Open ${dl.label} on ${'host' in dl ? dl.host : 'its host'}`}
                            target="_blank"
                            rel="noopener noreferrer"
                          >
                            {'host' in dl ? dl.host : dl.label}
                          </a>
                        ) : (
                          <Fragment>
                            <a
                              href={`${githubBase}/${dl.file}`}
                              class="inline-flex items-center gap-1 px-2 py-0.5 text-xs bg-accent/10 text-accent border border-accent/20 rounded hover:bg-accent/20 transition-colors"
                              title={`Download ${dl.file} from GitHub Releases`}
                            >
                              GitHub
                            </a>
                            <a
                              href={`${mirrorBase}/${dl.file}`}
                              class="inline-flex items-center gap-1 px-2 py-0.5 text-xs bg-accent/10 text-accent border border-accent/20 rounded hover:bg-accent/20 transition-colors"
                              title={`Download ${dl.file} from sotf.spinorama.org mirror`}
                            >
                              Mirror
                            </a>
                          </Fragment>
                        )}
                      </div>
                    ))}
                  </div>
                </td>
                <td class="py-3 px-4">
                  <span class={`inline-block px-2 py-0.5 text-xs rounded-full border ${qualityColors[variant.quality] || 'text-gray-400'}`}>
                    {variant.quality}
                  </span>
                </td>
                <td class="py-3 px-4 text-gray-400 text-xs">{variant.signature}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>

    <a
      href={releaseUrl}
      target="_blank"
      rel="noopener noreferrer"
      class="text-sm text-gray-400 hover:text-accent transition-colors"
    >
      All releases &rarr;
    </a>
  </div>
</section>
ASTRO_BODY

    log_success "Updated $download_file to v${VERSION}"
}

# -----------------------------------------------------------------------
# MAIN
# -----------------------------------------------------------------------

main() {
    log_info "=========================================="
    log_info "SotF Release Build v${VERSION}"
    log_info "=========================================="

    # Sync auxiliary version-pinned files (AppxManifest.xml, site/package.json)
    # to the workspace Cargo.toml version before building so artifacts and the
    # Windows manifest agree on the version string.
    log_info "Syncing version-pinned files to v${VERSION}..."
    "$SCRIPT_DIR/sync-version.sh"

    if ! $SKIP_BUILD; then
        build_all
    fi

    if $SIGN && ! $SKIP_BUILD; then
        sign_all
    fi

    generate_release_md
    if ! $SKIP_SITE; then
        update_site
    fi

    log_info "=========================================="
    log_success "Release v${VERSION} preparation complete!"
    log_info "=========================================="
    log_info ""
    log_info "Artifacts:     $DIST_DIR/"
    log_info "Release notes: $DIST_DIR/release-${VERSION}.md"
    if ! $SKIP_SITE; then
        log_info "Site updated:  site/src/components/Download.astro"
    fi
    log_info ""
    log_info "Next steps:"
    log_info "  1. Review $DIST_DIR/release-${VERSION}.md and add changelog"
    log_info "  2. Create GitHub release:"
    log_info "     gh release create v${VERSION} --title 'SotF v${VERSION}' \\"
    log_info "       --notes-file $DIST_DIR/release-${VERSION}.md \\"
    log_info "       $DIST_DIR/*"
    log_info "  3. Deploy site: cd site && npm run build && npm run deploy"
}

main "$@"
