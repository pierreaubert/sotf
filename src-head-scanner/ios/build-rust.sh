#!/bin/bash
#
# Build Rust library for iOS
#
# This script builds the head-scanner Rust library for iOS targets
# and creates a universal binary (XCFramework) for use in Xcode
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Rust library for iOS...${NC}"

# Navigate to parent directory (head-scanner root)
cd "$(dirname "$0")/.."

# iOS targets
IOS_TARGETS=(
    "aarch64-apple-ios"           # iOS devices (ARM64)
    "aarch64-apple-ios-sim"       # iOS simulator on Apple Silicon
    "x86_64-apple-ios"            # iOS simulator on Intel
)

# Add targets if not already added
echo -e "${YELLOW}Ensuring Rust targets are installed...${NC}"
for target in "${IOS_TARGETS[@]}"; do
    rustup target add "$target"
done

# Build for each target
echo -e "${YELLOW}Building for iOS targets...${NC}"

BUILD_MODE="${1:-release}"
if [ "$BUILD_MODE" = "release" ]; then
    CARGO_FLAGS="--release"
    BUILD_DIR="release"
else
    CARGO_FLAGS=""
    BUILD_DIR="debug"
fi

for target in "${IOS_TARGETS[@]}"; do
    echo -e "${GREEN}Building for $target...${NC}"
    cargo build $CARGO_FLAGS --target "$target" --lib
done

# Create output directory
OUTPUT_DIR="ios/HeadScanner/Frameworks"
mkdir -p "$OUTPUT_DIR"

# Create universal binary for simulator (combine x86_64 and arm64)
echo -e "${YELLOW}Creating universal binary for simulator...${NC}"
lipo -create \
    "target/aarch64-apple-ios-sim/$BUILD_DIR/libhead_scanner.a" \
    "target/x86_64-apple-ios/$BUILD_DIR/libhead_scanner.a" \
    -output "$OUTPUT_DIR/libhead_scanner_sim.a"

# Copy device binary
echo -e "${YELLOW}Copying device binary...${NC}"
cp "target/aarch64-apple-ios/$BUILD_DIR/libhead_scanner.a" \
   "$OUTPUT_DIR/libhead_scanner_device.a"

# Generate C header (cbindgen)
echo -e "${YELLOW}Generating C header...${NC}"
if command -v cbindgen &> /dev/null; then
    cbindgen --config cbindgen.toml --crate head_scanner --output "$OUTPUT_DIR/HeadScannerFFI.h" --lang c
else
    echo -e "${RED}Warning: cbindgen not found. Install with: cargo install cbindgen${NC}"
    echo -e "${YELLOW}Using existing header from ios/HeadScanner/Sources/Bridge/HeadScannerFFI.h${NC}"
fi

echo -e "${GREEN}✓ Build complete!${NC}"
echo -e "${GREEN}Libraries generated in: $OUTPUT_DIR${NC}"
echo ""
echo -e "${YELLOW}To use in Xcode:${NC}"
echo -e "1. Add libhead_scanner_device.a or libhead_scanner_sim.a to your target"
echo -e "2. Add HeadScannerFFI.h to your bridging header"
echo -e "3. Link against: -lc++ -lz (if needed)"
