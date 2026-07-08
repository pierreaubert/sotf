# 0.6.1 (unreleased)

## Changed

- Documented FFI ownership and lifetime contracts for handles, borrowed
  pointers, owned return values, handle-derived pointers, static strings, and
  thread-local error storage.
- Added unit tests covering handle destroy safety, UTF-8 validation, unknown
  plugin type rejection, process buffer sizing, parameter info lifetime,
  parameter round-trip, owned string/state buffer freeing, output-event
  buffer bounds, free-function null-pointer tolerance, static metadata pointer
  lifetime, thread-local error stability, state save/load round-trip, and
  reset-after-create usability.

# 0.6.0

## New

- Added ABI v3 capability metadata for AUv3, Windows/VST3, SwiftPM, preset
  documents, MIDI output, and Note Expression support.
- Added an iOS AUv3 scaffold with an iOS container app, EQ AU extension,
  XcodeGen spec, UIKit fallback view, and device/simulator build recipes.
- Added Swift Package Manager support with a C target modulemap, generated
  public headers, SwiftPM build validation, and an XCFramework staging recipe.
- Added Windows/VST3 FFI discovery metadata for native-language hosts using
  the portable C ABI.
- Added preset document helpers for UTType metadata, JSON export/import,
  safe filename suggestions, full-state documents, and macOS bookmarks.
- Added MIDI/Note Expression input and output bridge support, including AU
  MIDI event parsing, output queues/accessors, and ProcessContext
  Note Expression events.

## Changes

- `build.rs` now runs cbindgen and syncs generated headers to the root FFI
  header, AU shared header, and SwiftPM include header.
- Bumped the crate version to 0.6.0 because the exported FFI ABI version moved
  from 2 to 3.

# 0.5.3

## New

- Added missing new-ish plugin to AU plugins repo
- Added an AAE plugin (experimental)
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added playlist support across the board

## Fixes

- Added missing AU plugin, fix tests, update snapshots

## Changes

- Long overdue split of denoiser into denoiser+declick+hiss-reducer+speach-denoiser
- Listening + bug hunting session on plugins
- Next iteration on UI and testing for plugins this time with native look&feel
- Road the working AU plugins
- Cleanup: another round of clippy
- Next step of UI implementation for plugins
- AU plugins are working and I can load them but without a proper UI
