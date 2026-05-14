# math-dsp (lib: `math_audio_dsp`, version: 0.5.19)

DSP utilities for signal generation, analysis, and audio feature extraction.

## Modules

| Module | Description |
|---|---|
| `signals` | Signal generation: sine, two-tone, log sweep, white/pink/M noise, fade, padding, channel utils |
| `analysis` | FFT-based frequency analysis (Welch/single-FFT), acoustic metrics (RT60, C50/C80, THD), microphone compensation, CSV I/O |
| `simd` | SIMD-optimized DSP operations |
| `stft` | Short-Time Fourier Transform |
| `rtpghi` | Real-Time Phase Gradient Heap Integration (phase reconstruction) |
| `ebur128` | EBU R128 / ITU-R BS.1770-4 loudness implementation |
| `binaural_loudness` | ITU-R BS.1770-4 loudness for binaural (2-ch headphone) signals; BS.775 surround→binaural downmix |
| `esprit` | ESPRIT algorithm for frequency estimation |
| `instantaneous_frequency` | Instantaneous frequency extraction |
| `tonal_transient` | Tonal vs transient signal separation |
| `fdn` | Feedback Delay Network |
| `fast_math` | Fast math approximations |
| `audio_features` | Audio feature extraction: chroma, spectral, tempo, loudness, ZCR |
| `replaygain` | Replay Gain analysis (EBU R128-based) |
| `waveform` | Waveform visualization data |

## Binaries

- `wav2csv` -- Convert WAV files to CSV (frequency/SPL/phase)
- `simd-fuzzer` -- SIMD fuzzing test tool

## Key Types

- `WavAnalysisConfig`, `WavAnalysisOutput`, `AnalysisResult` -- analysis pipeline
- `MicrophoneCompensation` -- calibration data for measurement correction
- `ReplayGainAnalyzer`, `ReplayGainInfo` -- loudness normalization

## Dependencies

- `rustfft`, `realfft` -- FFT computation
- `num-complex` -- Complex number types
- `ndarray`, `nalgebra` -- Numerical arrays
- `hound` -- WAV I/O
- `math-iir-fir` -- Filter types (Biquad for octave-band analysis)
- `serde` -- Serialization
- `rand` -- RNG for noise generation

## Testing

```bash
cargo test -p math-dsp --lib
cargo check -p math-dsp && cargo clippy -p math-dsp
```
