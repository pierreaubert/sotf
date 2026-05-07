#!/bin/bash
#
# Sync auxiliary version-pinned files to match the workspace version in the
# top-level Cargo.toml.
#
# Updated files:
#   - builds/windows/AppxManifest.xml   (Identity Version="...")
#   - site/package.json                 (top-level "version": "...")
#
# Idempotent: a run with no drift exits 0 silently (apart from a single OK
# line). Drift is reported per file.
#
# Usage:
#   ./scripts/sync-version.sh                # Update files in-place if drifted
#   ./scripts/sync-version.sh --check        # Exit non-zero on drift, no writes
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CARGO_TOML="$PROJECT_ROOT/Cargo.toml"
APPX_MANIFEST="$PROJECT_ROOT/builds/windows/AppxManifest.xml"
SITE_PACKAGE_JSON="$PROJECT_ROOT/site/package.json"

CHECK_ONLY=false
case "${1:-}" in
    --check) CHECK_ONLY=true ;;
    "")      ;;
    *)
        echo "Unknown option: $1" >&2
        echo "Usage: $0 [--check]" >&2
        exit 1
        ;;
esac

VERSION=$(grep -m1 '^version = ' "$CARGO_TOML" | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from $CARGO_TOML" >&2
    exit 1
fi

drift=0

# Replace via tempfile so we work on both BSD and GNU sed without -i quirks.
apply_sed() {
    local file="$1"
    local script="$2"
    local tmp
    tmp=$(mktemp)
    sed -E "$script" "$file" > "$tmp"
    mv "$tmp" "$file"
}

sync_one() {
    local file="$1"
    local label="$2"
    local current="$3"
    local sed_script="$4"

    if [ -z "$current" ]; then
        echo "ERROR: Could not read current version from $label ($file)" >&2
        exit 1
    fi
    if [ "$current" = "$VERSION" ]; then
        return 0
    fi
    if $CHECK_ONLY; then
        echo "[DRIFT] $label: $current → should be $VERSION"
        drift=1
        return 0
    fi
    echo "[SYNC] $label: $current → $VERSION"
    apply_sed "$file" "$sed_script"
}

# --- AppxManifest.xml --------------------------------------------------
# The Identity element's Version attribute is the only line in the file
# whose stripped form starts with `Version=`. MinVersion / MaxVersionTested
# don't match this anchor.
appx_current=$(grep -E '^[[:space:]]*Version="[^"]*"' "$APPX_MANIFEST" \
    | head -1 \
    | sed -E 's/.*Version="([^"]+)".*/\1/')
sync_one "$APPX_MANIFEST" "AppxManifest.xml" "$appx_current" \
    "s/^([[:space:]]*)Version=\"[^\"]*\"/\1Version=\"$VERSION\"/"

# --- site/package.json -------------------------------------------------
# Only the top-level "version" key matches the leading-whitespace anchor;
# dependency entries are "package-name": "version-string".
pkg_current=$(grep -E '^[[:space:]]*"version":[[:space:]]*"[^"]*"' "$SITE_PACKAGE_JSON" \
    | head -1 \
    | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
sync_one "$SITE_PACKAGE_JSON" "site/package.json" "$pkg_current" \
    "s/^([[:space:]]*)\"version\":([[:space:]]*)\"[^\"]*\"/\1\"version\":\2\"$VERSION\"/"

# --- Final report ------------------------------------------------------
if $CHECK_ONLY && [ "$drift" -eq 1 ]; then
    echo "Version drift detected. Run: $0"
    exit 1
fi

echo "Version sync OK: v$VERSION"
