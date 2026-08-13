# Delay plugin code review — 2026-08-12

## Remediation status

All P1-P3 findings are remediated and regression-tested in 0.5.10:

- Exact zero-delay routing: `zero_delay_is_sample_exact_wet_passthrough` and
  `per_channel_zero_and_one_sample_delays_are_exact`.
- Validated constructors/factory ranges: `factory_params_reject_invalid_values`,
  `fallible_constructor_rejects_every_invalid_scalar_boundary`, and the factory
  coverage in the universal facade.
- Buffer and release-state safety: `process_rejects_wrong_buffer_length_without_advancing_state`
  plus zero-channel, overflow, and corrupted-state unit cases.
- Tape-style modulation contract: documentation and
  `lfo_modulation_has_documented_tape_pitch_excursion_without_clicks` quantify
  Doppler excursion and maximum adjacent-sample discontinuity.
- Boundary LFO behavior: `test_effective_delay_samples_preserves_feasible_half_cycle_near_min_delay`
  and `effective_delay_is_continuous_at_both_boundaries`.
- LFO channel policy is explicitly coherent: one phase is shared across
  channels to avoid unintended image rotation; independent phases are outside
  this delay effect's topology.
- Click-free allpass changes: `allpass_live_changes_are_smoothed` verifies the
  enable and coefficient ramps while the allpass state remains continuously active.
- Bounded memory: per-channel routing uses an explicit automation maximum;
  `short_per_channel_delay_uses_bounded_memory_at_192khz` covers 12 channels.
- Conservative compile metadata: `compile_metadata_is_conservative_for_every_delay_state`.
- Allocation-free realtime writes: `realtime_parameter_writes_do_not_allocate`;
  cached schemas are built once and no longer rebuilt by `apply_values`.
- Integer-tap performance: `integer_delay_read_ignores_fractional_guard_samples`
  proves exact taps bypass the fractional interpolator.

Pitch-preserving delay-time changes remain a separate optional feature rather
than a correctness fix: this plugin intentionally exposes tape-style motion and
now documents/tests that behavior without claiming pitch preservation.

## Findings

### P1 — Zero-delay per-channel routes are forced to one sample, but latency remains reported as zero

Per-channel parameters explicitly allow 0 ms and are used for RoomEQ route alignment (`crates/sotf-plugins/crates/sotf-plugin-delay/src/lib/delay_plugin.rs:91-136,142-170`). Yet `effective_delay_samples` clamps every delay to at least one sample (`delay_plugin.rs:242-262`). A nominal zero-delay route is therefore delayed, including the all-zero configuration, while compile metadata reports zero latency (`delay_plugin.rs:273-275`). This adds unreported common latency and violates exact route-delay semantics.

Implement an explicit zero-delay path (with carefully defined feedback behavior), or add/report the unavoidable common sample and compensate it in graph construction. Test impulses for `[0, 0]`, `[0, 1 sample]`, and mixed RoomEQ delays through a complete graph, including reported latency.

### P1 — Constructors and factory presets bypass declared ranges and finite checks

`new`, `new_per_channel`, and `from_params` accept raw delay, feedback, mix, LFO, and per-channel values (`delay_plugin.rs:50-136,175-216`). Only allpass coefficient is clamped; NaN survives that clamp. Negative, non-finite, >5 s delay, feedback above unity, and invalid mix can reach smoothers/DSP. Runtime setters are schema-validated by the trait wrapper, but factory JSON follows this unchecked path. Values beyond 5 s can also use power-of-two buffer headroom beyond the advertised maximum.

Make construction fallible and validate with the same authoritative specs as runtime writes. Reject non-finite values and enforce delay, feedback, mix, LFO, allpass, and channel-count constraints. Add universal-factory tests for every boundary and NaN/infinity.

### P1 — `process_in_place` can panic on short buffers or inconsistent release-mode state

The processing loop indexes `buffer[frame * channels + ch]` without checking the context-derived required length (`delay_plugin.rs:477-560`). The per-channel parallel-array invariant is only a debug assertion (`delay_plugin.rs:484-491`), so release builds proceed to indexing if internal state drifts. Frame/channel multiplication is unchecked.

Reject zero channels, use checked multiplication, validate the input buffer and per-channel arrays in all builds before modifying state, and return descriptive errors. Add short-buffer, oversized-context, zero-channel, and invariant-corruption tests that demonstrate no panic.

### P2 — Delay automation is “smooth” but not artifact-free as documented

The delay tap moves continuously through a 4-point Lagrange reader (`delay_plugin.rs:220-262,511-546`). This avoids hard discontinuities, but changing delay necessarily resamples time and creates Doppler pitch shift; the documentation repeatedly promises artifact-free/no-pitch-glitch automation and also incorrectly calls the interpolation linear (`USAGE.md`, “Delay Line” and “Smooth Parameter Changes”). A 50 ms smoother merely controls the glide rate.

Document the Doppler behavior accurately. For pitch-preserving time changes, use dual read heads with a windowed crossfade (or expose selectable tape/clean modes). Add spectral tests of delay automation on a steady sinusoid and quantify pitch excursion/click energy.

### P2 — LFO headroom scaling collapses modulation near either delay boundary

`effective_delay_samples` limits the entire signed LFO depth to the smaller of upward and downward headroom (`delay_plugin.rs:242-262`). Near the minimum or maximum delay, one side has zero headroom, so `max_lfo_depth` becomes zero and modulation is disabled in both directions. This avoids asymmetric clipping but makes depth control unexpectedly vanish rather than preserving the feasible half-cycle.

Choose and document a boundary policy: shift the modulation center, reduce depth explicitly and expose the effective value, or use a smooth bounded mapping. Test modulation depth/frequency over base delays near both boundaries and ensure no discontinuous change in effective depth.

### P2 — Enabling/disabling or retuning the feedback allpass changes state discontinuously

Allpass enable uses existing state, disable resets it immediately, and coefficient updates replace coefficients in-place without smoothing (`delay_plugin.rs:358-389`; `src/lib/allpass_state.rs:24-38`). These live parameters can create feedback-tail discontinuities/clicks. Factory construction also accepts NaN coefficient state.

Smooth stable coefficients and crossfade bypass/active feedback paths, or classify changes as non-automatable. Test changes during a long feedback tail for finite bounded output and limited discontinuity energy.

### P2 — Allocation scales with a rounded-up five-second buffer even for short delays

Initialization allocates `next_power_of_two(5 s + guard) * channels` samples regardless of configured maximum need (`delay_plugin.rs:429-452`). At 192 kHz this rounds to 1,048,576 samples per channel—about 48 MiB for 12 channels—plus reallocation/deallocation when sample rate changes. Power-of-two wrapping is fast, but memory footprint/cache pressure is substantial for common short RoomEQ delays.

Size capacity from an explicit maximum automation range or use a segmented/ring allocation that avoids a large power-of-two overshoot. Keep allocation outside processing and benchmark memory/cache behavior at 12 channels and high sample rates.

### P2 — The static linear metadata understates time variation and feedback behavior

Compile metadata always describes a linear transform with a fixed shape (`delay_plugin.rs:273-275`), while active LFO, delay smoothing, feedback, and allpass changes make it stateful and potentially time-varying. Whether the exact flags currently prevent fusion/reordering depends on host interpretation, but metadata should conservatively express the active state so future compiled plans cannot commute or combine it incorrectly.

Remediated in 0.5.9. Delay remains classified as linear, but metadata now marks
it as not block-invariant, stateful, non-gain-absorbable, and an explicit
scheduling/fusion boundary. Regression coverage exercises static, automated,
LFO, feedback, allpass, and per-channel states.

### P3 — Dynamic per-channel parameter caching allocates on each parameter update

Every `apply_values` call rebuilds the complete parameter vector and formats every per-channel ID/name (`delay_plugin.rs:142-170,323-405`). This is not in the sample loop, but it creates avoidable control-thread churn and is unsafe if hosts deliver automation setters on the audio thread.

Prebuild IDs/names/schema once and update only values, or keep setters off the callback by contract. Extend allocation tests to runtime parameter writes.

## Realtime allocation and performance assessment

The steady-state sample loop is allocation-, lock-, log-, and I/O-free. Buffers, allpass state, and smoothers are preallocated; deinterleaved delay storage gives contiguous per-channel rings, power-of-two masks avoid division, FTZ/DAZ is enabled, and output denormals are flushed. Cost is O(frames × channels), with four ring reads, cubic interpolation, optional allpass, and a sine per frame when LFO is active. The main realtime hazards are unchecked buffers and live state transitions, not ordinary processing allocation.

## Scope reviewed

Read every plugin-owned file without omission: all five Markdown documents, `Cargo.toml`, all seven source/parameter modules, every unit/integration/property/basic test, and `bin/qa_delay.rs`. Also checked facade/factory/catalog/bridge/AB-compare call sites, parametric adapter validation, host smoother and compiled-metadata contracts, realtime allocation coverage, and RoomEQ per-channel usage. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-delay`: 63 tests passed across five suites.
- TokenSave file inventory/test-risk preceded direct reads; it identified `effective_delay_samples` as the highest-risk untested helper.

## Suggested verification after fixes

- Run crate and realtime-allocation suites plus the QA binary in scalar/per-channel modes.
- Add impulse and automation tests at 44.1–192 kHz, mono–12 channels, and multiple callback partitions.
- Measure feedback-tail stability, modulation spectra, transition clicks, and high-rate memory/CPU.
- Verify complete RoomEQ graph delay alignment and reported latency.
