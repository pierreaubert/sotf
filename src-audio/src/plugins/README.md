# Audio Plugin System

A flexible plugin system for audio processing in the AutoEQ audio pipeline. Plugins can be chained together in a host, with each plugin processing N input channels and producing P output channels.

## Architecture

The plugin system consists of several key components:

### Core Traits

- **`Plugin`**: The main trait for audio processing plugins that can transform N input channels to P output channels
- **`InPlacePlugin`**: A simpler trait for plugins that process audio in-place (same number of input and output channels)
- **`InPlacePluginAdapter`**: Adapter to convert `InPlacePlugin` implementations to the `Plugin` trait

### Plugin Host

- **`PluginHost`**: Chains multiple plugins together, routing audio through them sequentially
- **`SharedPluginHost`**: Thread-safe wrapper for `PluginHost` using `Arc<Mutex<>>`

### Parameter System

- **`Parameter`**: Defines a plugin parameter with metadata (name, description, min/max values)
- **`ParameterValue`**: Type-safe parameter values (Float, Int, Bool)
- **`ParameterId`**: Unique identifier for parameters
- **`ParameterStore`**: Manages parameter values

### Example Plugins

- **`GainPlugin`**: Simple gain/volume control plugin (example implementation)
- **`UpmixerPlugin`**: Stereo to 5.0 surround upmixer using FFT-based Direct/Ambient decomposition

## Integration with Audio Pipeline

The plugin host integrates into the `AudioStreamingManager` pipeline:

```
Audio Decoder → Plugin Host → CamillaDSP → Audio Output
```

The plugin processing happens **before** the audio is sent to CamillaDSP, allowing you to apply custom DSP effects.

## Usage Examples

### Basic Plugin Usage

```rust
use sotf_audio::{GainPlugin, InPlacePluginAdapter, PluginHost};

// Create a plugin host for 2-channel audio at 44.1kHz
let mut host = PluginHost::new(2, 44100);

// Create a gain plugin and wrap it in an adapter
let gain_plugin = GainPlugin::new(2, -6.0); // -6dB attenuation
let adapter = InPlacePluginAdapter::new(gain_plugin);

// Add to the host
host.add_plugin(Box::new(adapter)).unwrap();

// Process audio
let input = vec![1.0, 1.0, 1.0, 1.0]; // 2 frames, 2 channels
let mut output = vec![0.0; 4];
host.process(&input, &mut output).unwrap();
```

### Chaining Multiple Plugins

```rust
let mut host = PluginHost::new(2, 44100);

// Add multiple plugins - they process in order
let gain1 = GainPlugin::new(2, -6.0);
host.add_plugin(Box::new(InPlacePluginAdapter::new(gain1))).unwrap();

let gain2 = GainPlugin::new(2, -3.0);
host.add_plugin(Box::new(InPlacePluginAdapter::new(gain2))).unwrap();

// Audio flows through gain1 first, then gain2
```

### Using with AudioStreamingManager

```rust
use sotf_audio::{AudioStreamingManager, GainPlugin, InPlacePluginAdapter};

let mut manager = AudioStreamingManager::new(camilla_binary_path);

// Load audio file
manager.load_file("audio.flac").await.unwrap();

// Enable plugin host
manager.enable_plugin_host().unwrap();

// Add plugins via the with_plugin_host closure
manager.with_plugin_host(|host| {
    let gain = GainPlugin::new(2, -3.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
}).unwrap().unwrap();

// Start playback - audio will flow through plugins
manager.start_playback(None, vec![], channel_map_mode, None, None).await.unwrap();
```

### Dynamic Parameter Changes

```rust
use sotf_audio::{GainPlugin, ParameterId, ParameterValue, InPlacePlugin};

let mut plugin = GainPlugin::new(2, 0.0);

// Change gain dynamically
let gain_id = ParameterId::from("gain_db");
plugin.set_parameter(gain_id, ParameterValue::Float(-12.0)).unwrap();

// Query parameters
let params = plugin.parameters();
for param in params {
    println!("{}: {:?}", param.name, param.default_value);
}
```

## Creating Custom Plugins

### Implementing InPlacePlugin (Simpler)

For plugins that don't change the channel count:

```rust
use sotf_audio::{InPlacePlugin, PluginInfo, ProcessContext, Parameter,
                 ParameterId, ParameterValue};

pub struct MyPlugin {
    channels: usize,
    sample_rate: u32,
}

impl InPlacePlugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "My Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
            description: "Does something cool".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("param1", "Parameter 1", 0.0, -10.0, 10.0)
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue)
        -> Result<(), String>
    {
        // Handle parameter changes
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Return parameter values
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> Result<(), String> {
        self.sample_rate = sample_rate;
        Ok(())
    }

    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)
        -> Result<(), String>
    {
        // Process audio in-place
        // buffer is interleaved: [L0, R0, L1, R1, ...]
        for sample in buffer.iter_mut() {
            *sample *= 0.5; // Example: attenuate by 50%
        }
        Ok(())
    }
}
```

### Implementing Plugin (More Flexible)

For plugins that change channel count (e.g., stereo to mono, upmixers):

```rust
use sotf_audio::{Plugin, PluginInfo, ProcessContext, Parameter,
                 ParameterId, ParameterValue};

pub struct StereoToMonoPlugin;

impl Plugin for StereoToMonoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Stereo to Mono".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
            description: "Converts stereo to mono by averaging".to_string(),
        }
    }

    fn input_channels(&self) -> usize { 2 }
    fn output_channels(&self) -> usize { 1 }

    fn parameters(&self) -> Vec<Parameter> { vec![] }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue)
        -> Result<(), String>
    {
        Err("No parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(&mut self, input: &[f32], output: &mut [f32],
               context: &ProcessContext) -> Result<(), String>
    {
        // Input: [L0, R0, L1, R1, ...]
        // Output: [M0, M1, ...]
        for frame in 0..context.num_frames {
            let left = input[frame * 2];
            let right = input[frame * 2 + 1];
            output[frame] = (left + right) * 0.5;
        }
        Ok(())
    }
}
```

## Audio Format

All audio data is in **interleaved f32** format:
- Stereo: `[L0, R0, L1, R1, L2, R2, ...]`
- 5.1 surround: `[FL0, FR0, C0, LFE0, BL0, BR0, FL1, FR1, ...]`

Each sample is a 32-bit float, typically in the range -1.0 to +1.0.

## Performance Considerations

- **Buffer Allocation**: The plugin host allocates intermediate buffers when processing. Buffers are reused across calls if the frame count doesn't change.
- **In-Place Processing**: Use `InPlacePlugin` when possible - it's more efficient as it copies less data.
- **Thread Safety**: `SharedPluginHost` provides thread-safe access via `Arc<Mutex<>>`.
- **Zero-Copy**: The last plugin in the chain writes directly to the output buffer (no extra copy).

## Testing

Run tests:
```bash
cargo test --package sotf_audio
```

Integration tests are in `tests/test_plugins.rs`.
Unit tests are in each module (`plugins/gain.rs`, etc.).

## Thread Safety

The plugin host is integrated into the audio decoder thread in `AudioStreamingManager`. Plugins are called from this dedicated audio thread, so they don't need to be thread-safe internally. However, parameter changes from the UI thread are protected by mutexes in the `AudioStreamingManager` API.

## Future Enhancements

Potential future additions:
- VST3 wrapper for loading third-party plugins
- SIMD optimizations for processing loops
- Latency compensation when chaining plugins
- MIDI parameter automation
