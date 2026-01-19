#!/bin/bash

# AutoEQ MenuBar Uninstallation Script
# This script removes the daemon, menubar app, and LaunchAgents

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  AutoEQ MenuBar - Uninstallation                      ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""

DAEMON_PLIST="$HOME/Library/LaunchAgents/org.spinorama.autoeq.daemon.plist"
MENUBAR_PLIST="$HOME/Library/LaunchAgents/org.spinorama.autoeq.menubar.plist"

# Stop services
echo -e "${YELLOW}[1/4] Stopping services...${NC}"
launchctl unload "$DAEMON_PLIST" 2>/dev/null && echo -e "${GREEN}✓ Daemon stopped${NC}" || echo -e "${YELLOW}⚠ Daemon not running${NC}"
launchctl unload "$MENUBAR_PLIST" 2>/dev/null && echo -e "${GREEN}✓ Menubar app stopped${NC}" || echo -e "${YELLOW}⚠ Menubar app not running${NC}"
echo ""

# Remove LaunchAgents
echo -e "${YELLOW}[2/4] Removing LaunchAgents...${NC}"
rm -f "$DAEMON_PLIST" && echo -e "${GREEN}✓ Daemon LaunchAgent removed${NC}"
rm -f "$MENUBAR_PLIST" && echo -e "${GREEN}✓ Menubar LaunchAgent removed${NC}"
echo ""

# Remove daemon binary
echo -e "${YELLOW}[3/4] Removing daemon binary...${NC}"
DAEMON_INSTALL_DIR="$HOME/.local/bin"
rm -f "$DAEMON_INSTALL_DIR/sotf_daemon" && echo -e "${GREEN}✓ Daemon binary removed${NC}"
echo ""

# Remove menubar app
echo -e "${YELLOW}[4/4] Removing menubar application...${NC}"
rm -rf "/Applications/AutoEQMenuBar.app" && echo -e "${GREEN}✓ Menubar app removed${NC}"
echo ""

# Remove socket file
rm -f /tmp/autoeq_audio.sock 2>/dev/null

echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Uninstallation Complete!                             ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""
echo "All components have been removed."
echo ""
echo "Log files (not removed automatically):"
echo "  • /tmp/autoeq-daemon.log"
echo "  • /tmp/autoeq-daemon.error.log"
echo "  • /tmp/autoeq-menubar.log"
echo "  • /tmp/autoeq-menubar.error.log"
echo ""
echo "To remove logs: rm /tmp/autoeq-*.log"
echo ""
