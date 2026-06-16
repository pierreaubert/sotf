# sotf-plugin-dither

TPDF dither with F-weighted noise shaping for bit-depth reduction.

## Architecture

- `lib.rs` — Main `DitherPlugin`, implements `ParametricParametricInPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `DitherPlugin` implementing `ParametricParametricInPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-dither
```

## Important Notes

- TPDF (Triangular Probability Density Function) dither
- F-weighted noise shaping pushes quantization noise to less audible frequencies
- Apply as the last processing step before bit-depth reduction
