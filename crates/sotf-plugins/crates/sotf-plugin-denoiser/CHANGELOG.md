# 0.5.4

- Removed the bundled hiss reducer, transient/click repair, and RNNoise speech-denoiser features. They now live in dedicated plugins (`sotf-plugin-hiss-reducer`, `sotf-plugin-declick`, `sotf-plugin-speech-denoiser`), which all share the new `plugins-denoiser` DSP crate.
- Removed the `algorithm`, `crack_sensitivity`, `transient_enabled`, `hiss_enabled`, `hiss_threshold_db`, `hiss_frequency_hz`, and `hiss_strength` parameters. Existing presets that set those keys still deserialize because the fields are now ignored, but new chains should compose the dedicated plugins instead.
- Plugin focus narrowed to broadband denoising via Wiener filtering with MCRA / IMCRA noise estimation, decision-directed SNR, psychoacoustic masking, multi-resolution dual-STFT, and harmonic/percussive separation.
