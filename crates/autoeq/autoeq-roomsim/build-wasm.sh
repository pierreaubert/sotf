#!/bin/bash
# Build WASM module with parallel execution support (wasm-bindgen-rayon)
#
# Requirements:
# - Rust nightly toolchain
# - wasm-pack: cargo install wasm-pack
# - wasm32-unknown-unknown target: rustup target add wasm32-unknown-unknown
#
# The built WASM will support multi-threading via Web Workers + SharedArrayBuffer.
# To use parallel execution in the browser, serve with these headers:
#   Cross-Origin-Opener-Policy: same-origin
#   Cross-Origin-Embedder-Policy: require-corp

set -e

cd "$(dirname "$0")"

echo "Building WASM with parallel execution support..."
echo "Using nightly toolchain with atomics and bulk-memory features"

# Build with threading support
# -C target-feature=+atomics,+bulk-memory enables WebAssembly threads
# -Z build-std rebuilds the standard library with thread support
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
    cargo +nightly build \
    --target wasm32-unknown-unknown \
    --release \
    -Z build-std=panic_abort,std

# Run wasm-bindgen to generate JS bindings
# The WASM output is in the workspace root's target directory
echo "Generating JS bindings..."
wasm-bindgen \
    --target web \
    --out-dir plotting/pkg \
    ../target/wasm32-unknown-unknown/release/autoeq_roomsim.wasm

echo ""
echo "Build complete! Output in plotting/pkg/"
echo ""
echo "To use parallel execution, serve with Cross-Origin Isolation headers:"
echo "  Cross-Origin-Opener-Policy: same-origin"
echo "  Cross-Origin-Embedder-Policy: require-corp"
echo ""
echo "For local testing, you can use:"
echo "  npx serve plotting --cors"
