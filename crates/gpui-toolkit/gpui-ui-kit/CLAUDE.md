# gpui-ui-kit (lib: `gpui_ui_kit`, version: 0.6.0)

Reusable UI component library for the GPUI framework.

## Key Components

- Button, Input, Slider, Dropdown, Modal, Tabs, Toggle
- Theme system integration
- Node graph editor
- Workflow builder
- Read `GPUI.md` at the project root before working on GPUI code

## Dependencies

- `gpui` - GPU-accelerated UI framework
- `gpui-ui-kit-macros` - Procedural macros for component definitions
- `serde`, `uuid`

## Examples

```bash
cargo run --release --example showcase -p gpui-ui-kit      # Component gallery
cargo run --release --example workflow_debug -p gpui-ui-kit # Workflow editor
```

## Testing

```bash
cargo test -p gpui-ui-kit --lib
cargo check -p gpui-ui-kit && cargo clippy -p gpui-ui-kit
```
