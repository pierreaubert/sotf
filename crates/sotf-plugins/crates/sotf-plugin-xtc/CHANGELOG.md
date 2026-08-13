# 0.5.44

## Testing

- Strengthen the rapid-automation regression to wait for the real coalescing
  worker's final generation, compare its published filters with the final
  requested model, adopt and crossfade them without callback allocations, and
  verify that the superseded snapshot is reclaimed off the callback thread.

# 0.5.43

## Fixes

- Replace the saturated reclaimer's leak fallback with fixed-slot ownership
  backpressure. A full queue retains one retry snapshot and defers subsequent
  adoption/crossfade retirement until the background reclaimer makes room;
  callback destruction, blocking, allocation, and leaks are all avoided.

## Testing

- Added a deterministic queue-plus-retry saturation test that verifies bounded
  ownership, deferred acceptance, and eventual reclamation of every snapshot.

# 0.5.42

## Fixes

- Keep the last good filters when an enabled room-IR reload fails, and expose
  the generation-tagged worker error instead of silently publishing a
  reflection-free model.
- Validate pending filter width before publishing either the shared or cached
  snapshot.
- Reclaim completed crossfade and pending-update snapshots on a dedicated
  background thread rather than destroying their buffers in the audio callback.

## Testing

- Added post-enable room-IR failure, wrong-width publication, and background
  crossfade reclamation regressions.

# 0.5.41

## Fixes

- Coalesce rapid geometry/head automation through one latest-request worker per
  instance instead of spawning unbounded jobs on Rayon's global pool.
- Make source mode, HRTF, room IR, and roomEQ matrix structural; callback
  adoption accepts same-width updates only.
- Consolidate runtime, factory, serde/preset, and generated UI parameters on
  `XtcPluginParams`; align defaults and make shadow cutoff/slope effective.
- Decode integer and floating-point PCM room-IR WAVs and reject corruption,
  unsupported channel counts, and silently truncated long IRs.
- Replace the redundant integrated effort pass with a per-frequency
  loudspeaker-row power constraint.
- Separate validation oracles from production helpers and reject non-finite
  validation measurements.

## Testing

- Added coalescing-worker, structural-layout, parameter parity/preset,
  cutoff/slope response, PCM16/long-IR, effort-power, and non-finite validation
  regressions.

# 0.5.40

## Fixes

- Source-mode and HRTF-path parameter changes are now validated as a complete
  configuration before they are committed. Missing or invalid HRTF artifacts,
  and contradictory synthetic/HRTF state, return an error without leaving the
  visible parameters ahead of the active filters.

# 0.5.39

## Fixes

- Brown–Duda plant construction now applies the contralateral ITD once; the
  geometric path phase owns delay while Brown–Duda contributes magnitude.
- Source modes are checked at construction: synthetic mode cannot silently
  consume an HRTF path, and HRTF mode requires an explicit file.
- RoomEQ output width is declared configurable in the plugin catalog so graph
  metadata matches recommended matrices with more than two speaker outputs.
- Room IR loading now reports packet read errors instead of treating truncated
  input as EOF.

# 0.5.38

## Fixes

- Corrected host latency reporting to one full FFT frame. Streamed impulse
  measurements showed that the previous `fft_size - hop_size` value
  under-compensated at host block sizes smaller than the STFT hop.

# 0.5.37

## Fixes (from code review 2026-05-11)

### Fixed
- **Air absorption documentation and coverage** (`reflections.rs`, `tests.rs`): Kept the
  conservative quadratic dB/m fit after checking the review's proposed `5e-7 * f^1.5`
  replacement, and added a regression assertion for the documented 8 kHz / 5 m value.
- **Denormal regression test** (`tests.rs`): Corrected the test to count only IEEE-754
  subnormal values below `f32::MIN_POSITIVE`; small normal values such as `1e-35`
  are intentionally preserved by `flush_denormals_inplace`.
- **Room reflection pressure coefficient** (`reflections.rs:119`): amplitude now uses
  `sqrt(1 - wall_absorption)` (pressure reflection coefficient) instead of
  `(1 - wall_absorption)` (energy coefficient). For α = 0.3 the reflected amplitude
  changes from 0.70 to 0.84; results in correctly estimated reflected energy.
- **HRTF sample-rate guard** (`lib.rs:119`): SOFA files without a `DataSamplingRate`
  attribute are now rejected with an error rather than silently accepted, preventing
  spectral feature shifts (e.g., a 44.1 kHz SOFA at 48 kHz shifts all notches by ~8.8 %).
- **`compute_2x2_inverse` determinant guard** (`filters.rs:836`): threshold changed from
  absolute `1e-10` to relative `1e-10 * diag`, preventing false-positive singularity
  detection when transfer-function magnitudes are small (e.g., deep notches with |H| ~ 1e-3).
- **Frequency-domain crossfade** (`lib.rs:process_stft_frame`): stereo and speaker-mode
  crossfades now blend prev/current filters per frequency bin and run a single IFFT per
  channel, halving IFFT cost from 4 to 2 per hop during crossfades. Correctness is
  preserved by IFFT linearity.
- **`latency_samples`** (`lib.rs:1809`): now reports `fft_size - hop_size` (= 3/4 of
  fft_size for 75 % overlap) instead of `fft_size`, saving `hop_size` samples of
  unnecessary host-side latency compensation.

### Deferred
- **Brown-Duda `alpha_min` comment** (review §1.1): review says to "correct or document";
  the function already documents this as a simplified approximation. Adding published
  curve comparison tests deferred as cross-crate benchmark work.
- **`explicit_delay` unity amplitude at LF** (review §1.4): the ~0.3 dB error for
  typical geometry is below audibility threshold; deferred.
- **Condition-number double-counting of beta frequency boosts** (review §2.3): requires
  architectural changes to the filter computation pipeline; deferred.
- **COLA output_scale correctness** (review §2.4): requires verifying `generate_hann_window`
  window type in `sotf-host`; deferred as cross-crate.
- **Dual parameter struct consolidation** (review §4.1, §4.2, §4.3): cross-crate refactor;
  deferred.
- **`compute_room_params_hash` over-invalidation** (review §4.4): medium risk change to
  caching logic; deferred.
- **`build_reflection_data_ir` missing end window** (review §4.5): enhancement; deferred.
- **STFT COLA perfect-reconstruction test** (review §4.6): deferred.
- **`flush_denormals_inplace` SIMD** (review §4.7): optimization in `sotf-host`; deferred.

# 0.5.36

- Added `source_mode = "roomeq_recommended"` with
  `recommended_matrix_file`, allowing the plugin to load RoomEQ
  `recommended_xtc_matrix.json` FIR artifacts instead of recomputing filters
  from synthetic geometry or HRTF data.
- RoomEQ recommended matrices are validated before activation: invalid JSON,
  sample-rate mismatches, and missing filters are rejected instead of silently
  falling back to synthetic filters; matrices with two or more speaker outputs
  expose the matching plugin output channel count.
- RoomEQ recommended processing now maps stereo ear-intent input to N speaker
  outputs with dynamic overlap-add buffers, bypass handling, limiter coverage,
  and no stereo-only AutoGain assumptions for multichannel matrices.
- Fixed RoomEQ matrix tap orientation for the off-diagonal paths and added
  regression coverage for asymmetric recommended matrices.
- Async filter recompute now builds room/HRTF/filter data off-thread, publishes a pending generation, and starts the crossfade only when process() adopts the completed update.
- Large process calls are chunked through preallocated scratch buffers, so the hot path no longer resizes for offline-sized blocks.
- Unwritten output tail is zeroed before return.
- Brown-Duda now returns a complex head-shadowing gain and applies its phase in both symmetric and asymmetric plant paths.
- Added coverage proving Brown-Duda contributes a phase term.
