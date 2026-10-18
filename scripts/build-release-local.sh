#!/bin/bash
#
# Local release build orchestrator for SotF
#
# Builds all platform binaries from a Mac:
#   - macOS ARM64 + x86_64: built and signed locally
#   - Linux x86_64 + ARM64: built on a remote Linux x86 machine via SSH
#   - Windows x86_64: built on a remote Windows machine via SSH
#
# After builds: cosign non-Apple artifacts, generate checksums, update site,
# and prep GitHub release.
#
# Usage:
#   ./build-release-local.sh                     # Full release build
#   ./build-release-local.sh --arch arm64         # Apple Silicon + Linux ARM64
#   ./build-release-local.sh --arch x86_64        # macOS/Linux x86_64 only
#   ./build-release-local.sh --arch all           # Both architectures (default)
#   ./build-release-local.sh --dry-run           # Show what would be done
#   ./build-release-local.sh --skip-linux        # Skip Linux builds
#   ./build-release-local.sh --skip-windows      # Skip Windows builds
#   ./build-release-local.sh --skip-macos        # Skip macOS builds
#   ./build-release-local.sh --skip-sign         # Skip all signing
#   ./build-release-local.sh --skip-site         # Skip site/docs update
#   ./build-release-local.sh --deploy-site        # Also push site + binaries to the VPS via update_prod.sh
#   ./build-release-local.sh --post-only         # Only post-build steps (sign, checksums, site)
#   ./build-release-local.sh --clean-remote      # Wipe target/, dist/, etc. on remote builders before building
#   ./build-release-local.sh --config <file>     # Use alternate config file
#
# Configuration:
#   Create ~/.sotf-release.conf with SSH and signing details (see --init-config).
#   ./build-release-local.sh --init-config       # Generate a template config file
#
# Prerequisites:
#   - macOS with Xcode Command Line Tools
#   - Rust toolchain with x86_64-apple-darwin target
#   - SSH access to Linux and Windows build machines
#   - cosign (brew install cosign)
#   - For macOS signing: DEVELOPER_ID, APPLE_ID in config
#   - For Windows signing: WINDOWS_CERT_FILE, WINDOWS_CERT_PASSWORD in config
#

set -uo pipefail
# NOTE: we do NOT use set -e (errexit). Remote operations are wrapped in
# explicit error handling so the user sees which step failed instead of a
# silent exit.

trap 'log_error "Unexpected error at line $LINENO (exit $?)"' ERR

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
AGGREGATE_ROOT="$(cd "$PROJECT_ROOT/.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
CONFIG_FILE="${HOME}/.sotf-release.conf"

# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"
source "$SCRIPT_DIR/lib/macos_plugin_helpers.sh"

# Extract version from root Cargo.toml
VERSION=$(sotf_version "$PROJECT_ROOT")
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

GITHUB_RELEASE_URL="https://github.com/pierreaubert/sotf/releases/download/v${VERSION}"

# Options
DRY_RUN=false
SKIP_LINUX=false
SKIP_WINDOWS=false
SKIP_MACOS=false
SKIP_SIGN=false
SKIP_SITE=false
DEPLOY_SITE=false
POST_ONLY=false
CLEAN_REMOTE=false
INIT_CONFIG=false
ARCH_SELECTION=all

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)       DRY_RUN=true; shift ;;
        --skip-linux)    SKIP_LINUX=true; shift ;;
        --skip-windows)  SKIP_WINDOWS=true; shift ;;
        --skip-macos)    SKIP_MACOS=true; shift ;;
        --skip-sign)     SKIP_SIGN=true; shift ;;
        --skip-site)     SKIP_SITE=true; shift ;;
        --deploy-site)   DEPLOY_SITE=true; shift ;;
        --post-only)     POST_ONLY=true; shift ;;
        --clean-remote)  CLEAN_REMOTE=true; shift ;;
        --init-config)   INIT_CONFIG=true; shift ;;
        --arch)
            if [ "$#" -lt 2 ]; then
                echo "--arch requires arm64, x86_64, or all"
                exit 1
            fi
            case "$2" in
                arm64|x86_64|all) ARCH_SELECTION="$2" ;;
                *)
                    echo "Invalid architecture '$2' (choose arm64, x86_64, or all)"
                    exit 1
                    ;;
            esac
            shift 2
            ;;
        --config)        CONFIG_FILE="$2"; shift 2 ;;
        --help|-h)
            sed -n '2,/^$/p' "$0" | sed 's/^#//' | sed 's/^ //'
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# -----------------------------------------------------------------------
# COLOR OUTPUT
# -----------------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_phase()   { echo -e "\n${BOLD}${CYAN}════════════════════════════════════════${NC}"; echo -e "${BOLD}${CYAN}  $1${NC}"; echo -e "${BOLD}${CYAN}════════════════════════════════════════${NC}\n"; }
log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }
log_step()    { echo -e "${BOLD}→${NC} $1"; }
log_dry()     { echo -e "${YELLOW}[DRY-RUN]${NC} Would: $1"; }

# -----------------------------------------------------------------------
# CONFIG TEMPLATE
# -----------------------------------------------------------------------

generate_config_template() {
    cat > "$CONFIG_FILE" << 'CONF'
# SotF Release Build Configuration
# Generated by build-release-local.sh --init-config
#
# SSH connection details for remote build machines.
# The script syncs the repo via rsync, builds remotely, and copies artifacts back.

# --- Linux build machine (x86_64) ---
# Must have: Rust toolchain, Docker (for ARM64 cross builds), just, build-essential,
# and the system deps listed in .github/workflows/release.yml
LINUX_HOST="user@linux-builder.local"
LINUX_PORT="22"
LINUX_PROJECT_DIR="/home/user/src/sotf"
# SSH identity file (optional, uses default SSH config if empty)
LINUX_SSH_KEY=""

# --- Windows build machine (x86_64) ---
# Must have: Rust toolchain, Visual Studio Build Tools or MSVC,
#   Git Bash (C:/Program Files/Git), and rsync installed in Git Bash.
# The script routes all remote commands through Git Bash automatically,
# even though Windows OpenSSH defaults to cmd.exe.
# Use Git Bash / MSYS2 path format for the project directory:
WINDOWS_HOST="user@windows-builder.local"
WINDOWS_PORT="22"
WINDOWS_PROJECT_DIR="/c/Users/user/src/sotf"
WINDOWS_SSH_KEY=""
# Path to bash.exe on the Windows machine (auto-detected if empty)
WINDOWS_BASH_PATH=""

# --- macOS signing ---
# Apple Developer ID certificate name (from Keychain)
# Example: "Developer ID Application: Your Name (TEAMID)"
DEVELOPER_ID=""
# Apple Developer ID Installer certificate (for .pkg files)
INSTALLER_DEVELOPER_ID=""
# Apple ID for notarization
APPLE_ID=""
# Notarization keychain profile name (set up via: xcrun notarytool store-credentials)
NOTARIZATION_PROFILE="autoeq-notarization"

# --- Windows signing ---
# Path to .pfx code signing certificate
WINDOWS_CERT_FILE=""
WINDOWS_CERT_PASSWORD=""
WINDOWS_TIMESTAMP_URL="http://timestamp.digicert.com"
# For native Windows signing with SignTool:
WINDOWS_CERT_THUMBPRINT=""

# --- Cosign (Linux/Windows artifact signing) ---
# Leave empty for keyless OIDC signing (opens browser)
# Set path for key-based signing
COSIGN_KEY=""

# --- Site deployment ---
# rsync target for the website
SITE_DEPLOY_TARGET="spin@vps-c2ea73ea.vps.ovh.net:/var/www/html/spinorama-sotf"
CONF

    echo "Config template written to $CONFIG_FILE"
    echo "Edit it with your SSH hosts and signing credentials."
}

if $INIT_CONFIG; then
    if [ -f "$CONFIG_FILE" ]; then
        echo "Config already exists at $CONFIG_FILE"
        echo "Remove it first if you want a fresh template."
        exit 1
    fi
    generate_config_template
    exit 0
fi

# -----------------------------------------------------------------------
# LOAD CONFIG
# -----------------------------------------------------------------------

if [ ! -f "$CONFIG_FILE" ]; then
    log_error "Config file not found: $CONFIG_FILE"
    log_info "Run: $0 --init-config"
    exit 1
fi

# shellcheck source=/dev/null
source "$CONFIG_FILE"

# -----------------------------------------------------------------------
# HELPERS
# -----------------------------------------------------------------------

# Build the ssh transport string used by both ssh and rsync -e
# Result goes into the SSH_TRANSPORT_CMD variable (array).
build_ssh_transport() {
    local port="$1"
    local key="$2"
    SSH_TRANSPORT_CMD=(ssh)
    [ -n "$port" ] && SSH_TRANSPORT_CMD+=(-p "$port")
    [ -n "$key" ]  && SSH_TRANSPORT_CMD+=(-i "$key")
    return 0
}

# Run an SSH command with optional key and port
ssh_cmd() {
    local host="$1"
    local port="$2"
    local key="$3"
    shift 3
    build_ssh_transport "$port" "$key"
    "${SSH_TRANSPORT_CMD[@]}" -o BatchMode=yes -o ConnectTimeout=10 "$host" "$@"
}

# Detect the path to bash.exe on the remote Windows machine.
# Sets WIN_BASH_PATH (used by win_ssh_cmd and rsync helpers).
# Called once at the start of the Windows build phase.
detect_win_bash() {
    local host="$1"
    local port="$2"
    local key="$3"

    # Use explicit config if provided
    if [ -n "${WINDOWS_BASH_PATH:-}" ]; then
        WIN_BASH_PATH="$WINDOWS_BASH_PATH"
        return 0
    fi

    # Try common Git Bash locations via raw SSH (cmd.exe shell)
    local candidates=(
        "C:/Program Files/Git/bin/bash.exe"
        "C:/Program Files (x86)/Git/bin/bash.exe"
        "C:/msys64/usr/bin/bash.exe"
    )

    build_ssh_transport "$port" "$key"
    for candidate in "${candidates[@]}"; do
        # cmd.exe can check file existence with 'if exist'
        if "${SSH_TRANSPORT_CMD[@]}" -o BatchMode=yes -o ConnectTimeout=10 "$host" \
            "if exist \"$candidate\" echo FOUND" 2>/dev/null | grep -q FOUND; then
            WIN_BASH_PATH="$candidate"
            log_info "Found bash at: $WIN_BASH_PATH"
            return 0
        fi
    done

    log_error "Could not find bash.exe on remote Windows machine"
    log_info "Tried: ${candidates[*]}"
    log_info "Set WINDOWS_BASH_PATH in your config file to the correct path"
    return 1
}

# Detect the path to rsync.exe on the remote Windows machine.
# Sets WIN_RSYNC_PATH. Called once at the start of the Windows build phase.
detect_win_rsync() {
    local host="$1"
    local port="$2"
    local key="$3"

    # Ask Git Bash for rsync, but do not trust the first PATH hit blindly:
    # Git for Windows can have a stale/broken rsync while MSYS2 has a working
    # one installed at /c/msys64/usr/bin/rsync.
    local rsync_path
    rsync_path=$(win_ssh_cmd "$host" "$port" "$key" '
        candidates=()
        path_rsync=$(command -v rsync 2>/dev/null || true)
        [ -n "$path_rsync" ] && candidates+=("$path_rsync")
        candidates+=(
            "/c/msys64/usr/bin/rsync"
            "/c/msys64/mingw64/bin/rsync"
            "/c/msys64/ucrt64/bin/rsync"
            "/c/cygwin64/bin/rsync"
            "/usr/bin/rsync"
            "/mingw64/bin/rsync"
            "/c/Program Files/Git/usr/bin/rsync"
        )

        seen=":"
        for candidate in "${candidates[@]}"; do
            [ -n "$candidate" ] || continue
            case "$seen" in *:"$candidate":*) continue ;; esac
            seen="${seen}${candidate}:"
            if [ -x "$candidate" ] && "$candidate" --version >/dev/null 2>&1; then
                printf "%s\n" "$candidate"
                exit 0
            fi
        done
        exit 1
    ' 2>/dev/null)
    rsync_path=$(echo "$rsync_path" | tr -d '\r' | tail -n 1)

    if [ -z "$rsync_path" ]; then
        log_error "rsync not found on remote Windows"
        return 1
    fi

    if ! win_ssh_cmd "$host" "$port" "$key" "'$rsync_path' --version >/dev/null 2>&1" >/dev/null 2>&1; then
        local missing_deps
        missing_deps=$(win_ssh_cmd "$host" "$port" "$key" "ldd '$rsync_path' 2>&1 | grep -i 'not found' || true" 2>/dev/null)
        log_error "rsync exists at $rsync_path but cannot run on remote Windows"
        if [ -n "$missing_deps" ]; then
            log_info "Missing rsync runtime dependencies:"
            while IFS= read -r line; do
                [ -n "$line" ] && log_info "  $line"
            done <<< "$missing_deps"
        fi
        return 1
    fi

    # Convert MSYS path (/usr/bin/rsync) to a short Windows path for
    # --rsync-path. The remote SSH shell is cmd.exe, so using 8.3 paths avoids
    # quote-sensitive spaces like "C:\Program Files\...".
    local win_rsync
    win_rsync=$(win_ssh_cmd "$host" "$port" "$key" "cygpath -m -s '$rsync_path' 2>/dev/null || cygpath -m '$rsync_path' 2>/dev/null || echo '$rsync_path'" 2>/dev/null)
    WIN_RSYNC_PATH=$(echo "$win_rsync" | tr -d '\r\n')
    log_info "Found rsync at: $WIN_RSYNC_PATH"
    return 0
}

# Run an SSH command on Windows, routing through Git Bash.
# Windows OpenSSH defaults to cmd.exe which can't run bash scripts or find
# MSYS2 tools (rsync, mkdir -p, etc.). This wraps the command in Git Bash.
# Requires detect_win_bash to have been called first.
WIN_BASH_PATH=""
WIN_RSYNC_PATH=""
win_ssh_cmd() {
    local host="$1"
    local port="$2"
    local key="$3"
    shift 3
    local cmd="$*"
    build_ssh_transport "$port" "$key"
    # Run the Git Bash script over stdin. Passing the command as a remote
    # command-line argument lets cmd.exe reinterpret shell metacharacters like
    # pipes, &&, ||, and redirections before bash sees them.
    printf '%s\n' "$cmd" | "${SSH_TRANSPORT_CMD[@]}" -o BatchMode=yes -o ConnectTimeout=30 "$host" \
        "\"$WIN_BASH_PATH\"" -s
}

# Common rsync excludes — mirrors .gitignore for build/generated dirs.
# Using an array avoids repeating between rsync_to and rsync_to_win.
RSYNC_EXCLUDES=(
    # Build artifacts
    # No trailing slash: these may be symlinks to external build volumes.
    --exclude='/target'
    --exclude='/target-static'
    --exclude='/.docker-target'
    --exclude='/dist'
    --exclude='/tools'
    --exclude='/certs'
    --exclude='/coverage'
    --exclude='/mutants.out'
    --exclude='/appimage-builder-*'
    --exclude='/site/dist'
    # Python
    --exclude='/venv'
    --exclude='/.venv'
    --exclude='__pycache__'
    # Generated data
    --exclude='/data'
    --exclude='/data_cached'
    --exclude='/data_generated'
    # Node
    --exclude='node_modules'
    # Git and editor
    --exclude='.git/'
    --exclude='.DS_Store'
    --exclude='/.vs/'
    --exclude='/.vscode/'
    --exclude='/.zed/'
    --exclude='/.idea/'
    --exclude='/.fleet/'
    --exclude='/.qwen/'
    --exclude='/.worktrees/'
    # AI assistant / code-graph state — large, machine-specific, never needed
    # on remote builders.
    --exclude='/.claude/'
    --exclude='/.tokensave/'
    --exclude='/.serena/'
    # Per-arch Xcode DerivedData for the AU plugins (gigabytes of host-arch
    # binaries, intermediates, and ModuleCache; rebuilt remotely anyway).
    --exclude='/crates/sotf-plugins/crates/plugins-au/build/'
    # Object files
    --exclude='*.o'
    --exclude='*.d'
    # Product/marketing docs
    --exclude='/product/'
)

# The Linux builder consumes the aggregate checkout, not just the SOTF child.
# Its fresh snapshot deliberately omits every repository's build output and
# VCS metadata, while keeping source files from all sibling workspaces.
AGGREGATE_RSYNC_EXCLUDES=(
    --exclude='.git/'
    --exclude='target/'
    --exclude='target-static/'
    --exclude='.docker-target/'
    --exclude='dist/'
    --exclude='node_modules/'
    --exclude='venv/'
    --exclude='.venv/'
    --exclude='__pycache__/'
    --exclude='.tokensave/'
    --exclude='.claude/'
    --exclude='.serena/'
    --exclude='DerivedData/'
    --exclude='coverage/'
    --exclude='mutants.out/'
    --exclude='data_cached/'
    --exclude='data_generated/'
    --exclude='.DS_Store'
    --exclude='.vs/'
    --exclude='.vscode/'
    --exclude='.zed/'
    --exclude='.idea/'
    --exclude='.fleet/'
    --exclude='.qwen/'
    --exclude='.worktrees/'
    --exclude='*.o'
    --exclude='*.d'
)

handle_rsync_status() {
    local status="$1"
    local label="$2"
    local allow_partial="${3:-false}"

    if [ "$status" -eq 0 ]; then
        return 0
    fi

    # Exit 24 means files vanished while rsync was reading the live working
    # tree. That is common for editor/build cache churn and should not abort a
    # release build sync.
    if [ "$status" -eq 24 ]; then
        log_warning "$label completed with vanished source files (rsync exit 24); continuing"
        return 0
    fi

    # Exit 23 is a partial transfer. During the Windows source sync this is
    # usually a non-critical POSIX metadata/symlink mismatch on the receiver;
    # the subsequent cargo build will still fail if a required source file did
    # not arrive.
    if [ "$status" -eq 23 ] && [ "$allow_partial" = true ]; then
        log_warning "$label completed with partial transfer warnings (rsync exit 23); continuing"
        return 0
    fi

    return "$status"
}

rsync_to() {
    local host="$1"
    local port="$2"
    local key="$3"
    local remote_dir="$4"
    build_ssh_transport "$port" "$key"
    rsync -avz --delete \
        "${RSYNC_EXCLUDES[@]}" \
        -e "${SSH_TRANSPORT_CMD[*]}" \
        "$PROJECT_ROOT/" "${host}:${remote_dir}/"
    handle_rsync_status "$?" "rsync to ${host}:${remote_dir}"
}

# rsync to a Windows host — tells the remote side to use the detected rsync.
# detect_win_rsync normalizes this to a short cmd.exe-safe path.
rsync_to_win() {
    local host="$1"
    local port="$2"
    local key="$3"
    local remote_dir="$4"
    build_ssh_transport "$port" "$key"
    rsync -avz --delete \
        --no-perms --no-owner --no-group --omit-dir-times \
        "${RSYNC_EXCLUDES[@]}" \
        -e "${SSH_TRANSPORT_CMD[*]}" \
        --rsync-path="$WIN_RSYNC_PATH" \
        "$PROJECT_ROOT/" "${host}:${remote_dir}/"
    handle_rsync_status "$?" "rsync to ${host}:${remote_dir}" true
}

rsync_from() {
    local host="$1"
    local port="$2"
    local key="$3"
    local remote_path="$4"
    local local_path="$5"
    shift 5
    build_ssh_transport "$port" "$key"
    rsync -avz \
        -e "${SSH_TRANSPORT_CMD[*]}" \
        "$@" \
        "${host}:${remote_path}" "$local_path"
    handle_rsync_status "$?" "rsync from ${host}:${remote_path}"
}

# rsync from a Windows host — tells the remote side to use the detected rsync
rsync_from_win() {
    local host="$1"
    local port="$2"
    local key="$3"
    local remote_path="$4"
    local local_path="$5"
    shift 5
    build_ssh_transport "$port" "$key"
    rsync -avz \
        -e "${SSH_TRANSPORT_CMD[*]}" \
        --rsync-path="$WIN_RSYNC_PATH" \
        "$@" \
        "${host}:${remote_path}" "$local_path"
    handle_rsync_status "$?" "rsync from ${host}:${remote_path}"
}

# Track build results
declare -a BUILD_RESULTS=()
declare -a RELEASE_ARTIFACTS=()
BUILD_FAILED=false
record_result() {
    BUILD_RESULTS+=("$1")
    case "$1" in
        *FAILED*) BUILD_FAILED=true ;;
    esac
}

record_artifact() {
    local path="$1"
    local label="${2:-$path}"
    if [ ! -s "$path" ]; then
        log_error "Expected fresh artifact is missing or empty: $path"
        record_result "$label: FAILED (missing artifact)"
        return 1
    fi
    RELEASE_ARTIFACTS+=("$path")
    return 0
}

arch_is_selected() {
    [ "$ARCH_SELECTION" = all ] || [ "$ARCH_SELECTION" = "$1" ]
}

clear_versioned_artifacts() {
    local directory="$1"
    [ -d "$directory" ] || return 0
    local path
    local failed=false
    shopt -s nullglob
    for path in "$directory"/*"$VERSION"*; do
        [ -f "$path" ] || [ -L "$path" ] || continue
        if ! rm -f -- "$path"; then
            log_error "Could not remove stale current-version artifact: $path"
            failed=true
        fi
    done
    shopt -u nullglob
    [ "$failed" = false ]
}

is_excluded_linux_source_path() {
    local path="$1"
    case "/$path/" in
        */target/*|*/target-static/*|*/.docker-target/*|*/dist/*|\
        */node_modules/*|*/venv/*|*/.venv/*|*/__pycache__/*|\
        */.git/*|*/.tokensave/*|*/.claude/*|*/.serena/*|\
        */DerivedData/*|*/coverage/*|*/mutants.out/*|\
        */data_cached/*|*/data_generated/*|*/.vs/*|*/.vscode/*|\
        */.zed/*|*/.idea/*|*/.fleet/*|*/.qwen/*|*/.worktrees/*)
            return 0
            ;;
    esac
    case "$path" in
        *.o|*.d|*/.DS_Store|.DS_Store|.release-source-manifest-*) return 0 ;;
    esac
    return 1
}

write_linux_source_manifest() {
    local aggregate_root="$1"
    local manifest="$2"
    local source_file_list="$3"
    local manifest_name="$4"
    local repo rel head path candidate entries
    local entries_file="${source_file_list}.entries"
    : > "$manifest" || return 1
    : > "$source_file_list" || return 1

    for repo in . sotf sotf-daw sotf-systemwide sotf-capture autoeq math-audio gpui-toolkit sofa-reader; do
        if [ ! -d "$aggregate_root/$repo" ] || ! git -C "$aggregate_root/$repo" rev-parse --show-toplevel >/dev/null 2>&1; then
            log_error "Aggregate Linux source tree is missing a Git workspace: $repo"
            rm -f -- "$entries_file"
            return 1
        fi
        rel="$repo"
        [ "$repo" = . ] && rel=all_of_sotf
        head=$(git -C "$aggregate_root/$repo" rev-parse HEAD 2>/dev/null) || {
            log_error "Could not record source revision for $rel"
            rm -f -- "$entries_file"
            return 1
        }
        printf '# source %s head %s\n' "$rel" "$head" >> "$manifest" || return 1

        if ! git -C "$aggregate_root/$repo" ls-files -z --cached > "$entries_file"; then
            log_error "Could not enumerate tracked source files for $rel"
            rm -f -- "$entries_file"
            return 1
        fi
        while IFS= read -r -d '' path; do
            if [ "$repo" = . ]; then
                case "$path" in
                    sotf|sotf/|sotf-daw|sotf-daw/|sotf-systemwide|sotf-systemwide/|\
                    sotf-capture|sotf-capture/|autoeq|autoeq/|math-audio|math-audio/|\
                    gpui-toolkit|gpui-toolkit/|sofa-reader|sofa-reader/) continue ;;
                esac
                candidate="$path"
            else
                candidate="$repo/$path"
            fi
            is_excluded_linux_source_path "$candidate" && continue
            printf '%s\0' "$candidate" >> "$source_file_list" || return 1
        done < "$entries_file"

        if ! git -C "$aggregate_root/$repo" ls-files -z --others --exclude-standard > "$entries_file"; then
            log_error "Could not enumerate untracked source files for $rel"
            rm -f -- "$entries_file"
            return 1
        fi
        while IFS= read -r -d '' path; do
            if [ "$repo" = . ]; then
                case "$path" in
                    sotf|sotf/|sotf-daw|sotf-daw/|sotf-systemwide|sotf-systemwide/|\
                    sotf-capture|sotf-capture/|autoeq|autoeq/|math-audio|math-audio/|\
                    gpui-toolkit|gpui-toolkit/|sofa-reader|sofa-reader/) continue ;;
                esac
                candidate="$path"
            else
                candidate="$repo/$path"
            fi
            # Include committed data fixtures while leaving local, untracked
            # datasets out of release snapshots. Git-ignored datasets are
            # already omitted by --exclude-standard.
            case "$candidate" in */data/*) continue ;; esac
            is_excluded_linux_source_path "$candidate" && continue
            printf '%s\0' "$candidate" >> "$source_file_list" || return 1
        done < "$entries_file"
    done
    rm -f -- "$entries_file"

    while IFS= read -r -d '' path; do
        if [ ! -f "$aggregate_root/$path" ] && [ ! -L "$aggregate_root/$path" ]; then
            log_error "Source file listed for Linux snapshot is missing: $path"
            return 1
        fi
        (cd "$aggregate_root" && shasum -a 256 "$path") >> "$manifest" || return 1
    done < "$source_file_list"
    printf '%s\0' "$manifest_name" >> "$source_file_list" || return 1
}

verify_linux_source_manifest() {
    local host="$1" port="$2" key="$3" remote_root="$4" manifest_name="$5"
    ssh_cmd "$host" "$port" "$key" \
        "cd '$remote_root' && grep -v '^#' '$manifest_name' | shasum -a 256 -c - >/dev/null"
}

verify_linux_appimage_arch() {
    local path="$1" arch="$2" description
    description=$(file -b "$path") || {
        log_error "Could not inspect Linux AppImage architecture: $path"
        record_result "Linux ${arch} AppImage: FAILED (file inspection)"
        return 1
    }
    case "$arch:$description" in
        x86_64:*x86-64*|x86_64:*x86_64*|arm64:*aarch64*|arm64:*ARM\ aarch64*) return 0 ;;
        *)
            log_error "Linux ${arch} AppImage has unexpected file type: $description"
            record_result "Linux ${arch} AppImage: FAILED (architecture mismatch)"
            return 1
            ;;
    esac
}

write_artifact_hashes() {
    local output="$1"
    local artifact artifact_name
    : > "$output" || return 1
    for artifact in "${RELEASE_ARTIFACTS[@]}"; do
        [ -s "$artifact" ] || return 1
        artifact_name=$(basename "$artifact") || return 1
        (cd "$DIST_DIR" && shasum -a 256 "$artifact_name") >> "$output" || return 1
    done
}

# Time a phase
phase_start_time=0
start_timer() { phase_start_time=$(date +%s); }
elapsed() {
    local now
    now=$(date +%s)
    local diff=$((now - phase_start_time))
    printf '%dm%02ds' $((diff / 60)) $((diff % 60))
}

# -----------------------------------------------------------------------
# PHASE 0: PREFLIGHT CHECKS
# -----------------------------------------------------------------------

preflight() {
    log_phase "Phase 0: Preflight Checks"

    log_step "Version: v${VERSION}"
    log_step "Dist dir: ${DIST_DIR}"

    # Sync auxiliary version-pinned files (AppxManifest.xml, site/package.json)
    # to the workspace Cargo.toml version BEFORE any remote rsync, so the
    # Windows builder picks up the correct manifest.
    log_step "Syncing version-pinned files to v${VERSION}..."
    if $DRY_RUN; then
        log_dry "./scripts/sync-version.sh"
    else
        if ! "$SCRIPT_DIR/sync-version.sh"; then
            log_error "Version sync failed"
            exit 1
        fi
    fi

    # Check local tools
    local missing=()
    command -v cargo &>/dev/null || missing+=("cargo")
    command -v just &>/dev/null  || missing+=("just")
    command -v rsync &>/dev/null || missing+=("rsync")

    if ! $SKIP_SIGN; then
        command -v cosign &>/dev/null || missing+=("cosign")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing required tools: ${missing[*]}"
        exit 1
    fi
    log_success "Local tools OK"

    # Check git is clean
    if [ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]; then
        log_warning "Working directory has uncommitted changes"
        log_info "Builds will use the current working tree state"
    fi

    # Check SSH connectivity
    if ! $SKIP_LINUX && ! $POST_ONLY; then
        log_step "Testing SSH to Linux: ${LINUX_HOST:-<not configured>}"
        if [ -z "${LINUX_HOST:-}" ]; then
            log_error "LINUX_HOST not configured in $CONFIG_FILE"
            exit 1
        fi
        if $DRY_RUN; then
            log_dry "ssh ${LINUX_HOST} echo ok"
        else
            if ssh_cmd "$LINUX_HOST" "${LINUX_PORT:-22}" "${LINUX_SSH_KEY:-}" "echo ok" &>/dev/null; then
                log_success "Linux SSH OK"
            else
                log_error "Cannot SSH to $LINUX_HOST"
                exit 1
            fi
        fi
    fi

    if ! $SKIP_WINDOWS && ! $POST_ONLY; then
        log_step "Testing SSH to Windows: ${WINDOWS_HOST:-<not configured>}"
        if [ -z "${WINDOWS_HOST:-}" ]; then
            log_error "WINDOWS_HOST not configured in $CONFIG_FILE"
            exit 1
        fi
        if $DRY_RUN; then
            log_dry "ssh ${WINDOWS_HOST} echo ok"
        else
            if ssh_cmd "$WINDOWS_HOST" "${WINDOWS_PORT:-22}" "${WINDOWS_SSH_KEY:-}" "echo ok" &>/dev/null; then
                log_success "Windows SSH OK"
            else
                log_error "Cannot SSH to $WINDOWS_HOST"
                exit 1
            fi
        fi
    fi

    mkdir -p "$DIST_DIR"
    log_success "Preflight complete"
}

# -----------------------------------------------------------------------
# PHASE 1: macOS BUILDS (local)
# -----------------------------------------------------------------------

build_macos_target() {
    local MACOSX_DEPLOYMENT_TARGET=15.0
    export MACOSX_DEPLOYMENT_TARGET
    local arch="$1"
    local target="$2"
    local features="$3"
    local target_dir="$PROJECT_ROOT/target/$target/dist"
    local tui_out="$DIST_DIR/sotf-tui-${VERSION}-macos-${arch}"
    local desktop_out="$DIST_DIR/sotf-desktop-${VERSION}-macos-${arch}"
    local label="macOS ${arch}"

    log_step "Building ${label} (dynamic, profile=dist)..."
    if ! rustup target add "$target" 2>/dev/null; then
        log_warning "Rust target $target is already installed or could not be added; Cargo will report whether it is usable"
    fi
    if ! cargo build --profile dist --target "$target" -p sotf-tui --features "$features" ||
       ! cargo build --profile dist --target "$target" -p sotf-gpui --features "shipping,$features" ||
       ! build_macos_plugin_helpers "$PROJECT_ROOT/../sotf-daw" "$PROJECT_ROOT/target" "$target"; then
        log_error "${label} Cargo build failed"
        record_result "${label}: FAILED (cargo build)"
        return 1
    fi
    if ! mkdir -p "$DIST_DIR" ||
       ! cp "$target_dir/sotf-tui" "$tui_out" ||
       ! cp "$target_dir/sotf-desktop" "$desktop_out"; then
        log_error "${label} artifact copy failed"
        record_result "${label}: FAILED (artifact copy)"
        return 1
    fi
    if ! record_artifact "$tui_out" "${label} TUI" ||
       ! record_artifact "$desktop_out" "${label} desktop"; then
        return 1
    fi
    if ! lipo -verify_arch "$arch" "$tui_out" >/dev/null 2>&1 ||
       ! lipo -verify_arch "$arch" "$desktop_out" >/dev/null 2>&1; then
        log_error "${label} artifact architecture verification failed"
        record_result "${label}: FAILED (architecture mismatch)"
        return 1
    fi

    local dmg="$DIST_DIR/sotf-desktop-${VERSION}-macos-${arch}.dmg"
    if ! ./scripts/build-dmg-sotf.sh --arch "$arch" --binary "$target_dir/sotf-desktop"; then
        log_error "${label} DMG packaging failed"
        record_result "${label} DMG: FAILED (packaging)"
        return 1
    fi
    if ! record_artifact "$dmg" "${label} DMG"; then
        return 1
    fi
    record_result "${label}: OK"
    return 0
}

build_macos() {
    if $SKIP_MACOS; then
        log_info "Skipping macOS builds (--skip-macos)"
        return
    fi

    log_phase "Phase 1: macOS Builds (local)"
    start_timer
    local phase_failed=false

    cd "$PROJECT_ROOT"

    # We deliberately do NOT use `just cross-macos-{arm64,x86}` here. Those
    # recipes historically rebuilt the GPUI binary with
    # `RUSTFLAGS="-C target-feature=+crt-static"` into `./target-static/` before
    # wrapping it in a DMG. The static binary cannot be signed/notarized
    # cleanly, so this pipeline builds the dynamically-linked binary first and
    # then asks the DMG script to package that exact binary.

    if $DRY_RUN; then
        if arch_is_selected arm64; then
            log_dry "cargo build --profile dist --target aarch64-apple-darwin (sotf-tui, sotf-desktop)"
            log_dry "./scripts/build-dmg-sotf.sh --arch arm64 --binary target/aarch64-apple-darwin/dist/sotf-desktop"
        fi
        if arch_is_selected x86_64; then
            log_dry "cargo build --profile dist --target x86_64-apple-darwin (sotf-tui, sotf-desktop)"
            log_dry "./scripts/build-dmg-sotf.sh --arch x86_64 --binary target/x86_64-apple-darwin/dist/sotf-desktop"
        fi
    else
        if arch_is_selected arm64; then
            build_macos_target arm64 aarch64-apple-darwin hal,onnx || phase_failed=true
        fi
        if arch_is_selected x86_64; then
            build_macos_target x86_64 x86_64-apple-darwin hal || phase_failed=true
        fi
        $phase_failed && record_result "macOS required targets: FAILED"
    fi

    # --- macOS Systemwide pkg (moved) ---
    # Systemwide packaging now lives in ../sotf-systemwide (scripts/build-systemwide.sh).

    # --- Sign macOS artifacts ---
    if ! $SKIP_SIGN && [ -n "${DEVELOPER_ID:-}" ]; then
        log_step "Signing macOS artifacts..."
        if $DRY_RUN; then
            log_dry "./scripts/sign-macos.sh --notarize"
        else
            export DEVELOPER_ID
            export INSTALLER_DEVELOPER_ID="${INSTALLER_DEVELOPER_ID:-}"
            export APPLE_ID="${APPLE_ID:-}"

            if ./scripts/sign-macos.sh; then
                log_success "macOS code signing complete"
                if [ -n "${APPLE_ID:-}" ]; then
                    log_step "Notarizing macOS artifacts..."
                    if ./scripts/sign-macos.sh --notarize; then
                        log_success "macOS notarization complete"
                        record_result "macOS notarization: OK"
                    else
                        log_error "macOS notarization failed"
                        record_result "macOS notarization: FAILED"
                        phase_failed=true
                    fi
                else
                    log_warning "APPLE_ID not set, skipping notarization"
                fi
                record_result "macOS signing: OK"
            else
                log_error "macOS code signing failed"
                record_result "macOS signing: FAILED"
                phase_failed=true
            fi
        fi
    elif ! $SKIP_SIGN; then
        log_warning "DEVELOPER_ID not set, skipping macOS signing"
        record_result "macOS signing: SKIPPED (no DEVELOPER_ID)"
    fi

    if ! $DRY_RUN; then
        if ! write_artifact_hashes "$DIST_DIR/BUILD-SHA256SUMS-${VERSION}"; then
            log_error "Could not write the build artifact hash manifest"
            record_result "macOS artifact hashes: FAILED"
            phase_failed=true
        fi
    fi
    if $phase_failed; then
        log_error "macOS phase failed ($(elapsed))"
        return 1
    fi
    log_success "macOS builds complete ($(elapsed))"
    return 0
}

# -----------------------------------------------------------------------
# PHASE 2: LINUX BUILDS (remote SSH)
# -----------------------------------------------------------------------

build_linux() {
    if $SKIP_LINUX; then
        log_info "Skipping Linux builds (--skip-linux)"
        return
    fi

    log_phase "Phase 2: Linux Builds (remote: ${LINUX_HOST})"
    start_timer

    local host="$LINUX_HOST"
    local port="${LINUX_PORT:-22}"
    local key="${LINUX_SSH_KEY:-}"
    local configured_project_dir="${LINUX_PROJECT_DIR}"
    local run_id="$(date +%Y%m%d%H%M%S)-$$"
    local remote_snapshot_root="${configured_project_dir%/*}/.sotf-release-${VERSION}-${run_id}"
    local remote_dir="$remote_snapshot_root/$(basename "$PROJECT_ROOT")"
    local manifest_name=".release-source-manifest-${VERSION}-${run_id}.sha256"
    local manifest_path="$AGGREGATE_ROOT/$manifest_name"
    local source_file_list="$DIST_DIR/.release-source-files-${VERSION}-${run_id}.nul"
    local fetched_ok=true

    # Use a unique sibling-root snapshot so the build cannot resolve a stale
    # or unknown checkout already present on the Linux host. The manifest
    # records every transferred source file and the HEAD of each sibling.
    log_step "Syncing aggregate source snapshot to ${host}:${remote_snapshot_root}..."
    if $DRY_RUN; then
        log_dry "write source hashes for ${AGGREGATE_ROOT}"
        log_dry "rsync ${AGGREGATE_ROOT}/ to ${host}:${remote_snapshot_root}/ (aggregate siblings, fresh path)"
        log_dry "verify ${manifest_name} on ${host}:${remote_snapshot_root}"
    else
        if [ -e "$manifest_path" ]; then
            log_error "Refusing to overwrite an existing release source manifest: $manifest_path"
            record_result "Linux sync: FAILED (manifest collision)"
            return 1
        fi
        if ! write_linux_source_manifest "$AGGREGATE_ROOT" "$manifest_path" "$source_file_list" "$manifest_name"; then
            rm -f -- "$manifest_path"
            rm -f -- "$source_file_list" "${source_file_list}.entries"
            log_error "Could not record aggregate source hashes"
            record_result "Linux sync: FAILED (manifest generation)"
            return 1
        fi
        if ! ssh_cmd "$host" "$port" "$key" \
            "mkdir -p '${remote_snapshot_root%/*}' && mkdir '$remote_snapshot_root'"; then
            rm -f -- "$manifest_path"
            rm -f -- "$source_file_list"
            log_error "Could not create Linux snapshot directory"
            record_result "Linux sync: FAILED (snapshot directory)"
            return 1
        fi
        build_ssh_transport "$port" "$key"
        if ! rsync -avz --delete --from0 --files-from="$source_file_list" \
            "${AGGREGATE_RSYNC_EXCLUDES[@]}" \
            -e "${SSH_TRANSPORT_CMD[*]}" \
            "$AGGREGATE_ROOT/" "${host}:${remote_snapshot_root}/"; then
            rm -f -- "$manifest_path"
            rm -f -- "$source_file_list"
            log_error "rsync to Linux failed"
            record_result "Linux sync: FAILED"
            return 1
        fi
        if ! verify_linux_source_manifest "$host" "$port" "$key" "$remote_snapshot_root" "$manifest_name"; then
            rm -f -- "$manifest_path"
            rm -f -- "$source_file_list"
            log_error "Linux snapshot source hash verification failed"
            record_result "Linux sync: FAILED (source hash mismatch)"
            return 1
        fi
        mkdir -p "$DIST_DIR"
        local retained_manifest="$DIST_DIR/source-manifest-linux-${VERSION}-${run_id}.sha256"
        if ! cp "$manifest_path" "$retained_manifest"; then
            rm -f -- "$manifest_path"
            rm -f -- "$source_file_list"
            log_error "Could not retain the Linux source hash manifest"
            record_result "Linux sync: FAILED (manifest copy)"
            return 1
        fi
        record_artifact "$retained_manifest" "Linux source manifest" || fetched_ok=false
        rm -f -- "$manifest_path"
        rm -f -- "$source_file_list"
        if $CLEAN_REMOTE; then
            log_info "The Linux snapshot is fresh; no existing remote build artifacts are reused"
        else
            log_info "Linux builds use a fresh snapshot directory; prior remote sources and artifacts are not consulted"
        fi
        log_success "Source synced"
    fi

    local native_machine native_arch
    if $DRY_RUN; then
        native_machine="$(uname -m)"
        log_dry "ssh ${host} uname -m (mock/native target detection)"
    else
        native_machine=$(ssh_cmd "$host" "$port" "$key" "uname -m") || {
            log_error "Could not detect Linux builder architecture"
            record_result "Linux architecture detection: FAILED"
            return 1
        }
    fi
    case "$native_machine" in
        x86_64|amd64) native_arch=x86_64 ;;
        aarch64|arm64) native_arch=arm64 ;;
        *)
            log_error "Unsupported Linux builder architecture: $native_machine"
            record_result "Linux architecture detection: FAILED ($native_machine)"
            return 1
            ;;
    esac
    log_info "Linux builder native architecture: $native_arch"

    if $DRY_RUN; then
        log_dry "ssh ${host} 'cd ${remote_dir} && mkdir -p dist && rm -f dist/*${VERSION}*'"
    elif ! ssh_cmd "$host" "$port" "$key" \
        "cd '$remote_dir' && mkdir -p dist && rm -f dist/*${VERSION}*"; then
        log_error "Could not clear stale Linux artifacts for v${VERSION}"
        record_result "Linux cleanup: FAILED"
        return 1
    fi

    local arch build_command method appimage_expected
    for arch in x86_64 arm64; do
        arch_is_selected "$arch" || continue
        if [ "$arch" = "$native_arch" ]; then
            build_command="./scripts/build-linux.sh --appimage"
            method="native"
            appimage_expected=true
        elif [ "$arch" = arm64 ]; then
            build_command="just docker-linux-arm64"
            method="Docker"
            appimage_expected=true
        else
            build_command="just docker-linux-x86"
            method="Docker"
            # On an ARM host the x86 recipe uses QEMU and deliberately builds
            # TUI-only, so its tarball is the only required output.
            appimage_expected=false
        fi
        log_step "Building Linux ${arch} (${method} on remote)..."
        if $DRY_RUN; then
            log_dry "ssh ${host} 'cd ${remote_dir} && ${build_command}'"
        elif ssh_cmd "$host" "$port" "$key" \
            "cd '$remote_dir' && ${build_command}"; then
            record_result "Linux ${arch}: BUILD OK"
        else
            log_error "Linux ${arch} build failed"
            record_result "Linux ${arch}: FAILED"
            fetched_ok=false
        fi
    done

    # Copy artifacts back (current version only)
    log_step "Fetching Linux artifacts (v${VERSION})..."
    if $DRY_RUN; then
        log_dry "rsync ${host}:${remote_dir}/dist/*${VERSION}* -> ${DIST_DIR}/"
    else
        if ! rsync_from "$host" "$port" "$key" \
            "${remote_dir}/dist/" "$DIST_DIR/" \
            --include='*/' --include="*${VERSION}*" --exclude='*'; then
            log_error "rsync from Linux failed"
            record_result "Linux fetch: FAILED"
            return 1
        fi
        local arch artifact artifact_label appimage_expected
        for arch in x86_64 arm64; do
            arch_is_selected "$arch" || continue
            appimage_expected=true
            if [ "$arch" = x86_64 ] && [ "$native_arch" = arm64 ]; then
                appimage_expected=false
            fi
            local expected_artifacts=("sotf-desktop-${VERSION}-linux-${arch}.tar.gz")
            $appimage_expected && expected_artifacts+=("sotf-desktop-${VERSION}-linux-${arch}.AppImage")
            for artifact in "${expected_artifacts[@]}"; do
                artifact_label="Linux ${arch} ${artifact##*.}"
                if ! record_artifact "$DIST_DIR/$artifact" "$artifact_label"; then
                    fetched_ok=false
                elif [[ "$artifact" == *.AppImage ]] &&
                    ! verify_linux_appimage_arch "$DIST_DIR/$artifact" "$arch"; then
                    fetched_ok=false
                fi
            done
        done
        if [ "$fetched_ok" = true ]; then
            if ! write_artifact_hashes "$DIST_DIR/BUILD-SHA256SUMS-${VERSION}"; then
                log_error "Could not write the build artifact hash manifest"
                record_result "Linux artifact hashes: FAILED"
                return 1
            fi
            log_success "Linux artifacts fetched and required outputs verified"
            record_result "Linux required artifacts: OK"
        fi
    fi

    if [ "$fetched_ok" != true ]; then
        log_error "Linux phase failed ($(elapsed))"
        return 1
    fi
    log_success "Linux builds complete ($(elapsed))"
    return 0
}

# -----------------------------------------------------------------------
# PHASE 3: WINDOWS BUILDS (remote SSH)
# -----------------------------------------------------------------------

build_windows() {
    if $SKIP_WINDOWS; then
        log_info "Skipping Windows builds (--skip-windows)"
        return
    fi

    log_phase "Phase 3: Windows Builds (remote: ${WINDOWS_HOST})"
    start_timer
    local first_result_index=${#BUILD_RESULTS[@]}

    local host="$WINDOWS_HOST"
    local port="${WINDOWS_PORT:-22}"
    local key="${WINDOWS_SSH_KEY:-}"
    local remote_dir="${WINDOWS_PROJECT_DIR}"

    # Windows OpenSSH defaults to cmd.exe which can't run bash commands or
    # find MSYS2/Git Bash tools. All remote commands go through win_ssh_cmd
    # which routes them via Git Bash. rsync uses --rsync-path to invoke the
    # Git Bash rsync binary on the remote side.

    # Detect Git Bash location on the remote machine
    log_step "Detecting Git Bash on remote..."
    if ! $DRY_RUN; then
        if ! detect_win_bash "$host" "$port" "$key"; then
            record_result "Windows: FAILED (no Git Bash found)"
            return
        fi

        # Detect rsync location via Git Bash
        log_step "Detecting rsync on remote..."
        if ! detect_win_rsync "$host" "$port" "$key"; then
            log_info "rsync is not bundled with standard Git for Windows."
            log_info "Install it by one of these methods:"
            log_info "  1. If you have MSYS2: pacman -S rsync"
            log_info "  2. Download Git for Windows SDK (includes pacman):"
            log_info "     https://github.com/git-for-windows/build-extra/releases"
            log_info "  3. Manually copy rsync.exe into C:/Program Files/Git/usr/bin/"
            log_info "     If rsync is present but a DLL is missing, reinstall rsync and its MSYS dependencies."
            record_result "Windows: FAILED (no rsync)"
            return
        fi
        log_success "Git Bash + rsync OK on remote"
    fi

    # Sync source to Windows machine
    log_step "Syncing source to ${host}:${remote_dir}..."
    if $DRY_RUN; then
        log_dry "rsync project to ${host}:${remote_dir}"
        if $CLEAN_REMOTE; then
            log_dry "win_ssh_cmd ${host} 'rm -rf ${remote_dir}/{target,target-static,node_modules,dist,venv}'"
        fi
    else
        # Create remote directory via Git Bash
        win_ssh_cmd "$host" "$port" "$key" "mkdir -p ${remote_dir}" 2>/dev/null || true
        if ! rsync_to_win "$host" "$port" "$key" "$remote_dir"; then
            log_error "rsync to Windows failed"
            log_info "Debug: ssh ${host} '\"$WIN_RSYNC_PATH\" --version'"
            record_result "Windows sync: FAILED"
            return
        fi
        if $CLEAN_REMOTE; then
            log_step "Cleaning build artifacts on remote (--clean-remote)..."
            win_ssh_cmd "$host" "$port" "$key" \
                "rm -rf ${remote_dir}/target ${remote_dir}/target-static ${remote_dir}/node_modules ${remote_dir}/dist ${remote_dir}/venv" \
                || log_warning "Remote cleanup failed (non-fatal)"
        else
            log_info "Reusing remote build artifacts (pass --clean-remote to wipe)"
        fi
        log_success "Source synced"
    fi

    if ! $DRY_RUN; then
        if ! win_ssh_cmd "$host" "$port" "$key" \
            "mkdir -p ${remote_dir}/dist && rm -f ${remote_dir}/dist/*${VERSION}*"; then
            log_error "Could not clear stale Windows artifacts for v${VERSION}"
            record_result "Windows cleanup: FAILED"
            return 1
        fi
    fi

    # Build Windows x86_64 (host arch)
    # build-msix.ps1 needs both sotf-desktop.exe AND sotf-tui.exe present in
    # the corresponding target dir before it runs.
    log_step "Building Windows x86_64 (native MSVC on remote, profile=dist)..."
    local win_build_cmd="cd ${remote_dir} && mkdir -p dist"
    # Release cuts use the `dist` profile (fat LTO + codegen-units=1).
    # Artifacts land in target/dist/ instead of target/release/.
    win_build_cmd+=" && cargo build --profile dist -p sotf-tui --bin sotf-tui"
    win_build_cmd+=" && cargo build --profile dist -p sotf-gpui --bin sotf-desktop"
    # Canonical artifact naming: <product>-<version>-<os>-<arch>.<ext>
    win_build_cmd+=" && cp target/dist/sotf-tui.exe dist/sotf-tui-${VERSION}-windows-x86_64.exe"

    if $DRY_RUN; then
        log_dry "win_ssh_cmd ${host} '${win_build_cmd}'"
    else
        if ! win_ssh_cmd "$host" "$port" "$key" "$win_build_cmd"; then
            log_error "Windows x86_64 build failed"
            record_result "Windows x86_64: FAILED"
            return
        fi
        record_result "Windows x86_64: OK"
    fi

    # Build Windows arm64 (cross-compiled on the x86_64 build host).
    # Required for Microsoft Store ARM64 listing and for native execution on
    # Windows-on-ARM (Surface Pro X, Copilot+ PCs). Failure here doesn't kill
    # the x86_64 result; we just skip the arm64 MSIX.
    log_step "Building Windows arm64 (cross-compiled on remote, profile=dist)..."
    local win_arm_build_cmd="cd ${remote_dir}"
    win_arm_build_cmd+=" && rustup target add aarch64-pc-windows-msvc"
    win_arm_build_cmd+=" && cargo build --profile dist --target aarch64-pc-windows-msvc -p sotf-tui --bin sotf-tui"
    win_arm_build_cmd+=" && cargo build --profile dist --target aarch64-pc-windows-msvc -p sotf-gpui --bin sotf-desktop"
    win_arm_build_cmd+=" && cp target/aarch64-pc-windows-msvc/dist/sotf-tui.exe dist/sotf-tui-${VERSION}-windows-arm64.exe"

    local windows_arm64_ok=false
    if $DRY_RUN; then
        log_dry "win_ssh_cmd ${host} '${win_arm_build_cmd}'"
        windows_arm64_ok=true
    else
        if win_ssh_cmd "$host" "$port" "$key" "$win_arm_build_cmd"; then
            record_result "Windows arm64: OK"
            windows_arm64_ok=true
        else
            log_warning "Windows arm64 build failed (continuing with x86_64 only)"
            log_info "On the remote: rustup target add aarch64-pc-windows-msvc"
            log_info "and ensure VS 2022 has the ARM64 build tools component installed."
            record_result "Windows arm64: UNAVAILABLE (optional target)"
        fi
    fi

    # Sign on Windows if cert thumbprint available
    if ! $SKIP_SIGN && [ -n "${WINDOWS_CERT_THUMBPRINT:-}" ]; then
        log_step "Signing Windows executables (SignTool on remote)..."
        local sign_cmd="cd ${remote_dir}"
        sign_cmd+=" && powershell.exe -ExecutionPolicy Bypass -File scripts/sign-windows.ps1"
        sign_cmd+=" -CertificateThumbprint ${WINDOWS_CERT_THUMBPRINT}"
        if $DRY_RUN; then
            log_dry "win_ssh_cmd ${host} sign-windows.ps1"
        else
            if win_ssh_cmd "$host" "$port" "$key" "$sign_cmd"; then
                log_success "Windows signing complete"
                record_result "Windows signing (SignTool): OK"
            else
                log_warning "Windows signing failed (continuing)"
                record_result "Windows signing (SignTool): FAILED"
            fi
        fi
    fi

    # Build MSIX package on remote. Honour --skip-sign so the test path
    # doesn't try to invoke SignTool / a code-signing cert.
    #
    # Pre-clean any stale Windows artifacts for this version on the remote so
    # that arch suffix changes (e.g. the historical x64 → x86_64 rename) don't
    # leave both names in dist/ and get rsynced back into the local dist/.
    #
    # We invoke build-msix.ps1 directly via powershell.exe rather than the
    # bash port via Git Bash. Native PowerShell sees `/o`, `/v`, `/d`, `/p`
    # for what they are (MakeAppx flags) instead of MSYS path-style args, and
    # has no need for cygpath, MSYS_NO_PATHCONV, or a third-party `zip.exe`
    # on PATH.
    log_step "Building MSIX package..."
    local ps_sign=""
    if ! $SKIP_SIGN; then
        ps_sign=" -Sign"
        # Forward cert credentials to the remote PowerShell. SSH usually
        # strips env vars (no AcceptEnv on Windows OpenSSH by default), so
        # we pass them as explicit -CertThumbprint / -CertFile / etc. flags
        # which build-msix.ps1 prefers over $env:WINDOWS_CERT_*.
        if [ -n "${WINDOWS_CERT_THUMBPRINT:-}" ]; then
            ps_sign+=" -CertThumbprint ${WINDOWS_CERT_THUMBPRINT}"
        elif [ -n "${WINDOWS_CERT_FILE:-}" ]; then
            ps_sign+=" -CertFile '${WINDOWS_CERT_FILE}'"
            if [ -n "${WINDOWS_CERT_PASSWORD:-}" ]; then
                ps_sign+=" -CertPassword '${WINDOWS_CERT_PASSWORD}'"
            fi
        fi
        if [ -n "${WINDOWS_TIMESTAMP_URL:-}" ]; then
            ps_sign+=" -TimestampUrl '${WINDOWS_TIMESTAMP_URL}'"
        fi
    fi
    local msix_cleanup="cd ${remote_dir} && rm -f dist/sotf-desktop-${VERSION}-windows-*.msix"
    # Pass -BuildDir explicitly so build-msix.ps1 can't auto-detect the wrong
    # cargo output dir. The x86_64 cargo build above is native (no --target),
    # so its binaries land in target/dist/; arm64 is cross-compiled to
    # target/aarch64-pc-windows-msvc/dist/.
    local msix_x86="powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-msix.ps1 -Arch x86_64 -BuildDir target/dist${ps_sign}"
    local msix_arm="powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-msix.ps1 -Arch arm64 -BuildDir target/aarch64-pc-windows-msvc/dist${ps_sign}"

    if $DRY_RUN; then
        log_dry "win_ssh_cmd ${host} '${msix_cleanup}'"
        log_dry "win_ssh_cmd ${host} '${msix_x86}'"
        $windows_arm64_ok && log_dry "win_ssh_cmd ${host} '${msix_arm}'"
    else
        # Cleanup runs once before either MSIX so we don't wipe the first one.
        if ! win_ssh_cmd "$host" "$port" "$key" "$msix_cleanup"; then
            log_error "Could not clear stale Windows MSIX files"
            record_result "Windows MSIX cleanup: FAILED"
            return 1
        fi

        if win_ssh_cmd "$host" "$port" "$key" "cd ${remote_dir} && ${msix_x86}"; then
            record_result "Windows MSIX x86_64: OK"
        else
            log_warning "MSIX x86_64 build failed (continuing)"
            record_result "Windows MSIX x86_64: FAILED"
        fi

        # Only try the arm64 MSIX if the arm64 binaries actually built.
        # Skipping silently when the cargo build above failed avoids a second
        # confusing red error message for the same root cause.
        if $windows_arm64_ok; then
            if win_ssh_cmd "$host" "$port" "$key" "cd ${remote_dir} && ${msix_arm}"; then
                record_result "Windows MSIX arm64: OK"
            else
                log_warning "MSIX arm64 build failed (continuing)"
                record_result "Windows MSIX arm64: UNAVAILABLE (optional target)"
            fi
        else
            log_info "Skipping arm64 MSIX (arm64 binaries didn't build)"
            record_result "Windows MSIX arm64: SKIPPED"
        fi
    fi

    # Copy artifacts back (current version only)
    log_step "Fetching Windows artifacts (v${VERSION})..."
    if $DRY_RUN; then
        log_dry "rsync ${host}:${remote_dir}/dist/*${VERSION}* -> ${DIST_DIR}/"
    else
        if ! rsync_from_win "$host" "$port" "$key" \
            "${remote_dir}/dist/" "$DIST_DIR/" \
            --include='*/' --include="*${VERSION}*" --exclude='*'; then
            log_error "rsync from Windows failed"
            record_result "Windows fetch: FAILED"
            return
        fi
        log_success "Windows artifacts fetched"
        local windows_artifact
        for windows_artifact in \
            "$DIST_DIR/sotf-tui-${VERSION}-windows-x86_64.exe" \
            "$DIST_DIR/sotf-desktop-${VERSION}-windows-x86_64.msix"; do
            record_artifact "$windows_artifact" "Windows required artifact" || true
        done
    fi

    local result_index windows_failed=false
    for ((result_index = first_result_index; result_index < ${#BUILD_RESULTS[@]}; result_index++)); do
        case "${BUILD_RESULTS[$result_index]}" in
            *FAILED*) windows_failed=true ;;
        esac
    done
    if $windows_failed; then
        log_error "Windows phase failed ($(elapsed))"
        return 1
    fi
    log_success "Windows builds complete ($(elapsed))"
    return 0
}

# -----------------------------------------------------------------------
# PHASE 4: POST-BUILD (cosign, checksums, site, release prep)
# -----------------------------------------------------------------------

post_build() {
    log_phase "Phase 4: Post-Build (signing, checksums, site)"
    start_timer

    # --- Normalize artifact filenames to <product>-<version>-<os>-<arch>.<ext> ---
    # Some helper scripts (e.g. build-dmg-sotf.sh) emit <product>-<os>-<arch>-<version>.<ext>;
    # rename them here so checksums and cosign signatures use the canonical name.
    normalize_artifact_names
    if [ "$BUILD_FAILED" = true ]; then
        log_error "Artifact normalization failed; stopping post-build processing"
        return 1
    fi

    # --- Cosign non-Apple artifacts ---
    if ! $SKIP_SIGN; then
        sign_with_cosign
    fi
    if [ "$BUILD_FAILED" = true ]; then
        log_error "Artifact signing failed; stopping post-build processing"
        return 1
    fi

    # --- SHA256 checksums ---
    generate_checksums
    if [ "$BUILD_FAILED" = true ]; then
        log_error "Checksum generation failed; stopping post-build processing"
        return 1
    fi

    # --- Release notes ---
    generate_release_notes
    if [ "$BUILD_FAILED" = true ]; then
        log_error "Release notes generation failed; stopping post-build processing"
        return 1
    fi

    # --- Update site ---
    if ! $SKIP_SITE; then
        update_site
    fi

    log_success "Post-build complete ($(elapsed))"
}

normalize_artifact_names() {
    log_step "Normalizing artifact filenames..."

    if $DRY_RUN; then
        log_dry "Rename any <product>-<os>-<arch>-<version>.<ext> in dist/ to <product>-<version>-<os>-<arch>.<ext>"
        return
    fi

    # Match: <product>-<os>-<arch>-<version>.<ext>
    # Use bash regex with BASH_REMATCH; .ext intentionally greedy so .tar.gz survives.
    local pattern='^(.+)-(macos|darwin|linux|windows|win)-(arm64|aarch64|x86_64|amd64|x86|arm)-([0-9]+\.[0-9]+\.[0-9]+)(\..+)$'
    local renamed=0
    shopt -s nullglob
    for f in "$DIST_DIR"/*; do
        [ -f "$f" ] || continue
        local base
        base=$(basename "$f")
        if [[ "$base" =~ $pattern ]]; then
            local product="${BASH_REMATCH[1]}"
            local os="${BASH_REMATCH[2]}"
            local arch="${BASH_REMATCH[3]}"
            local ver="${BASH_REMATCH[4]}"
            local ext="${BASH_REMATCH[5]}"
            local newname="${product}-${ver}-${os}-${arch}${ext}"
            if [ "$base" = "$newname" ]; then
                continue
            fi
            if [ -e "$DIST_DIR/$newname" ]; then
                log_warning "Skip rename: target exists ($newname)"
                continue
            fi
            log_info "Renaming: $base → $newname"
            if ! mv "$f" "$DIST_DIR/$newname"; then
                log_error "Could not normalize artifact name: $base"
                record_result "Artifact rename ${base}: FAILED"
                continue
            fi
            renamed=$((renamed + 1))
        fi
    done
    shopt -u nullglob

    if [ "$renamed" -gt 0 ]; then
        log_success "Renamed $renamed artifact(s) to canonical naming"
    else
        log_info "No artifacts needed renaming"
    fi
}

sign_with_cosign() {
    log_step "Signing artifacts with cosign..."

    if ! command -v cosign &>/dev/null; then
        log_warning "cosign not installed, skipping"
        record_result "Cosign: SKIPPED (not installed)"
        return
    fi

    local use_key=false
    if [ -n "${COSIGN_KEY:-}" ]; then
        use_key=true
        log_info "Using key-based signing: $COSIGN_KEY"
    else
        log_info "Using keyless OIDC signing (will open browser)"
    fi

    local signed_count=0

    for artifact in "$DIST_DIR"/*; do
        [ -f "$artifact" ] || continue
        local name
        name=$(basename "$artifact")

        # Cosign whitelist: only sign Windows .exe and Linux .AppImage / .tar.gz.
        # macOS .dmg / .pkg / bare binaries are already signed by Apple Developer ID.
        # .msix is signed by the Microsoft Store at submission time, not by us.
        # Logs, checksums, release notes, and existing cosign bundles are not signed.
        case "$name" in
            *.exe|*.AppImage|*.appimage|*.tar.gz) ;;
            *) continue ;;
        esac

        # Only sign artifacts for the current version.
        case "$name" in
            *"$VERSION"*) ;;
            *) continue ;;
        esac

        # Respect --skip-* flags so we don't sign artifacts for platforms we
        # didn't build this run. Canonical naming carries the OS segment.
        case "$name" in
            *linux*)   if $SKIP_LINUX;   then continue; fi ;;
            *windows*) if $SKIP_WINDOWS; then continue; fi ;;
        esac

        if $DRY_RUN; then
            log_dry "cosign sign-blob $name"
        else
            log_info "Signing: $name"
            if $use_key; then
                cosign sign-blob \
                    --key "$COSIGN_KEY" \
                    --bundle "${artifact}.bundle" \
                    "$artifact" || {
                    log_warning "Failed to sign $name"
                    record_result "Cosign ${name}: FAILED"
                    continue
                }
        else
            cosign sign-blob \
                --bundle "${artifact}.bundle" \
                "$artifact" || {
                log_warning "Failed to sign $name"
                record_result "Cosign ${name}: FAILED"
                continue
            }
            fi
            if [ ! -s "${artifact}.bundle" ]; then
                log_warning "cosign returned success without creating a bundle for $name"
                record_result "Cosign ${name}: FAILED (missing bundle)"
                continue
            fi
            signed_count=$((signed_count + 1))
        fi
    done

    if [ "$signed_count" -gt 0 ]; then
        log_success "Cosigned $signed_count artifacts"
        record_result "Cosign: OK ($signed_count artifacts)"
    else
        log_info "No artifacts to cosign"
        record_result "Cosign: nothing to sign"
    fi
}

generate_checksums() {
    log_step "Generating SHA256 checksums..."

    if $DRY_RUN; then
        log_dry "sha256sum dist/*${VERSION}* > dist/SHA256SUMS"
        return
    fi

    local checksum_file="$DIST_DIR/SHA256SUMS"
    rm -f "$checksum_file"

    cd "$DIST_DIR"
    local count=0
    for f in *; do
        [ -f "$f" ] || continue
        # Skip signatures, checksums, markdown
        case "$f" in
            *.bundle|*.sig|*.cert|*.md|SHA256SUMS*) continue ;;
        esac
        # Only checksum current version artifacts
        case "$f" in
            *"$VERSION"*) ;;
            *) continue ;;
        esac
        if ! shasum -a 256 "$f" >> "$checksum_file"; then
            log_error "Failed to checksum $f"
            record_result "Checksums: FAILED"
            cd "$PROJECT_ROOT"
            return 1
        fi
        count=$((count + 1))
    done
    cd "$PROJECT_ROOT"

    if [ "$count" -gt 0 ]; then
        log_success "SHA256 checksums for $count files: $checksum_file"
    else
        log_warning "No artifacts found for checksumming"
    fi
}

generate_release_notes() {
    log_step "Generating release notes..."

    # Forward our --skip-site to build-release.sh so it doesn't rewrite
    # site/src/components/Download.astro when the user has asked us to leave
    # the site alone.
    local args=(--skip-build)
    if $SKIP_SITE; then
        args+=(--skip-site)
    fi

    if $DRY_RUN; then
        log_dry "./scripts/build-release.sh ${args[*]}"
        return
    fi

    # Reuse the existing release notes generator
    if ./scripts/build-release.sh "${args[@]}"; then
        record_result "Release notes: OK"
    else
        log_error "Release notes generation failed"
        record_result "Release notes: FAILED"
    fi
}

update_site() {
    log_step "Updating website Download component..."

    if $DRY_RUN; then
        log_dry "Update site/src/components/Download.astro to v${VERSION}"
        return
    fi

    # The build-release.sh --skip-build already updates the site component,
    # but we also build the site and optionally deploy it.

    local site_dir="$PROJECT_ROOT/site"
    if [ ! -d "$site_dir" ]; then
        log_warning "Site directory not found, skipping"
        return
    fi

    log_step "Building site..."
    cd "$site_dir"
    if ! npm run build; then
        cd "$PROJECT_ROOT"
        log_error "Site build failed"
        record_result "Site build: FAILED"
        return
    fi
    cd "$PROJECT_ROOT"

    log_success "Site built"

    if $DEPLOY_SITE; then
        if [ ! -x "$site_dir/update_prod.sh" ]; then
            log_warning "$site_dir/update_prod.sh missing or not executable; skipping deploy"
            record_result "Site deploy: SKIPPED (script not found)"
        else
            log_step "Deploying site + release binaries via update_prod.sh..."
            cd "$site_dir"
            if ./update_prod.sh; then
                log_success "Site + binaries deployed"
                record_result "Site deploy: OK"
            else
                log_error "update_prod.sh failed"
                record_result "Site deploy: FAILED"
            fi
            cd "$PROJECT_ROOT"
        fi
    elif [ -n "${SITE_DEPLOY_TARGET:-}" ]; then
        log_info "Site deploy target: $SITE_DEPLOY_TARGET"
        log_info "Re-run with --deploy-site to push, or manually: ./site/update_prod.sh"
    fi

    record_result "Site update: OK"
}

# -----------------------------------------------------------------------
# SUMMARY
# -----------------------------------------------------------------------

print_summary() {
    log_phase "Release Build Summary — v${VERSION}"

    # List artifacts
    log_step "Artifacts in ${DIST_DIR}:"
    local artifact_count=0
    for f in "$DIST_DIR"/*"$VERSION"*; do
        if [ -f "$f" ]; then
            local size
            size=$(du -h "$f" | cut -f1)
            local name
            name=$(basename "$f")
            printf "  %-50s %s\n" "$name" "$size"
            artifact_count=$((artifact_count + 1))
        fi
    done

    if [ "$artifact_count" -eq 0 ]; then
        log_warning "No artifacts found for v${VERSION}"
    fi

    echo ""

    # Build results
    if [ ${#BUILD_RESULTS[@]} -gt 0 ]; then
        log_step "Build results:"
        for result in "${BUILD_RESULTS[@]}"; do
            echo "  $result"
        done
        echo ""
    fi

    # Checksums
    if [ -f "$DIST_DIR/SHA256SUMS" ]; then
        log_step "Checksums: $DIST_DIR/SHA256SUMS"
    fi

    # Release notes
    if [ -f "$DIST_DIR/release-${VERSION}.md" ]; then
        log_step "Release notes: $DIST_DIR/release-${VERSION}.md"
    fi

    echo ""
    log_step "Next steps:"
    echo "  1. Review release notes: $DIST_DIR/release-${VERSION}.md"
    echo "  2. Add changelog entries"
    echo "  3. Create GitHub release:"
    echo "     git tag v${VERSION}"
    echo "     git push github v${VERSION}"
    echo "     gh release create v${VERSION} --title 'SotF v${VERSION}' \\"
    echo "       --notes-file $DIST_DIR/release-${VERSION}.md \\"
    echo "       $DIST_DIR/*${VERSION}* $DIST_DIR/SHA256SUMS"
    echo "  4. Deploy site: cd site && ./update_prod.sh"
}

# -----------------------------------------------------------------------
# MAIN
# -----------------------------------------------------------------------

main() {
    log_phase "SotF Release Build v${VERSION}"

    if $DRY_RUN; then
        log_warning "DRY RUN — no actions will be taken"
    fi

    preflight

    if ! $POST_ONLY; then
        if ! $DRY_RUN; then
            if ! clear_versioned_artifacts "$DIST_DIR"; then
                record_result "Artifact cleanup: FAILED"
                print_summary
                return 1
            fi
            if ! rm -f -- "$DIST_DIR/SHA256SUMS"; then
                log_error "Could not remove stale SHA256SUMS"
                record_result "Artifact cleanup: FAILED"
                print_summary
                return 1
            fi
            RELEASE_ARTIFACTS=()
        fi
        build_macos || true
        build_linux || true
        build_windows || true
    else
        log_info "Post-only mode — skipping builds"
    fi

    if [ "$BUILD_FAILED" = true ]; then
        print_summary
        log_error "One or more required release steps failed; skipping post-build signing and release metadata"
        return 1
    fi

    post_build
    print_summary
    if [ "$BUILD_FAILED" = true ]; then
        log_error "One or more required release steps failed; refusing a successful release status"
        return 1
    fi
    return 0
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main
fi
