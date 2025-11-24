#!/bin/bash
set -e

# AutoEQ MenuBar Complete Installation Script
# This script:
# 1. Builds the audio daemon (sotf_daemon)
# 2. Builds the menubar application
# 3. Installs both to appropriate locations
# 4. Sets up LaunchAgents for auto-start

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  AutoEQ MenuBar - Complete Installation              ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""

# Step 1: Build the audio daemon
echo -e "${YELLOW}[1/5] Building audio daemon (sotf_daemon)...${NC}"
cd "$PROJECT_ROOT/src-audio"
cargo build --release --bin sotf_daemon
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Audio daemon built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build audio daemon${NC}"
    exit 1
fi
echo ""

# Step 2: Build the menubar app
echo -e "${YELLOW}[2/5] Building menubar application...${NC}"
cd "$SCRIPT_DIR"
./build.sh
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Menubar app built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build menubar app${NC}"
    exit 1
fi
echo ""

# Step 3: Install the daemon
echo -e "${YELLOW}[3/5] Installing audio daemon...${NC}"
DAEMON_INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$DAEMON_INSTALL_DIR"
cp "$PROJECT_ROOT/src-audio/target/release/sotf_daemon" "$DAEMON_INSTALL_DIR/"
chmod +x "$DAEMON_INSTALL_DIR/sotf_daemon"

# Create LaunchAgent for daemon
DAEMON_PLIST="$HOME/Library/LaunchAgents/org.spinorama.autoeq.daemon.plist"
cat > "$DAEMON_PLIST" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.spinorama.autoeq.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>$DAEMON_INSTALL_DIR/sotf_daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>/tmp/autoeq-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/autoeq-daemon.error.log</string>
</dict>
</plist>
EOF

echo -e "${GREEN}✓ Daemon installed to $DAEMON_INSTALL_DIR${NC}"
echo ""

# Step 4: Install the menubar app
echo -e "${YELLOW}[4/5] Installing menubar application...${NC}"
rm -rf "/Applications/AutoEQMenuBar.app"
cp -R "$SCRIPT_DIR/../AutoEQMenuBar.app" "/Applications/"
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Menubar app installed to /Applications${NC}"
else
    echo -e "${RED}✗ Failed to install menubar app${NC}"
    exit 1
fi
echo ""

# Create LaunchAgent for menubar app
MENUBAR_PLIST="$HOME/Library/LaunchAgents/org.spinorama.autoeq.menubar.plist"
cat > "$MENUBAR_PLIST" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.spinorama.autoeq.menubar</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Applications/AutoEQMenuBar.app/Contents/MacOS/AutoEQMenuBar</string>
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
    <string>/tmp/autoeq-menubar.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/autoeq-menubar.error.log</string>
</dict>
</plist>
EOF

# Step 5: Start the services
echo -e "${YELLOW}[5/5] Starting services...${NC}"

# Stop if already running
launchctl unload "$DAEMON_PLIST" 2>/dev/null || true
launchctl unload "$MENUBAR_PLIST" 2>/dev/null || true

# Start daemon first
launchctl load "$DAEMON_PLIST"
echo -e "${GREEN}✓ Audio daemon started${NC}"

# Wait a moment for daemon to initialize
sleep 2

# Start menubar app
launchctl load "$MENUBAR_PLIST"
echo -e "${GREEN}✓ Menubar app started${NC}"

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Installation Complete!                               ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""
echo "The AutoEQ menubar app should now appear in your menu bar."
echo ""
echo "Installed components:"
echo "  • Audio daemon: $DAEMON_INSTALL_DIR/sotf_daemon"
echo "  • Menubar app: /Applications/AutoEQMenuBar.app"
echo "  • LaunchAgents:"
echo "    - org.spinorama.autoeq.daemon"
echo "    - org.spinorama.autoeq.menubar"
echo ""
echo "Logs:"
echo "  • Daemon: /tmp/autoeq-daemon.log"
echo "  • Menubar: /tmp/autoeq-menubar.log"
echo ""
echo "To uninstall:"
echo "  ./uninstall.sh"
echo ""
