# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

# you need to define the path
autoeq_dir := env('AUTOEQ_DIR')
# should be done automatically
dyld_fallback_library_path := '/Applications/Xcode.app/Contents/Framework'
# opencv
opencv_haarcascades_path := '/opt/homebrew/Cellar/opencv/4.12.0_15/share/opencv4/haarcascades'

default:
	just --list

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

download-once: download-spinorama download-sofa

download-spinorama:
	cargo run --bin autoeq_download_speakers --release

download-sofa:
	mkdir -p data_cached/org.sofacoustics/mit
	wget -O data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa https://sofacoustics.org/data/database/mit/mit_kemar_normal_pinna.sofa
	wget -O data_cached/org.sofacoustics/mit/kemar_large.sofa https://sofacoustics.org/data/database/mit/mit_kemar_large_pinna.sofa

test-generate-audio-tests: prod-generate-audio-tests
	cargo run --bin prod-generate-audio-tests --release

test-rust:
	cargo check --all-targets
	cargo test --lib

test-ts:
	npm run test

test: test-rust test-ts

# ----------------------------------------------------------------------
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt: fmt-rust fmt-ts

fmt-rust:
	cargo fmt --all

fmt-ts:
	npm run fmt

# ----------------------------------------------------------------------
# PROD
# ----------------------------------------------------------------------

alias build := prod

prod: prod-workspace prod-autoeq prod-sotf-player prod-sotf-recorder prod-generate-audio-tests prod-roomeq
	cargo build --release --bin plot_functions
	cargo build --release --bin download
	cargo build --release --bin benchmark_autoeq_speaker
	cargo build --release --bin benchmark_convergence
	cargo build --release --bin plot_autoeq_de
	cargo build --release --bin run_autoeq_de

prod-generate-audio-tests:
	cargo build --release --bin generate_audio_tests

prod-workspace:
	cargo build --release --workspace

prod-autoeq:
	cargo build --release --bin autoeq

prod-roomeq:
	cargo build --release --bin roomeq

prod-sotf-player:
	cargo build --release --bin sotf_player

prod-sotf-recorder:
	cargo build --release --bin sotf_recorder

prod-hal:
	cargo build --release -p soft_hal

prod-configbar:
	./src-configbar/scripts/build.sh
	./src-configbar/scripts/create_icon.sh

prod-macos: prod-hal prod-configbar

prod-head-scanner:
	cargo build --release -p head-scanner

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
	# Create universal binary
	mkdir -p src-audio-plugins-au/Resources
	lipo -create \
		target/x86_64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		target/aarch64-apple-darwin/release/libsotf_audio_plugins_ffi.a \
		-output src-audio-plugins-au/Resources/libsotf_audio_plugins_ffi.a
	# Copy header file
	cp src-audio-plugins-ffi/sotf_audio_ffi.h src-audio-plugins-au/Shared/
	echo "✅ Universal Rust FFI library created"

# Build Audio Unit plugins in Xcode
build-au-swift: build-au-rust
	#!/usr/bin/env bash
	set -euxo pipefail
	if [ ! -d "src-audio-plugins/SOTFAudioUnits.xcodeproj" ]; then
		echo "⚠️  Xcode project not found. Please create it manually first."
		echo "   See src-audio-plugins/README.md for instructions"
		exit 1
	fi
	xcodebuild -project src-audio-plugins/SOTFAudioUnits.xcodeproj \
		-scheme EQAudioUnit \
		-configuration Release \
		build
	echo "✅ Audio Unit built successfully"

# Install Audio Units to system
install-au:
	#!/usr/bin/env bash
	set -euxo pipefail
	mkdir -p ~/Library/Audio/Plug-Ins/Components/
	if [ -d "build/Release/EQAudioUnit.appex" ]; then
		cp -r build/Release/EQAudioUnit.appex ~/Library/Audio/Plug-Ins/Components/
		echo "✅ EQ Audio Unit installed"
	else
		echo "⚠️  No Audio Unit build found. Run 'just build-au-swift' first"
		exit 1
	fi
	# Restart Audio Component registration
	killall -9 AudioComponentRegistrar 2>/dev/null || true
	echo "✅ Audio Units installed to ~/Library/Audio/Plug-Ins/Components/"

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

bench: bench-convergence bench-autoeq-speaker

bench-convergence:
	cargo run --release --bin benchmark_convergence

bench-autoeq-speaker:
	# either jobs=1 or --no-parallel ; or a mix if you have a lot of
	# CPU cores
	cargo run --release --bin benchmark_autoeq_speaker -- --qa --jobs 1

# ----------------------------------------------------------------------
# CLEAN
# ----------------------------------------------------------------------

clean:
	cargo clean
	rm -rf src-*/dist
	rm -rf src-*/node_modules
	find . -name '*~' -exec rm {} \; -print
	find . -name 'Cargo.lock' -exec rm {} \; -print
	find . -name 'package-lock.json' -exec rm {} \; -print

# ----------------------------------------------------------------------
# DEV
# ----------------------------------------------------------------------

dev:
	cargo build --workspace
	cargo build --bin autoeq
	cargo build --bin plot_functions
	cargo build --bin download
	cargo build --bin benchmark_convergence
	cargo build --bin benchmark_autoeq_speaker
	cargo build --bin plot_autoeq_de
	cargo build --bin run_autoeq_de
	cargo build --bin sotf_audio_test

# ----------------------------------------------------------------------
# UPDATE
# ----------------------------------------------------------------------

update: update-rust update-pre-commit update-ts

update-rust:
	rustup update
	cargo update

update-pre-commit:
	pre-commit autoupdate

update-ts:
	npm run tauri update
	npm run upgrade

# ----------------------------------------------------------------------
# DEMO
# ----------------------------------------------------------------------

demo: headphone_loss_demo plot_functions

headphone_loss_demo:
	cargo run --release --example headphone_loss_demo -- \
	--spl "./data_tests/headphones/asr/bowerwilkins_p7/Bowers & Wilkins P7.csv" \
	--target "./data_tests/targets/harman-over-ear-2018.csv"

plot_functions:
	cargo run --release --bin plot_functions

# ----------------------------------------------------------------------
# EXAMPLES
# ----------------------------------------------------------------------

examples : examples-iir examples-de examples-autoeq examples-testfunctions

examples-iir :
	cargo run --release --example format_demo
	cargo run --release --example readme_example

examples-de :
	cargo run --release --example optde_basic
	cargo run --release --example optde_adaptive_demo
	cargo run --release --example optde_linear_constraints
	cargo run --release --example optde_nonlinear_constraints
	cargo run --release --example optde_parallel

examples-autoeq:
	cargo run --release --example headphone_loss_validation

examples-testfunctions:
	cargo run --release --example test_hartman_4d

# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

cross : cross-macos-arm-2-linux-x86

# Debug: Build Docker image and open interactive shell
cross-debug-x86 :
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell. Try: cargo build --release --target x86_64-unknown-linux-gnu"
	docker run -it --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 /bin/bash

cross-macos-arm-2-linux-x86 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 \
		cargo build --release --target x86_64-unknown-linux-gnu
	@echo "Done! Binary at: target/x86_64-unknown-linux-gnu/release/autoeq"

cross-macos-arm-2-linux-arm64 :
	echo "This can take minutes!"
	cross build --release --target aarch64-unknown-linux-gnu

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
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-unknown-linux-musl --bin sotf_player_tui
	@echo "Done! Binary at: target/x86_64-unknown-linux-musl/release/sotf_player_tui"

# Linux ARM64 static binary (musl)
cross-static-linux-arm64:
	@echo "Building static Linux ARM64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target aarch64-unknown-linux-musl --bin sotf_player_tui
	@echo "Done! Binary at: target/aarch64-unknown-linux-musl/release/sotf_player_tui"

# Windows x86_64 static binary (MSVC with static CRT)
cross-static-windows-x86:
	@echo "Building static Windows x86_64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc --bin sotf_player_tui
	@echo "Done! Binary at: target/x86_64-pc-windows-msvc/release/sotf_player_tui.exe"

# macOS universal binary (NOT fully static - limited by macOS system restrictions)
# Apple requires dynamic linking to system frameworks (CoreAudio, CoreFoundation, etc.)
# This creates a universal binary supporting both Intel (x86_64) and Apple Silicon (ARM64)
cross-static-macos:
	@echo "Building macOS binaries..."
	@echo "Note: macOS binaries cannot be fully static due to Apple's security policies"
	@echo "      System frameworks (CoreAudio, etc.) will be dynamically linked"
	cargo build --release --target x86_64-apple-darwin --bin sotf_player_tui
	cargo build --release --target aarch64-apple-darwin --bin sotf_player_tui
	@echo "Creating universal binary (Intel + Apple Silicon)..."
	lipo -create \
		target/x86_64-apple-darwin/release/sotf_player_tui \
		target/aarch64-apple-darwin/release/sotf_player_tui \
		-output target/sotf_player_tui-macos-universal
	@echo "✓ Done! Universal binary at: target/sotf_player_tui-macos-universal"

# Build static binary for current platform
# Note: Requires bash/sh shell. On Linux, builds musl static binary.
# On macOS, builds regular binary (static linking limited by Apple).
build-static-local:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Building static binary for current platform..."
	if [ "$(uname)" = "Linux" ]; then
		if [ "$(uname -m)" = "x86_64" ]; then
			cargo build --release --target x86_64-unknown-linux-musl --bin sotf_player_tui
			echo "✓ Built: target/x86_64-unknown-linux-musl/release/sotf_player_tui"
		elif [ "$(uname -m)" = "aarch64" ]; then
			cargo build --release --target aarch64-unknown-linux-musl --bin sotf_player_tui
			echo "✓ Built: target/aarch64-unknown-linux-musl/release/sotf_player_tui"
		else
			echo "❌ Unsupported Linux architecture: $(uname -m)"
			exit 1
		fi
	elif [ "$(uname)" = "Darwin" ]; then
		cargo build --release --bin sotf_player_tui
		echo "✓ Built: target/release/sotf_player_tui"
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
	~/.cargo/bin/cargo install cargo-bininstall
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
	brew install nlopt cmake netcdf opencv


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

install-ubuntu-node:
		# use nvm
		curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
		$HOME/.nvm/bin/nvm install stable

install-ubuntu-x86: install-ubuntu-common install-ubuntu-x86-driver install-ubuntu-node

install-ubuntu-arm64: install-ubuntu-common install-ubuntu-arm64-driver install-ubuntu-node


# ----------------------------------------------------------------------
# publish
# ----------------------------------------------------------------------

publish-autoeq:
	cd autoeq-testfunctions && cargo publish
	cd autoeq-de && cargo publish
	cd autoeq-cea2034 && cargo publish

publish-math:
	cd math-bem && cargo publish
	cd math-convexhull3d && cargo publish

publish-gpui:
	cd gpui-ui-kit && cargo publish
	cd gpui-d3rs && cargo publish
	cd gpui-px && cargo publish

publish: publish-math publish-autoeq publish-gpui

# ----------------------------------------------------------------------
# QA
# ----------------------------------------------------------------------

qa: prod-autoeq \
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
	--loss headphone-score --min-spacing-oct 0.08 \
	--algo autoeq:de --population 70 --maxeval 8000 --seed 42 \
	--qa 14.0

qa-edifierw830nb-mhrga:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.08 --atolerance 0.000001 --tolerance 0.0000001 --algo mh:rga --population 100 --maxeval 20000 \
	--qa 4.0

qa-edifierw830nb-mhfirefly:
	./target/release/autoeq -n 5 \
	--curve data_tests/headphones/asr/edifierw830nb/Edifier\ W830NB.csv \
	--target ./data_tests/targets/harman-over-ear-2018.csv \
	--min-freq 50 --max-freq 16000 --max-q 8 --max-db 8 \
	--loss headphone-score \
	--min-spacing-oct 0.08 --atolerance 0.000001 --tolerance 0.0000001 --algo mh:rga --population 80 --maxeval 3000 \
	--qa 4.0

# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

post-install-npm:
	cd src-ui-frontend && npm install .

post-install-rust:
	$HOME/.cargo/bin/rustup default stable
	$HOME/.cargo/bin/cargo install just
	$HOME/.cargo/bin/cargo install tauri-cli
	$HOME/.cargo/bin/cargo check
	cd src-tauri && $HOME/.cargo/bin/cargo tauri icon

post-install: post-install-rust post-install-npm

# ----------------------------------------------------------------------
# SIGNING
# ----------------------------------------------------------------------



