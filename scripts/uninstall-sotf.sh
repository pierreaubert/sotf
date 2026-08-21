#!/bin/bash
#
# SotF Uninstaller Script
#
# Removes all SotF components:
#   - SotF Systemwide app
#   - sotf-daemon (embedded in app)
#   - HAL driver
#   - LaunchAgents
#   - Socket files and logs
#
# Bundle identifiers:
#   - org.spinorama.sotf-systemwide
#   - org.spinorama.sotf-hal
#   - org.spinorama.sotf-daemon
#
# Usage:
#   ./uninstall-sotf.sh              # Uninstall everything
#   ./uninstall-sotf.sh --keep-logs  # Keep log files
#   ./uninstall-sotf.sh --hal-only   # Only remove HAL driver
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Bundle identifiers
SYSTEMWIDE_BUNDLE_ID="org.spinorama.sotf-systemwide"
HAL_BUNDLE_ID="org.spinorama.sotf-hal"
DAEMON_BUNDLE_ID="org.spinorama.sotf-daemon"

# Paths
USER_HOME="$HOME"
LAUNCHAGENTS_DIR="${USER_HOME}/Library/LaunchAgents"
SYSTEMWIDE_PLIST="${LAUNCHAGENTS_DIR}/${SYSTEMWIDE_BUNDLE_ID}.plist"
DAEMON_PLIST="${LAUNCHAGENTS_DIR}/${DAEMON_BUNDLE_ID}.plist"
# Legacy plists
LEGACY_TOOLBAR_PLIST="${LAUNCHAGENTS_DIR}/org.spinorama.sotf-toolbar.plist"
LEGACY_DAEMON_PLIST="${LAUNCHAGENTS_DIR}/org.spinorama.sotf.daemon.plist"
LEGACY_CONFIGBAR_PLIST="${LAUNCHAGENTS_DIR}/org.spinorama.sotf.configbar.plist"

SYSTEMWIDE_APP="/Applications/sotf-systemwide.app"
LEGACY_SYSTEMWIDE_APP="/Applications/SotF Systemwide.app"
LEGACY_TOOLBAR_APP="/Applications/SotF Toolbar.app"
LEGACY_CONFIGBAR_APP="/Applications/SotF ConfigBar.app"
DAEMON_BIN="/usr/local/bin/sotf-daemon"
HAL_DRIVER="/Library/Audio/Plug-Ins/HAL/sotf.driver"
SOCKET_PATH="/tmp/autoeq_audio.sock"

# Options
KEEP_LOGS=false
HAL_ONLY=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --keep-logs)
            KEEP_LOGS=true
            shift
            ;;
        --hal-only)
            HAL_ONLY=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --keep-logs    Don't remove log files"
            echo "  --hal-only     Only remove HAL driver"
            echo "  --help         Show this help message"
            echo ""
            echo "Bundle identifiers that will be removed:"
            echo "  - $SYSTEMWIDE_BUNDLE_ID"
            echo "  - $HAL_BUNDLE_ID"
            echo "  - $DAEMON_BUNDLE_ID"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SotF Uninstaller${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# HAL-only mode
if $HAL_ONLY; then
    log_info "Removing HAL driver only..."

    if [ -d "${HAL_DRIVER}" ]; then
        sudo rm -rf "${HAL_DRIVER}"
        log_success "HAL driver removed"

        log_info "Restarting CoreAudio..."
        sudo killall coreaudiod 2>/dev/null || true
        log_success "CoreAudio restarted"
    else
        log_info "HAL driver not installed"
    fi

    echo ""
    echo -e "${GREEN}HAL driver uninstallation complete!${NC}"
    exit 0
fi

# Full uninstall
NEED_SUDO=false

log_info "[1/6] Stopping running processes..."

# Stop Systemwide
if pgrep -x "sotf-systemwide" > /dev/null 2>&1; then
    killall "sotf-systemwide" 2>/dev/null || true
    log_success "Systemwide stopped"
else
    log_info "Systemwide not running"
fi

# Stop legacy Toolbar
if pgrep -x "sotf-toolbar" > /dev/null 2>&1; then
    killall "sotf-toolbar" 2>/dev/null || true
    log_success "Legacy Toolbar stopped"
fi

# Stop legacy ConfigBar
if pgrep -x "sotf-configbar" > /dev/null 2>&1; then
    killall "sotf-configbar" 2>/dev/null || true
    log_success "ConfigBar stopped"
fi

# Stop daemon
if pgrep -x "sotf-daemon" > /dev/null 2>&1; then
    killall "sotf-daemon" 2>/dev/null || true
    log_success "Daemon stopped"
else
    log_info "Daemon not running"
fi

log_info "[2/6] Unloading LaunchAgents..."

# Boot out the daemon agent from the gui domain first; the plist uses
# KeepAlive so plain unload may not stop a launchd-managed daemon.
launchctl bootout "gui/$(id -u)/${DAEMON_BUNDLE_ID}" 2>/dev/null || true

# Unload Systemwide LaunchAgent
if [ -f "${SYSTEMWIDE_PLIST}" ]; then
    launchctl unload "${SYSTEMWIDE_PLIST}" 2>/dev/null || true
    log_success "Systemwide LaunchAgent unloaded"
fi

# Unload legacy Toolbar LaunchAgent
if [ -f "${LEGACY_TOOLBAR_PLIST}" ]; then
    launchctl unload "${LEGACY_TOOLBAR_PLIST}" 2>/dev/null || true
    log_success "Legacy Toolbar LaunchAgent unloaded"
fi

# Unload daemon LaunchAgent
if [ -f "${DAEMON_PLIST}" ]; then
    launchctl unload "${DAEMON_PLIST}" 2>/dev/null || true
    log_success "Daemon LaunchAgent unloaded"
fi

# Unload legacy LaunchAgents
if [ -f "${LEGACY_DAEMON_PLIST}" ]; then
    launchctl unload "${LEGACY_DAEMON_PLIST}" 2>/dev/null || true
    log_success "Legacy daemon LaunchAgent unloaded"
fi

if [ -f "${LEGACY_CONFIGBAR_PLIST}" ]; then
    launchctl unload "${LEGACY_CONFIGBAR_PLIST}" 2>/dev/null || true
    log_success "Legacy ConfigBar LaunchAgent unloaded"
fi

log_info "[3/6] Removing LaunchAgent plists..."

# Remove all SotF LaunchAgent plists
for plist in "${SYSTEMWIDE_PLIST}" "${DAEMON_PLIST}" "${LEGACY_TOOLBAR_PLIST}" "${LEGACY_DAEMON_PLIST}" "${LEGACY_CONFIGBAR_PLIST}"; do
    if [ -f "$plist" ]; then
        rm -f "$plist"
        log_success "Removed $(basename "$plist")"
    fi
done

# Remove the staged LaunchAgent plist shipped by the installer package
if [ -f "/Library/Application Support/SotF/org.spinorama.sotf-daemon.plist" ]; then
    sudo rm -f "/Library/Application Support/SotF/org.spinorama.sotf-daemon.plist"
    log_success "Removed staged daemon LaunchAgent plist"
fi

log_info "[4/6] Removing applications..."

# Remove Systemwide app
if [ -d "${SYSTEMWIDE_APP}" ]; then
    rm -rf "${SYSTEMWIDE_APP}"
    log_success "Systemwide app removed"
else
    log_info "Systemwide app not found"
fi

# Remove legacy Systemwide app (pre-rename: "/Applications/SotF Systemwide.app")
if [ -d "${LEGACY_SYSTEMWIDE_APP}" ]; then
    rm -rf "${LEGACY_SYSTEMWIDE_APP}"
    log_success "Legacy Systemwide app removed"
fi

# Remove legacy Toolbar app
if [ -d "${LEGACY_TOOLBAR_APP}" ]; then
    rm -rf "${LEGACY_TOOLBAR_APP}"
    log_success "Legacy Toolbar app removed"
fi

# Remove legacy ConfigBar app
if [ -d "${LEGACY_CONFIGBAR_APP}" ]; then
    rm -rf "${LEGACY_CONFIGBAR_APP}"
    log_success "Legacy ConfigBar app removed"
fi

# Remove daemon binary (if installed separately)
if [ -f "${DAEMON_BIN}" ]; then
    if rm -f "${DAEMON_BIN}" 2>/dev/null; then
        log_success "Daemon binary removed"
    else
        log_warning "Daemon binary requires sudo to remove"
        NEED_SUDO=true
    fi
fi

log_info "[5/6] Removing HAL driver..."

if [ -d "${HAL_DRIVER}" ]; then
    if sudo rm -rf "${HAL_DRIVER}" 2>/dev/null; then
        log_success "HAL driver removed"

        log_info "Restarting CoreAudio..."
        sudo killall coreaudiod 2>/dev/null || true
        log_success "CoreAudio restarted"
    else
        log_warning "HAL driver requires sudo to remove"
        NEED_SUDO=true
    fi
else
    log_info "HAL driver not installed"
fi

log_info "[6/6] Cleaning up..."

# Remove socket
if [ -e "${SOCKET_PATH}" ]; then
    rm -f "${SOCKET_PATH}"
    log_success "Socket file removed"
fi

# Remove logs (unless --keep-logs)
if [ "$KEEP_LOGS" = false ]; then
    rm -f /tmp/sotf-systemwide.log 2>/dev/null || true
    rm -f /tmp/sotf-systemwide.error.log 2>/dev/null || true
    rm -f /tmp/sotf-toolbar.log 2>/dev/null || true
    rm -f /tmp/sotf-toolbar.error.log 2>/dev/null || true
    rm -f /tmp/sotf-daemon.log 2>/dev/null || true
    rm -f /tmp/sotf-daemon.error.log 2>/dev/null || true
    rm -f /tmp/sotf-configbar.log 2>/dev/null || true
    rm -f /tmp/sotf-configbar.error.log 2>/dev/null || true
    rm -f /tmp/autoeq-daemon.log 2>/dev/null || true
    rm -f /tmp/autoeq-daemon.error.log 2>/dev/null || true
    rm -f /tmp/autoeq-menubar.log 2>/dev/null || true
    rm -f /tmp/autoeq-menubar.error.log 2>/dev/null || true
    log_success "Log files removed"
else
    log_info "Log files kept (--keep-logs)"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Uninstallation Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

if [ "$NEED_SUDO" = true ]; then
    log_warning "Some files could not be removed without sudo."
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
