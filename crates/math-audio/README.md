# Math Audio

Mathematical and DSP libraries for the SOTF audio processing project.

## Crates

| Crate | Lib Name | Description |
|---|---|---|
| [math-iir-fir](math-iir-fir/) | `math_audio_iir_fir` | IIR (Biquad) and FIR filter design, PEQ operations, 9 export formats, SVF |
| [math-dsp](math-dsp/) | `math_audio_dsp` | Signal generation, FFT analysis, acoustic metrics, SIMD, audio features, STFT |
| [math-optimisation](math-optimisation/) | `math_audio_optimisation` | Differential Evolution optimizer, L-SHADE, Levenberg-Marquardt |
| [math-test-functions](math-test-functions/) | `math_audio_test_functions` | 56+ benchmark functions for optimizer validation |
| [math-delaunay](math-delaunay/) | `math_delaunay` | Delaunay triangulation and Voronoi diagrams (port of d3-delaunay) |
| [math-rir](math-rir/) | `math_rir` | Room Impulse Response analysis: SSIR reflection detection, segmentation, mixing time |

## Dependency Graph

```
math-test-functions  (independent)
        |
        v
math-optimisation  (depends on math-test-functions)

math-iir-fir  (independent)
        |
        v
math-dsp  (depends on math-iir-fir)

math-delaunay  (independent)

math-rir  (independent)
```

## Build & Test

```bash
# Build all binaries
just prod-math

# Run all tests
cargo test -p math-iir-fir -p math-dsp -p math-optimisation -p math-test-functions -p math-delaunay -p math-rir --lib

# Run benchmarks
just bench-math

# Run examples
just examples-math
```

## Publish Order

Due to dependency ordering, publish in this sequence:

1. `math-test-functions`
2. `math-optimisation`
3. `math-iir-fir`
5. `math-rir`
4. `math-dsp`
5. `math-delaunay`
