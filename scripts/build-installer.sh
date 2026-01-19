#!/bin/bash
#
# Build unified macOS installer package for SotF
#
# This script builds and packages:
# - SotF ConfigBar (menu bar app)
# - sotf-daemon (background audio service)
# - HAL driver (optional virtual audio device)
#
# Usage: ./build-installer.sh [--sign] [--notarize] [--no-hal]
#

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION="1.0.0"
IDENTIFIER="org.spinorama.sotf"

# Output directories
BUILD_DIR="${PROJECT_ROOT}/target/installer"
PKG_ROOT="${BUILD_DIR}/pkg-root"
PACKAGES_DIR="${BUILD_DIR}/packages"
SCRIPTS_DIR="${BUILD_DIR}/scripts"
RESOURCES_DIR="${BUILD_DIR}/Resources"
OUTPUT_PKG="${BUILD_DIR}/SotF-${VERSION}.pkg"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
SIGN=false
NOTARIZE=false
BUILD_HAL=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --sign)
            SIGN=true
            shift
            ;;
        --notarize)
            NOTARIZE=true
            SIGN=true  # Notarization requires signing
            shift
            ;;
        --no-hal)
            BUILD_HAL=false
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--sign] [--notarize] [--no-hal]"
            echo ""
            echo "Options:"
            echo "  --sign       Sign the package with Developer ID"
            echo "  --notarize   Notarize the package (implies --sign)"
            echo "  --no-hal     Skip building HAL driver component"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  SotF macOS Installer Builder                                 ║${NC}"
echo -e "${BLUE}║  Version: ${VERSION}                                              ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Clean previous build
echo -e "${YELLOW}[1/8] Cleaning previous build...${NC}"
rm -rf "${BUILD_DIR}"
mkdir -p "${PKG_ROOT}"
mkdir -p "${PACKAGES_DIR}"
mkdir -p "${SCRIPTS_DIR}"
mkdir -p "${RESOURCES_DIR}"
echo -e "${GREEN}✓ Build directory prepared${NC}"

# Build the Rust daemon
echo -e "${YELLOW}[2/8] Building sotf-daemon...${NC}"
cd "${PROJECT_ROOT}"
cargo build --release -p sotf-daemon --features hal
if [ ! -f "${PROJECT_ROOT}/target/release/sotf-daemon" ]; then
    echo -e "${RED}✗ Failed to build sotf-daemon${NC}"
    exit 1
fi
echo -e "${GREEN}✓ sotf-daemon built successfully${NC}"

# Build ConfigBar
echo -e "${YELLOW}[3/8] Building SotF ConfigBar...${NC}"
cd "${PROJECT_ROOT}/crates/daemon/configbar"
./scripts/build.sh
CONFIGBAR_APP="${PROJECT_ROOT}/crates/daemon/target/sotf-configbar.app"
if [ ! -d "${CONFIGBAR_APP}" ]; then
    echo -e "${RED}✗ Failed to build ConfigBar app${NC}"
    exit 1
fi
echo -e "${GREEN}✓ ConfigBar built successfully${NC}"

# Build HAL driver (optional)
if [ "$BUILD_HAL" = true ]; then
    echo -e "${YELLOW}[4/8] Building HAL driver...${NC}"
    cd "${PROJECT_ROOT}"
    cargo build --release -p driver-hal
    cd "${PROJECT_ROOT}/crates/driver-hal"
    ./scripts/build_driver.sh
    HAL_DRIVER="${PROJECT_ROOT}/crates/target/sotf.driver"
    if [ ! -d "${HAL_DRIVER}" ]; then
        # Try alternate location
        HAL_DRIVER="${PROJECT_ROOT}/target/sotf.driver"
    fi
    if [ -d "${HAL_DRIVER}" ]; then
        echo -e "${GREEN}✓ HAL driver built successfully${NC}"
    else
        echo -e "${YELLOW}⚠ HAL driver not found, skipping...${NC}"
        BUILD_HAL=false
    fi
else
    echo -e "${YELLOW}[4/8] Skipping HAL driver (--no-hal specified)${NC}"
fi

# Stage files for packaging
echo -e "${YELLOW}[5/8] Staging files for packaging...${NC}"

# Stage ConfigBar
mkdir -p "${PKG_ROOT}/Applications"
cp -R "${CONFIGBAR_APP}" "${PKG_ROOT}/Applications/SotF ConfigBar.app"
echo "  ✓ ConfigBar staged to /Applications"

# Stage daemon
mkdir -p "${PKG_ROOT}/usr/local/bin"
cp "${PROJECT_ROOT}/target/release/sotf-daemon" "${PKG_ROOT}/usr/local/bin/"
echo "  ✓ Daemon staged to /usr/local/bin"

# Stage HAL driver (if built)
if [ "$BUILD_HAL" = true ] && [ -d "${HAL_DRIVER}" ]; then
    mkdir -p "${PKG_ROOT}/Library/Audio/Plug-Ins/HAL"
    cp -R "${HAL_DRIVER}" "${PKG_ROOT}/Library/Audio/Plug-Ins/HAL/"
    echo "  ✓ HAL driver staged to /Library/Audio/Plug-Ins/HAL"
fi

echo -e "${GREEN}✓ All files staged${NC}"

# Create post-install scripts
echo -e "${YELLOW}[6/8] Creating post-install scripts...${NC}"

# Daemon postinstall script
mkdir -p "${SCRIPTS_DIR}/daemon"
cat > "${SCRIPTS_DIR}/daemon/postinstall" << 'DAEMON_POST'
#!/bin/bash
# Post-install script for sotf-daemon

USER_HOME=$(eval echo ~$USER)
LAUNCHAGENTS_DIR="$USER_HOME/Library/LaunchAgents"
PLIST_FILE="$LAUNCHAGENTS_DIR/org.spinorama.sotf.daemon.plist"

# Create LaunchAgents directory if needed
mkdir -p "$LAUNCHAGENTS_DIR"

# Create LaunchAgent plist
cat > "$PLIST_FILE" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.spinorama.sotf.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/sotf-daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>/tmp/sotf-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/sotf-daemon.error.log</string>
</dict>
</plist>
EOF

# Load the agent
launchctl load "$PLIST_FILE" 2>/dev/null || true

exit 0
DAEMON_POST
chmod +x "${SCRIPTS_DIR}/daemon/postinstall"
echo "  ✓ Daemon postinstall script created"

# ConfigBar postinstall script
mkdir -p "${SCRIPTS_DIR}/configbar"
cat > "${SCRIPTS_DIR}/configbar/postinstall" << 'CONFIGBAR_POST'
#!/bin/bash
# Post-install script for SotF ConfigBar

USER_HOME=$(eval echo ~$USER)
LAUNCHAGENTS_DIR="$USER_HOME/Library/LaunchAgents"
PLIST_FILE="$LAUNCHAGENTS_DIR/org.spinorama.sotf.configbar.plist"

# Create LaunchAgents directory if needed
mkdir -p "$LAUNCHAGENTS_DIR"

# Create LaunchAgent plist
cat > "$PLIST_FILE" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.spinorama.sotf.configbar</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Applications/SotF ConfigBar.app/Contents/MacOS/sotf-configbar</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>LimitLoadToSessionType</key>
    <array>
        <string>Aqua</string>
    </array>
    <key>StandardOutPath</key>
    <string>/tmp/sotf-configbar.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/sotf-configbar.error.log</string>
</dict>
</plist>
EOF

# Load the agent and start the app
launchctl load "$PLIST_FILE" 2>/dev/null || true

# Start the app
open "/Applications/SotF ConfigBar.app"

exit 0
CONFIGBAR_POST
chmod +x "${SCRIPTS_DIR}/configbar/postinstall"
echo "  ✓ ConfigBar postinstall script created"

# HAL driver postinstall script (if building HAL)
if [ "$BUILD_HAL" = true ]; then
    mkdir -p "${SCRIPTS_DIR}/hal"
    cat > "${SCRIPTS_DIR}/hal/postinstall" << 'HAL_POST'
#!/bin/bash
# Post-install script for SotF HAL Driver

DRIVER_PATH="/Library/Audio/Plug-Ins/HAL/sotf.driver"

# Sign the driver with ad-hoc signature
codesign --force --deep --sign - "$DRIVER_PATH" 2>/dev/null || true

# Restart CoreAudio to load the new driver
# Note: This requires root privileges (installer runs as root)
killall coreaudiod 2>/dev/null || true

echo "HAL driver installed. You may need to log out and back in for changes to take effect."

exit 0
HAL_POST
    chmod +x "${SCRIPTS_DIR}/hal/postinstall"
    echo "  ✓ HAL driver postinstall script created"
fi

echo -e "${GREEN}✓ Post-install scripts created${NC}"

# Create installer resources
echo -e "${YELLOW}[7/8] Creating installer resources...${NC}"

# Welcome.html
cat > "${RESOURCES_DIR}/Welcome.html" << 'WELCOME'
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            padding: 20px;
            line-height: 1.6;
        }
        h1 { color: #333; }
        h2 { color: #666; margin-top: 20px; }
        ul { padding-left: 20px; }
        .note {
            background: #f5f5f5;
            padding: 15px;
            border-radius: 8px;
            margin-top: 20px;
        }
    </style>
</head>
<body>
    <h1>Welcome to SotF Installer</h1>
    <p><strong>Sound of the Future</strong> - Advanced Audio Processing for macOS</p>

    <h2>What will be installed:</h2>
    <ul>
        <li><strong>SotF ConfigBar</strong> - Menu bar app for audio configuration</li>
        <li><strong>sotf-daemon</strong> - Background audio processing service</li>
        <li><strong>HAL Driver</strong> (optional) - Virtual audio device for system-wide processing</li>
    </ul>

    <h2>BlackHole Support</h2>
    <p>If you have BlackHole installed, SotF can use it as an audio source instead of the HAL driver.</p>

    <div class="note">
        <strong>Note:</strong> The HAL driver requires administrator privileges to install.
        You can skip it if you prefer to use BlackHole.
    </div>
</body>
</html>
WELCOME
echo "  ✓ Welcome.html created"

# Conclusion.html
cat > "${RESOURCES_DIR}/Conclusion.html" << 'CONCLUSION'
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            padding: 20px;
            line-height: 1.6;
        }
        h1 { color: #333; }
        h2 { color: #666; margin-top: 20px; }
        .success {
            color: #28a745;
            font-size: 18px;
        }
        code {
            background: #f5f5f5;
            padding: 2px 6px;
            border-radius: 4px;
        }
        .steps {
            background: #f5f5f5;
            padding: 15px;
            border-radius: 8px;
            margin-top: 15px;
        }
        .steps ol {
            margin: 0;
            padding-left: 20px;
        }
    </style>
</head>
<body>
    <h1 class="success">✓ Installation Complete!</h1>

    <h2>Getting Started</h2>
    <div class="steps">
        <ol>
            <li>Look for the SotF icon in your menu bar</li>
            <li>Click it to open the configuration window</li>
            <li>Select your audio source (HAL Driver or BlackHole)</li>
            <li>Choose your output device</li>
            <li>Configure audio processing plugins as needed</li>
        </ol>
    </div>

    <h2>If Using BlackHole</h2>
    <p>Set BlackHole as your macOS System Output in <strong>System Preferences → Sound</strong></p>

    <h2>Troubleshooting</h2>
    <ul>
        <li>Logs: <code>/tmp/sotf-daemon.log</code> and <code>/tmp/sotf-configbar.log</code></li>
        <li>If the HAL driver doesn't appear, try logging out and back in</li>
    </ul>

    <h2>Uninstalling</h2>
    <p>Run: <code>/usr/local/bin/uninstall-sotf.sh</code></p>
</body>
</html>
CONCLUSION
echo "  ✓ Conclusion.html created"

# License (simple MIT-style for now)
cat > "${RESOURCES_DIR}/License.txt" << 'LICENSE'
SotF - Sound of the Future
Copyright (c) 2024 Spinorama

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
LICENSE
echo "  ✓ License.txt created"

echo -e "${GREEN}✓ Installer resources created${NC}"

# Build packages
echo -e "${YELLOW}[8/8] Building installer packages...${NC}"

# Create component packages
echo "  Building ConfigBar component package..."
pkgbuild \
    --root "${PKG_ROOT}/Applications" \
    --install-location "/Applications" \
    --scripts "${SCRIPTS_DIR}/configbar" \
    --identifier "${IDENTIFIER}.configbar" \
    --version "${VERSION}" \
    "${PACKAGES_DIR}/SotFConfigBar.pkg"

echo "  Building Daemon component package..."
pkgbuild \
    --root "${PKG_ROOT}/usr" \
    --install-location "/usr" \
    --scripts "${SCRIPTS_DIR}/daemon" \
    --identifier "${IDENTIFIER}.daemon" \
    --version "${VERSION}" \
    "${PACKAGES_DIR}/SotFDaemon.pkg"

if [ "$BUILD_HAL" = true ] && [ -d "${PKG_ROOT}/Library/Audio/Plug-Ins/HAL" ]; then
    echo "  Building HAL Driver component package..."
    pkgbuild \
        --root "${PKG_ROOT}/Library/Audio/Plug-Ins/HAL" \
        --install-location "/Library/Audio/Plug-Ins/HAL" \
        --scripts "${SCRIPTS_DIR}/hal" \
        --identifier "${IDENTIFIER}.hal" \
        --version "${VERSION}" \
        "${PACKAGES_DIR}/SotFHALDriver.pkg"
fi

# Create Distribution.xml
echo "  Creating distribution file..."

if [ "$BUILD_HAL" = true ]; then
    HAL_CHOICE_XML='
    <choice id="hal" title="HAL Driver" description="Virtual audio device for system-wide audio capture. Optional if you prefer to use BlackHole.">
        <pkg-ref id="org.spinorama.sotf.hal"/>
    </choice>
    <pkg-ref id="org.spinorama.sotf.hal" version="'"${VERSION}"'" installKBytes="500">SotFHALDriver.pkg</pkg-ref>'
    HAL_LINE_XML='<line choice="hal"/>'
else
    HAL_CHOICE_XML=""
    HAL_LINE_XML=""
fi

cat > "${BUILD_DIR}/Distribution.xml" << DISTXML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>SotF - Sound of the Future</title>
    <organization>${IDENTIFIER}</organization>
    <welcome file="Welcome.html"/>
    <license file="License.txt"/>
    <conclusion file="Conclusion.html"/>

    <options customize="allow" require-scripts="false" hostArchitectures="arm64,x86_64"/>

    <domains enable_localSystem="true" enable_currentUserHome="false"/>

    <choices-outline>
        <line choice="configbar"/>
        <line choice="daemon"/>
        ${HAL_LINE_XML}
    </choices-outline>

    <choice id="configbar" title="SotF ConfigBar" description="Menu bar application for controlling audio processing. Required." start_selected="true" start_enabled="false">
        <pkg-ref id="${IDENTIFIER}.configbar"/>
    </choice>

    <choice id="daemon" title="Audio Daemon" description="Background service for audio processing. Required." start_selected="true" start_enabled="false">
        <pkg-ref id="${IDENTIFIER}.daemon"/>
    </choice>
    ${HAL_CHOICE_XML}

    <pkg-ref id="${IDENTIFIER}.configbar" version="${VERSION}" installKBytes="1000">SotFConfigBar.pkg</pkg-ref>
    <pkg-ref id="${IDENTIFIER}.daemon" version="${VERSION}" installKBytes="5000">SotFDaemon.pkg</pkg-ref>
</installer-gui-script>
DISTXML

# Build product package
echo "  Building final installer package..."
productbuild \
    --distribution "${BUILD_DIR}/Distribution.xml" \
    --package-path "${PACKAGES_DIR}" \
    --resources "${RESOURCES_DIR}" \
    "${OUTPUT_PKG}"

echo -e "${GREEN}✓ Installer package created${NC}"

# Sign if requested
if [ "$SIGN" = true ]; then
    echo -e "${YELLOW}Signing package...${NC}"
    DEVELOPER_ID="${DEVELOPER_ID:-}"
    if [ -z "$DEVELOPER_ID" ]; then
        echo -e "${YELLOW}⚠ DEVELOPER_ID not set. Skipping signing.${NC}"
        echo "  Set DEVELOPER_ID environment variable to sign the package."
    else
        SIGNED_PKG="${BUILD_DIR}/SotF-${VERSION}-signed.pkg"
        productsign \
            --sign "Developer ID Installer: ${DEVELOPER_ID}" \
            "${OUTPUT_PKG}" \
            "${SIGNED_PKG}"
        mv "${SIGNED_PKG}" "${OUTPUT_PKG}"
        echo -e "${GREEN}✓ Package signed${NC}"
    fi
fi

# Notarize if requested
if [ "$NOTARIZE" = true ]; then
    echo -e "${YELLOW}Notarizing package...${NC}"
    APPLE_ID="${APPLE_ID:-}"
    TEAM_ID="${TEAM_ID:-}"
    if [ -z "$APPLE_ID" ] || [ -z "$TEAM_ID" ]; then
        echo -e "${YELLOW}⚠ APPLE_ID or TEAM_ID not set. Skipping notarization.${NC}"
        echo "  Set APPLE_ID and TEAM_ID environment variables to notarize."
    else
        xcrun notarytool submit "${OUTPUT_PKG}" \
            --apple-id "${APPLE_ID}" \
            --team-id "${TEAM_ID}" \
            --wait
        xcrun stapler staple "${OUTPUT_PKG}"
        echo -e "${GREEN}✓ Package notarized${NC}"
    fi
fi

# Copy uninstaller script to output
cp "${SCRIPT_DIR}/uninstall-sotf.sh" "${BUILD_DIR}/" 2>/dev/null || true

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Build Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Installer package: ${OUTPUT_PKG}"
echo ""
echo "To test the installer:"
echo "  open ${OUTPUT_PKG}"
echo ""
echo "Package contents:"
ls -lh "${OUTPUT_PKG}"
echo ""
