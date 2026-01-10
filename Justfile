# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

# should be done automatically
dyld_fallback_library_path := '/Applications/Xcode.app/Contents/Framework'

# opencv
opencv_haarcascades_path := '/opt/homebrew/Cellar/opencv/4.12.0_15/share/opencv4/haarcascades'

default:
	just --list

# ----------------------------------------------------------------------
# Downloads
# ----------------------------------------------------------------------

download-once: download-sofa download-world-atlas generate-audio-tests

download-sofa:
	mkdir -p data_cached/org.sofacoustics/mit
	wget -O data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa https://sofacoustics.org/data/database/mit/mit_kemar_normal_pinna.sofa
	wget -O data_cached/org.sofacoustics/mit/kemar_large.sofa https://sofacoustics.org/data/database/mit/mit_kemar_large_pinna.sofa

download-world-atlas:
	wget -q -O gpui-d3rs/bin/showcase/data/land-50m.json https://cdn.jsdelivr.net/npm/world-atlas@2/land-50m.json

convert-sofa-to-sqlite:
	@for sofa in data_cached/org.sofacoustics/mit/*.sofa; do \
		hrtfdb="$${sofa%.sofa}.hrtfdb"; \
		if [ ! -f "$$hrtfdb" ]; then \
			echo "Converting $$sofa -> $$hrtfdb"; \
			cargo run --bin sofa_to_sqlite -p sotf-audio-plugins --features=sofa_support --release -- "$$sofa" "$$hrtfdb"; \
		else \
			echo "Skipping $$sofa (already converted)"; \
		fi \
	done

generate-audio-tests: prod-generate-audio-tests
	cargo run --bin generate-audio-tests --release

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test:
	# Exclude GPUI crates from check - they cause stack overflow in syn during test/example mode compilation
	RUST_MIN_STACK=16777216 cargo check --workspace --all-targets

# Build gpui-ui-kit examples to verify they compile (doesn't run them)
test-examples:
	@echo "Building gpui-ui-kit examples..."
	RUST_MIN_STACK=16777216 cargo build --examples -p gpui-ui-kit
	@echo "✓ All gpui-ui-kit examples compiled successfully"

test-negative:
	cargo test -p sotf-gpui --test negative

test-proptest:
	PROPTEST_CASES=10000 cargo test -p sotf-gpui --test proptest_tests

# Note: --lib is intentionally omitted to respect `test = false` in crates like sotf-gpui
# which have deeply nested GPUI macros that cause stack overflow in syn
ntest:
    RUST_MIN_STACK=16777216 cargo nextest run --release --no-fail-fast --workspace

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs)
run-gpui:
	cargo build --bin SotF
	codesign --force --deep --sign - --entitlements sotf-audio-player/macos/debug.entitlements target/debug/SotF
	./target/debug/SotF

# Run the GPUI player (release mode)
run-gpui-release:
	cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements sotf-audio-player/macos/entitlements.plist target/release/SotF
	./target/release/SotF

# Run the GPUI player (release mode)
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements sotf-audio-player/macos/entitlements.plist target/release/SotF
	./target/release/SotF

# Run the TUI player
run-tui:
	cargo run --release --bin sotf-tui

# Run the TUI player
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

prod: prod-workspace prod-sotf-player prod-sotf-recorder prod-generate-audio-tests

prod-generate-audio-tests:
	cargo build --release --bin generate-audio-tests -p sotf-audio-engine

prod-workspace:
	cargo build --release --workspace

prod-sotf-player: prod-sotf-tui prod-sotf-gpui
	cargo build --release --bin sotf-player

prod-sotf-gpui:
	cargo build --release --bin SotF -p sotf-gpui

prod-sotf-tui:
	cargo build --release --bin sotf-tui -p sotf-tui

prod-sotf-recorder:
	cargo build --release --bin sotf-recorder

prod-hal:
	cargo build --release -p soft-hal

# shortcuts
tui:
	cargo run --release --bin sotf-tui -p sotf-tui

gpui:
	cargo run --release --bin SotF -p sotf-gpui

gpui-release-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	sh -x ./sotf-audio-player/macos/build-dmg.sh --sign --notarize

gpui-release-windows:
	echo "cd sotf-audio-player/windows and launch the bat script"

# ----------------------------------------------------------------------
# AUDIO UNIT (macOS only)
# ----------------------------------------------------------------------

# Build Rust FFI library for Audio Units
build-au-rust:
	#!/usr/bin/env bash
	set -euxo pipefail
	# Build for both architectures
	cargo build --release -p sotf-audio-plugins-ffi --target x86_64-apple-darwin
	cargo build --release -p sotf-audio-plugins-ffi --target aarch64-apple-darwin
	cargo build --release -p gpui-au --target x86_64-apple-darwin
	cargo build --release -p gpui-au --target aarch64-apple-darwin
	# Create universal binaries
	mkdir -p sotf-audio-plugins/src-au/Resources
	lipo -create \
		target/x86_64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		target/aarch64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		-output sotf-audio-plugins/src-au/Resources/libsotf_audio_plugins_ffi.a
	lipo -create \
		target/x86_64-apple-darwin/release/libgpui_au.a \
		target/aarch64-apple-darwin/release/libgpui_au.a \
		-output sotf-audio-plugins/src-au/Resources/libgpui_au.a
	# Copy header files
	cp sotf-audio-plugins/src-ffi/sotf_audio_plugin_ffi.h sotf-audio-plugins/src-au/Shared/
	cp gpui-au/GPUIBridge.h sotf-audio-plugins/src-au/Shared/
	echo "✅ Universal Rust FFI libraries created"

# Build Audio Unit plugins in Xcode
build-au-swift: build-au-rust
	#!/usr/bin/env bash
	set -euxo pipefail
	cd sotf-audio-plugins/src-au
	# Generate Xcode project with XcodeGen
	if [ ! -d "SOTFAudioUnits.xcodeproj" ] || [ "project.yml" -nt "SOTFAudioUnits.xcodeproj/project.pbxproj" ]; then
		echo "🔨 Generating Xcode project with XcodeGen..."
		xcodegen generate
	fi
	# Build the Audio Unit
	xcodebuild -project SOTFAudioUnits.xcodeproj \
		-scheme EQAudioUnit \
		-configuration Release \
		build
	echo "✅ Audio Unit built successfully"

# Install Audio Units to system
install-au: build-au-rust build-au-swift
	#!/usr/bin/env bash
	set -euxo pipefail
	# Find the Xcode DerivedData build output - need the container .app
	XCODE_APP=$(find ~/Library/Developer/Xcode/DerivedData/SOTFAudioUnits-*/Build/Products/Release/SOTFAudioUnits.app -maxdepth 0 2>/dev/null | head -1)
	if [ -n "$XCODE_APP" ] && [ -d "$XCODE_APP" ]; then
		# Copy the entire app to Applications (AUv3 extensions require this)
		rm -rf ~/Applications/SOTFAudioUnits.app
		mkdir -p ~/Applications
		cp -r "$XCODE_APP" ~/Applications/
		echo "✅ SOTF Audio Units app installed to ~/Applications/"
		echo ""
		echo "IMPORTANT: You must launch ~/Applications/SOTFAudioUnits.app once to register the AU"
		echo "           Then it will be available in DAWs as 'SOTF: Parametric EQ'"
		echo ""
	else
		echo "⚠️  No Audio Unit build found in Xcode DerivedData"
		echo "    Run 'just build-au-swift' first"
		exit 1
	fi
	# Restart Audio Component registration
	killall -9 AudioComponentRegistrar coreaudiod 2>/dev/null || true

# Validate Audio Unit
validate-au:
	#!/usr/bin/env bash
	set -euxo pipefail
	echo "Validating SOTF EQ Audio Unit..."
	auval -v aufx SOEQ SOTF

# Complete AU build pipeline
build-au: build-au-rust build-au-swift
	echo "✅ Complete Audio Unit build finished"

# ----------------------------------------------------------------------
# BENCH
# ----------------------------------------------------------------------

bench:
	cargo run --release --bin binaural-decoder-benchmark
	cargo run --release --bin upmixer-benchmark
	cargo run --release --bin compressor-benchmark

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

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: update-rust update-pre-commit

update-rust:
	rustup update
	cargo update

update-pre-commit:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: demo-d3rs demo-px demo-ui-kit

demo-ui-kit:
	cargo run --release --example showcase -p gpui-ui-kit

demo-plot-functions:
	cargo run --release --bin plot-functions

demo-d3rs:
	cargo run --release --bin d3rs-showcase --features="gpui"
	cargo run --release --bin d3rs-spinorama --features="spinorama"

demo-px:
	cargo run --release --bin px-showcase

# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

cross : cross-macos-arm-2-linux-x86

# Debug: Build Docker image and open interactive shell
cross-debug-x86 :
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell. Try: cargo build --release --target x86_64-unknown-linux-gnu"
	docker run -it --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 /bin/bash

cross-macos-arm-2-linux-x86 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-x86 .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 \
		cargo build --release --target x86_64-unknown-linux-gnu
	@echo "Done! Binary at: target/x86_64-unknown-linux-gnu/release/autoeq"

cross-macos-arm-2-linux-arm64 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-arm64 -f ./builds/from_macos_arm/Dockerfile.aarch64-unknown-linux-musl .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-arm64 \
		cargo build --release --target aarch64-unknown-linux-musl
	@echo "Done! Binary at: target/aarch64-unknown-linux-musl/release/"

cross-macos-arm-2-win-x86-gnu :
	echo "This is not supported yet"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-gnu

cross-macos-arm-2-win-x86-msvc :
	echo "This can take minutes!"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc

cross-macos-arm-2-win-arm-gnu :
	echo "This is not supported!"

cross-macos-arm-2-win-arm-msvc :
	echo "This is not supported!"

# ----------------------------------------------------------------------
# STATIC BINARY CROSS-COMPILATION
# ----------------------------------------------------------------------

# Build static binaries for all platforms
cross-static-all: cross-static-linux-x86 cross-static-linux-arm64 cross-static-windows-x86 cross-static-macos

# Linux x86_64 static binary (musl)
cross-static-linux-x86:
	@echo "Building static Linux x86_64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-unknown-linux-musl --bin sotf-tui
	@echo "Done! Binary at: target/x86_64-unknown-linux-musl/release/sotf_player_tui"

# Linux ARM64 static binary (musl)
cross-static-linux-arm64:
	@echo "Building static Linux ARM64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target aarch64-unknown-linux-musl --bin sotf-tui
	@echo "Done! Binary at: target/aarch64-unknown-linux-musl/release/sotf_player_tui"

# Windows x86_64 static binary (MSVC with static CRT)
cross-static-windows-x86:
	@echo "Building static Windows x86_64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc --bin sotf-tui
	@echo "Done! Binary at: target/x86_64-pc-windows-msvc/release/sotf_player_tui.exe"

# macOS universal binary (NOT fully static - limited by macOS system restrictions)
# Apple requires dynamic linking to system frameworks (CoreAudio, CoreFoundation, etc.)
# This creates a universal binary supporting both Intel (x86_64) and Apple Silicon (ARM64)
cross-static-macos:
	@echo "Building macOS binaries..."
	@echo "Note: macOS binaries cannot be fully static due to Apple's security policies"
	@echo "      System frameworks (CoreAudio, etc.) will be dynamically linked"
	RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target aarch64-apple-darwin --bin sotf-tui
	RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target aarch64-apple-darwin --bin SotF -p sotf-gpui
	@echo "✓ Done! Universal binary at: target/sotf-tui-macos-universal"

# Build static binary for current platform
# Note: Requires bash/sh shell. On Linux, builds musl static binary.
# On macOS, builds regular binary (static linking limited by Apple).
build-static-local:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Building static binary for current platform..."
	if [ "$(uname)" = "Linux" ]; then
		if [ "$(uname -m)" = "x86_64" ]; then
			cargo build --release --target x86_64-unknown-linux-musl --bin sotf-tui
			echo "✓ Built: target/x86_64-unknown-linux-musl/release/sotf-tui"
		elif [ "$(uname -m)" = "aarch64" ]; then
			cargo build --release --target aarch64-unknown-linux-musl --bin sotf-tui
			echo "✓ Built: target/aarch64-unknown-linux-musl/release/sotf-tui"
		else
			echo "❌ Unsupported Linux architecture: $(uname -m)"
			exit 1
		fi
	elif [ "$(uname)" = "Darwin" ]; then
		RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --bin sotf-tui --target-dir ./target-static
		echo "✓ Built: target-static/release/sotf-tui"
		RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --bin SotF -p sotf-gpui --target-dir ./target-static
		echo "✓ Built: target-static/release/SotF"
		echo "Note: macOS binaries have limited static linking due to Apple restrictions"
	else
		echo "❌ Unsupported platform: $(uname)"
		exit 1
	fi

# ----------------------------------------------------------------------
# Install rustup
# ----------------------------------------------------------------------

install-rustup:
	curl https://sh.rustup.rs -sSf > ./scripts/install-rustup
	chmod +x ./scripts/install-rustup
	./scripts/install-rustup -y
	~/.cargo/bin/rustup default stable
	~/.cargo/bin/cargo install just

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

install-macos-cross:
	# use git version until 0.2.6 is out
	cargo install cross --git https://github.com/cross-rs/cross
	cross target add x86_64-apple-ios

install-macos-brew:
	curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh > ./scripts/install-brew
	chmod +x ./scripts/install-brew
	NONINTERACTIVE=1 ./scripts/install-brew

install-macos: install-macos-brew install-rustup
	# need xcode
	xcode-select --install
	# need metal
	xcodebuild -downloadComponent MetalToolchain
	# chromedriver sheanigans
	brew install chromedriver
	xattr -d com.apple.quarantine $(which chromedriver)
	# optimisation library
	brew install nlopt cmake netcdf opencv chafa


# ----------------------------------------------------------------------
# Install linux
# ----------------------------------------------------------------------

install-linux-root:
	sudo apt update && sudo apt -y install \
	   perl curl build-essential gcc g++ pkg-config cmake ninja-build gfortran \
	   libssl-dev \
	   ca-certificates \
	   patchelf libopenblas-dev gfortran \
	   chromium-browser chromium-chromedriver

install-linux: install-linux-root install-rustup

install-ubuntu-common:
		sudo apt install -y \
			 curl \
			 build-essential gcc g++ \
			 pkg-config \
			 libssl-dev \
			 ca-certificates \
			 cmake \
			 ninja-build \
			 perl \
			 libglib2.0-dev \
			 libxkbcommon-x11-dev \
			 libgtk-3-dev \
			 libwebkit2gtk-4.1-dev \
			 libayatana-appindicator3-dev \
			 librsvg2-dev \
			 patchelf \
			 libopenblas-dev \
			 gfortran \
			 libasound2-dev \
			 libnetcdf-dev \
			 libopencv-dev \
			 libclang-dev \
			 webkit2gtk-driver

install-ubuntu-x86-driver :
		sudo apt install -y \
			 chromium-browser \
			 chromium-chromedriver

install-ubuntu-arm64-driver :
		sudo apt install -y firefox
		# where is the geckodriver ?

install-ubuntu-x86: install-ubuntu-common install-ubuntu-x86-driver

install-ubuntu-arm64: install-ubuntu-common install-ubuntu-arm64-driver

# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

post-install:
	$HOME/.cargo/bin/rustup default stable
	$HOME/.cargo/bin/cargo install just
	$HOME/.cargo/bin/cargo install cargo-wizard
	$HOME/.cargo/bin/cargo install cargo-vcpkg
	$HOME/.cargo/bin/cargo install cargo-llvm-cov
	$HOME/.cargo/bin/cargo install cross
	$HOME/.cargo/bin/cargo install cargo-binstall
	$HOME/.cargo/bin/cargo binstall cargo-nextest --secure
	$HOME/.cargo/bin/cargo check

# ----------------------------------------------------------------------
# SIGNING
# ----------------------------------------------------------------------



