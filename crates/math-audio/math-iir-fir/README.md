# math-iir-fir (lib: `math_audio_iir_fir`)

IIR, FIR, and SVF filter implementations for audio processing, generic over `f32` and `f64`.

**Version:** 0.5.13

## Generic precision

All filter types (`Biquad`, `SvfFilter`, `Fir`, `BiquadBank`) are generic over `FilterFloat` and default to `f64`. Use `f32` when throughput matters more than precision (e.g., real-time multi-channel processing). Convenience aliases like `BiquadF32`, `SvfFilterF32`, etc. are provided.

## Features

- **Biquad Filters**: Peak, Lowpass, Highpass, Lowshelf, Highshelf, Bandpass, Notch, AllPass
- **SVF (State Variable Filter)**: Zero-delay feedback topology (Zavalishin TPT) for artifact-free parameter changes
- **FIR Filters**: Windowed sinc filter bank for linear-phase filtering, FIR design from frequency response, Kirkeby correction, pre-ringing suppression
- **Crossovers**: Linkwitz-Riley 4th-order IIR and linear-phase FIR crossovers
- **Offline filtering**: Zero-phase `filtfilt` and `sosfilt` for analysis
- **PEQ (Parametric Equalizer)**: Multi-band parametric equalization with SPL response computation, loudness compensation, and 9 export formats (APO, RME, AU Preset, CamillaDSP, EasyEffects, PipeWire, Roon, Wavelet)
- **Filter Design**: Butterworth and Linkwitz-Riley lowpass/highpass
- **Phase Smoothing**: Phase unwrapping, smoothing via group delay, complex interpolation
- **Warped LPC and Kautz filters**: Advanced filter design for room acoustics

## Filter Types

### Biquad Filter Types

- `BiquadFilterType::Lowpass`
- `BiquadFilterType::Highpass`
- `BiquadFilterType::HighpassVariableQ`
- `BiquadFilterType::Bandpass`
- `BiquadFilterType::Peak`
- `BiquadFilterType::Notch`
- `BiquadFilterType::Lowshelf`
- `BiquadFilterType::Highshelf`

## Usage Examples

### Basic Biquad Filter

```rust
use math_audio_iir_fir::{Biquad, BiquadFilterType};

// f64 (default)
let filter = Biquad::new(
    BiquadFilterType::Peak,
    1000.0,  // frequency
    48000.0, // sample rate
    1.0,     // Q factor
    3.0      // gain in dB
);
let response_db = filter.log_result(1000.0);

// f32 — same API, less precision, higher throughput
let mut filter_f32 = Biquad::<f32>::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
let output = filter_f32.process(0.5f32);
```

### Parametric EQ (PEQ)

```rust
use math_audio_iir_fir::{Biquad, BiquadFilterType, Peq, peq_spl, peq_preamp_gain, peq_format_apo};
use ndarray::Array1;

let mut peq: Peq = Vec::new();

let hp = Biquad::new(BiquadFilterType::Highpass, 80.0, 48000.0, 0.707, 0.0);
peq.push((1.0, hp));

let peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.5, 4.0);
peq.push((1.0, peak));

let hs = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.8, -2.0);
peq.push((1.0, hs));

let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 1000);
let response = peq_spl(&freqs, &peq);
let preamp = peq_preamp_gain(&peq);

let apo_config = peq_format_apo("My Custom EQ", &peq);
```

### Filter Design

```rust
use math_audio_iir_fir::{peq_butterworth_lowpass, peq_linkwitzriley_highpass};

let lp_filter = peq_butterworth_lowpass(4, 2000.0, 48000.0);
let hp_filter = peq_linkwitzriley_highpass(4, 2000.0, 48000.0);
```

## Key Types

- `Peq<T>` — `Vec<(T, Biquad<T>)>` where the first element is the weight/amplitude multiplier
- `BiquadBank<T>` — Pack 2 or 4 biquad operations per clock depending on hardware
- `SvfFilter<T>` — Zero-delay feedback state variable filter
- `Fir<T>` / `FirCrossover` — Linear-phase FIR filtering

## Dependencies

- `ndarray`, `num-traits` — Numerical arrays
- `rustfft`, `num-complex` — FFT for FIR design
- `serde` — Serialization
- `hound` — WAV I/O for filter testing

## Testing

```bash
cargo test -p math-iir-fir --lib
cargo check -p math-iir-fir && cargo clippy -p math-iir-fir
```

## License

See the root workspace `LICENSE` file.
