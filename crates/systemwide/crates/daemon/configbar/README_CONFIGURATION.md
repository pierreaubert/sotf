# ConfigBar - HAL Audio Configuration

The Swift menubar app provides an easy-to-use GUI for configuring the HAL driver and audio routing.

## Features

### HAL Input Configuration

Configure how many channels to capture from macOS applications:

- **Input Channels**: 1-32 channels (default: 2 for stereo)
- Common settings:
  - 1 = Mono
  - 2 = Stereo (most common)
  - 4 = Quadraphonic
  - 6 = 5.1 surround
  - 8 = 7.1 surround

### Audio Output Configuration

Configure where and how audio is sent to speakers:

- **Output Device**: Select from available audio devices
- **Output Channels**: 1-32 channels (default: 2 for stereo)
- **Volume Control**: Adjust playback volume (0-100%)

Common output configurations:
- 2 channels = Stereo speakers/headphones
- 5 channels = 5.0 surround (no subwoofer)
- 6 channels = 5.1 surround (with subwoofer)

### Volume Control

Real-time volume adjustment with visual feedback (0-100%).

## Usage

### Opening the Configuration Window

1. Start the daemon: `cargo run --release --bin sotf_daemon`
2. Launch the menubar app: `/Applications/AutoEQ.app`
3. Click the speaker icon in the menu bar

### Configuring HAL Channels

1. Open the configuration window
2. Under "HAL Input", select the number of input channels
3. Under "Audio Output", select the output device and channel count
4. Changes are applied immediately

The status bar shows the current configuration: `HAL: 2ch in → 2ch out`

### Example Configurations

#### Scenario 1: Stereo Passthrough (Default)
- **HAL Input**: 2 channels
- **Output**: 2 channels
- **Use case**: Standard stereo audio from apps → stereo speakers

#### Scenario 2: Stereo to 5.0 Upmix
- **HAL Input**: 2 channels
- **Output**: 5 channels
- **Use case**: Convert stereo audio to surround sound
- **Additional**: Add `upmixer` plugin for spatial processing

#### Scenario 3: 5.1 Surround Passthrough
- **HAL Input**: 6 channels
- **Output**: 6 channels
- **Use case**: Apps producing 5.1 audio → 5.1 speakers

#### Scenario 4: Downmix to Headphones
- **HAL Input**: 6 channels
- **Output**: 2 channels
- **Use case**: Convert 5.1 content to stereo for headphones
- **Additional**: Add `matrix` plugin for proper downmixing

## Audio Flow

```
┌─────────────────┐
│  macOS App      │ (Safari, Spotify, etc.)
│  Outputs Audio  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  HAL Driver     │ Virtual Audio Device
│  Buffer         │ configured_input_channels
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  hal_input      │ Plugin reads configured_input_channels
│  plugin         │ (Source plugin - generates audio)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Plugin Chain   │ (Optional: EQ, Upmixer, etc.)
│  (if any)       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  hal_output     │ Plugin writes configured_output_channels
│  plugin         │ (Loopback - optional monitoring)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Physical       │ Speakers/Headphones
│  Audio Device   │ configured_output_channels
└─────────────────┘
```

### Technical Implementation

The audio engine supports two modes:

**File Playback Mode** (normal operation):
- Decoder thread reads and decodes audio files
- Processing thread applies plugin chain
- Playback thread outputs to hardware

**HAL Playback Mode** (for live audio routing):
- Decoder thread sends empty frames (silent source mode)
- HAL input plugin reads from HAL buffer and generates audio
- Processing thread applies plugin chain
- Playback thread outputs to hardware

The HAL input plugin is a **source plugin** (0 input channels → N output channels) that doesn't need a file. When `input_channels: 0` is set in the engine config, the daemon automatically starts silent source mode, where empty frames trigger the HAL input plugin to read from the HAL driver buffer.

## JSON API

The menubar app communicates with the daemon via Unix socket. When you change the channel configuration, it sends:

```json
{
  "command": "load_plugins",
  "plugins": [
    {
      "plugin_type": "hal_input",
      "parameters": {
        "channels": 2
      }
    },
    {
      "plugin_type": "hal_output",
      "parameters": {
        "channels": 2
      }
    }
  ]
}
```

You can extend this with additional plugins:

```json
{
  "command": "load_plugins",
  "plugins": [
    {
      "plugin_type": "hal_input",
      "parameters": {"channels": 2}
    },
    {
      "plugin_type": "eq",
      "parameters": {
        "filters": [
          {"filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0}
        ]
      }
    },
    {
      "plugin_type": "upmixer",
      "parameters": {"mode": "stereo_to_5_0"}
    },
    {
      "plugin_type": "hal_output",
      "parameters": {"channels": 5}
    }
  ]
}
```

## Building the App

```bash
cd src-configbar
./scripts/build.sh
```

This creates `/Applications/AutoEQ.app`

## Installation

```bash
cd src-configbar
./scripts/install-all.sh
```

This installs:
1. The menubar app to `/Applications/`
2. Launch agent to start on login
3. Configures permissions

## Troubleshooting

### "No devices found"

The daemon is not running. Start it:
```bash
cargo run --release --bin sotf_daemon
```

### "Failed to load plugin chain"

Check the daemon logs. Common issues:
- Invalid channel count (must be 1-32)
- HAL not initialized (daemon should auto-initialize)
- Plugin configuration error

### Changes not applying

Restart the daemon:
```bash
# Stop daemon
pkill sotf_daemon

# Start daemon
cargo run --release --bin sotf_daemon
```

## Advanced Usage

### Custom Plugin Chains

Use the "Edit Plugins" button to create complex processing chains with:
- Parametric EQ
- Compressor
- Limiter
- Noise gate
- Crossover
- Matrix mixer
- Upmixer
- Loudness compensation

### Saving Configurations

Use "Save Configuration..." to export your plugin chain as JSON.
Use "Load Configuration..." to restore a saved configuration.

## System Requirements

- macOS 12.0+ (Monterey or later)
- Swift 5.9+
- Running sotf_daemon with HAL initialized

## See Also

- [Daemon Documentation](../../src-audio/bin/README_DAEMON.md)
- [HAL Driver Documentation](../../src-hal/README.md)
- [Plugin Documentation](../../src-audio/src/plugins/README.md)
