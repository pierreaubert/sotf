# Unreleased

## Encrypted HAL passthrough coverage

- Added an encrypted shared-memory passthrough regression that writes and reads
  a full frame block through `SharedAudioBuffer` with `AudioCipher` and verifies
  bit-exact recovery.
- Added a source guard that the Swift HAL encryption round-trip remains part of
  the cross-language encryption test suite.

## COM lifetime and CoreAudio object notifications

- Fixed the Swift HAL COM `QueryInterface` / `AddRef` / `Release` balance so
  repeated `IUnknown` queries on the factory-provided interface no longer leak
  `gRefCount`; `QueryInterface` now only calls `AddRef` when it returns a
  distinct interface pointer.
- Property-change notifications are now gated by the CoreAudio object lifecycle.
  The HAL tracks device creation/destruction and stream discovery, then skips
  `PropertiesChanged` callbacks for invalid or undiscovered objects to avoid
  CoreAudio `HALS_PlugIn::HostInterface_PropertiesChanged: the object is not
  valid` errors.
- Added a Swift regression guard for object-lifecycle-gated property
  notifications.

Verified:
- `swiftc -typecheck crates/systemwide/crates/driver-hal/swift/Sources/*.swift`
- `CARGO_TARGET_DIR=/Volumes/home_tmp/tmp/target-sotf cargo test -p driver-hal --test streaming_regression_tests`

## HAL shared-memory ownership and streaming regressions

- The Rust HAL driver now prepares the shared-memory file from the daemon side
  with `SharedAudioBuffer::create_or_open*`, including header initialization and
  geometry validation. This lets the restricted CoreAudio HAL process open an
  already-sized mmap instead of creating or resizing files itself.
- Daemon-initiated config requests now write `requested_sample_rate` and
  `requested_buffer_frames` with `configSource=2`, wait for the HAL to
  acknowledge the request, and return an error instead of false success when no
  HAL acknowledgement arrives.
- The Swift HAL shared-memory implementation is now open-only: it no longer
  creates directories/files, truncates the mmap file, or chmods paths from
  inside coreaudiod.
- `WriteMix` no longer retries shared-memory initialization from the CoreAudio
  IO path, avoiding filesystem/mmap work in the audio callback.
- Swift now runs shared-memory retry and daemon-config polling on a private
  dispatch timer, so a HAL instance that started before the daemon-created mmap
  can still connect without doing that work on the audio thread.
- Swift now consumes daemon-initiated config requests, applies supported sample
  rate / buffer-frame changes, acknowledges the result through shared memory,
  and notifies CoreAudio of the updated format properties.
- Daemon-initiated HAL format changes now go through CoreAudio's device
  configuration-change handshake before mutating sample rate or buffer size, so
  active IO is quiesced by the host before the new format is applied.
- Swift now reports a legal fixed zero timestamp period and aligns
  `GetZeroTimeStamp()` to that period instead of using the much smaller IO
  buffer size.
- `ReadInput` no longer consumes the same shared-memory ring used by `WriteMix`
  for app-to-daemon capture; until separate daemon-to-HAL IPC exists it only
  serves loopback audio or silence.
- Added regression tests for daemon-owned shared-memory initialization,
  config-request field semantics and acknowledgement timeout, HAL reconnect
  wiring, Swift callback restrictions, Swift open-only shared memory, daemon
  config polling, and `ReadInput` capture-ring isolation.

Verified:
- `cargo test -p driver-hal`
- `swiftc -parse crates/systemwide/crates/driver-hal/swift/Sources/*.swift`
- `cargo check -p sotf-daemon -p driver-hal -p sotf-engine --features
  sotf-daemon/hal,sotf-engine/hal`
