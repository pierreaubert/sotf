# plugins-denoiser

Shared denoiser DSP building blocks for SOTF plugins.

Modules:
- `transient` — fixed-lookahead robust click detection and interpolation.
- `hiss` — `HissReducer`, a zero-latency persistent low-level high-band
  downward expander with smoothed cutoff/bypass transitions.
- `spectral_hiss` — `SpectralHissReducer`, an allocation-free 1024-point WOLA
  minimum-statistics reducer with fixed 1024-sample latency.
- `rnnoise` — `RnnoiseBackend` wrapping `nnnoiseless` for 48 kHz mono/stereo
  voice denoising, with arbitrary host framing, fixed 480-sample latency, warm
  crossfaded bypass, sanitized model input, preallocated model workspace, and
  fixed-size access to the model's smoothed 22-band gains/VAD probability.
  Stereo applies one polarity-aware detector's spectral decisions to both
  original channels; it never reconstructs suppression from broadband RMS.

Used by `sotf-plugin-declick`, `sotf-plugin-hiss-reducer`, and `sotf-plugin-speech-denoiser` so each plugin stays a thin host adapter.
