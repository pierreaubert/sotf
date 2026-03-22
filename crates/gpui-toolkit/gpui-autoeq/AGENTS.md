# gpui-autoeq (lib: `gpui_autoeq`)

AutoEQ parameter form component for GPUI applications.

## Overview

Reusable GPUI form component for configuring EQ optimization parameters. Supports Room EQ, Speaker EQ, Headphone EQ, and Group optimization workflows.

## Dependencies

- `gpui` -- GPUI framework
- `gpui-ui-kit` -- UI form components (Input, Select, Slider, etc.)
- `gpui-ui-kit-macros` -- Theme derivation macros

## Testing

```bash
cargo check -p gpui-autoeq
cargo run --example autoeq_form_debug
```

## Important Notes

- Uses GPUI's native `div()`-based rendering, not HTML/SVG
- See parent `gpui-toolkit/AGENTS.md` for common patterns
