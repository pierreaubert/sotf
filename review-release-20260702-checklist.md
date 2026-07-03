# SOTF Release Review Checklist — 2026-07-02

Source: `review-release-20260702.md`

## P0 Blockers

### Plugin UI / Parameters
- [x] Saturation bool/float mismatch for `dc_blocker` and `use_adaa`.
- [x] EQ parameter schema mismatch: `max_filters`, `filter_type`, choice specs, and extra string params.
- [x] Binaural decoder `sofa_file` get/set file-path type mismatch.
- [x] Matrix and ambisonics reject unknown parameters instead of silently accepting them.

### Engine / Real-Time
- [x] Clamp f32 CPAL output path to `[-1, 1]`.
- [x] Remove analyzer cache bootstrap allocation from processing hot path.
- [x] Remove decoder `ensure_buffer_len` allocation fallback from decoder hot path.
- [x] Tighten playback runtime allocation tolerance to zero after warmup.

### Client / Server
- [x] Add HTTP-level playback endpoint integration tests.
- [x] Add HTTP-level queue endpoint integration tests.
- [x] Add HTTP-level media/range/auth integration tests.
- [x] Implement MPD `idle` / `noidle`.
- [x] Gate or implement Chromecast CASTV2 `LOAD`.
- [x] Fix capabilities document for pairing/search.
- [x] Emit `LibraryChanged` SSE events.
- [x] Emit `ScannerProgress` SSE events.

### iOS
- [x] Add Xcode build-script phase for the Rust library.
- [x] Remove committed `libsotf_ios.a` release artifact dependency.
- [x] Add launch screen.
- [x] Add app-icon asset catalog.
- [x] Add `PrivacyInfo.xcprivacy`.
- [x] Remove stale `armv7` device capability.
- [x] Fix version/plist mismatches.
- [x] Keep playback running on AirPlay/Bluetooth route changes.

### Systemwide
- [x] Eliminate Swift HAL IOProc allocations in encrypted path.
- [x] Propagate `buffer_frames` when rebuilding the pipeline.
- [x] Replace deprecated `OSAtomicAdd64` encrypted frame counter with stronger atomic semantics.

## P1 Quality Gaps

### Plugin UI / Performance
- [x] Cache EQ curve/control-point render data where safe.
- [x] Reduce upmixer render allocations where safe.
- [x] Cache `GpuiViewRegistry` instead of constructing per plugin render.
- [x] Re-enable the dev-API compile guard for release builds.
- [x] Replace dangerous `.unwrap()` calls in `three_panel_layout.rs`.
- [x] Add regression coverage for Channel Mute/Solo M/S/D click handlers.
- [x] Audit hardcoded pixels against `Ds` design tokens.

### Engine
- [x] Move plugin-host construction off the manager thread.
- [x] Reduce `AudioEngine::command_lock` contention.
- [x] Improve macOS real-time priority beyond QoS only.
- [x] Add/verify `NodeBuffer::clear()` stale-data safety.

### Client / Server
- [x] Improve media lookup from O(albums x tracks).
- [x] Add keep-alive support or explicitly document `Connection: close`.
- [x] Report actual MPD audio format instead of hard-coded `16:2`.
- [x] Add output-device server endpoints.
- [x] Add plugin-graph server endpoints.
- [x] Add plugin-preset server endpoints.
- [x] Avoid blocking decoder thread in `HttpMediaSource::reconnect()`.
- [x] Remove fixed 200 ms MPD stream sleep or justify it.
- [x] Gate/document Spotify and Tidal stubs/defaults.

### iOS / GPUI
- [ ] Honor left/right safe-area insets in landscape.
- [ ] Add phone-specific responsive scale reference.
- [ ] Fix `DensityMode::Expert` threshold for iPhones.
- [ ] Add Dynamic Type hooks.
- [ ] Add AirPlay route picker.
- [ ] Add memory-warning and low-power-mode handling.

### Systemwide
- [x] Replace destructive `pkill -9` daemon shutdown path.
- [x] Add IPC client idle timeout.
- [x] Add cross-language encrypted passthrough test.

### TUI
- [ ] Split `App` to satisfy struct-field budget.
- [ ] Split `RecordingTuiState` to satisfy struct-field budget.
- [ ] Make album-list truncation width-aware.
- [ ] Clamp modal dimensions on tiny terminals.
- [ ] Invalidate cached `image_protocol` on resize.
- [ ] Decide/defer mouse support.

## P2 / Explicit Deferrals

- [ ] Document federation merge/sync status.
- [ ] Rename or document AirPlay RAOP v1 scope.
- [ ] Document DLNA GENA/SCPD `501` scope.
- [ ] Add visual/snapshot regression tests for plugin UIs.
- [ ] Add iPhone-portrait EQ/upmixer breakpoint tests.
- [ ] Add Cast/AirPlay end-to-end protocol tests.
- [ ] Decide TLS scope for HTTP streaming sources and PCM stream server.
