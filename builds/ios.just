# iOS builds

# ----------------------------------------------------------------------
# SOTF iOS App (crates/app-ios)
# ----------------------------------------------------------------------

# Build SotF iOS Rust static library for simulator (arm64)
[group('ios')]
ios-rust-sim:
	cargo build -p sotf-ios --target aarch64-apple-ios-sim --release

# Build SotF iOS Rust static library for device (arm64)
[group('ios')]
ios-rust-device:
	cargo build -p sotf-ios --target aarch64-apple-ios --release

# Build SotF iOS Rust lib and copy to Xcode project (simulator)
[group('ios')]
ios-build-rust-sim: ios-rust-sim
	#!/usr/bin/env bash
	set -euo pipefail
	IOS_DIR="crates/app-ios/ios"
	mkdir -p "$IOS_DIR/lib"
	cp target/aarch64-apple-ios-sim/release/libsotf_ios.a "$IOS_DIR/lib/"
	echo "Copied libsotf_ios.a to $IOS_DIR/lib/"

# Build SotF iOS Rust lib and copy to Xcode project (device)
[group('ios')]
ios-build-rust-device: ios-rust-device
	#!/usr/bin/env bash
	set -euo pipefail
	IOS_DIR="crates/app-ios/ios"
	mkdir -p "$IOS_DIR/lib"
	cp target/aarch64-apple-ios/release/libsotf_ios.a "$IOS_DIR/lib/"
	echo "Copied libsotf_ios.a to $IOS_DIR/lib/"

# Generate SotF iOS Xcode project with XcodeGen
[group('ios')]
ios-xcodegen:
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-ios/ios
	if [ ! -d "SotFPlayer.xcodeproj" ] || [ "project.yml" -nt "SotFPlayer.xcodeproj/project.pbxproj" ]; then
		echo "Generating Xcode project..."
		xcodegen generate
	else
		echo "Xcode project is up to date"
	fi

# Build SotF iOS app for simulator
[group('ios')]
ios-build-sim: ios-build-rust-sim ios-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-ios/ios
	xcodebuild -project SotFPlayer.xcodeproj \
		-scheme SotFPlayer \
		-configuration Release \
		-sdk iphonesimulator \
		-arch arm64 \
		build

# Build SotF iOS app for device
[group('ios')]
ios-build-device: ios-build-rust-device ios-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-ios/ios
	xcodebuild -project SotFPlayer.xcodeproj \
		-scheme SotFPlayer \
		-configuration Release \
		-sdk iphoneos \
		-arch arm64 \
		build

# ----------------------------------------------------------------------
# GPUI UI Kit Showcase (crates/gpui-toolkit/gpui-ui-kit/ios)
# ----------------------------------------------------------------------

# Build Showcase iOS Rust static library for simulator (arm64)
[group('ios')]
showcase-rust-sim:
	cargo build -p gpui-ui-kit-ios-showcase --target aarch64-apple-ios-sim --release

# Build Showcase iOS Rust static library for device (arm64)
[group('ios')]
showcase-rust-device:
	cargo build -p gpui-ui-kit-ios-showcase --target aarch64-apple-ios --release

# Build Showcase iOS Rust lib and copy to Xcode project (simulator)
[group('ios')]
showcase-build-rust-sim: showcase-rust-sim
	#!/usr/bin/env bash
	set -euo pipefail
	IOS_DIR="crates/gpui-toolkit/gpui-ui-kit/ios"
	mkdir -p "$IOS_DIR/lib"
	cp target/aarch64-apple-ios-sim/release/libshowcase_ios.a "$IOS_DIR/lib/"
	echo "Copied libshowcase_ios.a to $IOS_DIR/lib/"

# Build Showcase iOS Rust lib and copy to Xcode project (device)
[group('ios')]
showcase-build-rust-device: showcase-rust-device
	#!/usr/bin/env bash
	set -euo pipefail
	IOS_DIR="crates/gpui-toolkit/gpui-ui-kit/ios"
	mkdir -p "$IOS_DIR/lib"
	cp target/aarch64-apple-ios/release/libshowcase_ios.a "$IOS_DIR/lib/"
	echo "Copied libshowcase_ios.a to $IOS_DIR/lib/"

# Generate Showcase iOS Xcode project with XcodeGen
[group('ios')]
showcase-xcodegen:
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/gpui-toolkit/gpui-ui-kit/ios
	if [ ! -d "GPUIShowcase.xcodeproj" ] || [ "project.yml" -nt "GPUIShowcase.xcodeproj/project.pbxproj" ]; then
		echo "Generating Xcode project..."
		xcodegen generate
	else
		echo "Xcode project is up to date"
	fi

# Build Showcase iOS app for simulator
[group('ios')]
showcase-build-sim: showcase-build-rust-sim showcase-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/gpui-toolkit/gpui-ui-kit/ios
	xcodebuild -project GPUIShowcase.xcodeproj \
		-scheme GPUIShowcase \
		-configuration Release \
		-sdk iphonesimulator \
		-arch arm64 \
		build

# Build Showcase iOS app for device
[group('ios')]
showcase-build-device: showcase-build-rust-device showcase-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/gpui-toolkit/gpui-ui-kit/ios
	xcodebuild -project GPUIShowcase.xcodeproj \
		-scheme GPUIShowcase \
		-configuration Release \
		-sdk iphoneos \
		-arch arm64 \
		build

# ----------------------------------------------------------------------
# Aggregate targets
# ----------------------------------------------------------------------

# Build all iOS apps for simulator
[group('ios')]
ios-sim: ios-build-sim showcase-build-sim
	@echo "All iOS simulator builds complete"

# Build all iOS apps for device
[group('ios')]
ios-device: ios-build-device showcase-build-device
	@echo "All iOS device builds complete"
