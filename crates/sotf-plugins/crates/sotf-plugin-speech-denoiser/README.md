# sotf-plugin-speech-denoiser

SOTF speech denoiser plugin — RNNoise voice denoising.

Wraps the `RnnoiseBackend` block (which itself wraps `nnnoiseless`) from `plugins-denoiser` in the SOTF host plugin trait.

The model runs at 48 kHz and uses 480-sample internal quanta, but the plugin
accepts arbitrary host callback sizes and preserves a constant 480-sample
latency with preallocated streaming FIFOs. Every successful callback returns
the requested frame count, including startup and partial model frames.

Only mono and stereo layouts are supported. Stereo forms one polarity-aware,
energy-normalized detector signal, then applies the model's same 22 smoothed,
bounded spectral suppression gains to both original channels. This avoids
anti-phase cancellation and independent channel decisions while preserving the
original stereo relationships. Wider layouts are rejected rather than
receiving undefined independent spatial processing.

Bypass keeps the same 480-sample latency, continues advancing the neural model,
and crossfades between delayed wet and dry paths over 480 samples. Non-finite
input is replaced with silence and finite input is clamped to the model domain
`[-1, 1]` before persistent state is updated.

`get_data()` exposes a fixed-size `SpeechDenoiserData` snapshot containing the
22 model gains after RNNoise's inter-frame release smoothing, the bounded VAD
probability, and a completed-model-frame generation. The realtime cache is
preallocated and skips a publication under prolonged reader contention rather
than allocating on the audio thread. These values are model decisions, not
quality scores or gains inferred from broadband output/input RMS.

RNNoise FFT plans/tables and all large FFT, pitch, feature, synthesis, and RNN
workspaces are prepared or allocated during initialization. Processing, reset,
and live `enabled` changes do not allocate. The Audio Unit rejects sample rates
other than 48 kHz and layouts other than matching mono or stereo during format
negotiation.
