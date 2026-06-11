# sotf-plugin-transient-shaper

Transient Shaper — SPL Transient Designer approach.

## Architecture

- `lib.rs` — Main `TransientShaperPlugin`, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions

## Key Public API

- `TransientShaperPlugin` implementing `InPlacePlugin`

## Testing

```bash
cargo test -p sotf-plugin-transient-shaper
```

## Important Notes

- Based on the SPL Transient Designer approach
- Separates transient (attack) from sustain using envelope detection
- Attack and sustain can be independently boosted or cut
- Threshold-independent — works on the signal shape, not level
