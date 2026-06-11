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

# Build the SotF Systemwide Swift app
[group('build-macos')]
prod-systemwide:
	#!/usr/bin/env bash
	set -euo pipefail
	CONFIGBAR_DIR="crates/systemwide/crates/daemon/configbar"
	BUILD_DIR="target/release"
	mkdir -p "$BUILD_DIR"
	echo "Building SotF Systemwide..."
	swiftc \
		-o "$BUILD_DIR/sotf-systemwide" \
		"$CONFIGBAR_DIR"/src/*.swift \
		-framework SwiftUI \
		-framework WebKit \
		-framework UserNotifications \
		-framework CoreAudio \
		-O
	echo "✅ Systemwide built: $BUILD_DIR/sotf-systemwide"

# Backward-compatible build target.
[group('build-macos')]
prod-toolbar: prod-systemwide
	@echo "✅ prod-toolbar is an alias for prod-systemwide"

# Build the Swift HAL driver bundle
[group('build-macos')]
prod-hal-driver:
	#!/usr/bin/env bash
	set -euo pipefail
	HAL_SWIFT_DIR="crates/systemwide/crates/driver-hal/swift"
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
	# Ad-hoc sign the driver bundle so CoreAudio will load it locally.
	# Distribution signing (Developer ID Application) is handled by
	# ./scripts/sign-macos.sh when packaging for release.
	echo "Ad-hoc signing HAL driver..."
	codesign --force --deep --sign - "$BUILD_DIR"
	echo "✅ HAL driver built: $BUILD_DIR"

# Build all macOS daemon components (daemon + Systemwide app + HAL driver)
[group('build-macos')]
prod-macos-daemon: prod-daemon prod-systemwide prod-hal-driver
	@echo "✅ All macOS daemon components built"

# ----------------------------------------------------------------------
# DIST variants (release-cut profile = fat LTO + codegen-units=1).
# Artifacts land in `target/dist/` instead of `target/release/`.
# Swift outputs follow the same convention so build-systemwide.sh can
# package every component from a single tree.
# ----------------------------------------------------------------------

[group('dist')]
dist-daemon:
	cargo build --profile dist -p sotf-daemon --features hal

[group('dist')]
dist-systemwide:
	#!/usr/bin/env bash
	set -euo pipefail
	CONFIGBAR_DIR="crates/systemwide/crates/daemon/configbar"
	BUILD_DIR="target/dist"
	mkdir -p "$BUILD_DIR"
	echo "Building SotF Systemwide (dist)..."
	swiftc \
		-o "$BUILD_DIR/sotf-systemwide" \
		"$CONFIGBAR_DIR"/src/*.swift \
		-framework SwiftUI \
		-framework WebKit \
		-framework UserNotifications \
		-framework CoreAudio \
		-O
	echo "✅ Systemwide built: $BUILD_DIR/sotf-systemwide"

[group('dist')]
dist-hal-driver:
	#!/usr/bin/env bash
	set -euo pipefail
	HAL_SWIFT_DIR="crates/systemwide/crates/driver-hal/swift"
	BUILD_DIR="target/dist/SotFHAL.driver"
	mkdir -p "$BUILD_DIR/Contents/MacOS"
	mkdir -p "$BUILD_DIR/Contents/Resources"
	echo "Building Swift HAL driver (dist)..."
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
	echo "Ad-hoc signing HAL driver..."
	codesign --force --deep --sign - "$BUILD_DIR"
	echo "✅ HAL driver built: $BUILD_DIR"

[group('dist')]
dist-macos-daemon: dist-daemon dist-systemwide dist-hal-driver
	@echo "✅ All macOS daemon components built (dist)"

[group('build-macos')]
gpui-release-macos:
	#!/usr/bin/env bash
	set -euo pipefail
	sh -x ./scripts/build-dmg-sotf.sh

# ----------------------------------------------------------------------
# AUDIO UNIT (macOS only)
# ----------------------------------------------------------------------
# Two independent per-arch packages instead of a universal/lipo binary.
# Each arch is a self-contained pipeline (cargo + xcodebuild) so a failure
# in one architecture does not block the other, and the two .app bundles
# can be signed and shipped separately.
#
# cargo emits target/<triple>/release/libplugins_ffi.a (crate name is
# `plugins-ffi`). The Xcode project links -lsotf_audio_plugins_ffi
# (project.yml:39), so the staging step copies the .a under that name into
# crates/sotf-plugins/crates/plugins-au/Resources/.
#
# Outputs:
#   crates/sotf-plugins/crates/plugins-au/build/au-arm64/Build/Products/Release/SOTFAudioUnits.app
#   crates/sotf-plugins/crates/plugins-au/build/au-x86_64/Build/Products/Release/SOTFAudioUnits.app

# Build the Rust FFI staticlib for arm64 and stage it for the Xcode project.
[group('build-macos')]
build-au-rust-arm64:
	#!/usr/bin/env bash
	set -euxo pipefail
	cargo build --release -p plugins-ffi --target aarch64-apple-darwin
	mkdir -p crates/sotf-plugins/crates/plugins-au/Resources
	cp target/aarch64-apple-darwin/release/libplugins_ffi.a \
		crates/sotf-plugins/crates/plugins-au/Resources/libsotf_audio_plugins_ffi.a
	cp crates/sotf-plugins/crates/plugins-ffi/sotf_audio_plugin_ffi.h \
		crates/sotf-plugins/crates/plugins-au/Shared/
	echo "✅ Rust FFI library built for arm64"

# Build the Rust FFI staticlib for x86_64 and stage it for the Xcode project.
[group('build-macos')]
build-au-rust-x86_64:
	#!/usr/bin/env bash
	set -euxo pipefail
	cargo build --release -p plugins-ffi --target x86_64-apple-darwin
	mkdir -p crates/sotf-plugins/crates/plugins-au/Resources
	cp target/x86_64-apple-darwin/release/libplugins_ffi.a \
		crates/sotf-plugins/crates/plugins-au/Resources/libsotf_audio_plugins_ffi.a
	cp crates/sotf-plugins/crates/plugins-ffi/sotf_audio_plugin_ffi.h \
		crates/sotf-plugins/crates/plugins-au/Shared/
	echo "✅ Rust FFI library built for x86_64"

# Build the SOTFAudioUnits.app for arm64 (Xcode, isolated DerivedData).
[group('build-macos')]
build-au-swift-arm64: build-au-rust-arm64
	#!/usr/bin/env bash
	set -euxo pipefail
	cd crates/sotf-plugins/crates/plugins-au
	if [ ! -d "SOTFAudioUnits.xcodeproj" ] || [ "project.yml" -nt "SOTFAudioUnits.xcodeproj/project.pbxproj" ]; then
		echo "🔨 Generating Xcode project with XcodeGen..."
		xcodegen generate
	fi
	rm -rf build/au-arm64
	xcodebuild -project SOTFAudioUnits.xcodeproj \
		-scheme EQAudioUnit \
		-configuration Release \
		-derivedDataPath build/au-arm64 \
		ARCHS=arm64 \
		ONLY_ACTIVE_ARCH=NO \
		build
	echo "✅ Audio Unit built for arm64: build/au-arm64/Build/Products/Release/SOTFAudioUnits.app"

# Build the SOTFAudioUnits.app for x86_64 (Xcode, isolated DerivedData).
[group('build-macos')]
build-au-swift-x86_64: build-au-rust-x86_64
	#!/usr/bin/env bash
	set -euxo pipefail
	cd crates/sotf-plugins/crates/plugins-au
	if [ ! -d "SOTFAudioUnits.xcodeproj" ] || [ "project.yml" -nt "SOTFAudioUnits.xcodeproj/project.pbxproj" ]; then
		echo "🔨 Generating Xcode project with XcodeGen..."
		xcodegen generate
	fi
	rm -rf build/au-x86_64
	xcodebuild -project SOTFAudioUnits.xcodeproj \
		-scheme EQAudioUnit \
		-configuration Release \
		-derivedDataPath build/au-x86_64 \
		ARCHS=x86_64 \
		ONLY_ACTIVE_ARCH=NO \
		build
	echo "✅ Audio Unit built for x86_64: build/au-x86_64/Build/Products/Release/SOTFAudioUnits.app"

# Build the complete arm64 AU package (FFI + Xcode .app).
[group('build-macos')]
build-au-arm64: build-au-swift-arm64
	@echo "✅ Audio Unit arm64 package ready"

# Build the complete x86_64 AU package (FFI + Xcode .app).
[group('build-macos')]
build-au-x86_64: build-au-swift-x86_64
	@echo "✅ Audio Unit x86_64 package ready"

# Validate Audio Unit
[group('build-macos')]
validate-au:
	#!/usr/bin/env bash
	set -euxo pipefail
	echo "Validating SOTF EQ Audio Unit..."
	auval -v aufx SOEQ SOTF

# Resources/libsotf_audio_plugins_ffi.a is overwritten between archs, but
# each xcodebuild finishes before the next arch's cargo build replaces it,
# so the link inputs always match the arch being compiled.

# Build both per-arch AU packages (arm64 + x86_64), sequentially.
[group('build-macos')]
build-au: build-au-arm64 build-au-x86_64
	@echo "✅ Audio Unit packages built for arm64 and x86_64"

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

# Build Systemwide package
[group('build-macos')]
build-systemwide:
	./scripts/build-systemwide.sh

# Backward-compatible alias for the old package target name.
[group('build-macos')]
build-daemon-dmg: build-systemwide
	@echo "✅ build-daemon-dmg is an alias for build-systemwide"

# Sign all macOS artifacts in dist/
[group('build-macos')]
sign-macos:
	./scripts/sign-macos.sh

# Sign and notarize all macOS artifacts in dist/
[group('build-macos')]
sign-macos-notarize:
	./scripts/sign-macos.sh --notarize

# Uninstall SotF components
[group('build-macos')]
uninstall-sotf:
	./scripts/uninstall-sotf.sh
