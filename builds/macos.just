# macOS sotf-systemwide (daemon, HAL driver, menubar app) build/test/packaging
# recipes moved to ../sotf-systemwide (builds/macos.just, builds/systemwide.just).

[group('build-macos')]
gpui-release-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	sh -x ./scripts/build-dmg-sotf.sh

# ----------------------------------------------------------------------
# AUDIO UNIT (macOS only)
# ----------------------------------------------------------------------
# AU sources and build outputs live in ../sotf-daw. Keep the old SOTF recipe
# names as forwarding aliases for callers of this Justfile.

# Build the Rust FFI staticlib for arm64 and stage it for the Xcode project.
[group('build-macos')]
build-au-rust-arm64:
	cd ../sotf-daw && just build-au-ffi-arm64

# Build the Rust FFI staticlib for x86_64 and stage it for the Xcode project.
[group('build-macos')]
build-au-rust-x86_64:
	cd ../sotf-daw && just build-au-ffi-x86_64

# Build the SOTFAudioUnits.app for arm64 (Xcode, isolated DerivedData).
[group('build-macos')]
build-au-swift-arm64:
	cd ../sotf-daw && just build-au-xcode-arm64

# Build the SOTFAudioUnits.app for x86_64 (Xcode, isolated DerivedData).
[group('build-macos')]
build-au-swift-x86_64:
	cd ../sotf-daw && just build-au-xcode-x86_64

# Build the complete arm64 AU package (FFI + Xcode .app).
[group('build-macos')]
build-au-arm64:
	cd ../sotf-daw && just build-au-all-arm64

# Build the complete x86_64 AU package (FFI + Xcode .app).
[group('build-macos')]
build-au-x86_64:
	cd ../sotf-daw && just build-au-all-x86_64

# Validate Audio Unit
[group('build-macos')]
validate-au:
	#!/usr/bin/env bash
	set -euxo pipefail
	echo "Validating SOTF EQ Audio Unit..."
	auval -v aufx SOEQ SOTF

# Build both per-arch AU packages (arm64 + x86_64), sequentially.
[group('build-macos')]
build-au:
	cd ../sotf-daw && just build-au-all

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

[group('install')]
install-macos-brew:
	curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh > ./scripts/install-brew
	chmod +x ./scripts/install-brew
	NONINTERACTIVE=1 ./scripts/install-brew

[group('install')]
install-macos: install-macos-brew install-rustup
	# need xcode
	xcode-select --install
	# need metal
	xcodebuild -downloadComponent MetalToolchain
	# chromedriver sheanigans
	# brew install chromedriver
	# xattr -d com.apple.quarantine $(which chromedriver)
	# build tools for native deps in the dep tree (skia, etc.)
	brew install cmake pluginval


# ----------------------------------------------------------------------
# MACOS INSTALLER
# ----------------------------------------------------------------------

# Build macOS installer package (unsigned)
[group('build-macos')]
build-installer:
	./scripts/build-installer.sh

# Build macOS installer package without HAL driver
[group('build-macos')]
build-installer-no-hal:
	./scripts/build-installer.sh --no-hal

# Sign all macOS artifacts in dist/
[group('build-macos')]
sign-macos:
	./scripts/sign-macos.sh

# Sign and notarize all macOS artifacts in dist/
[group('build-macos')]
sign-macos-notarize:
	./scripts/sign-macos.sh --notarize
