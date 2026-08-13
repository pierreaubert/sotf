# sotf-plugin-dynamic-eq

Dynamic EQ — frequency-selective dynamics processing.

## Architecture

- `lib.rs` — Main `DynamicEqPlugin`, implements `ParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `DynamicEqPlugin` implementing `ParametricInPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-dynamic-eq
```

## Important Notes

- Combines parametric EQ with dynamics (each band activates only when signal crosses threshold)
- ParametricInPlacePlugin — same channel count in/out
- Each band has: frequency, Q, gain, threshold, ratio, attack, release
- Filter/topology controls are structural; only dynamics and mix automate live.
