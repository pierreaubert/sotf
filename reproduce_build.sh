#!/bin/bash
set -e

# Setup paths
PROJECT_ROOT=$(pwd)
BUILD_DIR="$PROJECT_ROOT/target/release"
TEMP_BUNDLE_DIR="$PROJECT_ROOT/target/temp_driver_bundle"
DRIVER_BUNDLE="$TEMP_BUNDLE_DIR/sotf.driver"
VERSION="0.1.0"
HAL_BUNDLE_ID="org.spinorama.sotf-hal"

# Build cargo
cargo build --release -p driver-hal

# Create structure
rm -rf "$TEMP_BUNDLE_DIR"
mkdir -p "$DRIVER_BUNDLE/Contents/MacOS"
mkdir -p "$DRIVER_BUNDLE/Contents/Resources"

# Copy binary
cp "$BUILD_DIR/libsotf_hal.dylib" "$DRIVER_BUNDLE/Contents/MacOS/sotf_driver"

# Install name tool
install_name_tool -id "@rpath/sotf_driver" "$DRIVER_BUNDLE/Contents/MacOS/sotf_driver"

# Info.plist
cat > "$DRIVER_BUNDLE/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>

    <key>CFBundleExecutable</key>
    <string>sotf_driver</string>

    <key>CFBundleIdentifier</key>
    <string>${HAL_BUNDLE_ID}</string>

    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>

    <key>CFBundleName</key>
    <string>SotF HAL</string>

    <key>CFBundlePackageType</key>
    <string>drvr</string>

    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>

    <key>CFBundleSignature</key>
    <string>????</string>

    <key>CFBundleVersion</key>
    <string>1</string>

    <key>CFPlugInDynamicRegisterFunction</key>
    <string></string>

    <key>CFPlugInDynamicRegistration</key>
    <string>NO</string>

    <key>CFPlugInFactories</key>
    <dict>
        <!-- Factory UUID for our driver - must match exported symbol -->
        <key>5A4E28B8-93F4-4B8A-B5E2-3D9F6A8C7E01</key>
        <string>SotFHALDriverFactory</string>
    </dict>

    <key>CFPlugInTypes</key>
    <dict>
        <!-- kAudioHardwarePlugInTypeID - for HAL driver plugins -->
        <key>FBC16C0B-8A0D-11D4-91F0-0050E4C10664</key>
        <array>
            <string>5A4E28B8-93F4-4B8A-B5E2-3D9F6A8C7E01</string>
        </array>
    </dict>

    <key>SotFHalPlugIn</key>
    <dict>
        <key>Name</key>
        <string>SotFHal</string>

        <key>Manufacturer</key>
        <string>org.spinorama</string>

        <key>Version</key>
        <string>${VERSION}</string>
    </dict>

    <key>NSHumanReadableCopyright</key>
    <string>Copyright 2025 Pierre F. Aubert pierre@spinorama.org All rights reserved.</string>

    <key>OSBundleLibraries</key>
    <dict>
        <key>com.apple.CoreAudio</key>
        <string>1.0</string>
    </dict>
</dict>
</plist>
EOF

echo "Build complete."
ls -R "$TEMP_BUNDLE_DIR"
otool -L "$DRIVER_BUNDLE/Contents/MacOS/sotf_driver"
nm -gU "$DRIVER_BUNDLE/Contents/MacOS/sotf_driver" | grep SotFHALDriverFactory
