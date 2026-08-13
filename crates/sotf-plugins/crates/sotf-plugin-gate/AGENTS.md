# sotf-plugin-gate

Noise gate plugin with sidechain HPF, configurable detection modes, lookahead, hysteresis, and soft knee.

## Architecture

```
src/
  lib.rs    -- GatePlugin (ParametricInPlacePlugin), GatePluginParams, GateData
  params.rs -- Centralized parameter specs
```

Data flow: Input -> optional sidechain HPF (Butterworth) -> level detection (peak/RMS) -> gain computation (threshold, ratio, knee, hysteresis, range) -> attack/hold/release envelope -> lookahead delay -> gain application with dry/wet mix.

**Key types:**

- `GatePlugin` -- Main plugin implementing `ParametricInPlacePlugin`. Uses `LevelDetector` and `LookaheadBuffer` from sotf-host.
- `GatePluginParams` -- Serde config with all gate parameters.
- `GateData` -- Real-time monitoring data (input levels, open/closed state, per-channel attenuation).

## Key Public API

- `GatePlugin::try_new(...) -> Result<Self, String>` -- validated construction
- `GatePlugin::try_from_params(channels, params) -> Result<Self, String>` -- validated preset construction
- Exposes immutable `GateData` snapshots through `get_data()` for UI monitoring
- Implements `ParametricInPlacePlugin` trait

**Parameters:** `threshold` (-80 to 0 dB), `ratio` (1:1 to 100:1), `attack` (0.1-50 ms), `hold` (0-1000 ms), `release` (10-2000 ms), `mix` (0-1), `link_channels` (bool), `sidechain_hpf_hz` (0-200 Hz), `sidechain_hpf_order` (2nd/4th), `detection_mode` (Peak/RMS), `sidechain_external` (bool), `range_db` (0-120 dB; zero means unlimited), `hysteresis_db` (0-12 dB), `knee_db` (0-20 dB), `lookahead_ms` (0-20 ms).

## Testing

```bash
cargo test -p sotf-plugin-gate
```

## Important Notes

- Sidechain HPF uses Butterworth filters from `math-iir-fir` (`peq_butterworth_highpass`). Order options are 2nd (-12 dB/oct) and 4th (-24 dB/oct).
- Lookahead delays audio but not the sidechain, allowing the gate to "see ahead" and open before transients arrive. Max 20ms.
- Hysteresis creates separate open/close thresholds to prevent chattering: close threshold = threshold - hysteresis_db.
- Range limits maximum attenuation (0 = unlimited, default 80 dB).
- Uses fast math (`fast_log10`, `fast_pow10`) from `math-dsp` for performance.
- FTZ/DAZ enabled and denormals flushed post-processing.
- Call `initialize()` before processing. Every callback must match its sample rate and exact checked interleaved buffer size.
- Link mode, HPF frequency/order, detection mode, external-sidechain mode, and lookahead are structural and require graph rebuild; exact runtime no-ops are allowed.
- Programme processing, realtime setters, and reset allocate nothing. Non-finite programme/sidechain values are treated as silence before reaching state.
- Diagnostics publish on a sample-derived 30 Hz cadence and held snapshots remain immutable.
