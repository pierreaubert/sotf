# 0.5.37

## Fixes (from code review 2026-05-11)

### Fixed
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
- **Air absorption formula** (review §1.2): the review's suggested replacement
  `5e-7 * f^1.5` was verified to over-estimate more severely than the current quadratic
  formula at 1 kHz. The current formula (`0.001 * (f/1000)^2`) overestimates by ~1.8× at
  4 kHz vs ISO 9613-1, which is within acceptable bounds for room-scale distances. The
  doc-comment was updated to document the known over-estimation and its practical
  inaudibility.
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
