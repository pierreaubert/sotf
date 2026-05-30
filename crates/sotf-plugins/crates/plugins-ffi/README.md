# plugins-ffi

C FFI bindings for SOTF audio plugins.

The core FFI surface exposes SOTF plugins through opaque handles for native
hosts. macOS AUv3 uses the full GPUI bridge today; iOS AUv3 and Windows VST3
can consume the portable audio/parameter/state ABI while their host packaging
and UI adapters mature.

## Surfaces

- `plugin_create` / `plugin_process` / parameter and state calls for audio
  plugins.
- `plugin_process_with_midi` for allocation-free MIDI input bridging.
- `plugin_process_with_events` reserves ABI slots for MIDI output and Note
  Expression; those currently report unsupported until the core plugin trait
  can produce outgoing events.
- `plugin_preset_document_info` advertises the preset UTType,
  `.sotfpreset` extension, and document-state schema for AUv3 preset browsers.
- `plugin_ffi_capabilities` and `plugin_ffi_platform_info_json` let AUv3,
  SwiftPM, and future VST3 hosts feature-detect the current target.

## Header Sync

`build.rs` runs `cbindgen` automatically and writes:

- `sotf_audio_plugin_ffi.h` in this crate
- `../plugins-au/Shared/sotf_audio_plugin_ffi.h`
- `SwiftPackage/Sources/SOTFPluginFFI/include/sotf_audio_plugin_ffi.h`
- `../plugins-au/Shared/gpui_au_ffi.h` on macOS targets

Set `SOTF_FFI_HEADER_DIR=/path/to/headers` to generate into another directory,
or `SOTF_FFI_SKIP_HEADER_SYNC=1` for packaging jobs that need a read-only
source tree.

## Swift Package

`Package.swift` exposes the generated header as a SwiftPM C target named
`SOTFPluginFFI`. Build the Rust static library for the target Apple platform
and place it where the consuming Xcode project can resolve
`-lsotf_audio_plugins_ffi`; the package supplies the headers and linker
metadata for the common Apple frameworks.
