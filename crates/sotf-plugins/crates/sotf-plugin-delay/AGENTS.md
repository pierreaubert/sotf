# sotf-plugin-delay

Audio delay plugin with feedback, LFO modulation, allpass feedback, and 4-point Lagrange interpolation for fractional delay.

## Architecture

```
src/
  lib.rs        -- public module facade
  lib/delay_plugin.rs -- DelayPlugin (ParametricInPlacePlugin)
  params.rs     -- Centralized parameter specs
  param_specs.rs -- Parameter spec definitions
```

Data flow: Input -> write to circular delay buffer -> read with fractional delay (Lagrange interpolation) -> optional allpass filter on feedback path -> feedback into buffer -> dry/wet mix output.

**Key types:**

- `DelayPlugin` -- Main plugin implementing `ParametricInPlacePlugin`. Uses one contiguous ring segment per channel (`buffer[ch * max_samples + pos]`).
- `DelayPluginParams` -- Serde config: delay_ms, feedback, mix, LFO rate/depth, allpass toggle.
- `AllpassState` -- First-order allpass filter for the feedback path. Transfer function: `H(z) = (coeff + z^-1) / (1 + coeff * z^-1)`.

## Key Public API

- `DelayPlugin::try_new(channels, delay_ms, feedback, mix) -> Result<Self, String>`
- `DelayPlugin::try_new_with_max_delay(...) -> Result<Self, String>` for bounded memory
- `DelayPlugin::from_params(channels, params) -> Result<Self, String>`
- Implements `ParametricInPlacePlugin` trait

**Parameters:** `delay_ms` (0-5000 ms, or the instance maximum), `feedback` (-0.95-0.95), `mix` (0-1), `lfo_rate_hz` (0-20 Hz), `lfo_depth_ms` (0-10 ms), `allpass_feedback` (bool), and `allpass_coeff` (0-0.99).

## Testing

```bash
cargo test -p sotf-plugin-delay
```

## Important Notes

- Fractional delay uses 4-point Lagrange interpolation (`lagrange4`) for high-quality sub-sample accuracy. This is exact for linear and quadratic signals.
- LFO modulation applies a sine wave to delay time. At a ring boundary only the infeasible half-cycle clamps; modulation does not collapse in both directions.
- The scalar constructor supports 5000ms. Explicit-range and per-channel constructors size their power-of-two rings from the promised automation range, with guard samples for interpolation.
- Allpass feedback colors the feedback path spectrally without changing gain. Useful for creating more diffuse echoes.
- Smoothers: delay time (50ms), feedback/mix (5ms), allpass enable/coefficient (20ms).
- FTZ/DAZ enabled and denormals flushed post-processing.
