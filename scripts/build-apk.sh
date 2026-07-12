#!/bin/bash
#
# Build an unsigned APK for the SOTF Android app.
#
# Usage:
#   ./scripts/build-apk.sh
#   ./scripts/build-apk.sh --clean
#
# Output:
#   dist/sotf-android-<version>.apk
#

set -euo pipefail

APP_NAME="sotf-android"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"

VERSION=$(sotf_version "$PROJECT_ROOT")
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml" >&2
    exit 1
fi

ANDROID_DIR="$PROJECT_ROOT/crates/app-android/android"
GRADLE_DIR="$ANDROID_DIR/gradle"
JNI_LIBS_DIR="$GRADLE_DIR/app/src/main/jniLibs"
DIST_DIR="$PROJECT_ROOT/dist"

CLEAN=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1" >&2; }

usage() {
    sed -n '2,/^$/p' "$0" | sed 's/^#//' | sed 's/^ //'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean)        CLEAN=true; shift ;;
        --help|-h)      usage; exit 0 ;;
        *) log_error "Unknown option: $1"; usage; exit 1 ;;
    esac
done

check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo is not installed"
        exit 1
    fi

    if ! cargo ndk --version &> /dev/null; then
        log_error "cargo-ndk is not installed. Install with: cargo install cargo-ndk"
        exit 1
    fi

    if [ -z "${ANDROID_SDK_ROOT:-}" ] && [ -z "${ANDROID_HOME:-}" ]; then
        log_error "ANDROID_SDK_ROOT or ANDROID_HOME must be set"
        exit 1
    fi

    if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${ANDROID_NDK_ROOT:-}" ]; then
        log_error "ANDROID_NDK_HOME or ANDROID_NDK_ROOT must be set"
        exit 1
    fi

    log_success "Prerequisites check passed"
}

clean_build() {
    if $CLEAN; then
        log_info "Cleaning Android build outputs..."
        rm -rf "$JNI_LIBS_DIR"
        cd "$GRADLE_DIR"
        ./gradlew clean
        cd "$PROJECT_ROOT"
        cargo clean -p sotf-android
    fi
}

build_rust_lib() {
    log_info "Building Rust cdylib for arm64-v8a..."

    mkdir -p "$JNI_LIBS_DIR"
    cargo ndk -t arm64-v8a -P 26 -o "$JNI_LIBS_DIR" build -p sotf-android --release

    local so_path="$JNI_LIBS_DIR/arm64-v8a/libsotf_android.so"
    if [ ! -f "$so_path" ]; then
        log_error "Shared library not found at $so_path"
        exit 1
    fi

    log_success "Rust cdylib built: $so_path"
}

build_apk() {
    log_info "Building APK with Gradle..."

    cd "$GRADLE_DIR"
    ./gradlew assembleRelease -PsotfVersion="$VERSION"
    cd "$PROJECT_ROOT"

    local apk_path="$GRADLE_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
    if [ ! -f "$apk_path" ]; then
        log_error "APK not found at $apk_path"
        exit 1
    fi

    mkdir -p "$DIST_DIR"
    local dist_apk="$DIST_DIR/$APP_NAME-${VERSION}.apk"
    cp "$apk_path" "$dist_apk"

    log_success "APK built: $dist_apk"
    log_info "Size: $(du -h "$dist_apk" | cut -f1)"
    log_info ""
    log_info "To sign: apksigner sign --ks <keystore> $dist_apk"
}

main() {
    log_info "=========================================="
    log_info "Building $APP_NAME v$VERSION"
    log_info "=========================================="

    check_prerequisites
    clean_build
    build_rust_lib
    build_apk

    log_info "=========================================="
    log_success "Build complete!"
    log_info "=========================================="
}

main "$@"
