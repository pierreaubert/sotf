# plugins-bridge

Format-agnostic adapter for SOTF audio plugins (AU, VST3, CLAP).

## What It Does

Provides a shared bridge layer used by both Audio Unit (via `plugins-ffi`) and VST3/CLAP (via `plugins-nih`) wrappers. The universal plugin factory creates any SOTF plugin from a type string and JSON configuration, allowing format-specific wrappers to remain thin.

## Features

- Universal plugin factory: type string + JSON config -> `Box<dyn Plugin>`
- 30+ supported plugin types (EQ, Compressor, Limiter, Upmixer, Binaural, etc.)
- Case-insensitive plugin type matching
- ParamBridge for mapping ParamSpec parameters to format-specific systems
- Buffer interleave/deinterleave utilities
- State serialization helpers

## Usage

```rust
use plugins_bridge::create_plugin;

// Create a plugin with default parameters
let mut plugin = create_plugin("EQ", 2, 48000, "{}").unwrap();
plugin.initialize(48000).unwrap();

// Create with specific configuration
let config = r#"{"filters": [{"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": 3.0}]}"#;
let mut eq = create_plugin("EQ", 2, 48000, config).unwrap();
```

## Architecture

```
factory.rs      -- create_plugin() universal factory + available_plugin_types()
param_bridge.rs -- ParamBridge: ParamSpec <-> format-specific parameter mapping
buffers.rs      -- Buffer interleave/deinterleave
state.rs        -- Plugin state serialization
```

## Ownership and lifetime

`plugins-bridge` is a safe Rust adapter. `create_plugin()` returns a
`Box<dyn Plugin>` that owns the plugin instance. Format wrappers (AU via
`plugins-ffi`, VST3/CLAP via `plugins-nih`) hold that box inside a
`PluginHandle` or equivalent wrapper and release it on teardown. FFI callers
never receive raw `Plugin` pointers; all Rust-side references are scoped to the
wrapper lifetime.

## Testing

```bash
cargo test -p plugins-bridge
```

## License

Part of the SOTF (Sound of the Future) project.
