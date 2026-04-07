# plugins-gpui

Common GPUI rendering infrastructure for SOTF audio plugin UIs.

## Architecture

```
lib.rs            -- Re-exports: PluginViewHost, MeterTheme, audio_tokens_from_ds, ScaleType, TickConfig, render_tick_row, theme
host.rs           -- PluginViewHost trait: abstraction over host environment (app-gpui AppState or AU AuHostState)
common.rs         -- Shared rendering helpers (knobs, sliders, toggles, meters)
design_tokens.rs  -- audio_tokens_from_ds(): converts gpui-design tokens to audio-specific design tokens
meter_theme.rs    -- MeterTheme, LufsConfig, TruePeakConfig: visual styling for audio meters
theme.rs          -- Plugin UI color theme and styling constants
ticks.rs          -- ScaleType, TickConfig, render_tick_row(): frequency/dB scale tick marks for audio UIs
```

## Key Public API

- `PluginViewHost` trait -- Core abstraction enabling plugin UIs to work in both the GPUI player and standalone AU views. Methods: `set_plugin_param()`, `reset_plugin_param()`, `set_editing_plugin()`, `on_knob_drag_start()`/`end()`, band operations (add/remove/mute/solo), channel operations.
- `audio_tokens_from_ds()` -- Converts design system tokens to audio-specific tokens
- `MeterTheme` / `LufsConfig` / `TruePeakConfig` -- Meter visual configuration
- `ScaleType` / `TickConfig` / `render_tick_row()` -- Tick mark rendering for frequency/dB scales

## Testing

```bash
cargo check -p plugins-gpui
```

## Important Notes

- Plugin UIs are generic over `PluginViewHost` -- they never reference `AppState` or `AuHostState` directly
- In app-gpui context, `plugin_idx` identifies which plugin in the chain; in AU context it is always 0
- `PluginViewHost` provides default no-op implementations for band and channel operations (not all plugins use them)
- Depends on `gpui`, `gpui-design`, `gpui-ui-kit` for rendering; `math-iir-fir` for EQ response curves; `sotf-host` for ParamSpec/PluginLayout; `sotf-midi` for MIDI controller mapping
- Plugin-specific UIs live in each plugin crate's `ui` module behind the `gpui-ui` feature flag, not here
