# 0.5.5

## Bug fixes

- **wiener.rs:127** — Harmonic/percussive transient blend now targets 1.0 (preserve transient) instead of the hard-coded 0.5 constant. Previously `gain * (1-0.5*w) + w * 0.5` would drag a high Wiener gain (0.9) down toward 0.7 and over-preserve very low gains (floor=0.1 → 0.55). Fixed to `gain * (1-t) + t` (blend toward 1.0), which only ever raises the gain.

- **multi_resolution.rs:322** — Removed temporal smoothing from the small-FFT path. The large-FFT path in `calculate_wiener_gains` already applies attack/release smoothing after `combine_gains`; applying it in the small path too created double-smoothing that added ~2 extra frames of attack/release lag and over-attenuated transients.

- **wiener.rs:142, polyphonic.rs:76** — Psychoacoustic masking now guards on `speech_presence[ch][k] >= 0.1` before setting gain to 1.0. Previously, on noise-only frames the noise power could exceed its own masking threshold (noise masking itself, especially at low frequencies where Bark spreading is wide), causing the denoiser to pass noise it should attenuate.

- **lib.rs:946** — PND analyzers are now fed one per-channel block per `process_in_place` call instead of one sample at a time. Reduces function-call overhead from `num_frames × channels` to `channels` calls per block. No behavior change; `PndAnalyzer::analyze` already accepts variable-length slices.

- **wiener.rs:459** — Corrected comment: `0.13` in `log10(power)` units equals **1.3 dB** (not "3 dB" as stated previously; `10 × 0.13 = 1.3 dB`).

## Deferred

- **Spatial coherence (issue #4):** The inter-channel coherence formula (`2√(p0·p1)/(p0+p1)`) computes magnitude coherence rather than proper complex-domain MSC. Fixing this requires maintaining a short complex cross-spectrum averaging window and is a non-trivial algorithm change. Deferred to a future PR.
- **Performance passes (issues #10, #11, #12):** Fusing the 9 per-channel gain passes, precomputing a sparse Bark spreading matrix, and replacing the FTZ/DAZ RAII cleanup are all valid optimisations but require non-trivial restructuring. Deferred.

# 0.5.4

- Removed the bundled hiss reducer, transient/click repair, and RNNoise speech-denoiser features. They now live in dedicated plugins (`sotf-plugin-hiss-reducer`, `sotf-plugin-declick`, `sotf-plugin-speech-denoiser`), which all share the new `plugins-denoiser` DSP crate.
- Removed the `algorithm`, `crack_sensitivity`, `transient_enabled`, `hiss_enabled`, `hiss_threshold_db`, `hiss_frequency_hz`, and `hiss_strength` parameters. Existing presets that set those keys still deserialize because the fields are now ignored, but new chains should compose the dedicated plugins instead.
- Plugin focus narrowed to broadband denoising via Wiener filtering with MCRA / IMCRA noise estimation, decision-directed SNR, psychoacoustic masking, multi-resolution dual-STFT, and harmonic/percussive separation.
