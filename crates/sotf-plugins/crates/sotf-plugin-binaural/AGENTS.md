# sotf-plugin-binaural

HRTF-based binaural decoder plugin rendering multichannel surround to headphone-compatible stereo using overlap-add convolution.

## Architecture

```
src/
  lib.rs          -- BinauralDecoderPlugin (Plugin), BinauralState, main processing
  config.rs       -- BinauralDecoderParams, configuration types
  error.rs        -- BinauralError type
  filter.rs       -- Filter utilities (diffuse field EQ, LFE lowpass)
  hrtf.rs         -- HRTF loading and interpolation from SOFA files
  hrtf_database.rs -- HRTF database management and selection
  room.rs         -- Room model with reflections (RoomModel, Reflection, ReflectionHrtf)
  params.rs       -- Centralized parameter specs
```

**Algorithm pipeline:**
1. Multichannel input split into zero-padded hop partitions
2. Forward FFT per active input channel
3. Per-channel HRTF convolution in frequency domain (complex multiply-accumulate)
4. LFE channels processed through lowpass filter and mixed at configurable gain
5. Optional diffuse field equalization
6. Optional source-owned broadband-ILD room reflections via image source method
7. Sum left/right HRTF outputs across all channels
8. Inverse FFT + overlap-add
9. Output as stereo (2 channels)

**Key types:**

- `BinauralDecoderPlugin` -- Main plugin implementing `Plugin`. N input channels -> 2 output channels.
- `BinauralState` -- Lock-free swappable state (via `ArcSwap`): frequency-domain HRTF filters, diffuse field EQ.
- `BinauralDecoderParams` -- Config: HRTF path, speaker config, externalization, near-field, room model.
- `RoomModel` -- Room reflections using image source method with per-reflection HRTF.

## Key Public API

- `BinauralDecoderPlugin::new(input_channels, params) -> Result<Self, BinauralError>` (`lib.rs`)
- `BinauralDecoderPlugin::from_params(params) -> Result<Self, BinauralError>` (`config.rs`)
- Implements `Plugin` trait: N input channels -> 2 output channels
- `RoomModel`, `Reflection`, `ReflectionHrtf` re-exported for room simulation

**Parameters:** `externalization` (0-1, controls HRTF strength), `near_field_strength`, HRTF path, speaker config, room model settings.

## Testing

```bash
cargo test -p sotf-plugin-binaural
```

Benchmarks:
```bash
cargo bench -p sotf-plugin-binaural --bench binaural-decoder-benchmark
```

## Important Notes

- HRTF data loaded from SOFA files via `SofaFile` from sotf-host. The `hrtf_database.rs` module manages a database of available HRTFs.
- Speaker configuration is auto-detected from input channel count. HRTF filters are selected based on speaker positions from `SpeakerConfig`.
- Lock-free HRTF state swapping via `ArcSwap` allows changing HRTFs without blocking the audio thread.
- Output accumulator uses a power-of-2 ring buffer with bitmask wrapping (`output_accumulator_mask`).
- LFE channels are identified from the speaker config, processed through a lowpass filter, and mixed into both L/R outputs.
- SIMD operations: `complex_mul_add_simd` for HRTF convolution, `window_mul_simd` for analysis window application.
- Room reflections retain input/source ownership and use HRTF-derived broadband
  ILD. Full reflection ITD/pinna convolution is not claimed.
- Optional RTPGHI (Real-Time Phase Gradient Heap Integration) processor from `math-dsp` for phase reconstruction.
- The `sofa-reader` dev dependency is used only in benchmarks.
