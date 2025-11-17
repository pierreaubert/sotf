# Binaural Decoder Plugin

The binaural decoder plugin converts multi-channel audio (e.g., 5.0, 5.1 surround) to binaural stereo using Head-Related Transfer Functions (HRTFs) from SOFA files.

## Features

- **SOFA File Support**: Reads HRTFs from SOFA (Spatially Oriented Format for Acoustics) files
- **Multi-channel Input**: Supports stereo, 5.0, 5.1, and custom channel configurations
- **FFT-based Convolution**: Efficient overlap-add convolution for real-time processing
- **Standard Speaker Layouts**: Built-in mappings for common surround sound formats

## Dependencies

The binaural decoder requires the following system libraries:
- **NetCDF** (libnetcdf-dev)
- **HDF5** (libhdf5-dev)

On Ubuntu/Debian:
```bash
sudo apt-get install libnetcdf-dev libhdf5-dev
```

## SOFA Files

### Where to Get SOFA Files

1. **SADIE II Database** - University of York
   - URL: https://www.york.ac.uk/sadie-project/database.html
   - Includes KU 100 dummy head measurements
   - High-quality HRTFs for spatial audio

2. **KEMAR Database** - MIT Media Lab
   - URL: https://sound.media.mit.edu/resources/KEMAR.html
   - Industry-standard KEMAR dummy head
   - Various measurement sets available

3. **SOFA Files Repository**
   - URL: https://www.sofaconventions.org/mediawiki/index.php/Files
   - Collection of publicly available SOFA files
   - Multiple measurement conventions

4. **ARI HRTF Database** - Acoustics Research Institute
   - URL: https://www.kfs.oeaw.ac.at/index.php?view=article&id=608
   - Extensive individual HRTF measurements

### Recommended Test Files

For initial testing, we recommend:

1. **SADIE II KU 100** (`SADIE_D1_HRIR_48k.sofa`)
   - Format: SimpleFreeFieldHRIR
   - Sample Rate: 48 kHz
   - Measurements: 2818 positions
   - Download: https://www.york.ac.uk/sadie-project/database.html

2. **KEMAR** (`KEMAR_HRIR.sofa`)
   - Format: SimpleFreeFieldHRIR
   - Sample Rate: 44.1 kHz
   - Standard industry reference

## Usage

### Via JSON Configuration

```json
{
  "plugin_type": "binaural_decoder",
  "parameters": {
    "sofa_file": "/path/to/hrtf.sofa",
    "input_channels": 5,
    "fft_size": 4096
  }
}
```

### Via Rust API

```rust
use sotf_audio::plugins::{BinauralDecoderPlugin, BinauralDecoderParams};
use std::path::PathBuf;

// Create plugin parameters
let params = BinauralDecoderParams {
    sofa_file: "/path/to/SADIE_D1_HRIR_48k.sofa".to_string(),
    input_channels: 5,
    fft_size: 4096,
};

// Create plugin
let mut plugin = BinauralDecoderPlugin::from_params(params);

// Initialize with sample rate
plugin.initialize(48000)?;

// Process audio (5 channels -> 2 channels)
let input = vec![0.0f32; 1024 * 5];  // 1024 frames, 5 channels
let mut output = vec![0.0f32; 1024 * 2];  // 1024 frames, stereo

let context = ProcessContext {
    sample_rate: 48000,
    num_frames: 1024,
};

plugin.process(&input, &mut output, &context)?;
```

### Example: 5.0 to Binaural

```rust
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use serde_json::json;

let config = EngineConfig {
    frame_size: 512,
    buffer_ms: 50,
    output_sample_rate: 48000,
    input_channels: 5,  // 5.0 surround
    output_channels: 2, // Binaural stereo
    plugins: vec![
        PluginConfig {
            plugin_type: "binaural_decoder".to_string(),
            parameters: json!({
                "sofa_file": "/path/to/SADIE_D1_HRIR_48k.sofa",
                "input_channels": 5,
                "fft_size": 4096
            }),
        }
    ],
    volume: 1.0,
    muted: false,
    config_path: None,
    watch_config: false,
};

let mut engine = AudioEngine::new(config)?;
engine.play("/path/to/5ch_audio.flac")?;
```

## Speaker Layouts

The plugin includes standard speaker position mappings:

### 2.0 Stereo
- **L**: 30° (front left)
- **R**: -30° (front right)

### 5.0 Surround
- **FL**: 30° (front left)
- **FR**: -30° (front right)
- **C**: 0° (center)
- **LS**: 110° (left surround)
- **RS**: -110° (right surround)

### 5.1 Surround
Same as 5.0, plus:
- **LFE**: 0°, -90° elevation (subwoofer, passed through to both ears)

### Custom Layouts

For non-standard channel counts, the plugin automatically arranges channels in a circular pattern around the listener.

## How It Works

1. **HRTF Loading**: On initialization, the plugin loads the SOFA file and extracts HRTFs for each speaker position
2. **Nearest Neighbor**: For each speaker, the plugin finds the nearest measured HRTF in the SOFA dataset
3. **FFT Convolution**: Each input channel is convolved with its corresponding left/right ear HRTFs using overlap-add FFT
4. **Summation**: All convolved channels are summed to produce binaural stereo output

## Performance

- **FFT Size**: Default 4096 samples (85ms latency at 48kHz)
  - Larger FFT = better frequency resolution, higher latency
  - Smaller FFT = lower latency, more CPU usage
- **CPU Usage**: Approximately 10-15% per channel on modern CPUs (48kHz, 4096 FFT)
- **Memory**: ~50 MB for typical SOFA files with 2000+ measurements

## Troubleshooting

### "Failed to load SOFA file"
- Ensure the SOFA file path is correct and accessible
- Verify the file is in SimpleFreeFieldHRIR convention
- Check that NetCDF libraries are installed

### "No HRTF found for speaker"
- The SOFA file may not cover the full sphere
- Try a different SOFA database with more measurements
- Check speaker position angles are reasonable (-180° to 180° azimuth)

### High latency
- Reduce `fft_size` to 2048 or 1024
- Trade-off: smaller FFT increases CPU usage
- Latency = fft_size / sample_rate seconds

### Clicks or artifacts
- Increase `fft_size` to 8192 for smoother transitions
- Check that SOFA sample rate matches engine sample rate
- Ensure input audio doesn't have DC offset

## Testing

To test the binaural decoder with sample SOFA files:

```bash
# Download SADIE II KU 100 SOFA file
wget https://www.york.ac.uk/media/sadie-project/SADIE_D1_HRIR_48k.sofa

# Run audio playback with binaural decoder
cargo run --bin sotf_player --release -- \
    --input 5ch_audio.flac \
    --sofa SADIE_D1_HRIR_48k.sofa \
    --plugin binaural_decoder
```

## References

- AES69-2015: AES standard for file exchange - Spatial acoustic data file format
- SOFA Conventions: https://www.sofaconventions.org/
- Gardner & Martin (1995): "HRTF Measurements of a KEMAR"
- Algazi et al. (2001): "The CIPIC HRTF Database"

## License

This implementation is part of the SOTF audio system and follows the project's GPL-3.0-or-later license.
