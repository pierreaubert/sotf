# math-iir-fir (lib: `math_audio_iir_fir`, version: 0.3.2)

IIR and FIR filter design and implementation. Core DSP library used throughout the project.

## Key Types

### Biquad (`iir.rs`, ~97KB)

The fundamental filter type used across the entire codebase:
```rust
pub struct Biquad {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
    // ... biquad coefficients
}
```

Filter types: Peak, Lowshelf, Highshelf, Lowpass, Highpass, Bandpass, Notch, Allpass, and more.

### Fir (`fir.rs`, ~33KB)

Windowed sinc FIR filter bank for linear-phase filtering.

## Module Layout

- `iir.rs` - Biquad filter implementation (design, processing, response computation)
- `fir.rs` - FIR filter bank
- `fir_design.rs` - Frequency response matching for FIR design (~26KB)
- `phase_smooth.rs` - Phase unwrapping and smoothing

## Capabilities

- Frequency response computation
- Multiple export formats: APO, RME, AU Preset
- Phase smoothing and group delay calculation
- Kirkeby correction for inverse filters

## Testing

```bash
cargo test -p math-iir-fir --lib
cargo check -p math-iir-fir && cargo clippy -p math-iir-fir
```

## Benchmarks

```bash
cargo bench -p math-iir-fir -- biquad_bench
```

## Important Notes

- `iir.rs` is a large file (~97KB) — the Biquad struct is the most widely used type in the project
- Used by: `plugins` (EQ plugin), `autoeq` (optimization output), `engine` (filter processing)
