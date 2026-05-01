# sotf-plugin-speech-denoiser

RNNoise-based voice denoiser plugin. Wraps the `RnnoiseBackend` block from `plugins-denoiser` (which itself wraps `nnnoiseless`) in the SOTF host plugin trait.

## Architecture

- `lib.rs` — `SpeechDenoiserPluginParams` + `InPlacePlugin` impl driving `plugins_denoiser::rnnoise::RnnoiseBackend`.
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

- Implements `InPlacePlugin` — same in/out channel count.
- RNNoise expects 48 kHz mono frames; the wrapper handles framing/resampling at the host boundary.
- DSP body lives in `plugins-denoiser::rnnoise`; this crate is a thin host adapter.
