# ----------------------------------------------------------------------
# RELEASE BUILDS for non-host platforms
#   - Linux + Windows are produced via Docker (`docker-*` recipes below).
#   - macOS is built natively on the host (`cross-macos-*` recipes below).
#
# Umbrella recipes orchestrate both:
#   On an ARM machine:  just cross-arm    (docker linux-arm64 + windows-arm64 + native macos-arm64)
#   On an x86 machine:  just cross-x86    (docker linux-x86   + windows-x86   + native macos-x86)
#
# Naming: anything that runs `docker build` / `docker run` is `docker-*`.
# Native macOS builds keep `cross-macos-*` (no docker involved).
# ----------------------------------------------------------------------

# Build all 3 binaries for ARM
[group('cross')]
cross-arm: docker-linux-arm64 docker-windows-arm64 cross-macos-arm64

# Build all 3 binaries for x86
[group('cross')]
cross-x86: docker-linux-x86 docker-windows-x86 cross-macos-x86

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

[group('docker')]
docker-base-linux-arm64:
	@echo "Building base Docker image for Linux ARM64 (gnu)..."
	docker build -t sotf-base-linux-arm64 \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-unknown-linux-gnu .

[group('docker')]
docker-base-linux-x86:
	@echo "Building base Docker image for Linux x86_64..."
	docker build --platform linux/amd64 -t sotf-base-linux-x86 \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-unknown-linux-musl .

[group('docker')]
docker-base-linux-arm64-gnu:
	@echo "Building base Docker image for Linux ARM64 (gnu)..."
	docker build -t sotf-base-linux-arm64-gnu \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-unknown-linux-gnu .

[group('docker')]
docker-base-linux-x86-gnu:
	@echo "Building base Docker image for Linux x86_64 (gnu)..."
	docker build -t sotf-base-linux-x86-gnu \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-unknown-linux-gnu .

[group('docker')]
docker-base-win-arm64:
	@echo "Building base Docker image for Windows ARM64..."
	docker build -t sotf-base-win-arm64 \
		-f ./builds/from_macos_arm/Dockerfile.base.aarch64-pc-windows-gnullvm .

[group('docker')]
docker-base-win-x86:
	@echo "Building base Docker image for Windows x86_64..."
	docker build --platform linux/amd64 -t sotf-base-win-x86 \
		-f ./builds/from_macos_arm/Dockerfile.base.x86_64-pc-windows-gnullvm .

# ----------------------------------------------------------------------
# LINUX (AppImage + tarball via Docker)
# ----------------------------------------------------------------------

[group('docker')]
docker-linux-x86-distros: docker-linux-x86-ubuntu-24-04 docker-linux-x86-ubuntu-26-04 docker-linux-x86-debian-latest docker-linux-x86-alpine-latest

[group('docker')]
docker-linux-x86-ubuntu-24-04:
	just _docker-linux-x86-debian-distro ubuntu-24-04 ubuntu:24.04

[group('docker')]
docker-linux-x86-ubuntu-26-04:
	just _docker-linux-x86-debian-distro ubuntu-26-04 ubuntu:26.04

[group('docker')]
docker-linux-x86-debian-latest:
	just _docker-linux-x86-debian-distro debian-latest debian:latest

[group('docker')]
docker-linux-x86-alpine-latest:
	#!/usr/bin/env bash
	set -euo pipefail
	image="sotf-linux-x86-alpine-latest"
	volume="sotf-target-linux-x86-alpine-latest"
	version=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	out_dir="dist/linux-distros/alpine-latest"
	echo "Building Docker image for Alpine latest..."
	docker build --platform linux/amd64 -t "$image" \
		-f ./builds/linux-distros/Dockerfile.alpine-x86_64 .
	echo "Building TUI in Docker container..."
	docker run --rm --platform linux/amd64 \
		-v "$(pwd)":/project \
		-v "$volume":/build-target \
		-w /project \
		-e CARGO_TARGET_DIR=/build-target \
		"$image" ./scripts/build-linux.sh --tui-only
	mkdir -p "$out_dir"
	for artifact in \
		"dist/sotf-desktop-${version}-linux-x86_64.tar.gz"; do
		[ -f "$artifact" ] && cp "$artifact" "$out_dir/"
	done
	echo "Copied distro outputs to: $out_dir"
	echo "Done! Output in: dist/"

[group('docker')]
_docker-linux-x86-debian-distro distro base_image:
	#!/usr/bin/env bash
	set -euo pipefail
	image="sotf-linux-x86-{{distro}}"
	volume="sotf-target-linux-x86-{{distro}}"
	version=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	out_dir="dist/linux-distros/{{distro}}"
	echo "Building Docker image for {{base_image}}..."
	docker build --platform linux/amd64 \
		--build-arg BASE_IMAGE="{{base_image}}" \
		-t "$image" \
		-f ./builds/linux-distros/Dockerfile.debian-x86_64-gnu .
	echo "Building AppImage, .deb, and tarball in Docker container..."
	docker run --rm --platform linux/amd64 \
		-v "$(pwd)":/project \
		-v "$volume":/build-target \
		-w /project \
		-e CARGO_TARGET_DIR=/build-target \
		"$image" ./scripts/build-linux.sh --appimage --deb
	mkdir -p "$out_dir"
	for artifact in \
		"dist/sotf-desktop-${version}-linux-x86_64.tar.gz" \
		"dist/sotf-desktop-${version}-linux-x86_64.AppImage" \
		"dist/sotf-desktop_${version}_amd64.deb"; do
		[ -f "$artifact" ] && cp "$artifact" "$out_dir/"
	done
	echo "Copied distro outputs to: $out_dir"
	echo "Done! Output in: dist/"

# Linux ARM64: AppImage + tarball → dist/
# Use the GNU/Linux image for release packaging. The static musl image cannot
# provide the ALSA pkg-config metadata needed by cpal/alsa-sys.
[group('docker')]
docker-linux-arm64: docker-base-linux-arm64
	@echo "Building Docker image..."
	docker build -t sotf-linux-arm64 -f ./builds/from_macos_arm/Dockerfile.aarch64-unknown-linux-gnu .
	@echo "Building in Docker container..."
	docker run --rm -v "$(pwd)":/project -v sotf-target-linux-arm64:/build-target -w /project \
		-e CARGO_TARGET_DIR=/build-target \
		sotf-linux-arm64 ./scripts/build-linux.sh --appimage
	@echo "Done! Output in: dist/"

# Linux x86_64: AppImage + tarball → dist/
# On native x86_64: full build with AppImage. On ARM (macOS): TUI-only tarball via QEMU.
[group('docker')]
docker-linux-x86: docker-base-linux-x86
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
[group('docker')]
docker-debug-linux-x86: docker-base-linux-x86-gnu
	@echo "Building Docker image..."
	docker build -t sotf-linux-x86-gnu -f ./builds/from_macos_arm/Dockerfile.x86_64-unknown-linux-gnu .
	@echo "Starting interactive shell..."
	docker run -it --rm -v "$(pwd)":/project -w /project sotf-linux-x86-gnu /bin/bash

# ----------------------------------------------------------------------
# WINDOWS (cross-compiled in Docker via llvm-mingw)
# ----------------------------------------------------------------------

# Windows ARM64: sotf-tui.exe + signed MSIX → dist/
# Set WINDOWS_CERT_FILE and WINDOWS_CERT_PASSWORD to use a CA cert; otherwise self-signed is auto-generated
[group('docker')]
docker-windows-arm64: docker-base-win-arm64
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
	# Release cuts use the `dist` profile (fat LTO + codegen-units=1).
	# Artifacts land in /build-target/<triple>/dist/ instead of .../release/.
	docker run --rm -v "$(pwd)":/project -v sotf-target-win-arm64:/build-target -w /project "${docker_env[@]}" sotf-win-arm64 \
		bash -c 'cargo build --profile dist --target aarch64-pc-windows-gnullvm -p sotf-tui --bin sotf-tui && ./scripts/build-msix.sh --sign --build-dir /build-target/aarch64-pc-windows-gnullvm/dist --arch arm64 && cp /build-target/aarch64-pc-windows-gnullvm/dist/sotf-tui.exe dist/'
	mv "dist/sotf-tui.exe" "dist/sotf-tui-${VERSION}-windows-arm64.exe"
	echo "Done! dist/sotf-tui-${VERSION}-windows-arm64.exe + MSIX in dist/"

# Windows x86_64: sotf-tui.exe + signed MSIX → dist/
# Set WINDOWS_CERT_FILE and WINDOWS_CERT_PASSWORD to use a CA cert; otherwise self-signed is auto-generated
[group('docker')]
docker-windows-x86: docker-base-win-x86
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
	# Release cuts use the `dist` profile (fat LTO + codegen-units=1).
	docker run --rm --platform linux/amd64 -v "$(pwd)":/project -v sotf-target-win-x86:/build-target -w /project "${docker_env[@]}" sotf-win-x86 \
		bash -c 'cargo build --profile dist --target x86_64-pc-windows-gnullvm -p sotf-tui --bin sotf-tui && ./scripts/build-msix.sh --sign --build-dir /build-target/x86_64-pc-windows-gnullvm/dist --arch x86_64 && cp /build-target/x86_64-pc-windows-gnullvm/dist/sotf-tui.exe dist/'
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
	echo "Building macOS ARM64 binaries (profile=dist)..."
	rustup target add aarch64-apple-darwin 2>/dev/null || true
	# Release cuts use the `dist` profile (fat LTO + codegen-units=1).
	cargo build --profile dist --target aarch64-apple-darwin -p sotf-tui --features hal,onnx
	cargo build --profile dist --target aarch64-apple-darwin -p sotf-gpui --features hal,onnx
	mkdir -p dist
	cp target/aarch64-apple-darwin/dist/sotf-tui "dist/sotf-tui-${VERSION}-macos-arm64"
	cp target/aarch64-apple-darwin/dist/sotf-desktop "dist/sotf-desktop-${VERSION}-macos-arm64"
	echo "Building DMG..."
	./scripts/build-dmg-sotf.sh --arch arm64 --binary target/aarch64-apple-darwin/dist/sotf-desktop
	echo "Done! Binaries and DMG copied to dist/"

# macOS x86_64: SotF DMG + sotf-tui → dist/
[group('cross')]
cross-macos-x86:
	#!/usr/bin/env bash
	set -euo pipefail
	VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
	echo "Building macOS x86_64 binaries (profile=dist)..."
	rustup target add x86_64-apple-darwin 2>/dev/null || true
	# Release cuts use the `dist` profile (fat LTO + codegen-units=1).
	cargo build --profile dist --target x86_64-apple-darwin -p sotf-tui --features hal
	cargo build --profile dist --target x86_64-apple-darwin -p sotf-gpui --features hal
	mkdir -p dist
	cp target/x86_64-apple-darwin/dist/sotf-tui "dist/sotf-tui-${VERSION}-macos-x86_64"
	cp target/x86_64-apple-darwin/dist/sotf-desktop "dist/sotf-desktop-${VERSION}-macos-x86_64"
	echo "Building DMG..."
	./scripts/build-dmg-sotf.sh --arch x86_64 --binary target/x86_64-apple-darwin/dist/sotf-desktop
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
