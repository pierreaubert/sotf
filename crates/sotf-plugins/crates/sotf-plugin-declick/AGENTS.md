# sotf-plugin-declick

Bounded-lookahead click detection and interpolation plugin. Wraps the
`TransientSuppressor` block from `plugins-denoiser` in the SOTF host plugin
trait.

## Architecture

- `lib.rs` — `DeclickPluginParams` + `ParametricInPlacePlugin` implementation.
- `params.rs` — `PARAMS` array (parameter specs) and registration via `param_specs::find_by_key`.

## Parameters

- `enabled` — bypass toggle.
- `sensitivity` — click-detection sensitivity threshold.
- `link_channels` — linked adjacent-pair versus independent detection.

## Features

- `qa` — enables `sotf-host/qa` and the `qa-declick` benchmark binary.

## Testing

```bash
cargo check -p sotf-plugin-declick && cargo clippy -p sotf-plugin-declick
cargo test -p sotf-plugin-declick
cargo run -p sotf-plugin-declick --features qa --bin qa-declick
```

## Important Notes

- Fixed latency is `plugins_denoiser::transient::LOOKAHEAD_SAMPLES` (8 samples).
- Bypass remains latency matched and keeps detector history warm.
- Implements `ParametricInPlacePlugin` — same in/out channel count.
- Parameter registration must appear in `parameter_schema`/`current_values` and be applied in `apply_values`. Missing schema entries cause silent rejection.
- Heavy lifting lives in `plugins-denoiser`; this crate is intentionally a thin host adapter.
