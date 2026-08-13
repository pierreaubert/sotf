# sotf-plugin-speech-denoiser

RNNoise-based voice denoiser plugin. Wraps the `RnnoiseBackend` block from `plugins-denoiser` (which itself wraps `nnnoiseless`) in the SOTF host plugin trait.

## Architecture

- `lib.rs` — `SpeechDenoiserPluginParams` + `ParametricInPlacePlugin` impl driving `plugins_denoiser::rnnoise::RnnoiseBackend`.
- `params.rs` — `PARAMS` array (parameter specs).

## Parameters

- `enabled` — bypass toggle (default: enabled).

## Features

- `qa` — enables `sotf-host/qa` and the `qa-speech-denoiser` benchmark binary.

## Testing

```bash
cargo check -p sotf-plugin-speech-denoiser && cargo clippy -p sotf-plugin-speech-denoiser
cargo test -p sotf-plugin-speech-denoiser
cargo run -p sotf-plugin-speech-denoiser --features qa --bin qa-speech-denoiser
```

## Important Notes

- Implements `ParametricInPlacePlugin` — same in/out channel count.
- RNNoise expects 48 kHz mono frames; the wrapper handles arbitrary host
  framing and rejects non-48-kHz formats rather than resampling.
- Stereo uses one polarity-aware, energy-normalized detector and applies its
  22 bounded, smoothed model gains to both original channels.
- `get_data()` publishes fixed-size band-gain/VAD diagnostics through a
  preallocated realtime cache.
- DSP body lives in `plugins-denoiser::rnnoise`; this crate is a thin host adapter.
