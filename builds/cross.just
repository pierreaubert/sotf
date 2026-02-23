# ----------------------------------------------------------------------
# CROSS
# ----------------------------------------------------------------------

[group('cross')]
cross : cross-macos-arm-2-linux-x86

# Debug: Build Docker image and open interactive shell
[group('cross')]
cross-debug-x86 :
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell. Try: cargo build --release --target x86_64-unknown-linux-gnu"
	docker run -it --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 /bin/bash

[group('cross')]
cross-macos-arm-2-linux-x86 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-x86 .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-x86 \
		cargo build --release --target x86_64-unknown-linux-gnu
	@echo "Done! Binary at: target/x86_64-unknown-linux-gnu/release/autoeq"

[group('cross')]
cross-macos-arm-2-linux-arm64 :
	echo "This can take minutes!"
	@echo "Building Docker image..."
	docker build -t autoeq-linux-arm64 -f ./builds/from_macos_arm/Dockerfile.aarch64-unknown-linux-musl .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -w /project autoeq-linux-arm64 \
		cargo build --release --target aarch64-unknown-linux-musl
	@echo "Done! Binary at: target/aarch64-unknown-linux-musl/release/"

[group('cross')]
cross-macos-arm-2-win-x86-gnu :
	echo "This is not supported yet"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-gnu

[group('cross')]
cross-macos-arm-2-win-x86-msvc :
	echo "This can take minutes!"
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc

[group('cross')]
cross-macos-arm-2-win-arm-gnu :
	echo "This is not supported!"

[group('cross')]
cross-macos-arm-2-win-arm-msvc :
	echo "This is not supported!"

# ----------------------------------------------------------------------
# STATIC BINARY CROSS-COMPILATION
# ----------------------------------------------------------------------

# Build static binaries for all platforms
[group('cross')]
cross-static-all: cross-static-linux-x86 cross-static-linux-arm64 cross-static-windows-x86 cross-static-macos

# Linux x86_64 static binary (musl)
[group('cross')]
cross-static-linux-x86:
	@echo "Building static Linux x86_64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-unknown-linux-musl --bin sotf-tui
	@echo "Done! Binary at: target/x86_64-unknown-linux-musl/release/sotf_player_tui"

# Linux ARM64 static binary (musl)
[group('cross')]
cross-static-linux-arm64:
	@echo "Building static Linux ARM64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target aarch64-unknown-linux-musl --bin sotf-tui
	@echo "Done! Binary at: target/aarch64-unknown-linux-musl/release/sotf_player_tui"

# Windows x86_64 static binary (MSVC with static CRT)
[group('cross')]
cross-static-windows-x86:
	@echo "Building static Windows x86_64 binary..."
	CROSS_CONFIG=./builds/CrossFromMacARM.toml cross build --release --target x86_64-pc-windows-msvc --bin sotf-tui
	@echo "Done! Binary at: target/x86_64-pc-windows-msvc/release/sotf_player_tui.exe"

# macOS universal binary (NOT fully static - limited by macOS system restrictions)
# Apple requires dynamic linking to system frameworks (CoreAudio, CoreFoundation, etc.)
# This creates a universal binary supporting both Intel (x86_64) and Apple Silicon (ARM64)
[group('cross')]
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
[group('build')]
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

