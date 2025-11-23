# sotf-audio-player-midi

MIDI device management and control for SOTF audio players.

## Features

- **Device Enumeration**: List all available MIDI input and output devices
- **MIDI I/O**: Send and receive MIDI messages with a clean, type-safe API
- **Device Profiles**: Configure and save device-specific settings and control mappings
- **Message Parsing**: Automatic MIDI message parsing with support for all standard message types
- **Configuration Management**: JSON-based configuration with profile support

## Usage

### List MIDI Devices

```rust
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;

let input_devices = manager.list_input_devices()?;
let output_devices = manager.list_output_devices()?;

for device in input_devices {
    println!("Input: [{}] {}", device.index, device.name);
}

for device in output_devices {
    println!("Output: [{}] {}", device.index, device.name);
}
```

### Receive MIDI Messages

```rust
use sotf_audio_player_midi::MidiManager;

let mut manager = MidiManager::new()?;

manager.connect_input(0, |message| {
    println!("Received: {}", message.description());
})?;

// Keep running to receive messages
loop {
    std::thread::sleep(std::time::Duration::from_millis(100));
}
```

### Send MIDI Messages

```rust
use sotf_audio_player_midi::{MidiManager, MidiMessage};

let mut manager = MidiManager::new()?;
manager.connect_output(0)?;

// Send a note on message
manager.send_message(&MidiMessage::NoteOn {
    channel: 0,
    note: 60,
    velocity: 100,
})?;

// Send a control change
manager.send_message(&MidiMessage::ControlChange {
    channel: 0,
    controller: 7,  // Volume
    value: 127,
})?;
```

### Device Profiles

Create reusable device profiles with control mappings and initialization sequences:

```rust
use sotf_audio_player_midi::{DeviceConfig, DeviceProfile, MidiConfig, MidiManager};

// Create a device profile
let mut profile = DeviceProfile::new("My Controller".to_string());
profile.device_config = DeviceConfig::new()
    .with_manufacturer("ACME".to_string())
    .with_model("MK-1000".to_string())
    .with_channel(0);

// Add control mappings
profile.add_mapping(1, "modulation".to_string());
profile.add_mapping(7, "volume".to_string());
profile.add_mapping(10, "pan".to_string());

// Add initialization messages
profile.add_init_message(vec![0xB0, 0x00, 0x00]); // Bank select

// Create config and add profile
let mut config = MidiConfig::default();
config.add_profile("my_controller".to_string(), profile);
config.set_active_profile("my_controller".to_string());

// Save configuration
config.save("midi_config.json")?;

// Use with manager
let mut manager = MidiManager::with_config(config)?;
manager.connect_output(0)?;
manager.send_init_messages()?;
```

## MIDI Message Types

The crate supports all standard MIDI message types:

- **Note On/Off**: `NoteOn`, `NoteOff`
- **Control Change**: `ControlChange`
- **Program Change**: `ProgramChange`
- **Pitch Bend**: `PitchBend`
- **Aftertouch**: `PolyphonicAftertouch`, `ChannelAftertouch`
- **System Exclusive**: `SystemExclusive`
- **Raw Messages**: `Raw` for unsupported or custom messages

## Examples

The crate includes several examples demonstrating different use cases:

```bash
# List all MIDI devices
cargo run --example list_devices

# Monitor MIDI input
cargo run --example midi_monitor

# Send MIDI notes
cargo run --example send_notes

# Device profile management
cargo run --example device_profile
```

## Dependencies

- `midir`: Cross-platform MIDI I/O
- `serde`/`serde_json`: Configuration serialization
- `parking_lot`: Thread-safe data structures
- `dirs`: Platform-specific configuration directories

## Architecture

The crate is organized into several modules:

- `manager`: Main `MidiManager` for device connections and message I/O
- `message`: MIDI message types and encoding/decoding
- `device`: Device information and enumeration
- `config`: Configuration management and device profiles
- `error`: Error types and result handling

## Thread Safety

The `MidiManager` uses thread-safe primitives for concurrent access:
- Input callbacks run on a dedicated MIDI thread
- Output connections are protected by `Mutex` for thread-safe sending
- Configuration can be safely accessed from multiple threads

## License

GPL-3.0-or-later
