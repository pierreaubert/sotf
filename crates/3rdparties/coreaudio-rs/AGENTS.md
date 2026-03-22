# coreaudio-rs (lib: `coreaudio`)

**Vendored 3rd-party crate** -- fork of [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs).

Rust interface for Apple's CoreAudio API. Provides safe wrappers around AudioUnit, AudioToolbox, and CoreAudio hardware APIs.

## Features

- `audio_toolbox` (default) -- AudioToolbox/AudioUnit bindings
- `audio_unit` -- Audio Unit plugin hosting
- `core_audio` (default) -- CoreAudio hardware API
- `core_midi` -- CoreMIDI bindings

## Important Notes

- This is a vendored upstream crate -- minimize modifications
- macOS/iOS only
- Uses `objc2-*` crates for Objective-C interop
- Used by cpal and the HAL driver for audio hardware access
