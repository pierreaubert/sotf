# sotf-plugin-beamformer

Beamformer plugin — MVDR, superdirective, and GSC beamformers.

## Architecture

- `lib.rs` — Main `BeamformerPlugin`, implements `Plugin` trait
- `mvdr.rs` — Minimum Variance Distortionless Response beamformer
- `superdirective.rs` — Superdirective beamformer
- `gsc.rs` — Generalized Sidelobe Canceller
- `steering.rs` — Steering vector computation
- `params.rs` — Parameter definitions

## Key Public API

- `BeamformerPlugin` implementing `Plugin`

## Testing

```bash
cargo test -p sotf-plugin-beamformer
```

## Important Notes

- Plugin trait — changes channel count (multi-mic input → mono/stereo output)
- Multiple beamforming algorithms: MVDR, superdirective, GSC
- Steering vectors define the look direction
