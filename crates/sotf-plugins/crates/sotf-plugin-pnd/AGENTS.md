# sotf-plugin-pnd

Perceptual Noise Diffusion and varispeed plugin with polyphonic note detection, pitch shifting via phase vocoder, and time stretching via resampling.

## Architecture

```
src/
  lib.rs       -- PndPlugin (Plugin), PhaseVocoderChannel, main processing
  analysis.rs  -- PndAnalyzer: polyphonic note detection via FFT peak picking
  config.rs    -- PndPluginParams configuration
  params.rs    -- Centralized parameter specs
```

**Algorithm pipeline:**
1. Input deinterleaved to stereo planar buffers
2. Optional pitch detection via `PndAnalyzer` (FFT-based peak picking)
3. Time-domain resampling via rubato for varispeed (changes both pitch and tempo)
4. Phase vocoder for pitch shifting without tempo change (STFT analysis -> phase accumulation -> resynthesis)
5. Interleave output back to flat buffer

**Key types:**

- `PndPlugin` -- Main plugin implementing `Plugin`. 2 input -> 2 output channels (stereo).
- `PhaseVocoderChannel` -- Per-channel phase vocoder state: FFT analysis/synthesis with spectral-bin remapping and phase accumulation. It does not preserve formants.
- `PndAnalyzer` -- Polyphonic note detection: FFT peak picking with magnitude thresholding.
- `PndPluginParams` -- Config: pitch_semitones, speed, varispeed mode, analysis settings.

**Phase vocoder:** FFT size 2048, hop size 512 (75% overlap). Hann analysis window. Phase accumulation tracks inter-frame phase differences for coherent resynthesis at shifted pitch.

## Key Public API

- `PndPlugin::new(params) -> Self` (`lib.rs`)
- `PndPlugin::from_params(params) -> Self` (`lib.rs`)
- Implements `Plugin` trait: 2 input -> 2 output channels

**Parameters:** `pitch_semitones` (pitch shift amount), `speed` (playback speed), `varispeed` (bool, combined pitch+speed change), analysis-related params.

## Testing

```bash
cargo test -p sotf-plugin-pnd
```

## Important Notes

- Phase vocoder and resampler have separate roles: the resampler (rubato) handles varispeed (pitch and tempo change together), while the phase vocoder handles independent pitch shifting (preserves tempo).
- Resampler uses polynomial interpolation (rubato `PolynomialDegree`) for lower CPU than sinc, since quality requirements are less strict for varispeed.
- The `PndAnalyzer` uses FFT peak detection for polyphonic note identification. Results are exposed via `RealTimeCache` for UI monitoring.
- Stereo processing: input is deinterleaved, each channel processed independently, then reinterleaved.
- Resampler chunk size is fixed at 1024 frames. Input accumulation handles frame size mismatches.
- The plugin name "PND" stands for Perceptual Noise Diffusion, but the implementation also covers varispeed and pitch shifting functionality.
