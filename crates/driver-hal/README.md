# AutoEQ HAL Driver - Simplified Virtual Audio Device

A minimal macOS Core Audio HAL (Hardware Abstraction Layer) driver that creates a virtual audio device with bidirectional audio buffers.

## Overview

This is a **simplified HAL driver** designed to work with the audio player in `../src-audio`. The driver:

- Creates a virtual audio device that appears in macOS Sound preferences
- Provides bidirectional lock-free audio buffers
- Forwards audio data between macOS apps and the audio player
- Supports loopback functionality
- **Does NOT** handle audio processing (all processing done by audio player)

## Architecture

### Data Flow

**Input Path** (macOS → Audio Player):
```
macOS App → Virtual HAL Device → Input Buffer → Audio Player reads
```

**Output Path** (Audio Player → macOS, loopback):
```
Audio Player writes → Output Buffer → Virtual HAL Device → macOS App
```

### Key Components

1. **HAL Driver** (`src/hal_driver.rs`):
   - Creates virtual Core Audio device
   - Implements Core Audio HAL protocol
   - Provides I/O callback interface
   - Minimal configuration, all defaults to 48kHz stereo

2. **Audio Buffer** (`src/audio_buffer.rs`):
   - Bidirectional lock-free channels using `crossbeam`
   - Input channel: macOS apps → audio player
   - Output channel: audio player → macOS apps (loopback)
   - Thread-safe producer/consumer handles

3. **Integration API** (`src/api.rs`):
   - Simple Rust API for audio player integration
   - C API for potential C/C++ integration
   - `HalInputReader`: Read audio from HAL
   - `HalOutputWriter`: Write audio back to HAL
   - `HalAudioHandle`: Combined bidirectional access

## Usage

### For src-audio Integration

```rust
use autoeq_hal::{HalInputReader, HalOutputWriter};

// Create handles
let mut input_reader = HalInputReader::new().expect("HAL not initialized");
let mut output_writer = HalOutputWriter::new().expect("HAL not initialized");

// Read audio from macOS apps
let mut audio_buffer = vec![0.0f32; 512];
let samples_read = input_reader.read(&mut audio_buffer);

// Process audio through your plugin chain...
let processed = process_audio(&audio_buffer[..samples_read]);

// Write back to HAL (loopback)
output_writer.write(&processed);
```

### Using Combined Handle

```rust
use autoeq_hal::HalAudioHandle;

let mut handle = HalAudioHandle::new().expect("HAL not initialized");

loop {
    // Read from HAL
    let mut input_buffer = vec![0.0f32; 512];
    let read = handle.read_input(&mut input_buffer);

    if read > 0 {
        // Process audio
        let processed = process_audio(&input_buffer[..read]);

        // Write back to HAL
        handle.write_output(&processed);
    }
}
```

### C API

```c
#include <stdint.h>

// Read audio from HAL input buffer
int32_t hal_read_input(float* output, int32_t length);

// Write audio to HAL output buffer (loopback)
int32_t hal_write_output(const float* input, int32_t length);

// Get available samples/space
int32_t hal_input_available();
int32_t hal_output_available();
```

## Building

### Build the HAL Driver

```bash
# Build library
cargo build --release -p autoeq_hal

# Build driver bundle
./scripts/build_driver.sh
```

### Install Driver

```bash
# Install (requires sudo)
sudo ./scripts/install_driver.sh

# Reload Core Audio
sudo killall coreaudiod

# Verify installation
system_profiler SPAudioDataType | grep -i "autoeq\|audio hal"
```

### Uninstall Driver

```bash
sudo ./scripts/uninstall_driver.sh
sudo killall coreaudiod
```

## Testing

### Run Tests

```bash
# Test loopback functionality
cargo run --release --bin test_loopback

# Test integration example
cargo run --release --example audio_player_integration
```

### Debug Logs

```bash
# Watch driver logs in Console.app
./scripts/debug_driver.sh

# Or use log show command
log show --predicate 'subsystem contains "coreaudio"' --last 5m
```

## Configuration

The HAL driver uses sensible defaults:

- **Sample Rate**: 48kHz (supports 44.1kHz, 48kHz, 96kHz)
- **Buffer Size**: 512 frames (adjustable 64-4096)
- **Channels**: 2 (stereo)
- **Buffer Capacity**: 500ms

No configuration files needed - all audio processing and configuration handled by the audio player.

## Project Structure

```
src-hal/
├── src/
│   ├── lib.rs              # Main library entry point
│   ├── hal_driver.rs       # Core HAL driver implementation
│   ├── audio_buffer.rs     # Bidirectional audio buffers
│   ├── api.rs              # Public API for integration
│   ├── bridge.rs           # C/Rust FFI bridge
│   └── utils.rs            # Utility functions
│
├── bin/
│   └── test_loopback.rs    # Loopback functionality test
│
├── examples/
│   └── audio_player_integration.rs  # Integration example
│
├── scripts/
│   ├── build_driver.sh     # Build .driver bundle
│   ├── install_driver.sh   # Install to system
│   ├── uninstall_driver.sh # Remove from system
│   └── debug_driver.sh     # View driver logs
│
└── conf/
    └── (legacy config files, not used by simplified driver)
```

## Differences from Previous Version

### Removed

- ❌ AudioPipeline - No built-in audio processing
- ❌ AudioUnitHost - No AU plugin chain
- ❌ Configuration files - No TOML configs needed
- ❌ Routing configuration - Handled by audio player

### Added

- ✅ Simple lock-free buffers
- ✅ Bidirectional audio flow
- ✅ Loopback support
- ✅ Clean integration API
- ✅ C API for compatibility

### Benefits

- **Simpler**: Minimal HAL driver, easy to understand
- **Flexible**: Audio player controls all processing
- **Maintainable**: Less code, fewer dependencies
- **Performant**: Lock-free channels, low latency

## Integration with src-audio

The audio player in `../src-audio` can integrate with this HAL driver:

1. **Read from HAL**: Get audio from macOS apps
2. **Process**: Run through existing plugin chain (EQ, upmixer, etc.)
3. **Output**: Send to physical device (existing playback thread)
4. **Loopback**: Optionally write back to HAL

### Example Modification to src-audio

```rust
// In src-audio/src/engine/playback_thread.rs

use autoeq_hal::HalInputReader;

// Add HAL input reader alongside file decoder
let mut hal_reader = HalInputReader::new();

// In processing loop:
if let Some(ref mut reader) = hal_reader {
    let mut hal_buffer = vec![0.0f32; frame_size];
    let read = reader.read(&mut hal_buffer);

    if read > 0 {
        // Process HAL audio through plugin chain
        process_through_plugins(&hal_buffer[..read]);
    }
}
```

## Troubleshooting

### Driver Not Appearing

1. Check installation:
   ```bash
   ls -l /Library/Audio/Plug-Ins/HAL/AutoEQ.driver
   ```

2. Check logs:
   ```bash
   ./scripts/debug_driver.sh
   ```

3. Reload Core Audio:
   ```bash
   sudo killall coreaudiod
   ```

### Audio Glitches

- Increase buffer size in `hal_driver.rs` (default: 512 frames)
- Check buffer statistics using `BufferStats` API
- Monitor for buffer overflows/underflows in logs

### Build Errors

- Ensure Xcode Command Line Tools installed:
  ```bash
  xcode-select --install
  ```

- Clean and rebuild:
  ```bash
  cargo clean
  cargo build --release
  ```

## Performance

- **Latency**: ~10-20ms (depends on buffer size)
- **CPU Usage**: Minimal (<1% on modern CPUs)
- **Memory**: ~1MB for buffers + ~100KB driver overhead

## License

GPL-3.0-or-later

## Author

Pierre F. Aubert <pierre@spinorama.org>

## Related

- **Audio Player**: `../src-audio` - Processes audio from HAL driver
- **AutoEQ**: `../src-autoeq` - Speaker/headphone optimization
- **IIR Filters**: `../src-iir` - Used by audio player for EQ
