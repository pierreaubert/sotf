#!/bin/sh
# Push the static site + the release binaries to the production VPS.
#
# Two rsync passes:
#   1. site/dist/* (Astro build output) → /var/www/html/spinorama-sotf/
#   2. ../dist/<binaries>                → /var/www/html/spinorama-sotf/downloads/
#
# Run after `npm run build` (site) and after the release pipeline has
# populated the project-level dist/ with current-version artifacts.
#
# The website Download section (site/src/components/Download.astro) links to
# /downloads/<file> for the "From spinorama.org mirror" source — those URLs
# resolve to the files this script uploads.

set -eu

cd "$(dirname "$0")"

REMOTE_HOST="spin@vps-c2ea73ea.vps.ovh.net"
REMOTE_ROOT="/var/www/html/spinorama-sotf"
PROJECT_DIST="../dist"

# 1. Static site
if [ ! -d dist ]; then
    echo "ERROR: site/dist/ missing — run 'npm run build' first" >&2
    exit 1
fi

echo "==> Uploading static site → ${REMOTE_HOST}:${REMOTE_ROOT}/"
rsync -avrz --delete \
    --exclude='/downloads' \
    dist/ "${REMOTE_HOST}:${REMOTE_ROOT}/"

# 2. Release binaries
# Only upload the canonical release artifacts. Skip release-notes markdown,
# checksums get included separately if present, and any staging/leftover dirs
# are filtered out.
if [ ! -d "$PROJECT_DIST" ]; then
    echo "WARNING: $PROJECT_DIST not found — skipping binary upload" >&2
    exit 0
fi

echo "==> Uploading release binaries → ${REMOTE_HOST}:${REMOTE_ROOT}/downloads/"
rsync -avrz \
    --include='sotf-desktop-*' \
    --include='sotf-tui-*' \
    --include='sotf-systemwide-*' \
    --include='SHA256SUMS*' \
    --include='*.bundle' \
    --include='*.sig' \
    --include='*.cert' \
    --exclude='*' \
    "$PROJECT_DIST/" \
    "${REMOTE_HOST}:${REMOTE_ROOT}/downloads/"

echo "==> Done"
