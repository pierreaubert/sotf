# plugins-bridge

Format-agnostic adapter for SOTF audio plugins -- shared factory and parameter bridge used by AU and VST3/CLAP wrappers.

## Architecture

```
lib.rs          -- Re-exports create_plugin() and ParamBridge
factory.rs      -- create_plugin(): universal plugin factory (type string + JSON -> Box<dyn Plugin>)
                   available_plugin_types(): list of all 30+ supported plugin types
param_bridge.rs -- ParamBridge: maps between ParamSpec-based parameter system and plugin format hosts
buffers.rs      -- Buffer conversion utilities for interleave/deinterleave
state.rs        -- Plugin state serialization helpers
```

## Key Public API

- `create_plugin(plugin_type: &str, channels: usize, sample_rate: u32, config_json: &str) -> Result<Box<dyn Plugin>, String>` -- Universal factory for all SOTF plugins
- `available_plugin_types() -> &'static [&'static str]` -- Lists all 30+ supported plugin type strings
- `ParamBridge` -- Bridges ParamSpec definitions to format-specific parameter systems (AU, VST3, CLAP)

## Testing

```bash
cargo test -p plugins-bridge
```

## Important Notes

- Plugin type strings are case-insensitive for Wave 1 plugins (e.g., both "EQ" and "eq" work)
- "Compressor" and "Expander" route to their multiband counterparts in single-band mode (`num_bands=1`)
- "FletcherMunson" is a backward-compat alias that routes to LoudnessCompensation in Auto mode
- Some plugins need `sample_rate` at construction time (EQ, XTC, Convolution, LinearPhaseEQ); most receive it later via `initialize()`
- `parse_params()` accepts empty string, `"null"`, or `"{}"` for default parameters
- `ParametricInPlacePlugin` implementations are wrapped in `ParametricInPlacePluginAdapter` before returning as `Box<dyn Plugin>`
- This crate depends on all 30+ `sotf-plugin-*` crates -- it is the dependency aggregation point for plugin wrappers
