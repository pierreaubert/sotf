# sotf-audio-player-midi

MIDI device management and control for SOTF audio players with pre-configured profiles for professional audio hardware.

## Features

- **Device Enumeration**: List all available MIDI input and output devices
- **MIDI I/O**: Send and receive MIDI messages with a clean, type-safe API
- **Device Profiles**: Configure and save device-specific settings and control mappings
- **Message Parsing**: Automatic MIDI message parsing with support for all standard message types
- **Configuration Management**: JSON-based configuration with profile support
- **Hardware Profiles**: Pre-configured control for RME TotalMix FX, Genelec GLM, Xone K2/K3, and Launch Control XL

## Supported Hardware

### Audio Interfaces & DSP

#### RME UFX+ / TotalMix FX
- **Full fader control**: 192 faders across 3 rows (Input/Playback/Output)
- **CC-based control**: CC 102-117 for 16 faders per bank
- **Mackie Control Protocol**: Mute, solo, pan controls
- **Snapshot recall**: Program change messages
- **Main output volume**: Standard CC 7

#### Genelec GLM (SAM Monitors)
- **System volume control**: Percentage or MIDI value
- **Mute/Dim/Solo**: System-wide functions
- **Individual monitor control**: Solo/mute specific monitors by MIDI ID (GLM 5.0+)
- **Monitor groups**: Switch between configured groups
- **Volume presets**: Recall saved volume levels
- **Bass management**: Toggle subwoofer integration
- **System power**: Power on/off (GLM 5.0+)

### MIDI Controllers

#### Allen & Heath Xone:K2/K3
- **52 physical controls** across 3 layers
- **12 rotary pots**: Analog potentiometers
- **6 endless encoders**: With push switches
- **4 linear faders**: 0-127 range
- **30 backlit buttons**: RGB LED feedback
- **Fixed CC assignments**: Hardware-defined mapping

#### Novation Launch Control XL
- **24 knobs**: 3 rows × 8 columns
- **8 faders**: Independent channel control
- **24 RGB buttons**: Track focus and control
- **16 templates**: 8 user, 8 factory presets
- **Template-based CCs**: MIDI channel = template number
- **LED control via SysEx**: Full color feedback

## Quick Start

### Basic MIDI I/O

```rust
use sotf_audio_player_midi::{MidiManager, MidiMessage};

let mut manager = MidiManager::new()?;

// List devices
let input_devices = manager.list_input_devices()?;
let output_devices = manager.list_output_devices()?;

// Connect and send
manager.connect_output(0)?;
manager.send_message(&MidiMessage::NoteOn {
    channel: 0,
    note: 60,
    velocity: 100,
})?;
```

### Control RME TotalMix FX

```rust
use sotf_audio_player_midi::profiles::{TotalMixControl, TotalMixRow};
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;
manager.connect_output(0)?;

let totalmix = TotalMixControl::new(&mut manager)?;

// Set main volume
totalmix.set_main_volume(100)?;

// Control output faders
totalmix.set_fader(TotalMixRow::Output, 0, 0, 95)?;

// Mute/solo channels
totalmix.mute_channel(TotalMixRow::Input, 0, 4)?;
totalmix.solo_channel(TotalMixRow::Output, 0, 0)?;

// Set pan (0=left, 64=center, 127=right)
totalmix.set_pan(TotalMixRow::Output, 0, 0, 64)?;

// Recall snapshot
totalmix.recall_snapshot(5)?;
```

### Control Genelec GLM

```rust
use sotf_audio_player_midi::profiles::GLMControl;
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;
manager.connect_output(0)?;

let glm = GLMControl::new(&mut manager);

// Set volume as percentage
glm.set_volume_percent(75.0)?;

// Mute/Dim/Solo
glm.mute(true)?;
glm.dim(true)?;  // -20dB
glm.solo(true)?;

// Select monitor group
glm.select_monitor_group(2)?;

// Solo specific monitor by MIDI ID
glm.solo_monitor(1)?;

// Recall volume preset
glm.recall_volume_preset(3)?;
```

### Use Control Surfaces

```rust
use sotf_audio_player_midi::profiles::{XoneK2Profile, K2Control};
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;

manager.connect_input(0, |msg| {
    if let Some((control, value)) = XoneK2Profile::identify_control(&msg) {
        match control {
            K2Control::RotaryPot(n) => {
                println!("Pot {}: {}", n, value);
            }
            K2Control::Fader(n) => {
                println!("Fader {}: {}", n, value);
            }
            K2Control::Encoder(n) => {
                println!("Encoder {}: {}", n, value);
            }
            _ => {}
        }
    }
})?;
```

```rust
use sotf_audio_player_midi::profiles::{LaunchControlXLProfile, LCXLTemplate};
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;
let template = LCXLTemplate::factory_1();

manager.connect_input(0, move |msg| {
    if let Some(control) = LaunchControlXLProfile::identify_control(&msg, &template) {
        println!("Control: {}", control);
    }
})?;
```

## Hardware-Specific Details

### RME TotalMix FX Implementation

**Matrix Layout:**
- **Input Row**: MIDI channels 1-4 (banks 0-3), up to 64 faders
- **Playback Row**: MIDI channels 5-8 (banks 0-3), up to 64 faders
- **Output Row**: MIDI channels 9-12 (banks 0-3), up to 64 faders

**CC Assignments:**
- **Faders**: CC 102-117 (16 faders per bank)
- **Main Volume**: CC 7 on channel 1

**Mackie Control Protocol:**
- **Mute**: Notes 16-23 (channels 0-7)
- **Solo**: Notes 8-15 (channels 0-7)
- **Pan**: CC 16-23 (channels 0-7)

### Genelec GLM Configuration

**MIDI Setup Required:**
1. Open GLM software
2. Go to Settings > MIDI Remote
3. Select MIDI input device
4. Configure CC assignments for each function

**Default CC Assignments** (verify in GLM):
- Volume: CC 7
- Mute: CC 102
- Dim: CC 103
- Solo: CC 104
- Monitor Group: CC 105
- Volume Preset: CC 106
- Bass Management: CC 107
- System Power: CC 108 (GLM 5.0+)
- Solo/Mute Device: CC 109 (value = MIDI ID)

**Limitations:**
- SPL monitoring data is **not available via MIDI** (display only)
- No official API for programmatic control
- Proprietary network protocol

### Xone K2/K3 Layout

**Layer 1 CC Assignments:**
- **Rotary Pots**: CC 0-11 (top to bottom, left to right)
- **Encoders**: CC 12-17
- **Faders**: CC 44-47

**Button Note Assignments:**
- **Track Focus**: Notes 24-31 (top row)
- **Track Control**: Notes 32-39 (bottom row)
- **Function Buttons**: Notes 40-43
- **Encoder Switches**: Notes 48-53

**Important:** CC numbers are **FIXED** in hardware. You can only change MIDI channel.

### Launch Control XL Templates

**Factory Template 1 (Ableton Live):**
- **Top Row Knobs**: CC 13-20
- **Middle Row Knobs**: CC 29-36
- **Bottom Row Knobs**: CC 49-56
- **Faders**: CC 77-84

**Button Layout:**
- **Track Focus**: Notes 41-44, 57-60
- **Track Control**: Notes 73-76, 89-92
- **Function Buttons**: Notes 105-108

**LED Control:**
Use SysEx messages for RGB LED feedback:
```rust
let sysex = LaunchControlXLProfile::set_button_led(
    41,  // button note
    15,  // color (15 = red full)
    0    // template
);
manager.send_raw(&sysex)?;
```

## Examples

The crate includes comprehensive examples:

```bash
# Basic MIDI operations
cargo run --example list_devices
cargo run --example midi_monitor
cargo run --example send_notes
cargo run --example device_profile

# Hardware control
cargo run --example totalmix_control    # Interactive TotalMix FX control
cargo run --example glm_control          # Interactive GLM control
cargo run --example studio_integration   # Full studio setup demo
```

## Advanced Usage

### Device Profiles with Custom Mappings

```rust
use sotf_audio_player_midi::{DeviceConfig, DeviceProfile, MidiConfig, MidiManager};

let mut profile = DeviceProfile::new("My Setup".to_string());
profile.device_config = DeviceConfig::new()
    .with_manufacturer("RME".to_string())
    .with_model("UFX+".to_string())
    .with_channel(0);

// Add control mappings
profile.add_mapping(7, "main_volume".to_string());
profile.add_mapping(102, "channel_1".to_string());

// Add initialization sequence
profile.add_init_message(vec![0xB0, 0x00, 0x00]);

// Save configuration
let mut config = MidiConfig::default();
config.add_profile("studio".to_string(), profile);
config.save("studio_config.json")?;
```

### Multi-Device Integration

```rust
use sotf_audio_player_midi::MidiManager;
use std::sync::{Arc, Mutex};

let mut manager = MidiManager::new()?;

// Connect to TotalMix
manager.connect_output(0)?;
let totalmix = TotalMixControl::new(&mut manager)?;

// Connect to GLM
manager.connect_output(1)?;
let glm = GLMControl::new(&mut manager);

// Use control surface to control both
let volume = Arc::new(Mutex::new(100u8));
manager.connect_input(2, move |msg| {
    // Map K2 encoder to GLM volume
    if let Some((K2Control::Encoder(0), value)) = XoneK2Profile::identify_control(&msg) {
        *volume.lock().unwrap() = value;
        // Send to GLM via separate manager instance
    }
})?;
```

## Architecture

### Module Structure

- `manager`: Main `MidiManager` for device connections and message I/O
- `message`: MIDI message types and encoding/decoding
- `device`: Device information and enumeration
- `config`: Configuration management and device profiles
- `error`: Error types and result handling
- `profiles`: Pre-configured hardware profiles
  - `rme_totalmix`: RME UFX+ / TotalMix FX control
  - `genelec_glm`: Genelec GLM control
  - `xone_k2`: Allen & Heath Xone:K2/K3 controller
  - `launch_control_xl`: Novation Launch Control XL controller

### Thread Safety

The `MidiManager` uses thread-safe primitives:
- Input callbacks run on dedicated MIDI thread
- Output connections protected by `Mutex`
- Configuration safely accessed from multiple threads

## Dependencies

- `midir`: Cross-platform MIDI I/O
- `serde`/`serde_json`: Configuration serialization
- `parking_lot`: Thread-safe data structures
- `dirs`: Platform-specific configuration directories
- `chrono`: Timestamps for examples

## MIDI Message Types

Supports all standard MIDI message types:

- **Note On/Off**: `NoteOn`, `NoteOff`
- **Control Change**: `ControlChange`
- **Program Change**: `ProgramChange`
- **Pitch Bend**: `PitchBend`
- **Aftertouch**: `PolyphonicAftertouch`, `ChannelAftertouch`
- **System Exclusive**: `SystemExclusive`
- **Raw Messages**: `Raw` for unsupported or custom messages

## License

GPL-3.0-or-later
