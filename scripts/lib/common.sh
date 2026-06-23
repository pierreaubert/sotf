#!/usr/bin/env bash
# Shared helpers for SOTF build / release scripts.
# Source this file from a script in scripts/ to avoid duplicating common
# boilerplate.

# Print the absolute path to the project root.
# Uses the script that sourced this library as the anchor.
sotf_project_root() {
    local caller_script="${BASH_SOURCE[1]}"
    local script_dir
    script_dir="$(cd "$(dirname "$caller_script")" && pwd)"
    echo "$(cd "$script_dir/.." && pwd)"
}

# Extract the workspace version from the root Cargo.toml.
# Optional argument: path to project root (defaults to sotf_project_root).
sotf_version() {
    local root="${1:-$(sotf_project_root)}"
    grep -m1 '^version = ' "$root/Cargo.toml" | sed -E 's/version = "(.*)"/\1/'
}

# Color codes
SOTF_RED='\033[0;31m'
SOTF_GREEN='\033[0;32m'
SOTF_YELLOW='\033[1;33m'
SOTF_BLUE='\033[0;34m'
SOTF_CYAN='\033[0;36m'
SOTF_BOLD='\033[1m'
SOTF_NC='\033[0m'

sotf_log_info()    { echo -e "${SOTF_BLUE}[INFO]${SOTF_NC} $1"; }
sotf_log_success() { echo -e "${SOTF_GREEN}[OK]${SOTF_NC} $1"; }
sotf_log_warning() { echo -e "${SOTF_YELLOW}[WARN]${SOTF_NC} $1"; }
sotf_log_error()   { echo -e "${SOTF_RED}[ERROR]${SOTF_NC} $1"; }
