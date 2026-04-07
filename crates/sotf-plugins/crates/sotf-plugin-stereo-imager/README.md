# sotf-plugin-stereo-imager

Multi-band M/S stereo width control.

## What It Does

Controls the stereo width of audio using Mid/Side processing. The signal is split into frequency bands, and each band's stereo width can be independently adjusted — from mono (0%) through natural (100%) to hyper-wide (200%+). Useful for widening the high frequencies while keeping bass centered, or for collapsing problematic stereo information.

## Features

- **M/S processing**: Mid/Side encoding for precise width control
- **Multi-band**: Independent width per frequency band
- **Width range**: From mono (collapsed) to hyper-wide
- **Frequency-dependent**: Widen highs while keeping bass centered

## Architecture

```
src/
├── lib.rs     # StereoImagerPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-stereo-imager
```

## License

Part of the SOTF (Sound of the Future) project.
