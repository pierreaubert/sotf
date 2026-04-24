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

## Design-token drift guard

Spacing, text size, corner radius, and icon size must flow through the design
system (`components/design.rs` → `Ds::from_cx(cx)`, or `spacing::*`/`radius::*`
in `app/constants.rs`). This keeps fonts, icons, and layout scaling together
when the user invokes the font-zoom actions.

`scripts/check-design-tokens.py` fails CI when raw `px(N.0)` appears in
`components/` or `ui/` outside the allowlist or without justification. Run it
locally before committing UI changes:

```bash
python3 scripts/check-design-tokens.py
```

Legitimate exceptions:
- Same-line `// intentional: <reason>` trailing comment.
- `// intentional: <reason>` comment within 8 lines above (not crossing a
  blank line).
- File-level `// intentional-file: <reason>` marker anywhere in the file —
  use this for chart/meter/table code where pixel dimensions are
  intrinsically layout-driven.
