# sotf-tools

Tools for generating test signals, converting formats, and managing design tokens for SOTF.

## Overview

Binary-only crate providing utility programs for audio development, testing, and design-system maintenance.

## Binaries

### `generate-audio-tests`

Generate WAV files in multiple channel counts, sample rates, and bit depths for end-to-end audio validation.

Signals:
- `id` — per-channel identification tones (unique frequency per channel)
- `thd1k` — single-tone 1 kHz @ -3 dBFS (for THD)
- `thd100` — single-tone 100 Hz @ -3 dBFS (low-frequency THD)
- `imd_smpte` — SMPTE two-tone 60 Hz + 7 kHz (4:1 amplitude ratio)
- `imd_ccif` — CCIF two-tone 19 kHz + 20 kHz (equal amplitudes)
- `sweep` — logarithmic frequency sweep from 20 Hz to 20 kHz
- `white_noise` — white noise (flat spectrum)
- `pink_noise` — pink noise (1/f spectrum)
- `m_noise` — M-weighted noise (ITU-R 468 weighting)

```bash
cargo run --bin generate-audio-tests -- --help
```

### `generate-upmixer-golden`

Generate golden-reference output files for upmixer regression testing.

```bash
cargo run --bin generate-upmixer-golden
```

### `sofa-to-sqlite`

Convert SOFA HRTF files to SQLite `.hrtfdb` databases for faster loading.

```bash
cargo run --bin sofa-to-sqlite -- input.sofa output.hrtfdb
```

### `export-design-tokens`

Export design tokens from the SOTF design system.

```bash
cargo run --bin export-design-tokens
```

### `import-design-tokens`

Import design tokens into the SOTF design system.

```bash
cargo run --bin import-design-tokens
```

## Dependencies

- `sotf-engine` / `sotf-plugins` — Audio engine and plugins
- `sotf-gpui` — GPUI toolkit
- `gpui` — UI framework
- `symphonia` — Audio decoding
- `hound` — WAV I/O
- `image` — Image processing for album art thumbnails
- `clap` — CLI parsing

## Testing

```bash
cargo check -p sotf-tools
cargo clippy -p sotf-tools
```

## License

See the root workspace `LICENSE` file.
