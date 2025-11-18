# SOTF Daemon - Audio Control Daemon with HAL Integration

The `sotf_daemon` is a Unix socket daemon that provides IPC control for audio playback and processing. On macOS, it **automatically initializes the HAL driver** so users can process audio from macOS apps without any manual setup.

## Quick Start

### macOS

Just start the daemon - HAL is automatically configured:

```bash
cargo run --release --bin sotf_daemon
```

Output:
```
===============================================================================
🎵 AutoEQ Audio Control Daemon
===============================================================================

🎵 Initializing HAL driver...
   Buffer capacity: 500ms
   Sample rate: 48000 Hz
   Channels: 2
✅ HAL driver initialized successfully
   HAL input/output plugins are now available

📊 HAL Status:
   Platform supported: ✅ Yes
   Buffer initialized: ✅ Yes
   Driver installed:   ⚠️  No (optional)
   Ready to use:       ✅ Yes

💡 HAL plugins available:
   - hal_input:  Read audio from macOS apps
   - hal_output: Write processed audio back (loopback)

===============================================================================
🚀 Starting daemon...
===============================================================================
Audio daemon listening on /tmp/autoeq_audio.sock
```

That's it! The daemon is now ready to process audio from macOS applications.

### Linux/Windows

```bash
cargo run --release --bin sotf_daemon
```

HAL features won't be available (macOS-only), but all other daemon functionality works normally.

## Features

### Automatic HAL Initialization

On macOS, the daemon automatically:
- ✅ Initializes the HAL audio buffer
- ✅ Makes HAL plugins available (`hal_input`, `hal_output`)
- ✅ Shows HAL status on startup
- ✅ No user configuration required!

### Available Commands

Send JSON commands via Unix socket (`/tmp/autoeq_audio.sock`):

#### Playback Control
```json
{"command": "load", "path": "/path/to/audio.flac"}
{"command": "play"}
{"command": "pause"}
{"command": "stop"}
{"command": "seek", "position": 10.5}
```

#### Volume Control
```json
{"command": "set_volume", "volume": 0.8}
```

#### Plugin Management
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
      "plugin_type": "hal_output",
      "parameters": {"channels": 2}
    }
  ]
}
```

#### Status & Monitoring
```json
{"command": "status"}
{"command": "hal_status"}
{"command": "get_loudness"}
{"command": "list_devices"}
```

#### Shutdown
```json
{"command": "shutdown"}
```

## HAL Plugin Usage

### Example: Process Audio from macOS Apps

Here's a complete plugin chain that reads from macOS apps, applies EQ, and outputs:

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
          {"filter_type": "peak", "frequency": 100.0, "q": 0.7, "gain_db": 6.0},
          {"filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0},
          {"filter_type": "highshelf", "frequency": 8000.0, "q": 0.7, "gain_db": -2.0}
        ]
      }
    },
    {
      "plugin_type": "hal_output",
      "parameters": {"channels": 2}
    }
  ]
}
```

### Example: Upmix Stereo to 5.0 Surround

```json
{
  "command": "load_plugins",
  "plugins": [
    {
      "plugin_type": "hal_input",
      "parameters": {"channels": 2}
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

## Testing with netcat

```bash
# Start the daemon
cargo run --release --bin sotf_daemon

# In another terminal, send commands:
echo '{"command": "hal_status"}' | nc -U /tmp/autoeq_audio.sock

# Output:
# {"success":true,"data":{"buffer_initialized":true,"driver_installed":false,"platform_supported":true,"ready":true}}
```

## Architecture

```
┌─────────────┐
│  macOS App  │ (Safari, Spotify, etc.)
└──────┬──────┘
       │ Audio
       ▼
┌─────────────┐
│ HAL Driver  │ (Virtual Audio Device)
│   Buffer    │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────┐
│        sotf_daemon Process          │
│                                     │
│  ┌──────────────────────────────┐  │
│  │  HAL Manager                 │  │
│  │  (Auto-initialized)          │  │
│  └──────────┬───────────────────┘  │
│             │                       │
│  ┌──────────▼───────────────────┐  │
│  │  HalInputPlugin              │  │
│  │  (Reads from HAL buffer)     │  │
│  └──────────┬───────────────────┘  │
│             │                       │
│  ┌──────────▼───────────────────┐  │
│  │  Plugin Chain                │  │
│  │  (EQ, Upmixer, Compressor,   │  │
│  │   Limiter, etc.)             │  │
│  └──────────┬───────────────────┘  │
│             │                       │
│  ┌──────────▼───────────────────┐  │
│  │  HalOutputPlugin             │  │
│  │  (Writes to HAL buffer)      │  │
│  └──────────┬───────────────────┘  │
│             │                       │
│  ┌──────────▼───────────────────┐  │
│  │  cpal Output                 │  │
│  │  (Physical audio device)     │  │
│  └──────────────────────────────┘  │
└─────────────────────────────────────┘
```

## HAL Status Response

```json
{
  "success": true,
  "data": {
    "platform_supported": true,
    "buffer_initialized": true,
    "driver_installed": false,
    "ready": true
  }
}
```

- **platform_supported**: `true` on macOS, `false` on Linux/Windows
- **buffer_initialized**: Whether the HAL buffer was initialized successfully
- **driver_installed**: Whether the system HAL driver is installed (optional)
- **ready**: Overall readiness (`platform_supported && buffer_initialized`)

## Benefits

### For Users

✅ **Zero configuration** - Just run `sotf_daemon` and it works
✅ **No manual HAL driver installation needed** - Buffer initialization happens automatically
✅ **Process audio from any macOS app** - Safari, Spotify, YouTube, etc.
✅ **Full plugin chain support** - EQ, upmixer, compressor, limiter, etc.
✅ **Loopback support** - Monitor processed audio via HAL output

### For Developers

✅ **Automatic lifecycle management** - HAL initialized on startup, cleaned up on shutdown
✅ **Simple integration** - Just use `hal_input` and `hal_output` plugin types
✅ **Cross-platform graceful degradation** - Works on Linux/Windows without HAL features
✅ **IPC control** - Full control via Unix socket JSON API

## Limitations

1. **macOS Only**: HAL plugins only work on macOS (graceful fallback on other platforms)
2. **Single Process**: HAL buffer is in-process (not shared across processes)
3. **Optional System Driver**: The system HAL driver (`.driver` bundle) is **not required** for the daemon to work - it only needs the buffer initialization

## Troubleshooting

### HAL not initializing

**Symptom**: Daemon shows `buffer_initialized: false`

**Solution**: Check the logs for error messages. On non-macOS systems, this is expected behavior.

### Plugins not receiving audio

**Symptom**: `hal_input` plugin returns silence

**Possible causes**:
1. HAL buffer not initialized (check `hal_status`)
2. No audio source configured
3. Buffer underrun (check logs)

**Solution**:
```bash
# Check status
echo '{"command": "hal_status"}' | nc -U /tmp/autoeq_audio.sock

# Should show ready: true
```

## See Also

- [Plugin Documentation](../src/plugins/README.md)
- [HAL Driver Documentation](../../src-hal/README.md)
- [Audio Engine Documentation](../src/engine/README.md)
