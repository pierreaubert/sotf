# sotf-plugin-hiss-reducer

Stationary high-frequency-noise reducer. Wraps the `HissReducer` block from `plugins-denoiser` in the SOTF host plugin trait.

## Architecture

- `lib.rs` — `HissReducerPluginParams` + `InPlacePlugin` impl driving `plugins_denoiser::hiss::HissReducer`.
- `params.rs` — `PARAMS` array (parameter specs) and registration via `param_specs::find_by_key`.

## Parameters

- `enabled` — bypass toggle.
- `threshold_db` — hiss-detection threshold.
- `frequency_hz` — band-edge frequency.
- `strength` — reduction amount.
- `low_latency` — short vs. transparent processing window.

## Features

- `qa` — enables `sotf-host/qa` and the `qa-hiss-reducer` benchmark binary.

## Testing

```bash
cargo check -p sotf-plugin-hiss-reducer && cargo clippy -p sotf-plugin-hiss-reducer
cargo test -p sotf-plugin-hiss-reducer
cargo run -p sotf-plugin-hiss-reducer --features qa --bin qa-hiss-reducer
```

## Important Notes

- Implements `InPlacePlugin` — same in/out channel count.
- Parameter registration must appear in 3 places (see `param_bridge`).
- DSP body lives in `plugins-denoiser::hiss`; this crate is a thin host adapter.
