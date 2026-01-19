#!/bin/bash
#
# SotF Uninstaller Script
#
# This script removes all SotF components:
# - SotF ConfigBar app
# - sotf-daemon binary and LaunchAgent
# - HAL driver (requires sudo)
# - Socket files and logs
#
# Usage: ./uninstall-sotf.sh [--keep-logs]
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
KEEP_LOGS=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --keep-logs)
            KEEP_LOGS=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--keep-logs]"
            echo ""
            echo "Options:"
            echo "  --keep-logs    Don't remove log files"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  SotF Uninstaller                                             ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Paths
USER_HOME="$HOME"
LAUNCHAGENTS_DIR="${USER_HOME}/Library/LaunchAgents"
DAEMON_PLIST="${LAUNCHAGENTS_DIR}/org.spinorama.sotf.daemon.plist"
CONFIGBAR_PLIST="${LAUNCHAGENTS_DIR}/org.spinorama.sotf.configbar.plist"
CONFIGBAR_APP="/Applications/SotF ConfigBar.app"
DAEMON_BIN="/usr/local/bin/sotf-daemon"
HAL_DRIVER="/Library/Audio/Plug-Ins/HAL/sotf.driver"
SOCKET_PATH="/tmp/autoeq_audio.sock"

# Track if we need sudo for HAL driver
NEED_SUDO=false

echo -e "${YELLOW}[1/6] Stopping running processes...${NC}"

# Stop ConfigBar
if pgrep -x "sotf-configbar" > /dev/null 2>&1; then
    killall "sotf-configbar" 2>/dev/null || true
    echo "  ✓ ConfigBar stopped"
else
    echo "  - ConfigBar not running"
fi

# Stop daemon
if pgrep -x "sotf-daemon" > /dev/null 2>&1; then
    killall "sotf-daemon" 2>/dev/null || true
    echo "  ✓ Daemon stopped"
else
    echo "  - Daemon not running"
fi

echo -e "${GREEN}✓ Processes stopped${NC}"

echo -e "${YELLOW}[2/6] Unloading LaunchAgents...${NC}"

# Unload daemon LaunchAgent
if [ -f "${DAEMON_PLIST}" ]; then
    launchctl unload "${DAEMON_PLIST}" 2>/dev/null || true
    echo "  ✓ Daemon LaunchAgent unloaded"
else
    echo "  - Daemon LaunchAgent not found"
fi

# Unload ConfigBar LaunchAgent
if [ -f "${CONFIGBAR_PLIST}" ]; then
    launchctl unload "${CONFIGBAR_PLIST}" 2>/dev/null || true
    echo "  ✓ ConfigBar LaunchAgent unloaded"
else
    echo "  - ConfigBar LaunchAgent not found"
fi

echo -e "${GREEN}✓ LaunchAgents unloaded${NC}"

echo -e "${YELLOW}[3/6] Removing LaunchAgent plists...${NC}"

if [ -f "${DAEMON_PLIST}" ]; then
    rm -f "${DAEMON_PLIST}"
    echo "  ✓ Daemon plist removed"
fi

if [ -f "${CONFIGBAR_PLIST}" ]; then
    rm -f "${CONFIGBAR_PLIST}"
    echo "  ✓ ConfigBar plist removed"
fi

echo -e "${GREEN}✓ LaunchAgent plists removed${NC}"

echo -e "${YELLOW}[4/6] Removing applications...${NC}"

# Remove ConfigBar app
if [ -d "${CONFIGBAR_APP}" ]; then
    rm -rf "${CONFIGBAR_APP}"
    echo "  ✓ ConfigBar app removed"
else
    echo "  - ConfigBar app not found"
fi

# Remove daemon binary
if [ -f "${DAEMON_BIN}" ]; then
    # May need sudo
    if rm -f "${DAEMON_BIN}" 2>/dev/null; then
        echo "  ✓ Daemon binary removed"
    else
        echo "  - Daemon binary requires sudo to remove"
        NEED_SUDO=true
    fi
else
    echo "  - Daemon binary not found"
fi

echo -e "${GREEN}✓ Applications removed${NC}"

echo -e "${YELLOW}[5/6] Removing HAL driver...${NC}"

if [ -d "${HAL_DRIVER}" ]; then
    if sudo rm -rf "${HAL_DRIVER}" 2>/dev/null; then
        echo "  ✓ HAL driver removed"
        # Restart CoreAudio
        echo "  Restarting CoreAudio..."
        sudo killall coreaudiod 2>/dev/null || true
        echo "  ✓ CoreAudio restarted"
    else
        echo -e "${YELLOW}  ⚠ HAL driver requires sudo to remove${NC}"
        NEED_SUDO=true
    fi
else
    echo "  - HAL driver not found"
fi

echo -e "${GREEN}✓ HAL driver removal complete${NC}"

echo -e "${YELLOW}[6/6] Cleaning up...${NC}"

# Remove socket
if [ -e "${SOCKET_PATH}" ]; then
    rm -f "${SOCKET_PATH}"
    echo "  ✓ Socket file removed"
fi

# Remove logs (unless --keep-logs)
if [ "$KEEP_LOGS" = false ]; then
    rm -f /tmp/sotf-daemon.log 2>/dev/null || true
    rm -f /tmp/sotf-daemon.error.log 2>/dev/null || true
    rm -f /tmp/sotf-configbar.log 2>/dev/null || true
    rm -f /tmp/sotf-configbar.error.log 2>/dev/null || true
    rm -f /tmp/autoeq-daemon.log 2>/dev/null || true
    rm -f /tmp/autoeq-daemon.error.log 2>/dev/null || true
    rm -f /tmp/autoeq-menubar.log 2>/dev/null || true
    rm -f /tmp/autoeq-menubar.error.log 2>/dev/null || true
    echo "  ✓ Log files removed"
else
    echo "  - Log files kept (--keep-logs)"
fi

echo -e "${GREEN}✓ Cleanup complete${NC}"

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Uninstallation Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo ""

if [ "$NEED_SUDO" = true ]; then
    echo -e "${YELLOW}Note: Some files could not be removed without sudo.${NC}"
    echo "Run the following commands manually if needed:"
    echo ""
    [ -f "${DAEMON_BIN}" ] && echo "  sudo rm -f ${DAEMON_BIN}"
    [ -d "${HAL_DRIVER}" ] && echo "  sudo rm -rf ${HAL_DRIVER}"
    [ -d "${HAL_DRIVER}" ] && echo "  sudo killall coreaudiod"
    echo ""
fi

if [ "$KEEP_LOGS" = true ]; then
    echo "Log files were kept. To remove them:"
    echo "  rm -f /tmp/sotf-*.log /tmp/autoeq-*.log"
    echo ""
fi

echo "Thank you for using SotF!"
echo ""
