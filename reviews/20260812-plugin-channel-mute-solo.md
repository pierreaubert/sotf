# Channel Mute/Solo plugin code review — 2026-08-12

## Remediation status

All P0-P3 findings are fixed in `0.5.63` through `0.5.65`:

- **Fixed:** channel smoothers advance once per sample, producing the same exponential transition
  for every callback partition.
- **Fixed:** fallible construction, the plugin bridge factory, and the shared facade factory
  reject zero channels, non-finite/out-of-range dim gain, and invalid fade time.
- **Fixed:** ordinary processing uses checked sample counts and returns errors for short buffers,
  while leaving oversized tails untouched.
- **Fixed:** bulk channel-state updates reject mismatched lengths instead of silently succeeding.
- **Documented:** fade time is a one-pole time constant, not an exact-duration linear ramp.
- **Fixed:** transport reset preserves in-flight mute, solo, dim, enable, and disable fades; a
  sample-by-sample reference test detects any reset discontinuity.
- **Fixed:** settled smoothers use a static per-channel block kernel, with explicit path
  instrumentation in tests, and compile metadata is stateful only during a transition.
- **Fixed:** adapter updates defer descriptor refresh; schema discovery reuses cached descriptor
  storage and immutable IDs/names, verified by pointer/capacity and value-refresh assertions.
- **Fixed:** the dead `param_specs.rs` and duplicate default modules were removed. Runtime,
  serde, schema ranges/defaults, and documentation now derive from `params::PARAMS` and define
  `fade_ms` as a one-pole time constant.

Follow-up verification: the shared-factory regression
`channel_mute_solo_facade_factory_validates_constructor_contract` rejects an
out-of-range dim gain and zero channels.

## Findings

### P1 — Block-linear “smoothing” is grossly block-size dependent and does not approximate the configured exponential fade

The plugin advances each `Smoother` directly to its end-of-block exponential state with `next_n(num_frames)`, then replaces the samples between the endpoints with a straight line (`crates/sotf-plugins/crates/sotf-plugin-channel-mute-solo/src/lib/channel_mute_solo_plugin.rs:472-510`). The comment claims less than 0.3% error for a 512-frame block and 5 ms time constant. At 48 kHz, 512 frames is 2.13 time constants: a 1→0 exponential is about 0.344 halfway through, while the endpoint-linear ramp is about 0.559—roughly 62% high relative error (0.215 absolute gain). Different host block sizes therefore produce materially different envelopes and mute/solo timing.

This is audible, invalidates automation/block-boundary invariance, and can make long offline blocks fade over the whole block even when the requested 5 ms fade should be essentially complete. Apply the smoother per sample, use its closed-form exponential at each sample, or generate a multiplicative ramp whose ratio matches the one-pole recurrence. Add sample-by-sample reference comparisons across block partitions (1, 32, 128, 512, 4096 frames), checking identical concatenated output and the documented time constant.

### P1 — Factory/preset construction bypasses the advertised dim/fade ranges and finite-value checks

`from_params` passes `dim_gain_db` and `fade_ms` straight to public infallible setters (`channel_mute_solo_plugin.rs:90-105`, `159-177`). Those setters accept NaN, infinities, positive dim gain, arbitrarily large attenuation, and negative/infinite fade time. The adapter parameter path validates schema values, but factory construction uses deserialized `ChannelMuteSoloParams`, so presets bypass it.

Invalid dim gain can amplify rather than dim or propagate non-finite samples; invalid smoothing can create undefined transition semantics. Centralize validation for construction and runtime updates, make invalid construction fallible, and test the factory path for `[-60, 0] dB`, `[0, 100] ms`, NaN/infinity, and both endpoints.

### P2 — The release process path turns a buffer-contract error into an indexing panic

`process_in_place` checks exact length only with `debug_assert_eq!` (`channel_mute_solo_plugin.rs:447-461`). In release builds, an undersized buffer proceeds to frame slicing at lines 503-504 and panics; `num_frames * channels` can also overflow before the assertion. By contrast, the compiled entry point uses checked multiplication and returns descriptive errors (`channel_mute_solo_plugin.rs:517-548`).

Use the same checked sample-count and bounds validation for the normal path. Decide whether oversized tails are permitted and process only the active prefix consistently. Add release-behavior tests for zero frames, overflow, short buffers, and oversized buffers.

### P2 — Bulk `channel_states` updates silently report success when the length is wrong

`set_channel_states` performs an update only when `states.len() == channels`; otherwise it silently does nothing (`channel_mute_solo_plugin.rs:145-152`). `apply_values` parses JSON, calls this method, and then returns `Ok(())` regardless (`channel_mute_solo_plugin.rs:407-435`). A syntactically valid preset/automation update with the wrong count therefore appears accepted but leaves old routing state active. This differs from `from_params`, which explicitly truncates/pads mismatched state arrays (`channel_mute_solo_plugin.rs:96-100`).

Make the bulk setter return `Result` and use one documented mismatch policy everywhere—prefer rejecting live updates because silently changing channel mapping is risky. Test short, long, and empty arrays through the actual parameter adapter.

### P2 — Reset snaps to routing targets rather than preserving a click-free transition contract

`reset` calls `reset_smoothers_to_current`, which actually resets every smoother to its computed target (`channel_mute_solo_plugin.rs:265-275`; trait reset implementation in the same file's lifecycle section). A transport seek/reset during a fade therefore jumps immediately to zero/dim/unity. That may be suitable for initial preset construction, but it conflicts with the plugin's blanket “all state changes use a fade” documentation. The integration reset test only checks finiteness at the default all-unity state and cannot observe the jump.

Separate initialization/preset-load priming from transport reset semantics. Document whether reset must snap or restart transitions and add first-sample tests for mute, solo, dim, enable/disable, and mid-fade reset.

### P3 — Stable states still pay full per-frame ramp work and are always marked stateful when fade is configured

The only fast bypass is disabled plus all-unity (`channel_mute_solo_plugin.rs:463-470`). Enabled plugins whose gains have converged still copy start/end arrays, call `next_n`, and execute a frame×channel interpolation even though `start == end`. Compile metadata also declares the plugin stateful solely from `fade_ms > 0`, not whether a transition is active (`channel_mute_solo_plugin.rs:302-310`). This leaves a common monitoring configuration on the slow path indefinitely.

Track whether all smoothers are settled. Apply a whole-buffer static per-channel gain kernel when settled, and report time-invariant metadata when host recompilation/state synchronization permits it. Benchmark 2/6/8/16/32 channels and typical block sizes. This is a performance recommendation; the current loop is allocation-free.

### P3 — Parameter cache rebuilding still allocates and serializes after every adapter update

The changelog says JSON/cache rebuilding is deferred, but `apply_values` calls `rebuild_cached_parameters_if_dirty()` unconditionally before returning (`channel_mute_solo_plugin.rs:363-435`). Each toggle therefore formats three strings per channel, builds a new descriptor vector, and serializes all states (`channel_mute_solo_plugin.rs:199-249`). The work is control-thread-safe under the trait contract, but the claimed lazy optimization is not realized for normal adapter calls and can be noticeable in high-channel-count automation/UI interaction.

Leave the cache dirty until schema is requested, cache immutable per-channel IDs/names once, and separate current values from descriptors. Add an allocation-count test for repeated state mutation (outside the audio callback) if UI responsiveness matters.

### P3 — The documentation and dead parameter source disagree with the active schema/package

`src/param_specs.rs` is not module-declared and is dead duplicate metadata. The package is version `0.5.61` while the changelog claims `0.5.62`. `USAGE.md` says the smoother interpolates linearly and presents 5 ms as the fade duration, whereas implementation uses exponential endpoints plus block-linear interpolation, and the configured value acts as a time constant rather than a completion duration. These discrepancies obscured the P1 behavior above.

Remove the dead spec file, derive all defaults/ranges from `params::PARAMS`, align package/release notes, and define fade time precisely (one-pole tau, settling time, or exact-duration ramp).

## Algorithm and realtime assessment

The priority logic is internally consistent: any solo makes soloed channels unity and all others zero; absent solo, mute overrides dim (`channel_mute_solo_plugin.rs:252-289`). This also means a channel marked both solo and mute remains audible, matching the documented “solo takes priority” rule. Layout and frame count are preserved, latency is zero, state is per-channel, and the hot path performs no allocation, locks, serialization, or logging. QA explicitly counts audio-path allocations.

The algorithm is O(frames × channels). Preallocated `start_gains` and `cached_gains` are appropriate. The SIMD helper is used only for the single-frame case; the main multi-frame ramp is scalar. Correct block-invariant smoothing should come before SIMD optimization. There is no separate bypass parameter beyond `enabled`; disabling intentionally fades toward unity rather than bypassing instantly.

## Scope reviewed

Read every plugin-owned file: `AGENTS.md`, `README.md`, `USAGE.md`, `UI.md`, `CHANGELOG.md`, `Cargo.toml`, all files under `src/` including both test modules and the undeclared `param_specs.rs`, `tests/integration.rs`, and `bin/qa_channel_mute_solo.rs`. Also checked facade/factory/catalog wiring, host smoother and parametric in-place contracts, compiled-op behavior, and TokenSave test-risk results. No source-code changes were made.

## Existing strengths

- Per-channel state and DSP scratch are preallocated; QA verifies zero allocation in processing.
- Solo/mute/dim priority is simple, deterministic, and well covered by known-output tests.
- Dynamic parameter IDs, JSON state, defaults, serde round trips, channel mismatch construction, and compiled processing have substantial tests.
- Sample-rate initialization retunes all smoothers and target updates are centralized.
- The disabled-and-settled unity fast path avoids unnecessary audio work.

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-channel-mute-solo
cargo test -p sotf-plugins --test all_plugins_dsp_matrix
cargo clippy -p sotf-plugin-channel-mute-solo -- -W warnings
cargo check -p sotf-plugins
```
