# app-gpui (lib: `sotf_audio_player_gpui`, binary: `SotF`)

GPUI-based desktop music player with GPU-accelerated rendering.

## Key Features

- Modern desktop UI via GPUI framework
- Real-time spectrum and loudness visualization
- Plugin configuration UI
- Library management
- Read `GPUI.md` at the project root before working on GPUI code

## Architecture

- GPU-accelerated rendering via GPUI (Metal on macOS)
- Business logic delegated to the `player` crate
- Uses `gpui-ui-kit` for reusable components
- Uses `gpui-d3rs` / `gpui-px` for charts and visualizations

## Features

- `hal` - macOS HAL support

## Testing

```bash
cargo test -p app-gpui --lib
cargo check -p app-gpui && cargo clippy -p app-gpui
```

Note: `test = false` in lib config due to GPUI macro stack overflow issues in syn.

## Test Suites

Extensive testing: e2e, negative, proptest, component, lifecycle, event_integration, state_machine, config, migration.

## Running

```bash
cargo run --bin SotF --release
```
