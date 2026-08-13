# 0.5.30

- Process coefficient-stable LR24 two-way spans with a block kernel that
  selects output routing once and calls the scalar channel primitive directly;
  scalar-reference, callback-partition, and allocation tests protect exact DSP
  behavior.
- Add a Criterion block matrix for LR/FIR, 32--2048 frames, two/four bands,
  per-channel routing, 2/8 channels, and 63/511-tap FIR configurations.
- Expose `fir_memory_report()` so graph admission can account for coefficients,
  histories, multiband alignment, and scratch separately and in total.
- Stop retaining an unused two-way FIR and phase-coherent LR bank inside
  multiway FIR instances.

# 0.5.29

- Make LR multiway splitting phase coherent: every emitted band now traverses
  every Linkwitz-Riley split, and recombination is verified across the audio
  band rather than only at DC.
- Preserve stable crossover parameter identities by rejecting frequency
  crossings instead of sorting and silently rebinding automation lanes.
- Keep the 16-sample coefficient update phase across callback boundaries so
  smoothed automation is callback-partition invariant.
- Treat per-channel cutoff and routing changes as structural after initialize;
  they require a graph rebuild instead of hard-resetting live filters.
- Expose the compiled topology's exact runtime schema, use the crate version in
  plugin metadata, and classify FIR processing as convolution cost.

# 0.5.28

## Fixes

- Reject live FIR cutoff, extra-cutoff, and tap-count changes after
  initialization. These changes rebuild convolution histories and can change
  the graph's latency, so callers must rebuild the compiled graph instead of
  allocating/resetting DSP state from the control path.
- Reject per-channel cutoff writes at or above the current sample-rate Nyquist
  guard instead of silently clamping the requested value to a different
  operating frequency.

# 0.5.27

## Fixes

- Construction now enforces one to four bands, unique finite positive cutoff
  frequencies below the initial Nyquist limit, nonzero channels, and bounded
  overflow-safe odd FIR tap counts.
- Mode changes that would alter output channel layout are rejected and require
  a graph rebuild; lowpass/highpass changes remain safe because their port
  layout is identical.
- Processing uses checked frame/channel products and exact input/output buffer
  validation before indexing or advancing filter state.
- Initialization rejects zero sample rate and cutoffs invalid for the new
  Nyquist limit instead of silently changing the configured topology.

# 0.5.26

## Fixes

- **Review §1.3 multiband group-delay docs** (`math-iir-fir/src/lr4_crossover.rs`):
  `MultibandLr4Crossover` now documents that cascaded LR4 bands are not
  group-delay aligned and are not a phase-perfect linear-phase split.

- **Review §2.4 Biquad magnitude-response coverage** (`math-iir-fir/src/iir/biquad.rs`):
  Added `test_result_matches_complex_response_magnitude`, cross-validating the
  precomputed `result()` formula against `complex_response().norm()`.

- **Review §2.3 LogSmoother large-block overflow** (`math-dsp/src/smoothing.rs`):
  `LogSmoother::next_n` now advances in log space and clamps to the supported
  frequency range, avoiding `powi(n)` overflow during large offline-render
  blocks. Added `test_log_smoother_large_block_stays_finite`.

- **Review §3.2 Multiband LR4 copy-vs-swap** (`math-iir-fir/src/lr4_crossover.rs`):
  `MultibandLr4Crossover::process_frame` now swaps `carry` and `scratch`
  between stages instead of copying scratch into carry every frame.

- **Review §4.6 buffer bounds assertions** (`src/lib.rs`): Added debug assertions
  for input/output buffer sizes at `process()` entry to catch host contract bugs
  early in debug builds.

- **Review §1.2 denormal threshold status** (`math-dsp/src/simd.rs`): Confirmed
  the shared denormal threshold uses `f32::MIN_POSITIVE`, with coverage for
  subnormal zeroing and preserving tiny normal values.

# 0.5.25

## Added

- **Per-channel mode** for the RoomEQ factored graph: new constructor
  `CrossoverPlugin::new_per_channel(crossover_type, channel_frequencies_hz, channel_modes)`
  builds a plugin where each channel has its own LR24 crossover at its own
  cutoff with its own op mode. JSON params gain optional
  `channel_frequencies_hz: Vec<f32>` and `channel_modes: Vec<String>` fields;
  when present they switch the plugin into per-channel mode. `is_per_channel()`
  reports the mode.
- **`PerChannelOpMode` enum** with four variants:
  - `Lowpass` — channel runs the LP output of its LR24 cell.
  - `Highpass` — channel runs the HP output.
  - `Mute` — channel emits silence (used for source-less channels in routed
    bass-management graphs).
  - `Passthrough` — channel emits its input unchanged with no filtering or
    smoothing state. Used by destination-only channels in the RoomEQ factored
    graph so direct sub-feed signals reach the post-EQ stage without being
    silenced.
- Per-channel parameter ids: `channel_frequency_{N}` (Float, Hz) and
  `channel_mode_{N}` (String, one of `lowpass`/`highpass`/`mute`/`passthrough`).
  `set_parameter` validates the channel index and clamps frequency below
  Nyquist; `get_parameter` reads back the stored (clamped) value.

## Fixes

- `initialize()` in per-channel mode now writes the Nyquist-clamped frequency
  back into `channel_frequencies_hz`. Previously the clamped value was used
  to build the filter but the stored vec kept the original; `get_parameter`
  reported a frequency the plugin wasn't actually running at.
- `set_parameter("frequency")` and `set_parameter("mode")` now hard-error
  when the plugin is in per-channel mode. Previously they silently mutated
  unused global state, masking routing bugs in the host.
- `from_params(num_channels, params)` now hard-errors when
  `params.channel_frequencies_hz.len() != num_channels`. Previously it
  printed a warning to stderr and proceeded with the array length, producing
  a plugin whose `input_channels()` disagreed with the host's expectation.
- Clippy: `Result::and_then(|x| Ok(y))` → `.map(...)` in `from_params`.

# 0.5.24

## Added

- Added `LinearPhase`/`FIR` crossover mode backed by `FirCrossover`, including
  single-point and multiband processing, latency reporting, optional `fir_taps`
  configuration, and a reconstruction test proving low+high bands sum to the
  delayed input.

## Fixes

- **§2.1 `all_frequencies` sort order after parameter changes** (`src/lib.rs`): Re-sort and
  dedup `all_frequencies` after every `set_parameter` call that modifies a frequency. Previously,
  setting `frequency` above `frequency_2` left the vector unsorted, causing incorrect band
  overlap on the next `initialize()` call passed to `MultibandLr4Crossover::reinit`.

- **§2.2 `parse_extra_freq_index` aliasing** (`src/lib.rs:196-200`): Changed
  `idx.saturating_sub(2)` to `if idx >= 2 { Some(idx - 2) } else { None }`. Previously
  `"frequency_1"` would saturate to 0 and alias `"frequency_2"`. Now it correctly returns
  `None` for indices < 2, making the rejection explicit and safe against future validation
  changes.

- **§4.1 `crossover_type` parameter validated** (`src/lib.rs:new_multiway`): `new` and
  `new_multiway` now return an error for any `crossover_type` string other than `"lr24"` or
  `"lr4"` (case-insensitive). Previously passing `"LR12"` or `"BW18"` was silently accepted
  but LR4 was used, violating the API contract.

- **§4.2 `CrossoverMode::from_str` allocation removed** (`src/lib.rs:22-28`): Replaced
  `s.to_lowercase().as_str()` match with `eq_ignore_ascii_case` comparisons. Eliminates a
  `String` allocation on every call, making the hot path allocation-free.

- **§4.3 `reset()` snaps smoothers to target** (`src/lib.rs:336-341`): `reset()` now calls
  `smoother.reset(smoother.target())` for the primary and all extra frequency smoothers.
  Previously a mid-transition reset would leave the smoother's `current != target`, causing a
  discontinuous frequency jump (click) at the start of the next processed block.

- **§1.4 Nyquist clamp in `initialize()`** (`src/lib.rs:289`): Before passing frequencies to
  `Lr4Crossover::reinit` or `MultibandLr4Crossover::reinit`, each frequency is now clamped to
  `sample_rate * 0.5 * 0.99`. Prevents nonsense biquad coefficients when a stored frequency
  exceeds Nyquist at a low sample rate (e.g. 20 kHz crossover at 32 kHz SR).

- **§4.4 Test tolerance tightened** (`src/lib.rs:test_crossover_both_bands_sum_preserves_energy`):
  RMS energy-preservation tolerance tightened from 15 % to 1 % (`0.15` → `0.01`). Settle
  window increased from 2000 to 5000 samples to ensure full filter settlement.

## New Tests

- `test_all_frequencies_remain_sorted_after_primary_update` — verifies §2.1 fix for primary
  frequency above secondary.
- `test_all_frequencies_remain_sorted_after_extra_freq_update` — verifies §2.1 fix for
  `frequency_2` moved below primary.
- `test_parse_extra_freq_index_rejects_idx_less_than_2` — verifies §2.2 fix.
- `test_unsupported_crossover_type_returns_error` — verifies §4.1 fix; also confirms
  case-insensitive acceptance of `"lr24"`, `"LR4"`, `"LR24"`.
- `test_crossover_mode_from_str_is_case_insensitive` — verifies §4.2 fix.
- `test_reset_snaps_smoothers_to_target` — verifies §4.3 fix.
- `test_initialize_clamps_frequency_to_nyquist` — verifies §1.4 fix.

## Deferred (cross-crate or requires unsafe)

- **§1.1 Zipper noise from block-constant smoothing**: Fixing this requires per-sample
  coefficient updates inside the hot loop or a sub-block interpolation API from `LogSmoother`.
  Deferred to a follow-up that touches `math-dsp/smoothing.rs` and the crossover hot path
  together.

- **§1.2 `flush_denormals_inplace` threshold** (`math-dsp/simd.rs:909`): `DENORM_THRESHOLD`
  is `1e-30` instead of `f32::MIN_POSITIVE ≈ 1.18e-38`. Fix is a one-liner but lives in
  `math-dsp`, outside this crate's scope. Tracked for a `math-dsp` patch release.

- **§3.1 Per-frame `split_at_mut` in hot loop**: The suggested fix uses `unsafe` which is
  prohibited without explicit approval. Alternatively restructuring `MultibandLr4Crossover`
  to accept a flat output buffer would eliminate the per-frame slice construction without
  `unsafe`, but requires a `math-iir-fir` API change. Deferred.

- **§3.2 `MultibandLr4Crossover` copy-vs-swap**: The `copy_from_slice` → `mem::swap`
  optimization lives in `math-iir-fir/lr4_crossover.rs`. Deferred to a `math-iir-fir` patch.

- **§3.3 Redundant `flush_denormals_inplace` on x86_64**: Gating behind `#[cfg(target_arch = "aarch64")]`
  is harmless here but the root cause (§1.2 wrong threshold) is in `math-dsp`. Keeping the
  call as a safety net until §1.2 is fixed.

- **§3.4 `rebuild_cached_parameters` allocation on every set_parameter**: Not fixing in this
  pass to avoid scope creep; the schema-only-at-init refactor touches the parameter API
  contract. Tracked separately.

- **§4.5 / §4.6 Additional test coverage**: Stereo imaging, automation smoothness, multiband
  energy preservation, and buffer bounds `debug_assert`s are desirable additions; deferred to
  avoid enlarging this bugfix PR.

# 0.5.23

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Changes

- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
