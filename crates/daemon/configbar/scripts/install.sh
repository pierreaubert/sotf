#!/bin/bash
set -e

echo "Installing AutoEQMenuBar Menu Bar Application..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
APP_BUNDLE="$SCRIPT_DIR/AutoEQMenuBar.app"
LAUNCHAGENT_PLIST="$SCRIPT_DIR/org.spinorama.autoeq.menubar.plist"

# Destination paths
APPS_DIR="/Applications"
LAUNCHAGENTS_DIR="$HOME/Library/LaunchAgents"

echo -e "${BLUE}Installation Steps:${NC}"
echo "1. Copy AutoEQ.app to /Applications"
echo "2. Install LaunchAgent for auto-start"
echo "3. Start the application"
echo ""

# Check if app bundle exists
if [ ! -d "$APP_BUNDLE" ]; then
    echo -e "${RED}Error: AutoEQMenuBar.app not found at $APP_BUNDLE${NC}"
    echo "Please run this script from the MenuBarApp directory."
    exit 1
fi

# Step 1: Copy app to Applications
echo -e "${BLUE}[1/3] Copying AutoEQMenuBar.app to /Applications...${NC}"

# Stop the app if it's already running
if pgrep -x "AutoEQMenuBar" > /dev/null; then
    echo "Stopping currently running AutoEQMenuBar..."
    killall AutoEQMenuBar 2>/dev/null || true
    sleep 1
fi

# Remove old version if exists
if [ -d "$APPS_DIR/AutoEQMenuBar.app" ]; then
    echo "Removing old version..."
    rm -rf "$APPS_DIR/AutoEQMenuBar.app"
fi

# Copy new version
cp -R "$APP_BUNDLE" "$APPS_DIR/"
echo -e "${GREEN}✓ App copied to /Applications/AutoEQMenuBar.app${NC}"

# Step 2: Install LaunchAgent
echo -e "${BLUE}[2/3] Installing LaunchAgent...${NC}"

# Create LaunchAgents directory if it doesn't exist
mkdir -p "$LAUNCHAGENTS_DIR"

# Unload old LaunchAgent if it exists
if [ -f "$LAUNCHAGENTS_DIR/org.spinorama.autoeq.menubar.plist" ]; then
    echo "Unloading existing LaunchAgent..."
    launchctl unload "$LAUNCHAGENTS_DIR/org.spinorama.autoeq.menubar.plist" 2>/dev/null || true
fi

# Copy LaunchAgent plist
cp "$LAUNCHAGENT_PLIST" "$LAUNCHAGENTS_DIR/"
echo -e "${GREEN}✓ LaunchAgent installed${NC}"

# Step 3: Load LaunchAgent and start app
echo -e "${BLUE}[3/3] Starting AutoEQMenuBar...${NC}"

# Load the LaunchAgent
launchctl load "$LAUNCHAGENTS_DIR/org.spinorama.autoeq.menubar.plist"

# Give it a moment to start
sleep 2

# Check if app is running
if pgrep -x "AutoEQMenuBar" > /dev/null; then
    echo -e "${GREEN}✓ AutoEQMenuBar is now running${NC}"
else
    echo -e "${YELLOW}⚠ AutoEQMenuBar may not have started. Trying manual start...${NC}"
    open "$APPS_DIR/AutoEQMenuBar.app"
    sleep 2
    if pgrep -x "AutoEQMenuBar" > /dev/null; then
        echo -e "${GREEN}✓ AutoEQMenuBar is now running${NC}"
    else
        echo -e "${RED}✗ Failed to start AutoEQMenuBar. Check logs at /tmp/autoeq-menubar.error.log${NC}"
    fi
fi

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Installation Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo ""
echo "AutoEQMenuBar menu bar app is now installed and running."
echo "Look for the waveform icon (♪) in your menu bar."
echo ""
echo -e "${BLUE}What's next?${NC}"
echo "• Click the menu bar icon to access controls"
echo "• The app will auto-start on login"
echo "• Check logs: /tmp/autoeq-menubar.log"
echo ""
echo -e "${BLUE}To uninstall:${NC}"
echo "  launchctl unload ~/Library/LaunchAgents/org.spinorama.autoeq.menubar.plist"
echo "  rm ~/Library/LaunchAgents/org.spinorama.autoeq.menubar.plist"
echo "  rm -rf /Applications/AutoEQMenuBar.app"
echo ""
echo -e "${BLUE}To rebuild after changes:${NC}"
echo "  cd $SCRIPT_DIR"
echo "  swiftc -o AutoEQMenuBar.app/Contents/MacOS/AutoEQMenuBar AutoEQMenuBar/main.swift"
echo "  ./install.sh"
echo ""
