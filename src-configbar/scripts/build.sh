#!/bin/bash
set -e

echo "Building AutoEQ Config Bar Application..."

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
APP_DIR="$SCRIPT_DIR/../../target/sotf-configbar.app"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}[1/4] Creating app bundle structure...${NC}"
mkdir -p "$APP_DIR/Contents/"{MacOS,Resources}
echo -e "${GREEN}✓ Directory structure created${NC}"

echo -e "${BLUE}[2/4] Creating Info.plist...${NC}"
cat > "$APP_DIR/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>sotf-configbar</string>
	<key>CFBundleIdentifier</key>
	<string>org.spinorama.sotf.configbar</string>
	<key>CFBundleName</key>
	<string>SotF ConfigBar</string>
	<key>CFBundleDisplayName</key>
	<string>Sound of the Future - ConfigBar</string>
	<key>CFBundleVersion</key>
	<string>1.0.0</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0.0</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key>
	<true/>
</dict>
</plist>
EOF
echo -e "${GREEN}✓ Info.plist created${NC}"

echo -e "${BLUE}[3/4] Compiling Swift source...${NC}"
# Use the new ConfigBarApp.swift instead of main.swift
swiftc \
    -o "$APP_DIR/Contents/MacOS/sotf-configbar" \
    "$SCRIPT_DIR/../src/ConfigBar.swift" \
    -framework SwiftUI \
    -framework WebKit \
    -framework UserNotifications
echo -e "${GREEN}✓ Compilation successful${NC}"

echo -e "${BLUE}[4/4] Creating app icon...${NC}"
"$SCRIPT_DIR/create_icon.sh"

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Build Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo ""
echo "App bundle created at: $APP_DIR"
echo ""
echo "Next steps:"
echo "  1. Build the audio daemon: cd ../src-audio && cargo build --release --bin sotf_daemon"
echo "  2. Start the daemon: ../src-audio/target/release/sotf_daemon &"
echo "  3. Test configbar app: open $APP_DIR"
echo "  4. To install: ./install.sh"
echo ""
