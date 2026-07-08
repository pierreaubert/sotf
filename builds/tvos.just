# tvOS (Apple TV) builds
#
# tvOS is a Tier 3 Rust target — no pre-built stdlib.
# Requires: nightly toolchain + rust-src component.
#   rustup toolchain install nightly
#   rustup component add rust-src --toolchain nightly

# ----------------------------------------------------------------------
# SOTF tvOS App (crates/app-tvos)
# ----------------------------------------------------------------------

# Build SotF tvOS Rust static library for simulator (arm64)
[group('tvos')]
tvos-rust-sim:
	cargo +nightly build -p sotf-tvos --target aarch64-apple-tvos-sim --release -Zbuild-std

# Build SotF tvOS Rust static library for device (arm64)
[group('tvos')]
tvos-rust-device:
	cargo +nightly build -p sotf-tvos --target aarch64-apple-tvos --release -Zbuild-std

# Generate SotF tvOS Xcode project with XcodeGen
[group('tvos')]
tvos-xcodegen:
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-tvos/tvos
	if [ ! -d "SotFTV.xcodeproj" ] || [ "project.yml" -nt "SotFTV.xcodeproj/project.pbxproj" ]; then
		echo "Generating Xcode project..."
		xcodegen generate
	else
		echo "Xcode project is up to date"
	fi

# Build SotF tvOS app for simulator.
# The Xcode project runs build-rust.sh as a pre-build phase, which builds the
# static library into Xcode's DERIVED_FILE_DIR/rust/; no manual copy to tvos/lib
# is required.
[group('tvos')]
tvos-build-sim: tvos-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-tvos/tvos
	xcodebuild -project SotFTV.xcodeproj \
		-scheme SotFTV \
		-configuration Release \
		-sdk appletvsimulator \
		-arch arm64 \
		build

# Build SotF tvOS app for device.
[group('tvos')]
tvos-build-device: tvos-xcodegen
	#!/usr/bin/env bash
	set -euo pipefail
	cd crates/app-tvos/tvos
	xcodebuild -project SotFTV.xcodeproj \
		-scheme SotFTV \
		-configuration Release \
		-sdk appletvos \
		-arch arm64 \
		build

# Build tvOS for simulator (shorthand)
[group('tvos')]
tvos-sim: tvos-build-sim
	@echo "tvOS simulator build complete"

# Build tvOS for device (shorthand)
[group('tvos')]
tvos-device: tvos-build-device
	@echo "tvOS device build complete"
