# sotf-plugin-gate

Noise gate plugin with sidechain HPF, configurable detection modes, lookahead, hysteresis, and soft knee.

## Architecture

```
src/
  lib.rs    -- GatePlugin (InPlacePlugin), GatePluginParams, GateData
  params.rs -- Centralized parameter specs
```

Data flow: Input -> optional sidechain HPF (Butterworth) -> level detection (peak/RMS) -> gain computation (threshold, ratio, knee, hysteresis, range) -> attack/hold/release envelope -> lookahead delay -> gain application with dry/wet mix.

**Key types:**

- `GatePlugin` -- Main plugin implementing `InPlacePlugin`. Uses `LevelDetector` and `LookaheadBuffer` from sotf-host.
- `GatePluginParams` -- Serde config with all gate parameters.
- `GateData` -- Real-time monitoring data (input levels, open/closed state, per-channel attenuation).

## Key Public API

- `GatePlugin::new(channels) -> Self` -- Default construction (`lib.rs`)
- `GatePlugin::from_params(channels, params) -> Self` -- From JSON config (`lib.rs`)
- Exposes `GateData` via `analyzer_data()` for UI monitoring
- Implements `InPlacePlugin` trait

**Parameters:** `threshold` (-80 to 0 dB), `ratio` (1:1 to 100:1), `attack` (0.01-100 ms), `hold` (0-500 ms), `release` (1-5000 ms), `mix` (0-1), `link_channels` (bool), `sidechain_hpf_hz`, `sidechain_hpf_order` (6/12/18/24 dB/oct), `detection_mode` (peak/rms), `sidechain_external` (bool), `range_db` (max attenuation cap), `hysteresis_db`, `knee_db`, `lookahead_ms` (0-20 ms).

## Testing

```bash
cargo test -p sotf-plugin-gate
```

## Important Notes

- Sidechain HPF uses Butterworth filters from `math-iir-fir` (`peq_butterworth_highpass`). Order options: 6, 12, 18, 24 dB/oct.
- Lookahead delays audio but not the sidechain, allowing the gate to "see ahead" and open before transients arrive. Max 20ms.
- Hysteresis creates separate open/close thresholds to prevent chattering: close threshold = threshold - hysteresis_db.
- Range limits maximum attenuation (0 = unlimited, default 80 dB).
- Uses fast math (`fast_log10`, `fast_pow10`) from `math-dsp` for performance.
- FTZ/DAZ enabled and denormals flushed post-processing.
