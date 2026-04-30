# Unreleased

## HAL channel capacity

- The Systemwide HAL path now carries the requested input channel count through
  `load_plugins` and daemon-to-HAL configuration, with validation up to 32
  channels.
- The daemon pre-sizes HAL shared memory for 32-channel growth while preserving
  the current advertised channel count, so switching from stereo to immersive
  layouts does not leave the HAL stuck at 2 channels.
- The toolbar input/output channel controls now expose up to 32 channels and
  send the selected HAL input channel count to the daemon.

## Systemwide package upgrade and app identity

- The Systemwide package preinstall now asks a running `sotf-daemon` to shut
  down over its Unix control socket, waits for it to exit, then escalates to
  `TERM` and finally `KILL` only if it is still alive. This avoids replacing the
  daemon while CoreAudio is still using the old process during package upgrades.
- The Systemwide build entry point is now `scripts/build-systemwide.sh`.
- The Systemwide app icon is generated from the configbar SVG artwork instead
  of reusing the GPUI app artwork.

## Toolbar output device and plugin editing

- The toolbar now selects the current system default physical output device
  when CoreAudio reports one, and keeps the previous selection only as a
  fallback.
- Added a refresh button next to the output device picker.
- The output channel picker is now constrained by the selected output
  interface's reported channel count, and refresh/selection changes clamp the
  requested HAL output channels when needed.
- The plugin rack now warns when the plugin chain or a plugin's output layout
  asks for more channels than the selected output device exposes.
- Plugin edit sheets now keep parameter edits in a local draft until `Apply` or
  `Close`; `Cancel` dismisses without applying. The bottom button row now
  provides `Load`, `Save`, `Apply`, `Cancel`, and `Close`, with JSON as the
  default parameter file format and supported JSON shapes available from the
  save dialog.
- Added regression coverage for daemon package quiescing, Systemwide icon
  source selection, output-device channel clamping, default-device selection,
  plugin channel warnings, and batched plugin parameter edits.

Verified:
- `CARGO_TARGET_DIR=/Volumes/home_tmp/tmp/target-sotf cargo test -p sotf-daemon --test daemon_state_tests`
- `swiftc -typecheck crates/systemwide/crates/daemon/configbar/src/*.swift -framework SwiftUI -framework AppKit -framework UserNotifications -framework CoreAudio -framework WebKit`
- `bash -n scripts/build-systemwide.sh scripts/build-dmg-sotf.sh scripts/sign-macos.sh scripts/build-release-local.sh`

# 0.1.36

## Driver HAL streaming configuration

- Driver playback startup now reads the HAL driver's reported sample rate and
  channel count before starting the engine, falling back to 48 kHz stereo only
  when the driver has not reported a format yet.
- Driver reconfiguration now passes the negotiated HAL sample rate into the
  engine restart path instead of restarting through the default 48 kHz HAL
  helper.
- Sample-rate and buffer-frame commands now rely on an acknowledged HAL config
  request; if the HAL does not apply the change, `driver-hal` returns an error
  instead of reporting success optimistically.
- Reconfiguration now preserves the HAL input channel count when available and
  uses the explicit driver-format engine startup helper.
- Added regression coverage through `driver-hal`'s streaming guard tests to
  ensure the negotiated sample rate is wired into `reconfigure_audio_pipeline`.

Verified:
- `cargo test -p sotf-daemon`
- `cargo check -p sotf-daemon -p driver-hal -p sotf-engine --features
  sotf-daemon/hal,sotf-engine/hal`
