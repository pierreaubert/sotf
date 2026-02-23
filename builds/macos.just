# macos specific
# HDF5_DIR := '/opt/homebrew/Cellar/hdf5/2.0'

# should be done automatically
dyld_fallback_library_path := '/Applications/Xcode.app/Contents/Framework'

[group('build-macos')]
prod-hal:
	cargo build --release -p driver-hal

# Build the sotf-daemon binary (macOS with HAL support)
[group('build-macos')]
prod-daemon:
	cargo build --release -p sotf-daemon --features hal

# Build the ConfigBar/Toolbar Swift app
[group('build-macos')]
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
[group('build-macos')]
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
[group('build-macos')]
prod-macos-daemon: prod-daemon prod-toolbar prod-hal-driver
	@echo "✅ All macOS daemon components built"

[group('build-macos')]
gpui-release-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	sh -x ./scripts/build-dmg-sotf.sh --sign --notarize

# ----------------------------------------------------------------------
# AUDIO UNIT (macOS only)
# ----------------------------------------------------------------------

# Build Rust FFI library for Audio Units
[group('build-macos')]
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
[group('build-macos')]
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
[group('build-macos')]
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
[group('build-macos')]
validate-au:
	#!/usr/bin/env bash
	set -euxo pipefail
	echo "Validating SOTF EQ Audio Unit..."
	auval -v aufx SOEQ SOTF

# Complete AU build pipeline
[group('build-macos')]
build-au: build-au-rust build-au-swift
	echo "✅ Complete Audio Unit build finished"

# ----------------------------------------------------------------------
# Install macos
# ----------------------------------------------------------------------

[group('install')]
install-macos-cross:
	# use git version until 0.2.6 is out
	cargo install cross --git https://github.com/cross-rs/cross
	cross target add x86_64-apple-ios

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
	brew install chromedriver
	xattr -d com.apple.quarantine $(which chromedriver)
	# optimisation library
	brew install nlopt cmake netcdf opencv chafa


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

# Build signed macOS installer package
[group('build-macos')]
build-installer-signed:
	./scripts/build-installer.sh --sign

# Build signed and notarized macOS installer package
[group('build-macos')]
build-installer-notarized:
	./scripts/build-installer.sh --sign --notarize

# Build daemon + ConfigBar DMG (unsigned, for local testing)
[group('build-macos')]
build-daemon-dmg:
	./scripts/build-dmg-daemon.sh

# Build signed daemon + ConfigBar DMG
[group('build-macos')]
build-daemon-dmg-signed:
	./scripts/build-dmg-daemon.sh --sign

# Build signed and notarized daemon + ConfigBar DMG
[group('build-macos')]
build-daemon-dmg-notarized:
	./scripts/build-dmg-daemon.sh --sign --notarize

# Uninstall SotF components
[group('build-macos')]
uninstall-sotf:
	./scripts/uninstall-sotf.sh
