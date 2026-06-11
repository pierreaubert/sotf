# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

_default:
	just --list

import 'builds/install.just'
import 'builds/updates.just'
import 'builds/docs.just'
import 'builds/aggregates.just'
import 'builds/cross.just'
import 'builds/macos.just'
import 'builds/windows.just'
import 'builds/linux.just'
import 'builds/ios.just'
import 'builds/tvos.just'

import 'crates/math-audio/Justfile'
import 'crates/autoeq/Justfile'
import 'crates/gpui-toolkit/Justfile'
import 'crates/sotf-plugins/Justfile'
import 'crates/sotf-engine/Justfile'
import 'crates/sotf-tools/Justfile'

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
	cargo test --test negative --release {{release_test_features}}
	PROPTEST_CASES=10000 cargo test --test proptest_tests --release {{release_test_features}}
	CARGO_PROFILE_RELEASE_LTO=off cargo nextest run --release --no-fail-fast --workspace --lib --bins --tests --examples {{release_test_features}}

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
# PROD
# ----------------------------------------------------------------------

[group('build')]
prod-generate-audio-tests:
	cargo build --release --bin generate-audio-tests -p sotf-tools

[group('build')]
prod-workspace: prod-plot-bins
	cargo build --release --workspace

# Binaries gated by `required-features = ["plotly"]` are silently skipped by
# `cargo build --workspace`; build them explicitly so a `prod-workspace` run
# produces the full set of artifacts.
[group('build')]
prod-plot-bins:
	cargo build --release --bin roomeq-fuzzer -p autoeq --features plotly
	cargo build --release --bin plot-functions -p math-test-functions --features plotly
	cargo build --release --bin plot-de -p math-optimisation --features plotly

[group('build')]
prod-sotf-player: prod-sotf-tui prod-sotf-gpui

[group('build')]
prod-sotf-gpui:
	cargo build --release --bin sotf-desktop -p sotf-gpui --features onnx

[group('build')]
prod-sotf-tui:
	cargo build --release --bin sotf-tui -p sotf-tui --features "onnx, streaming, hls"

[group('build')]
prod-sotf-recorder:
	cargo build --release --bin sotf-recorder-cli -p app-cli

[group('build')]
prod-roomeq:
	cargo build --release --bin roomeq
	cargo build --release --bin roomeq-fuzzer -p autoeq --features plotly

# ----------------------------------------------------------------------
# DIST — release-cut profile (fat LTO + codegen-units = 1)
# ----------------------------------------------------------------------
# Builds land in `target/dist/` (NOT `target/release/`). Compile time is
# noticeably longer than `prod-*`; only run these for actual release cuts.

# Top-level umbrella — builds everything that ships, including the plot bins.
[group('dist')]
dist: dist-sotf-gpui dist-sotf-tui dist-sotf-recorder dist-roomeq dist-plot-bins

[group('dist')]
dist-sotf-gpui:
	cargo build --profile dist --bin sotf-desktop -p sotf-gpui --features onnx

[group('dist')]
dist-sotf-tui:
	cargo build --profile dist --bin sotf-tui -p sotf-tui --features "onnx, streaming, hls"

[group('dist')]
dist-sotf-recorder:
	cargo build --profile dist --bin sotf-recorder-cli -p app-cli

[group('dist')]
dist-roomeq:
	cargo build --profile dist --bin roomeq
	cargo build --profile dist --bin roomeq-fuzzer -p autoeq --features plotly

# Plotly-gated bins (skipped by `--workspace` because of required-features).
[group('dist')]
dist-plot-bins:
	cargo build --profile dist --bin roomeq-fuzzer -p autoeq --features plotly
	cargo build --profile dist --bin plot-functions -p math-test-functions --features plotly
	cargo build --profile dist --bin plot-de -p math-optimisation --features plotly

# Whole workspace under the dist profile (slow — 10+ minutes typical).
[group('dist')]
dist-workspace: dist-plot-bins
	cargo build --profile dist --workspace

# shortcuts
[group('build')]
[macos]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx, hal, iamf, dev-api, streaming, hls"

[group('build')]
[linux]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx,hal,iamf,streaming,hls"

[group('build')]
[windows]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx,hal,iamf,streaming,hls"

alias terminal := gpui

[group('build')]
gpui:
	cargo run --release --bin sotf-desktop -p sotf-gpui --features "onnx,hal,gpu-2d,gpu-3d,iamf"

alias desktop := gpui
alias native := gpui

# ----------------------------------------------------------------------
# CLEAN
# ----------------------------------------------------------------------

clean:
	cargo clean
	find . -name '*~' -exec rm {} \; -print
	find . -name 'Cargo.lock' -exec rm {} \; -print

# ----------------------------------------------------------------------
# DEV
# ----------------------------------------------------------------------

# Workspace debug build. Also builds sotf-desktop with the `dev-api` feature so
# scripted scenarios (sotf-dev-driver) can drive the running app.
# Release builds (`prod-*`, `run-gpui-release`) intentionally omit `dev-api`.
dev:
	cargo build --workspace
	cargo build -p sotf-gpui --bin sotf-desktop --features "onnx, hal, gpu-2d, gpu-3d, iamf, dev-api"
	cargo build -p sotf-dev-driver
	# Plotly-gated bins are skipped by `cargo build --workspace`; build them.
	cargo build --bin roomeq-fuzzer -p autoeq --features plotly
	cargo build --bin plot-functions -p math-test-functions --features plotly
	cargo build --bin plot-de -p math-optimisation --features plotly

[group('dev')]
systemwide-lab:
	SOTF_SYSTEMWIDE_DRIVER=lab SOTF_SYSTEMWIDE_RUNTIME_DIR="${SOTF_SYSTEMWIDE_RUNTIME_DIR:-/tmp/sotf-systemwide-lab-$USER}" cargo run -p sotf-daemon --bin sotf-daemon --features hal

