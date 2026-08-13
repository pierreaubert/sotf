# sotf-plugin-pnd

Reference-free polyphonic pitch-motion monitor with exact dry passthrough.

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
3. Confidence-weighted drift monitoring
4. Exact input-to-output copy

**Key types:**

- `PndPlugin` -- Main plugin implementing `Plugin`. 2 input -> 2 output channels (stereo).
- `PhaseVocoderChannel` -- Per-channel phase vocoder state: FFT analysis/synthesis with phase accumulation for formant-preserving pitch shift.
- `PndAnalyzer` -- Polyphonic note detection: FFT peak picking with magnitude thresholding.
- `PndPluginParams` -- Monitoring and analysis configuration plus reserved compatibility fields.

**Phase vocoder:** FFT size 2048, hop size 512 (75% overlap). Hann analysis window. Phase accumulation tracks inter-frame phase differences for coherent resynthesis at shifted pitch.

## Key Public API

- `PndPlugin::new(params) -> Self` (`lib.rs`)
- `PndPlugin::from_params(params) -> Self` (`lib.rs`)
- Implements `Plugin` trait: 2 input -> 2 output channels

**Parameters:** analysis window, drift smoothing, multi-channel consensus, confidence threshold, and reserved correction fields.

## Testing

```bash
cargo test -p sotf-plugin-pnd
```

## Important Notes

- Correction and phase-vocoder activation are rejected until the host provides an observable reference and a compatible asynchronous render boundary.
- The `PndAnalyzer` uses FFT peak detection for polyphonic note identification. Results are exposed via `RealTimeCache` for UI monitoring.
- Stereo processing: input is deinterleaved, each channel processed independently, then reinterleaved.
- Resampler chunk size is fixed at 1024 frames. Input accumulation handles frame size mismatches.
- PND is a monitor until an observable reference and compatible correction boundary are supplied.
