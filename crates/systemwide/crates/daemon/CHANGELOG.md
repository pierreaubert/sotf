# 0.1.37 (unreleased)

## Transactional systemwide DSP graphs

- `load_plugin_artifact` now accepts validated engine DAG artifacts while
  retaining rack compatibility and rejecting unconverted per-channel schemas.
- Desired/applied state records rack versus graph topology; channel, device,
  and HAL reconfiguration preserve the active artifact kind.
- Configbar selects the existing rack for linear chains or a graph editor for
  nonlinear topology, with stable IDs, connections, settings, transactional
  edits, save/load round trips, and true per-node bypass.

## Review-driven startup and IPC hardening

- Serialize daemon startup with a process-lifetime lock acquired before
  encryption-key rotation or stale-socket handling.
- Make startup key rotation daemon-owned so every process lifetime uses a fresh
  key without desynchronizing `KeyManager` from shared memory.
- Remove the unused Tokio runtime and call the synchronous engine/driver command
  handlers directly from client threads.
- Cache available-plugin descriptors with `OnceLock` instead of rebuilding
  plugin defaults for every discovery request.
- Extend regression coverage for startup ordering, real Unix-socket IPC, and
  Swift atomic shared-memory publication.

## Diagnostics and recovery UX (QA-SYS-003)

- Added the executable `just systemwide-lab` macOS gate. It combines daemon
  state tests, real Unix-socket lab-driver scenarios, HAL protocol/streaming
  regressions, and Configbar model tests without installing the HAL bundle.
- Process-level lab coverage now verifies coherent snapshots, transactional
  artifact rejection, 2 → 10 → 2 channel changes, transport configuration
  rejection/commit behavior, encryption enablement and key rotation, diagnostic
  dumps, shutdown, and clean restart.
- Encryption enable/rotation requests now return an explicit capability error
  when the daemon has no session cipher instead of reporting success while
  silently leaving encryption disabled.
- `SOTF_SYSTEMWIDE_RUNTIME_DIR` now isolates the daemon-private key and the
  Rust/Swift HAL-readable key copy alongside the lab socket and shared memory;
  focused path tests and the real subprocess scenario reject writes outside the
  temporary lab directory.
- Enriched the daemon `Status` response with explicit systemwide diagnostics:
  - `driver`: `installed`, `ready`, `capture_active`, `frame_size`, `sample_rate`,
    and `channel_count`.
  - `encryption`: `enabled` and `fingerprint`.
  - `active_route`: `desired_output_device`, `applied_output_device`,
    `playback_output_device`, and `capture_active`.
  - `recovery_actions`: deterministic list of suggested recovery steps such as
    `reinstall_driver`, `restart_daemon`, `select_output_device`,
    `rotate_encryption_key`, and `reset_shared_memory`.
- Updated unit tests and the `snapshot_status_response_shape` snapshot to cover
  the new fields.

## Security and IPC hardening (QA-SYS-001)

- Added focused non-device tests for daemon security boundaries:
  - `KeyManager::force_rotate` produces a new session fingerprint and keeps the
    HAL key copy at mode `0600`.
  - Session-key rotation remains daemon-owned across daemon restart and
    driver reconnection, preventing a stale HAL copy from being reused.
  - Peer classification falls back to the restricted `CoreAudioD` class for any
    UID that is not the daemon owner, root, or the macOS `_coreaudiod` user.
  - Socket path construction is deterministic, honors explicit absolute lab
    overrides, and defaults to a user-isolated path under `$TMPDIR`,
    `$XDG_RUNTIME_DIR`, or `/tmp/sotf-{uid}/`.
- Documented that real-device HAL install, reload, recovery, and audio-callback
  timing QA remain release blockers tracked separately under `QA-SYS-002`.

## Daemon startup cleanup

- ConfigBar startup cleanup now asks existing `sotf-daemon` processes to exit
  with an exact-name `TERM` signal instead of using destructive fuzzy
  `pkill -9 -f` matching.
- Added regression coverage to keep the toolbar cleanup path non-forceful and
  exact-name matched.
- Daemon IPC clients now get a bounded idle read timeout, and timeout/`WouldBlock`
  reads close the client instead of letting an idle socket block the client
  handler indefinitely.

## HAL channel capacity

- The Systemwide HAL path now carries the requested input channel count through
  `load_plugins` and daemon-to-HAL configuration, with validation up to 32
  channels.
- The daemon pre-sizes HAL shared memory for 32-channel growth while preserving
  the current advertised channel count, so switching from stereo to immersive
  layouts does not leave the HAL stuck at 2 channels.
- The toolbar input/output channel controls now expose up to 32 channels and
  send the selected HAL input channel count to the daemon.
- The Systemwide configuration meters now keep N-channel meter slots visible
  before analyzer data arrives, the loudness analyzer reports up to 32
  per-channel meter values, and runtime Downmix creation adapts to the current
  chain width so adding Downmix reduces the post-chain monitor to 2 channels.

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
