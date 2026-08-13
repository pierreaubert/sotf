# 0.5.8

## Fixes

- Spatial magnitude-squared coherence now smooths both auto-power spectra with the same
  time constant as the complex cross-spectrum. Silence explicitly resets the estimator.
- Spatial processing now has a documented channel contract: stereo/front, side, and rear pairs
  are processed in standard 5.1/7.1 layouts; unmatched centre and LFE channels are untouched.
- Noise-profile capture derives its frame target from sample rate and STFT hop, keeping capture
  duration at approximately one second in both 512- and 2048-point modes.
- Small-FFT failures propagate through the plugin result instead of panicking on the audio path.
- QA now covers zero-allocation processing and callback timing across mono, stereo, 5.1, 7.1,
  both FFT sizes, regular/irregular callback sizes, and default/all-optional-mode configurations.

## Tests

- Added amplitude-modulated coherence, multichannel topology/LFE, profile state-machine,
  multi-rate capture-duration, small-FFT fault-injection, and documentation-value regressions.

# 0.5.7

## Fixes

- Removed the unsafe aliased slice in the FFT path by passing explicitly disjoint configuration,
  FFT state, and input scratch to the forward transform helper.
- Added fallible construction validation for all serialized numeric controls and channel count;
  both plugin factories now reject invalid presets before allocating DSP state.
- Low-latency and multi-resolution topology controls now reject live changes instead of reporting
  a mode that does not match the allocated FFT state and latency.
- Multi-resolution analysis now advances only to each matching large-frame boundary, making output
  independent of host callback partitioning and preventing future-sample analysis.
- Live MCRA control changes now update both large- and small-FFT estimators.
- Monitoring reports captured-profile use only when a profile actually exists.

## Tests

- Added invalid-construction, structural-topology, and multi-resolution block-invariance regressions.

# 0.5.6

## Bug fixes

- **lib.rs:925** — `process_in_place` now uses `math_audio_dsp::simd::ScopedFtz`
  instead of open-coded `stmxcsr`/`ldmxcsr` restore blocks. Early validation
  errors now leave FTZ/DAZ cleanup to RAII, avoiding fragile duplicated restore
  paths. Added a regression test on x86_64/aarch64 that exercises buffer-size
  and oversized-block error returns and verifies the FPU control register state
  is restored.

# 0.5.5

## Bug fixes

- **wiener.rs:127** — Harmonic/percussive transient blend now targets 1.0 (preserve transient) instead of the hard-coded 0.5 constant. Previously `gain * (1-0.5*w) + w * 0.5` would drag a high Wiener gain (0.9) down toward 0.7 and over-preserve very low gains (floor=0.1 → 0.55). Fixed to `gain * (1-t) + t` (blend toward 1.0), which only ever raises the gain.

- **multi_resolution.rs:322** — Removed temporal smoothing from the small-FFT path. The large-FFT path in `calculate_wiener_gains` already applies attack/release smoothing after `combine_gains`; applying it in the small path too created double-smoothing that added ~2 extra frames of attack/release lag and over-attenuated transients.

- **wiener.rs / lib.rs / multi_resolution.rs:** Reworked inter-channel spatial denoise coherence to use a complex cross-spectrum path:
  - Added averaged complex cross-state (`spatial_cross`) and replaced magnitude-only coherence calculation with magnitude-squared of the averaged complex cross (`|E[X0 * conj(X1)]|² / (E[|X0|²]E[|X1|²])`).
  - Added smoothing on the complex cross estimate for stability and updated tests for coherent vs decorrelated multi-channel phasing.

- **wiener.rs:142, polyphonic.rs:76** — Psychoacoustic masking now guards on `speech_presence[ch][k] >= 0.1` before setting gain to 1.0. Previously, on noise-only frames the noise power could exceed its own masking threshold (noise masking itself, especially at low frequencies where Bark spreading is wide), causing the denoiser to pass noise it should attenuate.

- **lib.rs:946** — PND analyzers are now fed one per-channel block per `process_in_place` call instead of one sample at a time. Reduces function-call overhead from `num_frames × channels` to `channels` calls per block. No behavior change; `PndAnalyzer::analyze` already accepts variable-length slices.

- **fft.rs:52** — Removed dead `calculate_power_spectrum` helper that allocated a fresh `Vec` on demand; direct power-at-bin access is now the supported path.

- **lib.rs:657** / **multi_resolution.rs:220** — Removed `std::mem::take`-style scratch-buffer swap patterns in FFT pipeline setup; preallocated temp blocks are now copied in-place before shift, avoiding move/restore churn.

- **wiener.rs:459** — Corrected comment: `0.13` in `log10(power)` units equals **1.3 dB** (not "3 dB" as stated previously; `10 × 0.13 = 1.3 dB`).

## Deferred

- **Performance passes (issues #10, #11, #12):** Fusing the 9 per-channel gain passes, precomputing a sparse Bark spreading matrix, and replacing the FTZ/DAZ RAII cleanup are all valid optimisations but require non-trivial restructuring. Deferred.

# 0.5.4

- Removed the bundled hiss reducer, transient/click repair, and RNNoise speech-denoiser features. They now live in dedicated plugins (`sotf-plugin-hiss-reducer`, `sotf-plugin-declick`, `sotf-plugin-speech-denoiser`), which all share the new `plugins-denoiser` DSP crate.
- Removed the `algorithm`, `crack_sensitivity`, `transient_enabled`, `hiss_enabled`, `hiss_threshold_db`, `hiss_frequency_hz`, and `hiss_strength` parameters. Existing presets that set those keys still deserialize because the fields are now ignored, but new chains should compose the dedicated plugins instead.
- Plugin focus narrowed to broadband denoising via Wiener filtering with MCRA / IMCRA noise estimation, decision-directed SNR, psychoacoustic masking, multi-resolution dual-STFT, and harmonic/percussive separation.
