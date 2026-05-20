# math-rir (lib: `math_rir`)

Room Impulse Response analysis using the SSIR (Spatial Segmentation of Impulse Response) method.

**Version:** 0.5.5

## Overview

Segments a measured Room Impulse Response (RIR) into consecutive, variable-length sound events (direct sound + early reflections), each with a constant direction of arrival (DOA). This preserves the full temporal energy profile while enabling per-reflection analysis and manipulation.

Based on: Pawlak & Lee, "Spatial segmentation of impulse response for room reflection analysis and auralization", Applied Acoustics 249 (2026).

## Key algorithms

- **Direct sound onset detection** (Algorithm 1 from paper): log-magnitude peak detection with 11 dB threshold
- **Local Energy Ratio reflection detection**: 1ms analysis windows, median energy threshold, DOA/TOA validation
- **Onset refinement**: per-segment onset detection within pre-onset windows
- **Mixing time estimation**: Abel & Huang echo density method

## API

```rust
use math_rir::{analyze_rir, analyze_srir, SsirConfig};

// Mono RIR (energy-based detection only)
let result = analyze_rir(&rir_samples, &SsirConfig::new(48000.0));

// B-format SRIR (full DOA estimation + validation)
let result = analyze_srir(&[&w, &x, &y, &z], &SsirConfig::new(48000.0));

println!("{} events, mixing time {:.1}ms",
    result.num_events(), result.mixing_time_ms());
```

## Modules

- `detection` — Direct sound and reflection detection algorithms
- `segmentation` — SSIR segmentation and event building
- `mixing_time` — Echo density and mixing time estimation
- `metrics` — ISO 3382 room-acoustic metrics (EDT, T20, T30, C50, C80, D50, Ts)
- `bands` — Octave-band and third-octave-band filtering (ISO/IEC 61260)

## Dependencies

- `math-iir-fir` — Butterworth bandpass filters for octave-band analysis
- `rayon` — Parallel processing

## Testing

```bash
cargo test -p math-rir --lib
cargo check -p math-rir && cargo clippy -p math-rir
```

## Used by

- `sotf-plugin-binaural`: SSIR-analyzed measured room reflections with per-reflection HRTF
- `autoeq/roomeq`: SSIR-informed correction weights for room EQ optimization

## License

See the root workspace `LICENSE` file.
