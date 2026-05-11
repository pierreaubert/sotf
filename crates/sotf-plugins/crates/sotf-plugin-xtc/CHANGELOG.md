# 0.5.38

- **Critical bug fix**: Added missing `#[test]` attribute on `test_asymmetric_spectral_norm_scales_columns_not_rows`. The regression test for the asymmetric spectral normalization fix was dead code and never executed.
- **Critical bug fix**: Room reflection amplitude now uses the pressure reflection coefficient `sqrt(1 - alpha)` instead of the energy absorption coefficient `(1 - alpha)` directly. This corrects reflected energy levels for typical wall absorptions (e.g. alpha=0.3 now yields 0.84 instead of 0.70).
- **Major bug fix**: Brown-Duda head-shadowing `alpha_min` now implements the published rigid-sphere approximation `1 / sqrt(1 + (mu/2)^2)` instead of the previous ad-hoc linear fit `1 / (1 + 0.5*mu)`. The comment and implementation are now consistent.
- Added regression tests for all three fixes.


# 0.5.37

- **Bug fix**: `process()` now returns `context.num_frames` instead of `output_pos`. STFT plugins must return the requested frame count to prevent ring-buffer underruns in the host. Unproduced frames are zeroed.
- **Bug fix**: `beta_low_freq_boost` and `beta_high_freq_boost` parameters were exposed as UI knobs but had no effect on filter computation. They are now wired into the condition-number based beta regularization via frequency-dependent sigmoid multipliers.
- **Bug fix**: Mono room IR files incorrectly cloned the ipsilateral transfer function to the contralateral path. The contralateral path now derives from the ipsilateral response with a head-shadowing model (frequency-dependent attenuation + ITD delay), producing physically plausible reflection compensation for mono IRs.
- Added regression tests for all three fixes.


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
