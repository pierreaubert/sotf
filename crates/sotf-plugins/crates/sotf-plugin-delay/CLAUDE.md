# sotf-plugin-delay

Audio delay plugin with feedback, LFO modulation, allpass feedback, and 4-point Lagrange interpolation for fractional delay.

## Architecture

```
src/
  lib.rs        -- DelayPlugin (InPlacePlugin), DelayPluginParams, AllpassState
  params.rs     -- Centralized parameter specs
  param_specs.rs -- Parameter spec definitions
```

Data flow: Input -> write to circular delay buffer -> read with fractional delay (Lagrange interpolation) -> optional allpass filter on feedback path -> feedback into buffer -> dry/wet mix output.

**Key types:**

- `DelayPlugin` -- Main plugin implementing `InPlacePlugin`. Uses a flat circular buffer (`buffer[pos * channels + ch]`).
- `DelayPluginParams` -- Serde config: delay_ms, feedback, mix, LFO rate/depth, allpass toggle.
- `AllpassState` -- First-order allpass filter for the feedback path. Transfer function: `H(z) = (coeff + z^-1) / (1 + coeff * z^-1)`.

## Key Public API

- `DelayPlugin::new(channels, delay_ms, feedback, mix) -> Self` (`lib.rs`)
- `DelayPlugin::from_params(channels, params) -> Self` (`lib.rs`)
- Implements `InPlacePlugin` trait

**Parameters:** `delay_ms` (0.1-5000 ms), `feedback` (0-0.95), `mix` (0-1), `lfo_rate_hz` (0-10 Hz), `lfo_depth_ms` (0-5 ms), `allpass_feedback` (bool).

## Testing

```bash
cargo test -p sotf-plugin-delay
```

## Important Notes

- Fractional delay uses 4-point Lagrange interpolation (`lagrange4`) for high-quality sub-sample accuracy. This is exact for linear and quadratic signals.
- LFO modulation applies a sine wave to the delay time, creating chorus/flanger effects. Effective delay is clamped to `[1, max_samples-3]` to keep interpolation guard samples valid.
- Maximum delay is 5000ms. Buffer is pre-allocated at initialization for the maximum, with +4 guard samples for interpolation headroom.
- Allpass feedback colors the feedback path spectrally without changing gain. Useful for creating more diffuse echoes.
- Smoothers: delay time (50ms), feedback (5ms), mix (5ms). Delay time smoothing prevents zipper noise when modulating.
- FTZ/DAZ enabled and denormals flushed post-processing.
