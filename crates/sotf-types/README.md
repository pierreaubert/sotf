# sotf-types (lib: `sotf_types`)

Shared configuration types for the SOTF audio system.

## Overview

Provides lightweight, serializable types used across the SOTF workspace without pulling in audio processing dependencies (cpal, symphonia, rustfft). Any crate in the workspace can depend on `sotf-types` without bloating its dependency tree.

## Features

- `EngineConfig` — Complete audio engine configuration with JSON persistence and version migration
- `PluginConfig` — Plugin type + JSON parameters for the plugin chain
- `PluginGraphConfig` — DAG-based plugin routing configuration
- `AudioSource` — Audio source abstraction (file, URL, stream)
- `SinkType` / `SinkConfig` — Output sink configuration
- `PlaybackState` / `AudioEngineState` — Runtime state types

## Usage

```rust
use sotf_types::{EngineConfig, PluginConfig};

// Create a default engine config
let mut config = EngineConfig::default();
config.output_sample_rate = 96000;
config.output_channels = 5;

// Add plugins to the chain
config.plugins.push(PluginConfig {
    plugin_type: "EQ".to_string(),
    parameters: serde_json::json!({
        "filters": [
            {"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": 3.0}
        ]
    }),
});

// Save to disk
config.save_to_file(&"config.json".into()).unwrap();
```

## Architecture

```
src/
├── audio_source.rs   -- AudioSource, ServiceId
├── config.rs         -- EngineConfig (the central configuration type)
├── plugin_config.rs  -- PluginConfig, PluginGraphConfig
├── sink.rs           -- SinkConfig, SinkType, SinkOpenResult
└── state.rs          -- AudioEngineState, AudioFrame, PlaybackState
```

## Dependencies

- `serde` / `serde_json` — Serialization
- `log` — Logging

## Testing

```bash
cargo test -p sotf-types
cargo check -p sotf-types && cargo clippy -p sotf-types
```

## License

See the root workspace `LICENSE` file.
