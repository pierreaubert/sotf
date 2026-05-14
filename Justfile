# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

_default:
	just --list

import? 'builds/aggregates.just'
import? 'builds/cross.just'
import? 'builds/macos.just'
import? 'builds/windows.just'
import? 'builds/linux.just'
import? 'builds/ios.just'
import? 'builds/tvos.just'

import? 'crates/math-audio/Justfile'
import? 'crates/autoeq/Justfile'
import? 'crates/gpui-toolkit/Justfile'
import? 'crates/sotf-plugins/Justfile'
import? 'crates/sotf-engine/Justfile'
import? 'crates/sotf-plugins/crates/plugins-bridge/Justfile'
import? 'crates/sotf-plugins/crates/plugins-ffi/Justfile'
import? 'crates/sotf-plugins/crates/plugins-nih/Justfile'
import? 'crates/sotf-plugins/crates/plugins-au/Justfile'

# ----------------------------------------------------------------------
# Downloads
# ----------------------------------------------------------------------

[group('download')]
download-sofa:
	mkdir -p data_cached/org.sofacoustics/mit
	wget -O data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa https://sofacoustics.org/data/database/mit/mit_kemar_normal_pinna.sofa
	wget -O data_cached/org.sofacoustics/mit/kemar_large.sofa https://sofacoustics.org/data/database/mit/mit_kemar_large_pinna.sofa

[group('download')]
convert-sofa-to-sqlite:
	@for sofa in data_cached/org.sofacoustics/mit/*.sofa; do \
		hrtfdb="$${sofa%.sofa}.hrtfdb"; \
		if [ ! -f "$$hrtfdb" ]; then \
			echo "Converting $$sofa -> $$hrtfdb"; \
			cargo run --bin sofa-to-sqlite -p sotf-tools --release -- "$$sofa" "$$hrtfdb"; \
		else \
			echo "Skipping $$sofa (already converted)"; \
		fi \
	done

[group('download')]
generate-audio-tests:
	cargo run --bin generate-audio-tests -p sotf-tools --release --no-default-features
	cargo run --bin generate-upmixer-golden -p sotf-tools --release --no-default-features

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

[group('test')]
check:
	RUST_MIN_STACK=16777216 cargo check --workspace  --lib --bins --tests --examples --features="qa, onnx, hal, gpu-2d, gpu-3d"

[group('test')]
test:
	RUST_MIN_STACK=16777216 cargo test --workspace  --lib --bins --tests --examples --features="qa, onnx, hal, gpu-2d, gpu-3d"

[group('test')]
test-negative:
	cargo test -p sotf-gpui --test negative --release

[group('test')]
test-proptest:
	PROPTEST_CASES=10000 cargo test -p sotf-gpui --test proptest_tests  --release

# which have deeply nested GPUI macros that cause stack overflow in syn
[group('test')]
ntest: test-negative test-proptest
	AEQ_E2E_DEVICE='BlackHole 64ch' RUST_MIN_STACK=16777216 cargo nextest run --release --no-fail-fast --workspace --lib --bins --tests --examples --features="qa, onnx, hal, gpu-2d, gpu-3d, iamf"

# ----------------------------------------------------------------------
# LINT
# ----------------------------------------------------------------------

[group('lint')]
lint:
	cargo clippy --all -- -D warnings

# ----------------------------------------------------------------------
# DOC
# ----------------------------------------------------------------------

[group('doc')]
doc:
	cargo doc --all --no-deps

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs).
# Debug builds enable `dev-api`, exposing the scripted-test HTTP endpoint on
# 127.0.0.1:7777 (override via SOTF_DEV_API_PORT). Release builds never include it.
[group('run')]
run-gpui:
	cargo build --bin sotf-desktop --features "onnx, hal, gpu-2d, gpu-3d, iamf, dev-api"
	codesign --force --deep --sign - --entitlements scripts/debug.entitlements target/debug/sotf-desktop
	./target/debug/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-release:
	cargo build --release --bin sotf-desktop --features "onnx, hal, gpu-2d, gpu-3d, iamf"
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo build --release --bin sotf-desktop --features "onnx, hal, gpu-2d, gpu-3d, iamf"
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the TUI player
[group('run')]
[macos]
run-tui:
	cargo run --release --bin sotf-tui --features onnx,hal

[group('run')]
[linux]
run-tui:
	cargo run --release --bin sotf-tui --features onnx

[group('run')]
[windows]
run-tui:
	cargo run --release --bin sotf-tui --features onnx

# Run the TUI player (with debug info for leak detection)
[group('run')]
[macos]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui --features onnx,hal

[group('run')]
[linux]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui --features onnx

[group('run')]
[windows]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui --features onnx

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
	cargo build --release --bin sotf-tui -p sotf-tui --features onnx

[group('build')]
prod-sotf-recorder:
	cargo build --release --bin sotf-recorder-cli -p app-cli

[group('build')]
prod-roomeq:
	cargo build --release --bin roomeq
	cargo build --release --bin roomeq-fuzzer -p autoeq --features plotly

# shortcuts
[group('build')]
[macos]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx, hal, iamf"

[group('build')]
[linux]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx,hal,iamf"

[group('build')]
[windows]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui --features="onnx,hal,iamf"

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

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

[group('install')]
update: update-rust update-pre-commit

[group('install')]
update-rust:
	rustup update
	cargo update

[group('install')]
update-pre-commit:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# Install rustup
# ----------------------------------------------------------------------

[group('install')]
install-rustup:
	curl https://sh.rustup.rs -sSf > ./scripts/install-rustup
	chmod +x ./scripts/install-rustup
	./scripts/install-rustup -y
	~/.cargo/bin/rustup default stable
	~/.cargo/bin/cargo install just
	~/.cargo/bin/cargo install cargo-wizard
	~/.cargo/bin/cargo install cargo-llvm-cov
	~/.cargo/bin/cargo install cross
	~/.cargo/bin/cargo install cargo-binstall
	~/.cargo/bin/cargo binstall cargo-nextest --secure
	~/.cargo/bin/cargo install samply
	~/.cargo/bin/cargo install cargo-insta

# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

[group('install')]
post-install: post-install-rust post-install-python

[group('install')]
post-install-rust:
	$HOME/.cargo/bin/rustup default stable
	$HOME/.cargo/bin/cargo install just
	$HOME/.cargo/bin/cargo install cargo-wizard
	$HOME/.cargo/bin/cargo install cargo-vcpkg
	$HOME/.cargo/bin/cargo install cargo-llvm-cov
	$HOME/.cargo/bin/cargo install cross
	$HOME/.cargo/bin/cargo install cargo-binstall
	$HOME/.cargo/bin/cargo binstall cargo-nextest --secure
	$HOME/.cargo/bin/cargo check

[group('install')]
post-install-python:
	python3 -m venv venv
	./venv/bin/pip install -U pip
	./venv/bin/pip install -r ./scripts/requirements.txt

