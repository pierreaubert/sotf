# plugins-denoiser

Shared denoiser DSP building blocks for SOTF plugins.

Modules:
- `transient` — fixed-lookahead robust click detection and interpolation.
- `hiss` — `HissReducer`, a zero-latency persistent low-level high-band
  downward expander with smoothed cutoff/bypass transitions.
- `rnnoise` — `RnnoiseBackend` wrapping `nnnoiseless` for 48 kHz mono/stereo
  voice denoising, with arbitrary host framing, fixed 480-sample latency, warm
  crossfaded bypass, sanitized model input, and preallocated model workspace.

Used by `sotf-plugin-declick`, `sotf-plugin-hiss-reducer`, and `sotf-plugin-speech-denoiser` so each plugin stays a thin host adapter.
