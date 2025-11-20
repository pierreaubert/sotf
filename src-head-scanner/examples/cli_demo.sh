#!/bin/bash
# Head Scanner CLI Demo Script
#
# This script demonstrates the usage of the head-scanner-cli tool

set -e

echo "🎥 Head Scanner CLI Demo"
echo "======================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if binary exists
if [ ! -f "../target/release/head-scanner-cli" ]; then
    echo -e "${YELLOW}Building head-scanner-cli...${NC}"
    cd ..
    cargo build --release --bin head-scanner-cli
    cd examples
    echo -e "${GREEN}✓ Build complete${NC}"
    echo ""
fi

CLI="../target/release/head-scanner-cli"

# Step 1: Show camera info
echo -e "${BLUE}Step 1: Checking camera information${NC}"
$CLI info --camera 0 || echo "Camera 0 not available"
echo ""

# Step 2: Test camera
echo -e "${BLUE}Step 2: Testing camera (5 seconds)${NC}"
echo "This will capture frames to verify camera is working..."
$CLI test --duration 5 --camera 0 || echo "Camera test failed"
echo ""

# Step 3: Quick scan demo
echo -e "${BLUE}Step 3: Running quick scan demo${NC}"
echo "This will perform a short scan with lower quality settings..."
echo "Press Ctrl+C to stop early if needed"
echo ""

$CLI scan \
  --output demo_scan.obj \
  --width 640 \
  --height 480 \
  --min-coverage 50 \
  --max-duration 30 \
  --camera 0 \
  --verbose

echo ""
echo -e "${GREEN}✓ Demo complete!${NC}"
echo ""
echo "Output file: demo_scan.obj"
echo ""
echo "Next steps:"
echo "  - View the mesh: open demo_scan.obj (macOS) or meshlab demo_scan.obj"
echo "  - Run a full scan: $CLI scan --help"
echo "  - Adjust settings for better quality"
