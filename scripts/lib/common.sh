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

# Print the path of the DejaVu license bundled with the gpui-d3rs package.
# gpui-d3rs is a git dependency (see Cargo.toml/Cargo.lock), so resolve it
# from the cargo checkout of the pinned revision via `cargo metadata` —
# never from a sibling gpui-toolkit checkout on disk, which may not exist
# or may hold a different revision than the build actually uses.
# Argument: path to project root (defaults to sotf_project_root).
sotf_gpui_d3rs_license() {
    local root="${1:-$(sotf_project_root)}"
    cargo metadata --manifest-path "$root/Cargo.toml" \
        --format-version 1 2>/dev/null \
        | python3 -c '
import json, os, sys
try:
    metadata = json.load(sys.stdin)
except Exception:
    sys.exit(1)
for package in metadata.get("packages", []):
    if package.get("name") == "gpui-d3rs":
        print(os.path.join(
            os.path.dirname(package["manifest_path"]),
            "assets", "DejaVu-LICENSE"))
        break
else:
    sys.exit(1)
'
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
