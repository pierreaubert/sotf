# sotf-plugin-band-split

Split signal into frequency bands.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-band-split
```

## Important Notes

- Companion to sotf-plugin-band-merge — must be used in pairs
- Output channel count = input channels × number of bands
- Two-band Linkwitz-Riley outputs are magnitude-complementary; cascaded
  multiband outputs have unequal group delay and are not phase-perfect
- Frequency targets use a 20 ms logarithmic smoother; IIR coefficients update
  on a persistent eight-sample control cadence independent of callback partitioning.
- LR24/LR48 is structural and requires graph replacement. Exact no-op writes
  are accepted; live frequency and band-gain writes are allocation-free.
- Processing requires initialization, the initialized sample rate, and exact
  overflow-checked input/output buffer lengths.
