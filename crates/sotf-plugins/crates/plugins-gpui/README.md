# plugins-gpui

Common GPUI rendering infrastructure for SOTF audio plugin UIs.

## What It Does

Provides shared abstractions and rendering helpers that enable the same plugin UI code to work in both the full GPUI player app and standalone macOS Audio Unit plugin views. Plugin UIs are generic over the `PluginViewHost` trait, which abstracts parameter access and UI state management.

## Features

- `PluginViewHost` trait for host-agnostic plugin UI rendering
- Shared audio design tokens derived from the design system
- Meter theme configuration (LUFS, true peak, level meters)
- Frequency and dB scale tick mark rendering
- Knob drag state management
- Band operations (add, remove, mute, solo) for multi-band plugins
- Per-channel mode support

## Usage

```rust
use plugins_gpui::PluginViewHost;

// Plugin UIs accept any host that implements PluginViewHost
fn render_eq_ui<H: PluginViewHost>(host: &mut H, plugin_idx: usize) {
    // Set a parameter value
    host.set_plugin_param(plugin_idx, 0, 1000.0); // frequency

    // Add a new EQ band
    host.add_band(plugin_idx);
}
```

## Architecture

```
host.rs           -- PluginViewHost trait (core abstraction)
common.rs         -- Shared rendering helpers (knobs, sliders, meters)
design_tokens.rs  -- Audio-specific design tokens
meter_theme.rs    -- Meter visual configuration
theme.rs          -- Plugin UI color theme
ticks.rs          -- Scale tick mark rendering
```

## Testing

```bash
cargo check -p plugins-gpui
```

## License

Part of the SOTF (Sound of the Future) project.
