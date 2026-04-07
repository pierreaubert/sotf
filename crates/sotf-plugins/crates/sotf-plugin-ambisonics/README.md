# sotf-plugin-ambisonics

Ambisonics Decoder — AllRAD decoding from Higher-Order Ambisonics to speaker layouts.

## What It Does

Decodes Higher-Order Ambisonics (HOA) audio into speaker feeds for any loudspeaker arrangement. Uses the AllRAD (All-Round Ambisonic Decoding) algorithm which combines VBAP panning with Ambisonics theory for robust decoding to irregular speaker layouts.

## Features

- **AllRAD decoding**: Robust decoding for arbitrary speaker layouts
- **Higher-Order Ambisonics**: Supports multiple Ambisonics orders
- **Spherical harmonics**: Full SH evaluation for spatial processing
- **Flexible layouts**: Works with any speaker configuration via VBAP

## Architecture

```
src/
├── lib.rs                  # AmbisonicsDecoderPlugin
├── config.rs               # Decoder configuration
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
