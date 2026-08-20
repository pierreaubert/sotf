#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/test-systemwide.sh <macos|linux>

Runs the native systemwide contract/component gate for the selected platform.
Use the just recipes instead of invoking the Linux mode directly on a host.
EOF
}

phase() {
    printf '\n==> %s\n' "$1"
}

require_platform() {
    local expected_os=$1
    local actual_os
    actual_os=$(uname -s)
    if [[ "$actual_os" != "$expected_os" ]]; then
        printf 'error: this gate requires %s, found %s\n' "$expected_os" "$actual_os" >&2
        exit 1
    fi
}

require_arm64() {
    local actual_arch
    actual_arch=$(uname -m)
    case "$actual_arch" in
        arm64|aarch64)
            ;;
        *)
            printf 'error: this gate requires ARM64, found %s\n' "$actual_arch" >&2
            exit 1
            ;;
    esac
}

test_rust_common() {
    phase "driver-common tests"
    cargo test --locked -p driver-common

    phase "sotf-daemon tests (including real local IPC)"
    cargo test --locked -p sotf-daemon
}

test_macos() {
    require_platform Darwin

    phase "macOS HAL Rust tests"
    cargo test --locked -p driver-hal

    test_rust_common

    phase "strict Rust lint"
    cargo clippy --locked \
        -p driver-common \
        -p driver-hal \
        -p sotf-daemon \
        --all-targets \
        --no-deps \
        --features sotf-daemon/hal \
        -- \
        -D warnings

    local configbar_dir="crates/systemwide/crates/daemon/configbar"
    local module_cache="${TMPDIR:-/tmp}/sotf-clang-module-cache"
    mkdir -p "$module_cache"
    local swift_build_dir="${CARGO_TARGET_DIR:-target}/systemwide-swift-package"
    mkdir -p "$swift_build_dir"

    phase "Swift menu-bar client type-check"
    swift build \
        --package-path "$configbar_dir" \
        --scratch-path "$swift_build_dir" \
        --target SotFSystemwide

    local hal_sources="crates/systemwide/crates/driver-hal/swift/Sources"
    phase "Swift HAL driver type-check"
    swiftc \
        -module-cache-path "$module_cache" \
        -typecheck \
        -module-name SotFHAL \
        -import-objc-header "$hal_sources/BridgingHeader.h" \
        -framework CoreAudio \
        -framework CoreFoundation \
        -framework Foundation \
        "$hal_sources/Timing.swift" \
        "$hal_sources/RingBuffer.swift" \
        "$hal_sources/SharedMemory.swift" \
        "$hal_sources/Encryption.swift" \
        "$hal_sources/SotFHALDriver.swift"
}

test_linux() {
    require_platform Linux
    require_arm64

    phase "portable HAL streaming regression tests"
    cargo test --locked -p driver-hal --test streaming_regression_tests

    test_rust_common

    phase "strict Rust lint"
    cargo clippy --locked \
        -p driver-common \
        -p driver-hal \
        -p sotf-daemon \
        --all-targets \
        --no-deps \
        -- \
        -D warnings
}

if [[ $# -ne 1 ]]; then
    usage >&2
    exit 2
fi

case "$1" in
    macos)
        test_macos
        ;;
    linux)
        test_linux
        ;;
    -h|--help)
        usage
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac
