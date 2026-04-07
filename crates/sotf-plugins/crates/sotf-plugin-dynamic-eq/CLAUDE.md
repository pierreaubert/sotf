# sotf-plugin-dynamic-eq

Dynamic EQ — frequency-selective dynamics processing.

## Architecture

- `lib.rs` — Main `DynamicEqPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `DynamicEqPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-dynamic-eq
```

## Important Notes

- Combines parametric EQ with dynamics (each band activates only when signal crosses threshold)
- InPlacePlugin — same channel count in/out
- Each band has: frequency, Q, gain, threshold, ratio, attack, release
