# sotf-plugin-stereo-imager

Multi-band M/S stereo width control.

## What It Does

Controls the stereo width of audio using Mid/Side processing. The signal is split into frequency bands, and each band's stereo width can be independently adjusted — from mono (0%) through natural (100%) to hyper-wide (200%+). Useful for widening the high frequencies while keeping bass centered, or for collapsing problematic stereo information.

## Features

- **M/S processing**: Mid/Side encoding for precise width control
- **Multi-band**: Independent width per frequency band
- **Width range**: From mono (collapsed) to hyper-wide
- **Frequency-dependent**: Widen highs while keeping bass centered

The plugin is strictly stereo. Neutral width settings are sample-transparent:
the original M/S signal is retained and crossover bands contribute only width
corrections, so intermediate mix values do not comb-filter the dry signal.
Construction validates finite ranges and strict crossover ordering; initialization
also requires the upper crossover below Nyquist. Processing is bounded to the
preallocated 65536-frame capacity and performs no heap allocation.

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
