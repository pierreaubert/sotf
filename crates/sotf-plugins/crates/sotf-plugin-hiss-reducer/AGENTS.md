# sotf-plugin-hiss-reducer

Stationary high-frequency-noise reducer. Wraps the `HissReducer` block from `plugins-denoiser` in the SOTF host plugin trait.

## Architecture

- `lib.rs` — `HissReducerPluginParams` + `ParametricInPlacePlugin` impl driving `plugins_denoiser::hiss::HissReducer`.
- `params.rs` — `PARAMS` array (parameter specs) and registration via `param_specs::find_by_key`.

## Parameters

- `enabled` — bypass toggle.
- `threshold_db` — absolute dBFS high-band level threshold (not SNR).
- `frequency_hz` — one-pole band-edge frequency, limited to 0.45 × sample rate.
- `strength` — reduction amount.

## Features

- `qa` — enables `sotf-host/qa` and the `qa-hiss-reducer` benchmark binary.

## Testing

```bash
cargo check -p sotf-plugin-hiss-reducer && cargo clippy -p sotf-plugin-hiss-reducer
cargo test -p sotf-plugin-hiss-reducer
cargo run -p sotf-plugin-hiss-reducer --features qa --bin qa-hiss-reducer
```

## Important Notes

- Implements `ParametricInPlacePlugin` — same in/out channel count.
- Parameter registration must appear in 3 places (see `param_bridge`).
- DSP body lives in `plugins-denoiser::hiss`; this crate is a thin host adapter.
- Processing requires initialization and rejects context sample-rate mismatches.
- Realtime setters and steady processing must remain allocation-free.
