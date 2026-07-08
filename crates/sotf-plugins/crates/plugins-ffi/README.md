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
- `plugin_process_with_events` accepts MIDI input and copies queued MIDI output
  and Note Expression events into host-provided buffers.
- `plugin_enqueue_midi_output_event` and
  `plugin_enqueue_note_expression_output_event` let AUv3/VST3 wrappers bridge
  instrument-style output without allocating in the render callback.
- `plugin_preset_document_info` advertises the preset UTType,
  `.sotfpreset` extension, and document-state schema for AUv3 preset browsers.
- `plugin_export_preset_json`, `plugin_import_preset_json`, and
  `plugin_suggest_preset_filename` implement user-preset document round trips.
- `plugin_vst3_ffi_descriptor` exposes stable Windows/VST3 loader metadata for
  COM-style C#, Python, and other native bindings.
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

For binary-style distribution, run:

```bash
just build-spm-xcframework
```

This stages macOS, iOS device, and iOS simulator static libraries and wraps
them with the generated headers into
`SwiftPackage/Artifacts/SOTFPluginFFI.xcframework`.

## Ownership and lifetime

The C ABI follows a strict ownership model:

- **Handles:** `plugin_create()` returns an owned `PluginHandle*` that must be
  released with exactly one call to `plugin_destroy()`. After destroy the handle
  is invalid and must not be used for any other call.
- **Borrowed pointers:** `plugin_type`, `config_json`, `param_id`, process
  buffers, and input event arrays are borrowed only for the duration of the
  call. The caller keeps ownership and must keep the data valid and unchanged
  while the FFI function runs.
- **Owned return values:** Functions documented as returning an owned string or
  byte buffer (`plugin_get_info_json()`, `plugin_save_state()`,
  `plugin_export_preset_json()`, `plugin_suggest_preset_filename()`,
  `plugin_ffi_platform_info_json()`, `plugin_available_types()`) transfer
  ownership to the caller. The matching `plugin_free_string()` or
  `plugin_free_state()` must be called exactly once.
- **Handle-derived pointers:** `plugin_get_parameter_info()` returns a pointer
  that is valid only while the handle remains alive and undestroyed.
- **Static strings:** `plugin_preset_document_info()`,
  `plugin_vst3_ffi_descriptor()`, and `plugin_swift_package_info()` return
  pointers to static, null-terminated C strings with program lifetime. They
  must not be freed.
- **Thread-local errors:** `plugin_get_last_error()` points to thread-local
  storage that remains valid until the next FFI call on the same thread that
  may set an error.

## Windows VST3 FFI

`plugin_vst3_ffi_descriptor()` is the native-language discovery entrypoint.
Windows hosts can load the dynamic library, check the descriptor, then use the
same opaque-handle lifecycle as AUv3 (`plugin_create`, `plugin_process*`,
parameter, state, preset, and event functions). `plugins-nih` remains the
Rust-native VST3 plugin wrapper; this FFI surface is for external host
languages that need a stable C ABI. The descriptor reports no native COM
factory yet; hosts should call the exported C ABI directly.
