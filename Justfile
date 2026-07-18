# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

_default:
	just --list

import 'builds/install.just'
import 'builds/updates.just'
import 'builds/docs.just'
import 'builds/cross.just'
import 'builds/macos.just'
import 'builds/windows.just'
import 'builds/linux.just'
import 'builds/ios.just'
import 'builds/tvos.just'
import 'builds/android.just'
import 'builds/dev-driver.just'

import 'crates/sotf-plugins/Justfile'
import 'crates/sotf-engine/Justfile'
import 'crates/sotf-tools/Justfile'

import 'builds/aggregates.just'

# ----------------------------------------------------------------------
# VARIABLES
# ----------------------------------------------------------------------

list_test_features := "qa, onnx, hal, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls"
list_prod_features := "onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls"

test_features := '--features="qa, onnx, hal, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls"'
release_test_features := '--features="qa, onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls"'
prod_features := '--features="onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls"'

test_features_macos := test_features
test_features_linux := '--features="qa, onnx, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls"'
test_features_windows := '--features="qa, onnx, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls"'

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

[group('test')]
check:
	cargo check --workspace  --lib --bins --tests --examples {{test_features}}

[group('test')]
test:
	cargo test --workspace  --lib --bins --tests --examples {{test_features}}

[group('test')]
test-negative:
	cargo test --test negative --release {{release_test_features}}

[group('test')]
test-proptest:
	PROPTEST_CASES=10000 cargo test --test proptest_tests --release {{release_test_features}}

[group('test')]
ntest:
	CARGO_PROFILE_RELEASE_LTO=off cargo nextest run --release --no-fail-fast --workspace --lib --bins --examples {{release_test_features}}

[group('test')]
itest:
	PROPTEST_CASES=10000 CARGO_PROFILE_RELEASE_LTO=off cargo nextest run --release --no-fail-fast --workspace --tests {{release_test_features}}

[group('test')]
atest: test-negative test-proptest ntest itest

# Run the isolated macOS systemwide-audio lab. This does not install or touch
# the CoreAudio HAL bundle; subprocess tests use temporary Unix sockets and
# the deterministic lab driver.
[group('test')]
[macos]
systemwide-lab:
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" cargo test -p sotf-daemon --bin sotf-daemon testkit
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" cargo test -p sotf-daemon --test daemon_state_tests
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" cargo test -p sotf-daemon --features hal --test ipc_line_tests -- --test-threads=1
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" cargo test -p driver-hal --lib
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" cargo test -p driver-hal --test streaming_regression_tests
	SOTF_SYSTEMWIDE_RUNTIME_DIR="/private/tmp/sotf-systemwide-lab-$USER" swift test --package-path crates/systemwide/crates/daemon/configbar --scratch-path target/configbar-swiftpm

# ----------------------------------------------------------------------
# COVERAGE
# ----------------------------------------------------------------------

# Requires: cargo install cargo-llvm-cov
# Generates an LCOV report for CI / Codecov upload.
[group('coverage')]
coverage:
	cargo llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --lcov --output-path target/lcov.info

# Generates an HTML coverage report and opens it.
[group('coverage')]
coverage-html:
	cargo llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --html --open

# Prints a text summary to stdout (fastest coverage recipe).
[group('coverage')]
coverage-summary:
	cargo llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --summary

# Removes stale coverage artifacts.
[group('coverage')]
coverage-clean:
	cargo llvm-cov clean

# ----------------------------------------------------------------------
# LINT
# ----------------------------------------------------------------------

[group('lint')]
lint:
	cargo clippy --all {{test_features}} -- -D warnings

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs).
# Debug builds enable `dev-api`, exposing the scripted-test HTTP endpoint on
# 127.0.0.1:7777 (override via SOTF_DEV_API_PORT). Release builds never include it.
[group('run')]
run-gpui:
	cargo build --bin sotf-desktop {{test_features}}
	codesign --force --deep --sign - --entitlements scripts/debug.entitlements target/debug/sotf-desktop
	./target/debug/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-release:
	cargo build --release --bin sotf-desktop {{prod_features}}
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo build --release --bin sotf-desktop {{test_features}}
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the TUI player
[group('run')]
[macos]
run-tui:
	cargo run --release --bin sotf-tui {{test_features_macos}}

[group('run')]
[linux]
run-tui:
	cargo run --release --bin sotf-tui {{test_features_linux}}

[group('run')]
[windows]
run-tui:
	cargo run --release --bin sotf-tui {{test_features_windows}}

# Run the TUI player (with debug info for leak detection)
[group('run')]
[macos]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui {{test_features_macos}}

[group('run')]
[linux]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui {{test_features_linux}}

[group('run')]
[windows]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui {{test_features_windows}}

# ----------------------------------------------------------------------
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt:
	cargo fmt --all

# ----------------------------------------------------------------------
# DIST — release-cut profile (fat LTO + codegen-units = 1)
# ----------------------------------------------------------------------
# Builds land in `target/dist/` (NOT `target/release/`). Compile time is
# noticeably longer than `prod-*`; only run these for actual release cuts.

# Top-level umbrella — builds everything that ships.
[group('dist')]
dist: dist-sotf-gpui dist-sotf-tui dist-sotf-recorder

[group('dist')]
dist-sotf-gpui:
	cargo build --profile dist --bin sotf-desktop -p sotf-gpui --features onnx

[group('dist')]
dist-sotf-tui:
	cargo build --profile dist --bin sotf-tui -p sotf-tui --features "onnx, streaming, hls"

[group('dist')]
dist-sotf-recorder:
	cargo build --profile dist --bin sotf-recorder-cli -p app-cli

# Whole workspace under the dist profile (slow — 10+ minutes typical).
[group('dist')]
dist-workspace:
	cargo build --profile dist --workspace

# ----------------------------------------------------------------------
# BUILD
# ----------------------------------------------------------------------

# shortcuts
[group('build')]
[macos]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui {{test_features_macos}}

[group('build')]
[linux]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui {{test_features_linux}}

[group('build')]
[windows]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui {{test_features_windows}}

alias terminal := gpui

[group('build')]
gpui:
	cargo run --release --bin sotf-desktop -p sotf-gpui {{release_test_features}}

alias desktop := gpui
alias native := gpui

# ----------------------------------------------------------------------
# CLEAN
# ----------------------------------------------------------------------

clean:
	cargo clean
	find . -name '*~' -exec rm {} \; -print
	find . -name 'Cargo.lock' -exec rm {} \; -print
