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
- **AL11**: `fft_size` not validated as power of 2. Deferred (advisory).
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
