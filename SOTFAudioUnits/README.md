# SOTF Audio Units

macOS Audio Unit (AUv3) plugins built from SOTF Rust audio processing core.

## Structure

```
SOTFAudioUnits/
├── SOTFAudioUnits.xcodeproj/      # Xcode project
├── EQAudioUnit/                   # EQ plugin extension
│   ├── Info.plist
│   ├── EQAudioUnit.swift         # AUAudioUnit subclass
│   ├── EQViewController.swift    # SwiftUI UI
│   └── EQParameters.swift        # Parameter management
├── Shared/                        # Shared code
│   ├── BridgingHeader.h          # C FFI header
│   └── PluginHost.swift          # Common plugin hosting code
└── Resources/
    └── libsotf_audio_ffi.a       # Linked Rust static library

```

## Building

### 1. Build Rust FFI Library

```bash
cd /home/user/sotf
just build-au-rust
```

This creates a universal (x86_64 + arm64) static library at:
`SOTFAudioUnits/Resources/libsotf_audio_ffi.a`

### 2. Build Audio Unit in Xcode

```bash
just build-au-swift
```

Or open in Xcode:
```bash
open SOTFAudioUnits/SOTFAudioUnits.xcodeproj
```

### 3. Install to System

```bash
just install-au
```

This copies the `.appex` bundle to:
`~/Library/Audio/Plug-Ins/Components/`

## Testing

Test in a DAW:
- Logic Pro
- GarageBand
- Reaper
- Any AU host

Or use the command-line validator:
```bash
auval -v aufx SOEQ SOTF
```

Where:
- `aufx` = Audio Unit Effect
- `SOEQ` = Subtype (SOTF EQ)
- `SOTF` = Manufacturer code

## Adding More Plugins

To add another plugin (e.g., Compressor):

1. Create new target in Xcode: `CompressorAudioUnit`
2. Copy and modify EQ files
3. Update `plugin_factory.rs` to handle "Compressor" type
4. Rebuild FFI library
5. Build in Xcode

## Architecture

```
┌────────────────────────────────────────┐
│  Logic Pro / GarageBand (AU Host)      │
│  ├─ Discovers .appex bundles           │
│  └─ Loads AUAudioUnit component        │
└────────┬───────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│  EQAudioUnit.appex                     │
│  ┌──────────────────────────────────┐  │
│  │  Swift: EQAudioUnit.swift        │  │
│  │  - AUAudioUnit subclass          │  │
│  │  - internalRenderBlock           │  │
│  │  - Parameter management          │  │
│  └────────┬─────────────────────────┘  │
│           │ (FFI calls)                │
│  ┌────────▼─────────────────────────┐  │
│  │  C FFI: sotf_audio_ffi.h         │  │
│  │  - plugin_create()               │  │
│  │  - plugin_process()              │  │
│  │  - plugin_set_parameter()        │  │
│  └────────┬─────────────────────────┘  │
└───────────┼─────────────────────────────┘
            │
            ▼
┌──────────────────────────────────────┐
│  libsotf_audio_ffi.a (Rust)          │
│  ├─ Plugin trait wrapper             │
│  ├─ Parameter mapping                │
│  └─ EqPlugin, etc.                   │
└──────────────────────────────────────┘
```

## Troubleshooting

### Plugin not found in DAW

1. Check installation:
   ```bash
   ls ~/Library/Audio/Plug-Ins/Components/
   ```

2. Check code signing:
   ```bash
   codesign -dv --verbose=4 ~/Library/Audio/Plug-Ins/Components/EQAudioUnit.appex
   ```

3. Validate with auval:
   ```bash
   auval -a  # List all AU plugins
   ```

### Build errors

- Make sure Rust library is built first: `just build-au-rust`
- Check that `libsotf_audio_ffi.a` exists in Resources/
- Verify Xcode target links against the static library

### Runtime crashes

- Check Console.app for crash logs
- Look for FFI boundary issues (null pointers, memory corruption)
- Enable Address Sanitizer in Xcode scheme settings

## References

- [Apple: Creating an audio unit extension](https://developer.apple.com/documentation/avfaudio/audio_engine/audio_units/creating_an_audio_unit_extension/)
- [AUv3 Developer Documentation](https://developer.apple.com/documentation/audiounit)
