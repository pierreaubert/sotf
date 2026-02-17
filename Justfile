# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

# macos specific
HDF5_DIR := '/opt/homebrew/Cellar/hdf5/2.0'

# should be done automatically
dyld_fallback_library_path := '/Applications/Xcode.app/Contents/Framework'

default:
	just --list

# ----------------------------------------------------------------------
# Downloads
# ----------------------------------------------------------------------

download-once: download-sofa download-speakers generate-audio-tests generate-roomeq-tests

download-sofa:
	mkdir -p data_cached/org.sofacoustics/mit
	wget -O data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa https://sofacoustics.org/data/database/mit/mit_kemar_normal_pinna.sofa
	wget -O data_cached/org.sofacoustics/mit/kemar_large.sofa https://sofacoustics.org/data/database/mit/mit_kemar_large_pinna.sofa

download-speakers:
	cargo run --bin autoeq-download-speakers --release

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

generate-audio-tests: prod-generate-audio-tests
	cargo run --bin generate-audio-tests -p tools --release --no-default-features

generate-roomeq-tests: generate-roomeq-tests-bem generate-roomeq-tests-fem

generate-roomeq-tests-bem:
	cargo run --bin generate-roomeq-data --release -- --solver bem --output-dir data_tests/roomeq/generated

generate-roomeq-tests-fem:
	cargo run --bin generate-roomeq-data --release -- --solver fem --output-dir data_tests/roomeq/generated

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

test:
	# Exclude GPUI crates - they cause stack overflow in syn during test/example mode compilation
	RUST_MIN_STACK=16777216 cargo check --workspace --all-targets --exclude sotf-gpui
	cargo test --workspace --lib --exclude sotf-gpui --exclude gpui-px

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
ntest: test-negative test-proptest
	HDF5_DIR=$HDF5_DIR RUST_MIN_STACK=16777216 cargo nextest run --release --no-fail-fast --workspace --lib

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs)
run-gpui:
	cargo build --bin SotF
	codesign --force --deep --sign - --entitlements scripts/debug.entitlements target/debug/SotF
	./target/debug/SotF

# Run the GPUI player (release mode)
run-gpui-release:
	cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/SotF
	./target/release/SotF

# Run the GPUI player (release mode)
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" cargo build --release --bin SotF
	codesign --force --deep --sign - --entitlements scripts/entitlements.plist target/release/SotF
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

prod: prod-workspace prod-sotf-player prod-sotf-recorder prod-generate-audio-tests prod-autoeq prod-roomeq prod-math

prod-generate-audio-tests:
	cargo build --release --bin generate-audio-tests -p tools

prod-workspace:
	cargo build --release --workspace

prod-sotf-player: prod-sotf-tui prod-sotf-gpui

prod-sotf-gpui:
	cargo build --release --bin SotF -p sotf-gpui

prod-sotf-tui:
	cargo build --release --bin sotf-tui -p sotf-tui

prod-sotf-recorder:
	cargo build --release --bin sotf-recorder-cli -p app-cli

prod-autoeq:
	cargo build --release --bin autoeq
	cargo build --release --bin benchmark-autoeq-speaker

prod-roomeq:
	cargo build --release --bin roomeq
	cargo build --release --bin roomeq-fuzzer

prod-math:
	cargo build --release --bin plot_functions
	cargo build --release --bin plot-autoeq-de
	cargo build --release --bin run-autoeq-de

prod-hal:
	cargo build --release -p driver-hal

# Build the sotf-daemon binary (macOS with HAL support)
prod-daemon:
	cargo build --release -p sotf-daemon --features hal

# Build the ConfigBar/Toolbar Swift app
prod-toolbar:
	#!/usr/bin/env bash
	set -euo pipefail
	CONFIGBAR_DIR="crates/daemon/configbar"
	BUILD_DIR="target/release"
	mkdir -p "$BUILD_DIR"
	echo "Building ConfigBar/Toolbar..."
	swiftc \
		-o "$BUILD_DIR/sotf-toolbar" \
		"$CONFIGBAR_DIR"/src/*.swift \
		-framework SwiftUI \
		-framework WebKit \
		-framework UserNotifications \
		-framework CoreAudio \
		-O
	echo "✅ Toolbar built: $BUILD_DIR/sotf-toolbar"

# Build the Swift HAL driver bundle
prod-hal-driver:
	#!/usr/bin/env bash
	set -euo pipefail
	HAL_SWIFT_DIR="crates/driver-hal/swift"
	BUILD_DIR="target/release/SotFHAL.driver"
	mkdir -p "$BUILD_DIR/Contents/MacOS"
	mkdir -p "$BUILD_DIR/Contents/Resources"
	echo "Building Swift HAL driver..."
	swiftc \
		-emit-library \
		-o "$BUILD_DIR/Contents/MacOS/SotFHAL" \
		-module-name SotFHAL \
		-import-objc-header "$HAL_SWIFT_DIR/Sources/BridgingHeader.h" \
		-Xlinker -bundle \
		-Xlinker -rpath -Xlinker @loader_path/../Frameworks \
		-framework CoreAudio \
		-framework CoreFoundation \
		-framework Foundation \
		-O \
		"$HAL_SWIFT_DIR/Sources/Timing.swift" \
		"$HAL_SWIFT_DIR/Sources/RingBuffer.swift" \
		"$HAL_SWIFT_DIR/Sources/SharedMemory.swift" \
		"$HAL_SWIFT_DIR/Sources/Encryption.swift" \
		"$HAL_SWIFT_DIR/Sources/SotFHALDriver.swift"
	cp "$HAL_SWIFT_DIR/Info.plist" "$BUILD_DIR/Contents/Info.plist"
	chmod 755 "$BUILD_DIR/Contents/MacOS/SotFHAL"
	chmod 644 "$BUILD_DIR/Contents/Info.plist"
	# Sign the driver bundle with hardened runtime
	echo "Signing HAL driver..."
	if [ -n "${INSTALLER_DEVELOPER_ID:-}" ]; then
		codesign --force --sign "$INSTALLER_DEVELOPER_ID" \
			--options runtime \
			--timestamp \
			--deep \
			"$BUILD_DIR"
	else
		echo "INSTALLER_DEVELOPER_ID not set, using ad-hoc signing"
		codesign --force --deep --sign - "$BUILD_DIR"
	fi
	echo "✅ HAL driver built and signed: $BUILD_DIR"

# Build all macOS daemon components (daemon + toolbar + HAL driver)
prod-macos-daemon: prod-daemon prod-toolbar prod-hal-driver
	@echo "✅ All macOS daemon components built"

# shortcuts
tui:
	cargo run --release --bin sotf-tui -p sotf-tui

gpui:
	cargo run --release --bin SotF -p sotf-gpui

gpui-release-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	sh -x ./scripts/build-dmg-sotf.sh --sign --notarize

gpui-release-windows:
	echo "cd scripts and launch build-windows.bat or build-windows.ps1"

# ----------------------------------------------------------------------
# AUDIO UNIT (macOS only)
# ----------------------------------------------------------------------

# Build Rust FFI library for Audio Units
build-au-rust:
	#!/usr/bin/env bash
	set -euxo pipefail
	# Build for both architectures
	cargo build --release -p plugins-ffi --target x86_64-apple-darwin
	cargo build --release -p plugins-ffi --target aarch64-apple-darwin
	cargo build --release -p gpui-au --target x86_64-apple-darwin
	cargo build --release -p gpui-au --target aarch64-apple-darwin
	# Create universal binaries
	mkdir -p crates/plugins-au/Resources
	lipo -create \
		target/x86_64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		target/aarch64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		-output crates/plugins-au/Resources/libsotf_audio_plugins_ffi.a
	lipo -create \
		target/x86_64-apple-darwin/release/libgpui_au.a \
		target/aarch64-apple-darwin/release/libgpui_au.a \
		-output crates/plugins-au/Resources/libgpui_au.a
	# Copy header files
	cp crates/plugins-ffi/sotf_audio_plugin_ffi.h crates/plugins-au/Shared/
	cp crates/plugins-gpui/GPUIBridge.h crates/plugins-au/Shared/
	echo "✅ Universal Rust FFI libraries created"

# Build Audio Unit plugins in Xcode
build-au-swift: build-au-rust
	#!/usr/bin/env bash
	set -euxo pipefail
	cd crates/plugins-au
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

bench: bench-plugins bench-autoeq bench-math

bench-plugins:
	cargo bench -p plugins --bench binaural-decoder-benchmark
	cargo bench -p plugins --bench upmixer-benchmark
	cargo bench -p plugins --bench compressor-benchmark

bench-autoeq: bench-convergence bench-autoeq-speaker

bench-convergence:
	cargo run --release --bin benchmark-convergence

bench-autoeq-speaker:
	# either jobs=1 or --no-parallel ; or a mix if you have a lot of
	# CPU cores
	cargo run --release --bin benchmark-autoeq-speaker -- --qa --jobs 1

bench-math:
	cargo run --release --bin benchmark-convergence

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

update: update-rust update-pre-commit

update-rust:
	rustup update
	cargo update

update-pre-commit:
	pre-commit autoupdate

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: demo-d3rs demo-px demo-ui-kit demo-headphone-loss

demo-ui-kit:
	cargo build --release --example showcase -p gpui-ui-kit

demo-d3rs:
	cargo build --release --bin d3rs-spinorama --features="spinorama, gpu-3d"

demo-px:
	cargo build --release --bin px-spinorama -p gpui-px --features="autoeq, tokio, reqwest, urlencoding, gpu-3d"

demo-headphone-loss:
	cargo run --release --example headphone_loss_demo -- \
	--spl "./data_tests/headphones/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" \
	--target "./data_tests/targets/harman-over-ear-2018.csv"

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

examples: examples-autoeq examples-math

examples-autoeq:
	cargo run --release --example headphone_loss_validation

examples-math: examples-iir examples-de examples-testfunctions

examples-iir :
	cargo run --release --example format_demo
	cargo run --release --example readme_example

examples-de :
	cargo run --release --example optde_basic
	cargo run --release --example optde_adaptive_demo
	cargo run --release --example optde_linear_constraints
	cargo run --release --example optde_nonlinear_constraints
	cargo run --release --example optde_parallel

examples-testfunctions:
	cargo run --release --example test_hartman_4d

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
	~/.cargo/bin/cargo install cargo-wizard
	~/.cargo/bin/cargo install cargo-llvm-cov
	~/.cargo/bin/cargo install cross
	~/.cargo/bin/cargo install cargo-binstall
	~/.cargo/bin/cargo binstall cargo-nextest --secure

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
# PUBLISH
# ----------------------------------------------------------------------

publish: publish-autoeq publish-math

publish-autoeq:
	cd crates/autoeq-cea2034 && cargo publish
	cd crates/autoeq && cargo publish
	cd crates/autoeq-roomsim && cargo publish

publish-math:
	cd crates/math-test-functions && cargo publish
	cd crates/math-differential-evolution && cargo publish
	cd crates/math-iir-fir && cargo publish
	cd crates/math-solvers && cargo publish
	cd crates/math-wave && cargo publish
	cd crates/math-convex-hull && cargo publish
	cd crates/math-xem-common && cargo publish
	cd crates/math-bem && cargo publish
	cd crates/math-fem && cargo publish

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

qa: qa-autoeq qa-math qa-plugins qa-roomeq

qa-autoeq: prod-autoeq \
	qa-ascilab-6b \
	qa-jbl-m2-flat qa-jbl-m2-score \
	qa-beyerdynamic-dt1990pro \
	qa-edifierw830nb

qa-ascilab-6b:
	./target/release/autoeq --speaker="AsciLab F6B" --version asr --measurement CEA2034 \
	--algo autoeq:de --loss speaker-score -n 7 --min-freq=30 --max-q=6 \
	--qa 0.5

qa-jbl-m2-flat:
	./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
	--algo autoeq:de --loss speaker-flat -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk \
	--qa 0.5

qa-jbl-m2-score:
	./target/release/autoeq --speaker="JBL M2" --version eac --measurement CEA2034 \
	--algo autoeq:de --loss speaker-score -n 7 --min-freq=20 --max-q=6 --peq-model hp-pk \
	--qa 0.5

qa-beyerdynamic-dt1990pro: qa-beyerdynamic-dt1990pro-flat qa-beyerdynamic-dt1990pro-score	qa-beyerdynamic-dt1990pro-score2

qa-beyerdynamic-dt1990pro-score:
	./target/release/autoeq -n 5 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv --loss headphone-score  \
	--qa 3.0

qa-beyerdynamic-dt1990pro-score2:
	./target/release/autoeq -n 7 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--loss headphone-score	--max-db 6 --max-q 6 --algo mh:rga --maxeval 20000 --min-freq=20 --max-freq 10000 --peq-model hp-pk-lp --min-q 0.6 --min-db 0.25 \
	--qa 1.5

qa-beyerdynamic-dt1990pro-flat:
	./target/release/autoeq -n 5 \
	--curve ./data_tests/headphones/asr/beyerdynamic_dt1990pro/Beyerdynamic\ DT1990\ Pro\ Headphone\ Frequency\ Response\ Measurement.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--loss headphone-flat  --max-db 6 --max-q 6 --maxeval 20000 --algo mh:pso --min-freq=20 --max-freq 10000 --peq-model pk \
	--qa 0.5

qa-edifierw830nb: qa-edifierw830nb-autoeqde qa-edifierw830nb-mhrga qa-edifierw830nb-mhfirefly

qa-edifierw830nb-autoeqde:
	./target/release/autoeq -n 9 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score
	--min-spacing-oct 0.08 \
	--algo autoeq:de --population 70 --maxeval 8000 --seed 42 \
	--qa 14.0

qa-edifierw830nb-mhrga:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.04 --atolerance 0.00000001 --tolerance 0.0000001 --algo mh:rga --population 100 --maxeval 30000 \
	--qa 2.5

qa-edifierw830nb-mhfirefly:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.04 --atolerance 0.00000001 --tolerance 0.000000001 --algo mh:rga --population 80 --maxeval 30000 \
	--qa 2.5

qa-math: qa-fem qa-bem

qa-fem:
	cargo run --release --bin qa-suite -p math-fem --features="cli native parallel"

qa-bem:
	cargo run --release --bin qa-suite -p math-bem --features="native cli parallel"

qa-plugins: qa-plugin-fuzzer

qa-plugin-fuzzer:
	@for file in ./data_generated/test-audio/wav/pink_noise/pink_noise_*.wav; do \
		for plugin in gain eq compressor limiter gate delay loudness crossover upmixer expander mbcomp mbexp matrix mutesolo denoiser fletcher spectrum; do \
			echo "=== Fuzzing plugin: $plugin with $file ==="; \
			cargo run --release --bin plugin-fuzzer -- --file "$file" --plugin $plugin || exit 1; \
		done; \
	done

qa-roomeq: qa-roomeq-small-stereo-20 qa-roomeq-small-stereo-21 qa-roomeq-small-stereo-22 qa-roomeq-convergence

qa-roomeq-convergence:
	cargo run --bin roomeq-qa-quality --release

qa-roomeq-small-stereo-20:
	@for method in iir fir mixed; do \
	  for algo in bem fem; do \
	      mkdir -p ./data_generated/roomeq/generated/$algo/small_stereo_2_0; \
	      cargo run --bin roomeq --release -- \
	        --config       ./data_tests/roomeq/generated/$algo/small_stereo_2_0/config.json \
		    --override-config ./data_tests/roomeq/generated/optimiser-config/small_stereo_2_0/optimiser-$method.json \
		    --output       ./data_generated/roomeq/generated/$algo/small_stereo_2_0/dsp_$method.json; \
		  python3 ./scripts/display-roomeq.py \
	        --input        ./data_tests/roomeq/generated/$algo/small_stereo_2_0/config.json \
		                   ./data_generated/roomeq/generated/$algo/small_stereo_2_0/dsp_$method.json; \
	  done \
	done


qa-roomeq-small-stereo-21:
	@for method in iir fir mixed; do \
	  for algo in fem; do \
	    mkdir -p ./data_generated/roomeq/generated/$algo/small_stereo_2_1; \
	    cargo run --bin roomeq --release -- \
	        --config       ./data_tests/roomeq/generated/$algo/small_stereo_2_1/config.json \
		    --override-config ./data_tests/roomeq/generated/optimiser-config/small_stereo_2_1/optimiser-$method.json \
		    --output       ./data_generated/roomeq/generated/$algo/small_stereo_2_1/dsp_$method.json; \
		python3 ./scripts/display-roomeq.py \
	        --input        ./data_tests/roomeq/generated/$algo/small_stereo_2_1/config.json \
		                   ./data_generated/roomeq/generated/$algo/small_stereo_2_1/dsp_$method.json; \
	  done \
	done


qa-roomeq-small-stereo-22:
	@for method in iir fir mixed; do \
	  for algo in fem; do \
	    mkdir -p ./data_generated/roomeq/generated/$algo/small_stereo_2_2; \
	    cargo run --bin roomeq --release -- \
	        --config       ./data_tests/roomeq/generated/$algo/small_stereo_2_2/config.json \
		    --override-config ./data_tests/roomeq/generated/optimiser-config/small_stereo_2_2/optimiser-$method.json \
		    --output       ./data_generated/roomeq/generated/$algo/small_stereo_2_2/dsp_$method.json; \
		python3 ./scripts/display-roomeq.py \
	        --input        ./data_tests/roomeq/generated/$algo/small_stereo_2_2/config.json \
		                   ./data_generated/roomeq/generated/$algo/small_stereo_2_2/dsp_$method.json; \
	  done \
	done

# New comprehensive QA using roomeq-qa-full binary
qa-roomeq-coverage: prod-autoeq
	cargo run --bin roomeq-qa-coverage --release

qa-roomeq-quick: prod-autoeq
	cargo run --bin roomeq-qa-coverage --release -- --quick --maxeval 200

qa-roomeq-list:
	cargo run --bin roomeq-qa-coverage --release -- --list

qa-roomeq-matrix:
	cargo run --bin roomeq-qa-coverage --release -- --matrix


# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

post-install: post-install-rust post-install-python

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

post-install-python:
	python3 -m venv venv
	./venv/bin/pip install -U pip
	./venv/bin/pip install -r ./scripts/requirements.txt

# ----------------------------------------------------------------------
# MACOS INSTALLER
# ----------------------------------------------------------------------

# Build macOS installer package (unsigned)
build-installer:
	./scripts/build-installer.sh

# Build macOS installer package without HAL driver
build-installer-no-hal:
	./scripts/build-installer.sh --no-hal

# Build signed macOS installer package
build-installer-signed:
	./scripts/build-installer.sh --sign

# Build signed and notarized macOS installer package
build-installer-notarized:
	./scripts/build-installer.sh --sign --notarize

# Build daemon + ConfigBar DMG (unsigned, for local testing)
build-daemon-dmg:
	./scripts/build-dmg-daemon.sh

# Build signed daemon + ConfigBar DMG
build-daemon-dmg-signed:
	./scripts/build-dmg-daemon.sh --sign

# Build signed and notarized daemon + ConfigBar DMG
build-daemon-dmg-notarized:
	./scripts/build-dmg-daemon.sh --sign --notarize

# Uninstall SotF components
uninstall-sotf:
	./scripts/uninstall-sotf.sh
