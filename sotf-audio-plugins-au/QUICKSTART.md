# Quick Start - SOTF Audio Units

Get your Rust plugins running as macOS Audio Units in 5 steps.

## Prerequisites

- macOS 15.0+
- Xcode 15+ installed
- Rust toolchain (with x86_64 and aarch64 targets)
- `just` command runner: `cargo install just`

## 5-Minute Setup

### 1. Build Rust FFI Library

```bash
cd /home/user/sotf
just build-au-rust
```

**What this does:**
- Compiles `src-audio-ffi` for both Intel and Apple Silicon
- Creates universal binary: `SOTFAudioUnits/Resources/libsotf_audio_ffi.a`
- Generates C header: `SOTFAudioUnits/Shared/sotf_audio_ffi.h`

### 2. Create Xcode Project

```bash
cd SOTFAudioUnits
./create_xcode_project.sh
```

Follow the on-screen instructions to:
1. Create new Xcode project
2. Add Audio Unit Extension target
3. Configure build settings
4. Link Rust static library

**⚠️ This step is manual** - Xcode projects can't be scripted.

### 3. Build Audio Unit

```bash
just build-au-swift
```

Or in Xcode: Select **EQAudioUnit** scheme → ⌘B

### 4. Install

```bash
just install-au
```

Installs to: `~/Library/Audio/Plug-Ins/Components/`

### 5. Test

Open Logic Pro / GarageBand → Insert Audio Effect → Look for **"SOTF: Parametric EQ"**

Or validate from terminal:
```bash
just validate-au
```

## What You Get

✅ **10-band parametric EQ** with SwiftUI interface
✅ **All your Rust DSP code** working natively in AU hosts
✅ **Automated build system** via `just`
✅ **Ready to extend** to other plugins (Compressor, Limiter, etc.)

## Next: Add More Plugins

1. Edit `src-audio-ffi/src/plugin_factory.rs` - add new plugin type
2. Duplicate Xcode target (EQAudioUnit → CompressorAudioUnit)
3. Update Swift code to use new plugin type
4. Rebuild: `just build-au`

## Troubleshooting

**Plugin not showing up?**
```bash
# Check installation
ls ~/Library/Audio/Plug-Ins/Components/

# Restart audio component registrar
killall AudioComponentRegistrar
```

**Build errors?**
```bash
# Verify FFI library exists
ls -lh SOTFAudioUnits/Resources/libsotf_audio_ffi.a

# Check it's a universal binary
lipo -info SOTFAudioUnits/Resources/libsotf_audio_ffi.a
```

**More help:** See `SETUP_GUIDE.md` for detailed documentation.

## Files Created

```
src-audio-ffi/          ← Rust FFI layer
SOTFAudioUnits/         ← Xcode project location
├── Shared/
│   └── BridgingHeader.h
├── EQAudioUnit/
│   ├── EQAudioUnit.swift
│   ├── EQViewController.swift
│   └── Info.plist
└── Resources/
    └── libsotf_audio_ffi.a
```

**Build commands added to Justfile:**
- `just build-au-rust` - Build Rust library
- `just build-au-swift` - Build Xcode project
- `just build-au` - Full build pipeline
- `just install-au` - Install to system
- `just validate-au` - Validate with auval

---

Questions? See the full `SETUP_GUIDE.md`
