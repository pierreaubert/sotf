# SOTF Audio Units - Complete Setup Guide

This guide walks you through creating macOS Audio Unit (AUv3) plugins from your SOTF Rust audio processing code.

## Overview

You now have a complete Audio Unit framework that:

- ✅ Reuses all your existing Rust DSP code (EqPlugin, etc.)
- ✅ Provides C FFI bindings for macOS integration
- ✅ Includes native SwiftUI user interfaces
- ✅ Supports proper AU host integration (Logic Pro, GarageBand, etc.)
- ✅ Has automated build scripts via `just`

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Logic Pro / GarageBand (AU Host)                   │
│  - Loads .appex bundles from Components folder      │
│  - Provides audio stream and automation             │
└──────────────────┬──────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────┐
│  EQAudioUnit.appex (Audio Unit Extension)           │
│  ┌────────────────────────────────────────────────┐ │
│  │  Swift Layer (EQAudioUnit.swift)               │ │
│  │  - AUAudioUnit subclass                        │ │
│  │  - internalRenderBlock (audio processing)      │ │
│  │  - Parameter management (AUParameterTree)      │ │
│  │  - SwiftUI UI (EQViewController.swift)         │ │
│  └──────────────┬─────────────────────────────────┘ │
│                 │ C FFI calls                        │
│  ┌──────────────▼─────────────────────────────────┐ │
│  │  C Header (BridgingHeader.h)                   │ │
│  │  - plugin_create()                             │ │
│  │  - plugin_process()                            │ │
│  │  - plugin_set_parameter()                      │ │
│  └──────────────┬─────────────────────────────────┘ │
└─────────────────┼───────────────────────────────────┘
                  │ links to static library
┌─────────────────▼───────────────────────────────────┐
│  libsotf_audio_ffi.a (Rust Static Library)          │
│  ┌────────────────────────────────────────────────┐ │
│  │  FFI Layer (src-audio-ffi/src/lib.rs)         │ │
│  │  - PluginHandle (opaque pointer)              │ │
│  │  - plugin_create/destroy/process               │ │
│  │  - Parameter mapping (normalized 0-1)         │ │
│  ├────────────────────────────────────────────────┤ │
│  │  Plugin Factory (plugin_factory.rs)           │ │
│  │  - Creates plugins from JSON configs          │ │
│  │  - Supports: EQ, Compressor, etc.             │ │
│  ├────────────────────────────────────────────────┤ │
│  │  Your Existing Plugins (src-audio)            │ │
│  │  - EqPlugin (plugin_eq.rs)                    │ │
│  │  - CompressorPlugin (plugin_compressor.rs)    │ │
│  │  - All other SOTF plugins                     │ │
│  └────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

## Files Created

### Rust FFI Layer (`src-audio-ffi/`)

```
src-audio-ffi/
├── Cargo.toml                  # FFI crate config (staticlib + cdylib)
├── build.rs                    # Generates C header via cbindgen
├── cbindgen.toml              # cbindgen configuration
├── src/
│   ├── lib.rs                 # Main FFI functions (plugin_create, etc.)
│   ├── plugin_factory.rs      # Creates plugins from JSON
│   └── parameter_map.rs       # Parameter mapping (normalized 0-1)
└── sotf_audio_ffi.h           # Generated C header (auto-created)
```

**Key functions:**
- `plugin_create()` - Create plugin instance
- `plugin_destroy()` - Destroy plugin
- `plugin_process()` - Process audio (real-time safe)
- `plugin_set_parameter()` - Set parameter (normalized 0.0-1.0)
- `plugin_get_parameter()` - Get parameter value

### Audio Unit Extension (`SOTFAudioUnits/`)

```
SOTFAudioUnits/
├── README.md                   # This file
├── SETUP_GUIDE.md             # Comprehensive setup instructions
├── create_xcode_project.sh    # Helper script with instructions
├── Shared/
│   └── BridgingHeader.h       # C FFI interface for Swift
├── EQAudioUnit/
│   ├── Info.plist             # AU extension metadata
│   ├── EQAudioUnit.swift      # AUAudioUnit subclass
│   └── EQViewController.swift # SwiftUI UI
└── Resources/
    └── libsotf_audio_ffi.a    # Universal binary (x86_64 + arm64)
```

### Build Scripts

**Justfile targets:**
- `just build-au-rust` - Build Rust FFI as universal binary
- `just build-au-swift` - Build Audio Unit in Xcode
- `just build-au` - Complete build pipeline
- `just install-au` - Install to system
- `just validate-au` - Validate with auval

## Step-by-Step Setup

### 1. Build Rust FFI Library

```bash
cd /home/user/sotf
just build-au-rust
```

This:
1. Compiles `src-audio-ffi` for x86_64 and arm64
2. Creates universal binary with `lipo`
3. Generates C header with `cbindgen`
4. Copies everything to `SOTFAudioUnits/Resources/`

**Output:**
- `SOTFAudioUnits/Resources/libsotf_audio_ffi.a` (universal binary)
- `SOTFAudioUnits/Shared/sotf_audio_ffi.h` (C header)

### 2. Create Xcode Project

You need to manually create the Xcode project (can't be scripted):

```bash
cd SOTFAudioUnits
./create_xcode_project.sh
```

Follow the detailed instructions to:
1. Create App + Audio Unit Extension targets
2. Configure build settings (library search paths, bridging header)
3. Link `libsotf_audio_ffi.a`
4. Add Swift source files
5. Update Info.plist

**Important settings:**

- **Library Search Paths:** `$(PROJECT_DIR)/Resources`
- **Header Search Paths:** `$(PROJECT_DIR)/Shared`
- **Bridging Header:** `Shared/BridgingHeader.h`
- **Deployment Target:** macOS 15.0

### 3. Build Audio Unit

```bash
just build-au-swift
```

Or in Xcode:
1. Select **EQAudioUnit** scheme
2. Product → Build (⌘B)

### 4. Install to System

```bash
just install-au
```

This copies `EQAudioUnit.appex` to:
```
~/Library/Audio/Plug-Ins/Components/
```

### 5. Test

**In a DAW:**
1. Open Logic Pro or GarageBand
2. Insert audio effect
3. Look for "SOTF: Parametric EQ"

**Command-line validation:**
```bash
just validate-au
```

Or manually:
```bash
auval -v aufx SOEQ SOTF
```

Where:
- `aufx` = Audio Unit Effect
- `SOEQ` = Subtype (SOTF EQ)
- `SOTF` = Manufacturer code

## Adding More Plugins

To add another plugin (e.g., Compressor):

### 1. Update Rust FFI Factory

Edit `src-audio-ffi/src/plugin_factory.rs`:

```rust
pub fn create_plugin(...) -> Result<Box<dyn Plugin>, String> {
    match config.plugin_type.as_str() {
        "EQ" => create_eq_plugin(config, input_channels, output_channels),
        "Compressor" => create_compressor_plugin(...), // Add this
        _ => Err(format!("Unknown plugin type: {}", config.plugin_type)),
    }
}

fn create_compressor_plugin(...) -> Result<Box<dyn Plugin>, String> {
    // Similar to create_eq_plugin
    let params: CompressorPluginParams = serde_json::from_value(...)?;
    let plugin = CompressorPlugin::from_params(...)?;
    Ok(Box::new(plugin))
}
```

### 2. Create Xcode Target

1. Duplicate **EQAudioUnit** target
2. Rename to **CompressorAudioUnit**
3. Update bundle identifier
4. Update Info.plist (subtype: `SOCO`, name: "SOTF: Compressor")

### 3. Update Swift Code

Copy and modify:
- `CompressorAudioUnit.swift`
- `CompressorViewController.swift`

Change plugin creation:
```swift
plugin_create("Compressor", config_json, ...)
```

### 4. Rebuild

```bash
just build-au-rust  # Rebuild FFI with new factory code
just build-au-swift # Build all targets
```

## Development Workflow

### Typical Development Cycle

1. **Modify Rust DSP code** (e.g., add filter type to EQ)
   ```bash
   # Edit src-audio/src/plugins/plugin_eq.rs
   ```

2. **Rebuild FFI library**
   ```bash
   just build-au-rust
   ```

3. **Rebuild Audio Unit** (if Swift code changed)
   ```bash
   just build-au-swift
   ```
   Or just rebuild in Xcode (⌘B)

4. **Reinstall**
   ```bash
   just install-au
   ```

5. **Test**
   - Restart DAW
   - Or use `auval` for quick validation

### Debugging

**Console logs:**
```bash
log stream --predicate 'process == "Logic Pro X"' --level debug
```

**Check for crashes:**
```bash
open ~/Library/Logs/DiagnosticReports/
```

**Common issues:**

1. **Plugin not found:**
   - Check installation: `ls ~/Library/Audio/Plug-Ins/Components/`
   - Restart audio component registrar: `killall AudioComponentRegistrar`

2. **Build errors:**
   - Verify `libsotf_audio_ffi.a` exists in Resources/
   - Check library search paths in Xcode
   - Ensure bridging header path is correct

3. **Runtime crashes:**
   - Enable Address Sanitizer in Xcode scheme
   - Check FFI boundary (null pointers, memory safety)
   - Verify audio buffer sizes match expectations

## Parameter System

### How Parameters Work

1. **Rust plugin** has native parameters (e.g., `frequency: f64`, `q: f64`, `gain_db: f64`)

2. **FFI layer** (`parameter_map.rs`) maps to generic system:
   - Normalized values (0.0 = min, 1.0 = max)
   - String IDs (e.g., `"band0_freq"`)
   - Min/max ranges
   - Units (Hz, dB, etc.)

3. **Swift layer** creates `AUParameter` objects:
   - Exposes to AU host for automation
   - Handles UI bindings
   - Converts to/from FFI calls

### Adding New Parameters

**Example: Add "bypass" parameter to EQ**

1. **Update Rust plugin** (`plugin_eq.rs`):
   ```rust
   pub struct EqPlugin {
       bypass: bool,
       // ...
   }

   impl Plugin for EqPlugin {
       fn parameters(&self) -> Vec<Parameter> {
           vec![
               Parameter::new_bool("bypass", "Bypass", false)
                   .with_description("Bypass the plugin")
                   .with_group("General")
                   .with_importance(ParameterImportance::Useful)
           ]
       }
   }
   ```

2. **Update parameter map** (`parameter_map.rs`):
   ```rust
   parameters.push(ParameterMetadata {
       id: "bypass".to_string(),
       name: "Bypass".to_string(),
       min_value: 0.0,
       max_value: 1.0,
       steps: 1, // Boolean: 0 or 1
       // ...
   });
   ```

3. **Update UI** (`EQViewController.swift`):
   ```swift
   Toggle("Bypass", isOn: $viewModel.bypass)
   ```

4. Rebuild and reinstall

## Performance Considerations

### Real-Time Safety

The `plugin_process()` function is called in the audio thread:

- ✅ **No allocations** (pre-allocated buffers)
- ✅ **No locks** (lock-free parameter updates)
- ✅ **Fast** (direct FFI call to Rust)
- ❌ Avoid `println!()` or logging in process()
- ❌ Don't allocate memory
- ❌ Don't use Mutex/RwLock

### Optimization

Rust FFI is compiled with:
```toml
[profile.release]
lto = true              # Link-time optimization
opt-level = 3           # Maximum optimization
codegen-units = 1       # Single codegen unit for better optimization
```

### Latency

- IIR filters: ~0 samples latency
- FFT-based processing: Depends on block size
- Report via `plugin.latency_samples()` → Swift → AU host

## Troubleshooting

### Plugin validation fails

```bash
auval -v aufx SOEQ SOTF
```

**Common errors:**

- **"Cannot find component"**: Not installed or wrong identifier
  - Check: `ls ~/Library/Audio/Plug-Ins/Components/`
  - Verify Info.plist: type=`aufx`, subtype=`SOEQ`, manufacturer=`SOTF`

- **"Audio unit not initialized"**: Plugin creation failed
  - Check Console.app for Rust error messages
  - Verify JSON config is valid

- **"Render callback failed"**: Processing error
  - Check buffer sizes
  - Verify channel counts match

### Build failures

**"Undefined symbols for architecture arm64":**
- Rebuild FFI: `just build-au-rust`
- Verify universal binary: `lipo -info Resources/libsotf_audio_ffi.a`

**"Bridging header not found":**
- Check Xcode Build Settings → Swift Compiler → Bridging Header
- Should be: `Shared/BridgingHeader.h`

**"Library not loaded":**
- Static library must be linked, not loaded
- Verify in Build Phases → Link Binary With Libraries

## Next Steps

### Extend to More Plugins

Create Audio Units for:
- ✅ **EQ** (done)
- ⏳ **Compressor** (follow "Adding More Plugins" section)
- ⏳ **Limiter**
- ⏳ **Gate**
- ⏳ **Upmixer** (stereo → surround)

### Advanced Features

- **Preset management**: Implement `fullState` for DAW preset saving
- **Spectrum analyzer**: Add visual feedback (FFT display)
- **MIDI control**: Map MIDI CC to parameters
- **Sidechain**: Support multi-input busses

### Distribution

1. **Code signing**:
   ```bash
   codesign --force --sign "Developer ID Application: Your Name" \
            EQAudioUnit.appex
   ```

2. **Notarization** (for distribution outside Mac App Store)

3. **Installer**: Create .pkg with `pkgbuild`

## References

- [Apple: Creating an Audio Unit Extension](https://developer.apple.com/documentation/avfaudio/audio_engine/audio_units/creating_an_audio_unit_extension/)
- [AUv3 Developer Docs](https://developer.apple.com/documentation/audiounit)
- [Audio Unit Programming Guide](https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/AudioUnitProgrammingGuide/)

## Support

For issues:
1. Check Console.app for crash logs
2. Run with `auval -v aufx SOEQ SOTF` for detailed validation
3. Enable Xcode debugging with Audio Unit hosting app

---

Built with ❤️ using Rust + Swift
