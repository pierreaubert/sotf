# Suggested Commands

## Build & Check
```bash
# Full workspace check (with all features)
RUST_MIN_STACK=16777216 cargo check --workspace --lib --bins --tests --examples --features="qa, onnx, hal, gpu-2d, gpu-3d"

# Check a specific crate (use --no-default-features for plugins/engine due to hdf5 issues)
cargo check -p sotf-player
cargo check -p sotf-plugins --no-default-features
cargo check -p sotf-engine --no-default-features

# Clippy (lint)
cargo clippy --all -- -D warnings
```

## Test
```bash
# Full test suite
RUST_MIN_STACK=16777216 cargo test --workspace --lib --bins --tests --examples --features="qa, onnx, hal, gpu-2d, gpu-3d"

# Test a single crate
cargo test -p <crate-name>

# Negative tests & property tests
cargo test -p sotf-gpui --test negative --release
PROPTEST_CASES=10000 cargo test -p sotf-gpui --test proptest_tests --release
```

## Format
```bash
cargo fmt --all
```

## Run
```bash
# GPUI player (debug, with ad-hoc codesigning)
just run-gpui

# GPUI player (release)
just run-gpui-release

# TUI player (macOS)
cargo run --release --bin sotf-tui --features onnx,hal

# TUI player (Linux/Windows)
cargo run --release --bin sotf-tui --features onnx
```

## Build (Production)
```bash
just prod-sotf-gpui    # Release build of GPUI app
just prod-sotf-tui     # Release build of TUI app
just prod-workspace    # Full workspace release
```

## Utilities (system: Darwin/macOS)
```bash
git, ls, cd, grep, find  # Standard Unix tools (macOS versions)
just --list              # List all available Just recipes
```
