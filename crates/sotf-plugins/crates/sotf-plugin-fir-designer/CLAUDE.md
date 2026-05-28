# sotf-plugin-fir-designer

FIR Designer — FIR magnitude and phase design from parametric target bands.

## Architecture

- `lib.rs` — Main `FirDesignerPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `FirDesignerPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-fir-designer
```

## Important Notes

- Uses FIR filters instead of direct IIR processing
- Supports linear-phase and minimum-phase FIR design modes
- Useful when phase coherence or precise FIR target design matters
