# math-dsp (lib: `math_audio_dsp`, version: 0.3.0)

DSP utilities for signal generation and analysis.

## Key Components

- Signal generation: sine waves, frequency sweeps, pink noise, white noise
- FFT-based analysis tools
- WAV file utilities

## Binaries

- `wav2csv` - Convert WAV files to CSV for analysis

## Dependencies

- `rustfft` - FFT computation
- `hound` - WAV I/O
- `math-iir-fir` - Filter types

## Testing

```bash
cargo test -p math-dsp --lib
cargo check -p math-dsp && cargo clippy -p math-dsp
```
