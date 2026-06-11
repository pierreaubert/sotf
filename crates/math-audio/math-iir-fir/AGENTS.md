# math-iir-fir (lib: `math_audio_iir_fir`, version: 0.4.5)

IIR and FIR filter design and implementation. Core DSP library used throughout the project.

## Key Types

### Biquad (`iir.rs`, ~4800 lines)

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

Filter types: Peak, Lowshelf, Highshelf, Lowpass, Highpass, HighpassVariableQ, Bandpass, Notch.

Related types: `BiquadBank`, `BiquadCoefficients`, `FilterRow`, `Peq` (`Vec<(f64, Biquad)>`).

### FIR (`fir.rs`)

Windowed sinc FIR filter bank for linear-phase filtering. Types: `Fir`, `FirBank`, `FirFilterType`, `WindowType`.

### SVF (`svf.rs`)

Zero-Delay Feedback State Variable Filter (Zavalishin TPT topology). Types: `SvfFilter`, `SvfFilterType`.

## Module Layout

| Module | Description |
|---|---|
| `iir.rs` | Biquad filter implementation, PEQ operations, export formats, loudness compensation |
| `fir.rs` | FIR filter bank implementation and response computation |
| `fir_design.rs` | FIR design from frequency response, Kirkeby correction, pre-ringing analysis |
| `phase_smooth.rs` | Phase unwrapping, smoothing via group delay, complex interpolation |
| `svf.rs` | State Variable Filter (Zavalishin TPT) |
| `denormals.rs` | Denormal number handling for audio processing |
| `error.rs` | Error types (`IirError`) |

## Capabilities

- Frequency and phase response computation
- **9 export formats**: APO, RME Channel, RME Room, AU Preset, CamillaDSP, EasyEffects, PipeWire, Roon, Wavelet
- PEQ loudness compensation (K-weighting and A-weighting)
- Butterworth and Linkwitz-Riley filter design
- FIR design from target frequency response (`generate_fir_from_response`)
- Kirkeby correction for inverse filters (`generate_kirkeby_correction`)
- Pre-ringing analysis and suppression
- Phase smoothing and group delay calculation
- `bw2q()` / `q2bw()` bandwidth/Q conversion

## Used By

- `plugins` (EQ plugin), `autoeq` (optimization output), `engine` (filter processing), `math-dsp` (octave-band analysis)

## Testing

```bash
cargo test -p math-iir-fir --lib
cargo check -p math-iir-fir && cargo clippy -p math-iir-fir
```

## Benchmarks

```bash
cargo bench -p math-iir-fir -- biquad_bench
```

## Examples

```bash
cargo run --release --example format_demo -p math-iir-fir
cargo run --release --example readme_example -p math-iir-fir
cargo run --release --example fir_example -p math-iir-fir
cargo run --release --example format_rme_room_demo -p math-iir-fir
cargo run --release --example peq_loudness_compensation -p math-iir-fir
```
