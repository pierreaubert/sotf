#!/bin/bash
#
# Scan a macOS Mach-O binary for references to private Apple APIs and
# non-public frameworks. The Mac App Store static analyzer rejects
# uploads whose import table contains SPI symbols (e.g. the famous
# CGSSetWindowBackgroundBlurRadius), even from code paths that are
# unreachable at runtime — Apple checks the symbol *table*, not
# reachability.
#
# Run this as part of build-pkg-mas.sh's preflight to catch the
# rejection locally instead of after a 35 MB Transporter round trip.
#
# Usage:
#   ./scripts/check-mas-private-api.sh <path/to/binary>
#
# Exit codes:
#   0 — clean
#   1 — at least one private API reference or non-public framework link
#   2 — usage / setup error
#
# Allowlist:
#   Some PUBLIC Apple APIs happen to match a private-looking prefix
#   (e.g. CGShieldingWindowLevel from the otherwise-private CGS namespace,
#   or the TISCopy* family from HIToolbox). When a real public API
#   matches the suspicious-pattern regex, add it to ALLOWLIST below.

set -uo pipefail

BIN="${1:-}"
case "$BIN" in
    ""|--help|-h)
        sed -n '2,/^$/p' "$0" | sed 's/^#//' | sed 's/^ //'
        exit 2
        ;;
esac

[ -f "$BIN" ] || { echo "ERROR: binary not found: $BIN" >&2; exit 2; }
file "$BIN" 2>/dev/null | grep -q "Mach-O" \
    || { echo "ERROR: not a Mach-O binary: $BIN" >&2; exit 2; }

# ---- Allowlist: PUBLIC APIs whose names happen to match the regex ----
# Each line is a single symbol-name match (full leading-underscore form
# as returned by `nm -u`). Extend when a new false positive shows up;
# include a comment naming the framework + a docs URL when adding.
ALLOWLIST=(
    # CoreGraphics public — screensaver-overlay window level.
    # https://developer.apple.com/documentation/coregraphics/1454313-cgshieldingwindowlevel
    _CGShieldingWindowLevel
    # HIToolbox/Carbon public — Text Input Services.
    # https://developer.apple.com/documentation/carbon/1535533-tiscopycurrentkeyboardinputsource
    _TISCopyCurrentKeyboardInputSource
    _TISCopyCurrentKeyboardLayoutInputSource
    _TISGetInputSourceProperty
)

# ---- Suspicious patterns (private namespaces + naming conventions) ----
# Anchored at start of name where it makes sense; substring elsewhere.
# `_dyld_*` looks suspicious but the well-known names (image_count,
# get_image_header, get_image_name, get_image_vmaddr_slide, etc.) are
# all public per <mach-o/dyld.h>; not in the prefix list.
PREFIX_REGEX='^_(CGS|SLS|SLPS|CPS|SkyLight|AXEnableEnhanced|NSPrivate)'
SUBSTRING_REGEX='(Private|Internal|_SPI|_spi_)[A-Za-z0-9_]*$'

# ---- Symbol scan -----------------------------------------------------
imports=$(nm -u "$BIN" 2>/dev/null | awk '{print $1}')

raw_hits=$(echo "$imports" \
    | grep -E "${PREFIX_REGEX}|${SUBSTRING_REGEX}" \
    || true)

flagged_symbols=()
while IFS= read -r sym; do
    [ -z "$sym" ] && continue
    skip=false
    for ok in "${ALLOWLIST[@]}"; do
        [ "$sym" = "$ok" ] && skip=true && break
    done
    $skip || flagged_symbols+=("$sym")
done <<< "$raw_hits"

# ---- Linked-framework scan ------------------------------------------
# Public Apple frameworks live under /System/Library/Frameworks/. Anything
# under /System/Library/PrivateFrameworks/ is SPI; anything outside
# /System/Library or /usr/lib is a third-party dylib that needs to ship
# inside the .app (and would fail App Review for an MSIX-style sideloaded
# install but not necessarily for MAS — flag for a human read).
flagged_frameworks=()
while IFS= read -r fw; do
    [ -z "$fw" ] && continue
    case "$fw" in
        /System/Library/Frameworks/*) ;;
        /usr/lib/libSystem.B.dylib) ;;
        /usr/lib/libc++*.dylib) ;;
        /usr/lib/libobjc.A.dylib) ;;
        /usr/lib/libiconv.*.dylib) ;;
        /System/Library/PrivateFrameworks/*)
            flagged_frameworks+=("PRIVATE: $fw")
            ;;
        *)
            flagged_frameworks+=("THIRD-PARTY: $fw")
            ;;
    esac
done < <(otool -L "$BIN" 2>/dev/null | sed 1d | awk '{print $1}')

# ---- Report ---------------------------------------------------------
if [ ${#flagged_symbols[@]} -eq 0 ] && [ ${#flagged_frameworks[@]} -eq 0 ]; then
    echo "[OK] No private Apple API references found in:"
    echo "     $BIN"
    exit 0
fi

echo "[FAIL] Mac App Store private-API check failed for:"
echo "       $BIN"

if [ ${#flagged_symbols[@]} -gt 0 ]; then
    echo
    echo "  Suspicious imported symbols (${#flagged_symbols[@]}):"
    for s in "${flagged_symbols[@]}"; do
        echo "    - $s"
    done
fi

if [ ${#flagged_frameworks[@]} -gt 0 ]; then
    echo
    echo "  Suspicious framework links (${#flagged_frameworks[@]}):"
    for fw in "${flagged_frameworks[@]}"; do
        echo "    - $fw"
    done
fi

echo
echo "  How to resolve:"
echo "    - SPI / private symbols: vendor + patch the dependency"
echo "      pulling them in (see crates/3rdparties/gpui_macos/ for the"
echo "      established pattern, and docs/MAS-SUBMISSION.md for context)."
echo "    - False-positive public API: add the symbol to the ALLOWLIST"
echo "      array in $0 with a"
echo "      docs URL proving it's public."
echo "    - PrivateFrameworks: same as SPI symbols — vendor and remove"
echo "      the link."

exit 1
