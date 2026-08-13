# 0.5.22

## Fixes

- Replace circular sliding-window HRTF multiplication with causal hop-partition
  overlap-add and reject SOFA IRs beyond the verified linear-convolution
  capacity; streaming direct-FIR regressions cover irregular callbacks, FFT
  boundaries, and IR lengths from one sample through the capacity limit.
- Preserve per-input reflection ownership so silent configured channels cannot
  add another source's room paths; reflection rendering is explicitly
  broadband HRTF-derived ILD rather than claiming unused spectral/ITD filters.
- Consume complete oversized callbacks instead of dropping samples beyond the
  internal input-buffer fill.
- Retire replaced HRTF states on a bounded background queue rather than
  destroying their nested allocations in the audio callback.
- Unify construction and runtime state for crossfade and late-reverb controls,
  canonicalize the public SOFA key, reject unsupported channel counts, and
  align the factory catalog with the exact shared speaker layouts.
- Make runtime SOFA replacement transactional and force the rebound worker to
  converge to the current head target.
- Reject empty, inconsistent, and non-finite diffuse-field datasets; use
  log-frequency smoothing, level-relative regularization, common-ear
  normalization, and a final +12 dB boost ceiling.
- Reuse load-time diffuse EQ during head updates and defer repeated HRTF-state
  reclamation off the callback.

# 0.5.21

## Fixes

- Gate startup output with the reported FFT latency, so callback sizes below,
  equal to, or above one FFT frame observe the same causal delay instead of
  draining a frame using future samples from the current callback.
- Suppress finite-precision overlap-add residue below `-114 dBFS` after the
  STFT tail has drained, preserving an exact-zero silence contract.

# 0.5.20

## Fixes

- Corrected the sign of the first VBAP barycentric coordinate and use affine
  unit-sum HRTF interpolation weights, preserving constant HRTF and ITD fields.
- Configure the late-reverb FDN at the engine sample rate during initialize and
  clear its complete delay/absorption state on reset.
- Retry dropped head-tracking updates by advancing the last-requested angles
  only after a successful worker enqueue.
- Stop/rebind the head-tracking worker during runtime SOFA replacement and
  restore documented default filters when the SOFA path is cleared.
- Rank anthropometrically labelled SOFA candidates ahead of generic fallbacks.
- Use conservative compile-boundary metadata for time-varying/nonlinear modes.
- Activated the previously disconnected HRTF resampling/VBAP regression module.

# 0.5.19

## Fixes (from code review 2026-05-11)

- **AL11 (advisory)**: `BinauralDecoderPlugin::new` now rejects non-power-of-two
  `fft_size` values before constructing FFT buffers, matching the output-accumulator
  mask assumptions. Regression: `test_constructor_rejects_non_power_of_two_fft_size`.

# 0.5.18

## Fixes (from code review 2026-05-11)

- **A1 (critical)**: Fixed reflection panning formula off by 45°. The old formula
  `p = (az + π/4) * 0.5` centred the pan law at az=π/4 (45° right), so frontal
  sources were imaged hard-left. Replaced with standard constant-power sine-law:
  `pan = az.sin(); left = sqrt((1-pan)/2); right = sqrt((1+pan)/2)`. Fixed in
  `src/room.rs` for ISM 1st-order (`add_image_reflections`), 2nd-order loops, and
  SSIR-based reflections (`ssir_result_to_reflections`).

- **A3 (major)**: Removed arbitrary -3 dB (`FRAC_1_SQRT_2`) attenuation from LFE gain
  in `src/filter.rs:compute_lfe_filter`. LFE channels are calibrated +10 dB hotter
  than mains (ITU-R BS.775-3); the factor made subwoofer output too quiet. The
  user-adjustable `lfe_level` parameter is now the sole gain control beyond 1/r
  distance attenuation.

- **AL1 (critical)**: Fixed duplicate second-order ISM reflections. Mirroring image
  source A→B and B→A produces the same physical position, so the old code emitted
  both, boosting those paths by +6 dB. Added a `HashSet` of quantised (1 cm)
  3D positions to deduplicate before adding reflections (`src/room.rs`).

- **AL2 (major)**: Added bounds check for reflection delay line overflow. Delays
  exceeding the 16384-sample buffer (≈341 ms at 48 kHz) are now clamped with a
  `log::warn` in `initialize()` (`src/lib.rs`). Prevents wrap-around corrupting
  early-reflection timing for large rooms or long SRIRs.

- **AL3 (major)**: Removed `enable_optimization` parameter from the public UI.
  The field existed and was serialized but had no DSP effect. The field is kept on
  the struct for backward-compatible deserialization of old presets but is no longer
  listed in `parameters()`. Indices of all subsequent PARAMS entries shifted down.

- **AL4 (major)**: Removed `headphone_eq_enabled` parameter from the public UI.
  Same situation as AL3 — exposed but unimplemented. Kept as a serde stub only.

- **AL5 / P1 (major)**: Fixed synchronous file I/O in head-tracking path.
  `recompute_hrtf_for_head_angles` previously called `SofaFile::load()` and
  `resample_sofa()` on every 0.5° head movement inside the audio-thread path
  (causing potential dropouts). Now uses the `SofaFile` already cached in
  `BinauralState::_hrtf_data`, eliminating all disk I/O during head tracking.

- **AL6 (minor)**: FFT errors are now logged via `log::error!` instead of being
  silently swallowed with `.ok()`. All six `fft_r2c.process_with_scratch` and
  `fft_c2r.process_with_scratch` call sites in `src/lib.rs` updated.

- **AL7 (minor)**: Fixed VBAP out-of-triangle gain boost. When the target was
  outside the nearest-three triangle, weights were clamped and renormalized to
  sum 1.0, then an additional energy normalization `scale = 1/sqrt(energy)` was
  applied. For clamped weights like `[0, 0.5, 0.5]` this gave `scale ≈ 1.41` (+3 dB).
  Fixed: energy normalization is skipped for out-of-triangle (clamped) targets.

- **P2 (major)**: `process_audio_block` now uses `self.state.load()` (borrow guard)
  instead of `self.state.load_full()` (Arc clone) to avoid an atomic refcount
  increment on every audio-block hop. `Arc::clone(&new_state)` is only called
  when a state change is detected.

## Deferred

- **A2**: Near-field shadowing underestimates attenuation by ~20 dB. Implementing
  the full Woodworth-Schlosberg spherical-head model requires significant acoustic
  research and is deferred to a dedicated task.
- **A4**: Missing synthesis window in STFT (spectral leakage on LFE/transients).
  Deferred — requires retuning `output_scale` and verification against XTC plugin.
- **A5**: Frequency-independent diffuse-field EQ regularization. Deferred.
- **AL8**: LFE circular convolution. Deferred (advisory; negligible at fft_size≥512).
- **AL9**: HRTF normalization overly conservative. Deferred.
- **AL10**: Runtime exposure of `diffuse_field_eq` / LFE params. Deferred.
- **P3**: Cache-unfriendly delay-line access pattern. Deferred.
- **P4–P6**: Per-hop copy_within, ir_to_freq allocations, rebuild_cached_parameters
  allocations. Deferred (minor, not real-time critical in practice).
- **I1–I5**: Passthrough amplitude test, biquad LFE, RTPGHI gamma discrepancy.
  Deferred.

# 0.5.17

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added spectral crossfade to binaural morphing

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fix Phase 4 review: FDN param corruption, rebuild_cached_parameters, spatial ordering

## Changes

- Complete Phase 4: adaptive threshold, denoiser DSP, FDN reverb, binaural preview
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
