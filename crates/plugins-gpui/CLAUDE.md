# plugins-gpui (lib: `gpui-au`)

GPUI-based UI embedded in a macOS Audio Unit plugin.

## Purpose

Provides a GPU-accelerated UI for the Audio Unit plugin by embedding the GPUI renderer inside a macOS NSView.

## Key Features

- Embeds GPUI renderer in macOS Audio Unit
- NSView extraction for AUv3 integration
- Metal rendering backend

## Platform

macOS only.

## Binaries

- `gpui_au_test` - Test harness for the AU UI

## Testing

```bash
cargo check -p plugins-gpui && cargo clippy -p plugins-gpui
```

## Notes

- Read `GPUI.md` at the project root before working on GPUI code
- This crate bridges GPUI rendering into the Audio Unit host's view hierarchy
