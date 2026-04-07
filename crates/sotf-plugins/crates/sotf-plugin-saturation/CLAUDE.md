# sotf-plugin-saturation

Saturation / Harmonic Exciter plugin.

## Architecture

- `lib.rs` — Main `SaturationPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `SaturationPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-saturation
```

## Important Notes

- InPlacePlugin — same channel count in/out
- Adds harmonic content through nonlinear waveshaping
- Multiple saturation curves available (soft clip, hard clip, tape, tube)
- Uses ADAA (Anti-Derivative Anti-Aliasing) from sotf-host to reduce aliasing artifacts
