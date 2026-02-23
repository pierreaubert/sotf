# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------
import? 'builds/cross.just'

import? 'builds/macos.just'
import? 'builds/windows.just'
import? 'builds/linux.just'

import? 'crates/math-audio/Justfile'
import? 'crates/autoeq/Justfile'
import? 'crates/plugins/Justfile'

default:
	just --list

# ----------------------------------------------------------------------
# Downloads
# ----------------------------------------------------------------------

[group('download')]
download-once: download-sofa download-speakers generate-audio-tests generate-roomeq-tests

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
			cargo run --bin sofa-to-sqlite -p tools --features=sofa_support --release -- "$$sofa" "$$hrtfdb"; \
		else \
			echo "Skipping $$sofa (already converted)"; \
		fi \
	done

[group('download')]
generate-audio-tests:
	cargo run --bin generate-audio-tests -p tools --release --no-default-features
	cargo run --bin generate-upmixer-golden -p tools --release --no-default-features

[group('download')]
generate-roomeq-tests: generate-roomeq-tests-bem generate-roomeq-tests-fem

[group('download')]
generate-roomeq-tests-bem:
	cargo run --bin generate-roomeq-data --release -- --solver bem --output-dir data_tests/roomeq/generated

[group('download')]
generate-roomeq-tests-fem:
	cargo run --bin generate-roomeq-data --release -- --solver fem --output-dir data_tests/roomeq/generated

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

[group('test')]
test:
	# Exclude GPUI crates - they cause stack overflow in syn during test/example mode compilation
	RUST_MIN_STACK=16777216 cargo check --workspace --all-targets --exclude sotf-gpui
	cargo test --workspace --lib --exclude sotf-gpui --exclude gpui-px

# Build gpui-ui-kit examples to verify they compile (doesn't run them)
[group('test')]
test-examples:
	@echo "Building gpui-ui-kit examples..."
	RUST_MIN_STACK=16777216 cargo build --examples -p gpui-ui-kit
	@echo "✓ All gpui-ui-kit examples compiled successfully"

[group('test')]
test-negative:
	cargo test -p sotf-gpui --test negative

[group('test')]
test-proptest:
	PROPTEST_CASES=10000 cargo test -p sotf-gpui --test proptest_tests

# Note: --lib is intentionally omitted to respect `test = false` in crates like sotf-gpui
# which have deeply nested GPUI macros that cause stack overflow in syn
[group('test')]
ntest: test-negative test-proptest
	RUST_MIN_STACK=16777216 cargo nextest run --release --no-fail-fast --workspace --lib

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs)
[group('run')]
run-gpui:
	cargo build --bin SotF
	codesign --force --deep --sign - --entitlements scripts/debug.entitlements target/debug/SotF
	./target/debug/SotF

# Run the GPUI player (release mode)
[group('run')]
run-gpui-release:
	cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/SotF
	./target/release/SotF

# Run the GPUI player (release mode)
[group('run')]
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/SotF
	./target/release/SotF

# Run the TUI player
[group('run')]
run-tui:
	cargo run --release --bin sotf-tui

# Run the TUI player
[group('run')]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo run --release --bin sotf-tui

# ----------------------------------------------------------------------
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt:
	cargo fmt --all

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

alias build := prod

[group('build')]
prod: prod-workspace prod-sotf-player prod-sotf-recorder prod-generate-audio-tests prod-autoeq prod-roomeq prod-math

[group('build')]
prod-generate-audio-tests:
	cargo build --release --bin generate-audio-tests -p tools

[group('build')]
prod-workspace:
	cargo build --release --workspace

[group('build')]
prod-sotf-player: prod-sotf-tui prod-sotf-gpui

[group('build')]
prod-sotf-gpui:
	cargo build --release --bin SotF -p sotf-gpui

[group('build')]
prod-sotf-tui:
	cargo build --release --bin sotf-tui -p sotf-tui

[group('build')]
prod-sotf-recorder:
	cargo build --release --bin sotf-recorder-cli -p app-cli

[group('build')]
prod-roomeq:
	cargo build --release --bin roomeq
	cargo build --release --bin roomeq-fuzzer

# shortcuts
[group('build')]
tui:
	cargo run --release --bin sotf-tui -p sotf-tui

[group('build')]
gpui:
	cargo run --release --bin SotF -p sotf-gpui

# ----------------------------------------------------------------------
# BENCH
# ----------------------------------------------------------------------

[group('bench')]
bench: bench-plugins bench-autoeq bench-math

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

dev:
	cargo build --workspace
	cargo build --bin autoeq
	cargo build --bin plot-functions
	cargo build --bin benchmark-convergence
	cargo build --bin benchmark-autoeq-speaker

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
# DEMO
# ----------------------------------------------------------------------

[group('demo')]
demo: demo-d3rs demo-px demo-ui-kit demo-headphone-loss

[group('demo')]
demo-ui-kit:
	cargo build --release --example showcase -p gpui-ui-kit

[group('demo')]
demo-d3rs:
	cargo build --release --bin d3rs-spinorama --features="spinorama, gpu-3d"

[group('demo')]
demo-px:
	cargo build --release --bin px-spinorama -p gpui-px --features="autoeq, tokio, reqwest, urlencoding, gpu-3d"

[group('demo')]
demo-headphone-loss:
	cargo run --release --example headphone_loss_demo -- \
	--spl "./data_tests/headphones/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" \
	--target "./data_tests/targets/harman-over-ear-2018.csv"

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

[group('examples')]
examples: examples-autoeq examples-math

[group('examples')]
examples-math: examples-iir examples-de examples-testfunctions

[group('examples')]
examples-iir :
	cargo run --release --example format_demo
	cargo run --release --example readme_example

[group('examples')]
examples-de :
	cargo run --release --example optde_basic
	cargo run --release --example optde_adaptive_demo
	cargo run --release --example optde_linear_constraints
	cargo run --release --example optde_nonlinear_constraints
	cargo run --release --example optde_parallel

[group('examples')]
examples-testfunctions:
	cargo run --release --example test_hartman_4d

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

# ----------------------------------------------------------------------
# PUBLISH
# ----------------------------------------------------------------------

[group('publish')]
publish: publish-autoeq publish-math

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

[group('qa')]
qa: qa-autoeq qa-math qa-plugins qa-roomeq

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

