# ----------------------------------------------------------------------
# CROSS-COMPILATION (Docker-based, no `cross` tool needed)
#
# On an ARM machine:  just cross-arm    (linux-arm64 + windows-arm64 + macos-arm64)
# On an x86 machine:  just cross-x86    (linux-x86 + windows-x86 + macos-x86)
# ----------------------------------------------------------------------

# Build all 3 binaries for ARM
[group('cross')]
cross-arm: cross-linux-arm64 cross-windows-arm64 cross-macos-arm64

# Build all 3 binaries for x86
[group('cross')]
cross-x86: cross-linux-x86 cross-windows-x86 cross-macos-x86

# Default: build for current arch
[group('cross')]
cross:
	#!/usr/bin/env bash
	set -euo pipefail
	arch=$(uname -m)
	case "$arch" in
		aarch64|arm64) just cross-arm ;;
		x86_64)        just cross-x86 ;;
		*) echo "Unsupported architecture: $arch"; exit 1 ;;
	esac

# ----------------------------------------------------------------------
# BASE DOCKER IMAGES (system deps + rust + cargo fetch, cached per Cargo.lock)
# ----------------------------------------------------------------------

[group('cross')]
cross-base-linux-arm64:
	@echo "Building base Docker image for Linux ARM64..."
	docker build -t sotf-base-linux-arm64 \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-unknown-linux-musl .

[group('cross')]
cross-base-linux-x86:
	@echo "Building base Docker image for Linux x86_64..."
	docker build --platform linux/amd64 -t sotf-base-linux-x86 \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-unknown-linux-musl .

[group('cross')]
cross-base-linux-arm64-gnu:
	@echo "Building base Docker image for Linux ARM64 (gnu)..."
	docker build -t sotf-base-linux-arm64-gnu \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-unknown-linux-gnu .

[group('cross')]
cross-base-linux-x86-gnu:
	@echo "Building base Docker image for Linux x86_64 (gnu)..."
	docker build -t sotf-base-linux-x86-gnu \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-unknown-linux-gnu .

[group('cross')]
cross-base-win-arm64:
	@echo "Building base Docker image for Windows ARM64..."
	docker build -t sotf-base-win-arm64 \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-pc-windows-gnullvm .

[group('cross')]
cross-base-win-x86:
	@echo "Building base Docker image for Windows x86_64..."
	docker build --platform linux/amd64 -t sotf-base-win-x86 \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-pc-windows-gnullvm .

# ----------------------------------------------------------------------
# LINUX (AppImage + tarball via Docker)
# ----------------------------------------------------------------------

# Linux ARM64: AppImage + tarball → dist/
[group('cross')]
cross-linux-arm64: cross-base-linux-arm64
	@echo "Building Docker image..."
	docker build -t sotf-linux-arm64 -f ./builds/from_macos_arm/Dockerfile.aarch64-unknown-linux-musl .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -v sotf-target-linux-arm64:/build-target -w /project \
		-e CARGO_TARGET_DIR=/build-target \
		sotf-linux-arm64 ./scripts/build-linux.sh --appimage
	@echo "Done! Output in: dist/"

# Linux x86_64: AppImage + tarball → dist/
# On native x86_64: full build with AppImage. On ARM (macOS): TUI-only tarball via QEMU.
[group('cross')]
cross-linux-x86: cross-base-linux-x86
	#!/usr/bin/env bash
	set -euo pipefail
	host_arch=$(uname -m)
	echo "Building Docker image..."
	if [ "$host_arch" = "x86_64" ]; then
		docker build -t sotf-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-musl .
		echo "Building in Docker container..."
		docker run --rm -v "$(pwd)":/project -v sotf-target-linux-x86:/build-target -w /project \
			-e CARGO_TARGET_DIR=/build-target \
			sotf-linux-x86 ./scripts/build-linux.sh --appimage
	else
		docker build --platform linux/amd64 -t sotf-linux-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-musl .
		echo "Building in Docker container (QEMU)..."
		docker run --rm --platform linux/amd64 -v "$(pwd)":/project -v sotf-target-linux-x86:/build-target -w /project \
			-e CARGO_TARGET_DIR=/build-target \
			sotf-linux-x86 ./scripts/build-linux.sh --tui-only
	fi
	echo "Done! Output in: dist/"

# Debug: open interactive shell in Linux x86 container
[group('cross')]
cross-debug-linux-x86: cross-base-linux-x86-gnu
	@echo "Building Docker image..."
	docker build -t sotf-linux-x86-gnu -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell..."
	docker run -it --rm -v "$(pwd)":/project -w /project sotf-linux-x86-gnu /bin/bash

# ----------------------------------------------------------------------
# WINDOWS (cross-compiled in Docker via llvm-mingw)
# ----------------------------------------------------------------------

# Windows ARM64: sotf-tui.exe + signed MSIX → dist/
# Set WINDOWS_CERT_FILE and WINDOWS_CERT_PASSWORD to use a CA cert; otherwise self-signed is auto-generated
[group('cross')]
cross-windows-arm64: cross-base-win-arm64
	#!/usr/bin/env bash
	set -euo pipefail
	VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	echo "Building Docker image..."
	docker build -t sotf-win-arm64 -f ./builds/from_macos_arm/Dockerfile.aarch64-pc-windows-gnullvm .
	echo "Building in Docker container..."
	mkdir -p dist
	docker_env=(-e CARGO_TARGET_DIR=/build-target)
	# Forward CA cert env vars into Docker if set
	if [ -n "${WINDOWS_CERT_FILE:-}" ]; then
		docker_env+=(-e WINDOWS_CERT_FILE -e WINDOWS_CERT_PASSWORD)
		[ -n "${WINDOWS_TIMESTAMP_URL:-}" ] && docker_env+=(-e WINDOWS_TIMESTAMP_URL)
	fi
	docker run --rm -v "$(pwd)":/project -v sotf-target-win-arm64:/build-target -w /project "${docker_env[@]}" sotf-win-arm64 \
		bash -c 'cargo build --release --target aarch64-pc-windows-gnullvm -p sotf-tui --bin sotf-tui && ./scripts/build-msix.sh --sign --build-dir /build-target/aarch64-pc-windows-gnullvm/release --arch arm64 && cp /build-target/aarch64-pc-windows-gnullvm/release/sotf-tui.exe dist/'
	mv "dist/sotf-tui.exe" "dist/sotf-tui-${VERSION}-windows-arm64.exe"
	echo "Done! dist/sotf-tui-${VERSION}-windows-arm64.exe + MSIX in dist/"

# Windows x86_64: sotf-tui.exe + signed MSIX → dist/
# Set WINDOWS_CERT_FILE and WINDOWS_CERT_PASSWORD to use a CA cert; otherwise self-signed is auto-generated
[group('cross')]
cross-windows-x86: cross-base-win-x86
	#!/usr/bin/env bash
	set -euo pipefail
	VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	echo "Building Docker image..."
	docker build --platform linux/amd64 -t sotf-win-x86 -f ./builds/from_macos_arm/Dockerfile.x86_64-pc-windows-gnullvm .
	echo "Building in Docker container..."
	mkdir -p dist
	docker_env=(-e CARGO_TARGET_DIR=/build-target)
	# Forward CA cert env vars into Docker if set
	if [ -n "${WINDOWS_CERT_FILE:-}" ]; then
		docker_env+=(-e WINDOWS_CERT_FILE -e WINDOWS_CERT_PASSWORD)
		[ -n "${WINDOWS_TIMESTAMP_URL:-}" ] && docker_env+=(-e WINDOWS_TIMESTAMP_URL)
	fi
	docker run --rm --platform linux/amd64 -v "$(pwd)":/project -v sotf-target-win-x86:/build-target -w /project "${docker_env[@]}" sotf-win-x86 \
		bash -c 'cargo build --release --target x86_64-pc-windows-gnullvm -p sotf-tui --bin sotf-tui && ./scripts/build-msix.sh --sign --build-dir /build-target/x86_64-pc-windows-gnullvm/release --arch x64 && cp /build-target/x86_64-pc-windows-gnullvm/release/sotf-tui.exe dist/'
	mv "dist/sotf-tui.exe" "dist/sotf-tui-${VERSION}-windows-x86_64.exe"
	echo "Done! dist/sotf-tui-${VERSION}-windows-x86_64.exe + MSIX in dist/"

# ----------------------------------------------------------------------
# MACOS (native build, outputs to dist/)
# ----------------------------------------------------------------------

# macOS ARM64: SotF DMG + sotf-tui → dist/
[group('cross')]
cross-macos-arm64:
	#!/usr/bin/env bash
	set -euo pipefail
	VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	echo "Building macOS ARM64 binaries..."
	rustup target add aarch64-apple-darwin 2>/dev/null || true
	cargo build --release --target aarch64-apple-darwin -p sotf-tui --features hal,onnx
	cargo build --release --target aarch64-apple-darwin -p sotf-gpui --features hal,onnx
	mkdir -p dist
	cp target/aarch64-apple-darwin/release/sotf-tui "dist/sotf-tui-${VERSION}-macos-arm64"
	cp target/aarch64-apple-darwin/release/SotF "dist/SotF-${VERSION}-macos-arm64"
	echo "Building DMG..."
	./scripts/build-dmg-sotf.sh --arch arm64
	echo "Done! Binaries and DMG copied to dist/"

# macOS x86_64: SotF DMG + sotf-tui → dist/
[group('cross')]
cross-macos-x86:
	#!/usr/bin/env bash
	set -euo pipefail
	VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	echo "Building macOS x86_64 binaries..."
	rustup target add x86_64-apple-darwin 2>/dev/null || true
	cargo build --release --target x86_64-apple-darwin -p sotf-tui --features hal
	cargo build --release --target x86_64-apple-darwin -p sotf-gpui --features hal
	mkdir -p dist
	cp target/x86_64-apple-darwin/release/sotf-tui "dist/sotf-tui-${VERSION}-macos-x86_64"
	cp target/x86_64-apple-darwin/release/SotF "dist/SotF-${VERSION}-macos-x86_64"
	echo "Building DMG..."
	./scripts/build-dmg-sotf.sh --arch x86_64
	echo "Done! Binaries and DMG copied to dist/"

# macOS DMG (build only, sign separately with sign-macos.sh)
[group('cross')]
cross-macos-dmg:
	./scripts/build-dmg-sotf.sh

# macOS Systemwide pkg (build only, sign separately with sign-macos.sh)
[group('cross')]
cross-macos-systemwide:
	./scripts/build-systemwide.sh

# Backward-compatible alias for the old cross target name.
[group('cross')]
cross-macos-daemon: cross-macos-systemwide
	@echo "✅ cross-macos-daemon is an alias for cross-macos-systemwide"

# ----------------------------------------------------------------------
# DEMO ZIP
# ----------------------------------------------------------------------

# Create sotf-demo.zip with demo music files
[group('build')]
build-demo-zip:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Creating sotf-demo.zip..."
	mkdir -p dist/sotf-demo/music
	cp crates/app-gpui/assets/demo-audio/*.flac dist/sotf-demo/music/
	cp crates/app-gpui/assets/demo-audio/*.wav dist/sotf-demo/music/
	if [ -f crates/app-gpui/assets/demo-audio/README.md ]; then
		cp crates/app-gpui/assets/demo-audio/README.md dist/sotf-demo/music/
	fi
	cd dist && zip -r sotf-demo.zip sotf-demo/
	rm -rf sotf-demo
	echo "Done! dist/sotf-demo.zip"
