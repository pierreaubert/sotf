# sotf-plugin-ambisonics

Ambisonics Decoder — selectable regularized mode matching or AllRAD/VBAP decode from Higher-Order Ambisonics to speaker layouts.

## What It Does

Decodes ACN/SN3D Higher-Order Ambisonics (HOA) audio into speaker feeds using
one of two setup-time matrix builders. `mode_matching` uses a rank-revealing SVD
pseudoinverse of the physical loudspeaker spherical-harmonic matrix.
`allrad` decodes to a deterministic Fibonacci virtual sphere and projects each
virtual speaker to the physical layout with 2D pair or 3D triangle VBAP before
composing the final fixed matrix. The selected matrix is the only work done in
the realtime process path.

## Features

- **Regularized mode matching**: Scale-relative SVD/Tikhonov decode with rank, condition, reconstruction-error, and peak-gain diagnostics
- **AllRAD/VBAP**: Virtual-sphere decode followed by setup-time physical-layout remapping; 64/96/128 virtual speakers for orders 1/2/3
- **Higher-Order Ambisonics**: Supports orders 1–3 (4, 9, or 16 ACN/SN3D channels)
- **Spherical harmonics**: Full SH evaluation for spatial processing
- **Layouts**: 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, and 9.1.6

Input channels are ACN ordered and SN3D normalized; orders 1/2/3 require exactly
4/9/16 input channels. LFE rows are always silent. Output channel order follows
the selected SOTF speaker layout. Structural parameter changes require the host
to construct and initialize a new plugin instance.

Dual-band mode uses a complementary LR4 split at 700 Hz: the basic matrix feeds
LF and exact max-rE degree weights feed HF. It requires a sample rate above
1400 Hz and has frequency-dependent crossover phase but no fixed host-compensated
latency. Scratch is fixed to two 16-sample frames, so validated host blocks have
no plugin-owned frame limit and allocate no callback memory.

NaN and infinity reject the entire block before state mutation; subnormal values
are flushed to zero before the stateful crossover. Underdetermined or planar
layouts are decoded with a bounded minimum-norm solution and explicitly report
lost rank. Such layouts cannot reproduce every 3-D component, and decoded sums
are not peak normalized, so downstream processing must preserve headroom.

The `algorithm` structural choice defaults to `mode_matching` for serialized
compatibility. AllRAD uses the same ACN/SN3D conventions and keeps LFE silent;
irregular or underdetermined physical layouts fall back to bounded nearest
speaker projection for virtual directions outside the physical VBAP hull.

## Architecture

```
src/
├── lib.rs                  # AmbisonicsDecoderPlugin
├── decode_matrix.rs        # Decoding matrix computation
├── spherical_harmonics.rs  # SH evaluation
└── params.rs               # Parameters
```

## Testing

```bash
cargo test -p sotf-plugin-ambisonics
```

## License

Part of the SOTF (Sound of the Future) project.
