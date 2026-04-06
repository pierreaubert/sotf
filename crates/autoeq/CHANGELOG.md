# 0.4.24

## EPA scoring

- Sharpness-aware target curve — Instead of "flat" or "Harman tilt", compute the sharpness (weight
ed spectral centroid) of the corrected response and add a penalty when it deviates from a target sharpness value. This prevent the optimizer from creating a technically flat but perceptually harsh or dull result.
- Roughness penalty for close modes — Two room modes within a critical band create beating perceived asroughness. The optimizer detect mode pairs where |f1 - f2| < critical_bandwidth(f1) and prioritize correcting these over isolated modes, because the roughness they create is more annoying than the level error of a single mode.
- Loudness-weighted loss — Replace the current flat/asymmetric MSE with a loss weighted by ISO 226
  equal-loudness contours at the listening level. A 3dB error at 4kHz (where the ear is most sensitive) should cost more than a 3dB error at 50Hz.
- EPA scoring — Compute E, P, A scores from the corrected response and optimize to maximize Evaluation while preserving Potency. Implemented the psychoacoustic metric computations (Zwicker loudness, sharpness, roughness models).

## Taking care of CDT

The ear generates Cubic Distortion Tones (CDT) at 2*f1 - f2 when two tones f1, f2 are present. Over-correcting at these frequencies can strip perceived "warmth." We add a min_cut_envelope that limits how deep the optimizer can cut at any frequency, protecting CDT-sensitive regions. This mirrors the existing max_boost_envelope pattern exactly.

# 0.4.23

- Added Warped Biquad (Bark-scale resolution) and Kautz Filter (room-mode poles) support
- Temporal decay thresholds

# 0.4.22

- Frequency-dependent correction depth: max_boost_envelope field on OptimizerConfig with log-frequency interpolation. Applied in DE optimizer fitness evaluation.
- Decomposed correction as default:  decomposed_correction defaults to Some(enabled: true). Schroeder raised to 250Hz, steady-state weight lowered to 0.4. Falls back to freq-domain-only mode detection when no IR.
- Stronger bass assymetry: AsymmetricLossConfig extended with bass_peak_weight=5.0, bass_dip_weight=0.2, transition_freq=300Hz. Smooth sigmoid crossfade in loss computation.
- Channel matching priority: Threshold tightened 1.5→0.75dB, max_filters 3→5. Pre-pass computes shared mean SPL so all channels optimize toward same target.
- First-reflection cancellation: New reflection_cancel.rs module. Uses SSIR to identify first reflection, designs LP-filtered IIR echo subtraction (Johnston method) below 500Hz.
- Windowed measurement: direct/early/late windows using SSIR boundaries, computes per-window FR with smoothing.

# 0.4.21

- implemented proper delay detection and analysis (following AES presentation Acoustic and Psychoacoustic issues in Room Correction James D. (jj) Johnston and Serge Smirnov)


