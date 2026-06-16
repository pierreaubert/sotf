# sotf-plugin-loudness-compensation

Equal-loudness contour compensation based on ISO 226.

## Architecture

- `lib.rs` — Main plugin struct, implements `ParametricParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions and JSON deserialization
- `iso226.rs` — ISO 226 equal-loudness contour data and interpolation

## Key Public API

- Main plugin struct implementing `sotf_host::parametric_in_place_plugin::ParametricParametricInPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-loudness-compensation
```

## Important Notes

- Applies frequency-dependent gain based on playback level to match perceived loudness
- Uses ISO 226:2003 equal-loudness contours
- Requires knowledge of current playback SPL for accurate compensation
