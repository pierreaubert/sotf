# sotf-plugin-limiter

Peak limiter plugin with true peak detection, lookahead, dual release, ISP mode, and feed-forward/feedback topology.

## Architecture

```
src/
  lib.rs    -- LimiterPlugin (ParametricInPlacePlugin), LimiterPluginParams, LimiterData
  params.rs -- Centralized parameter specs
```

Data flow: Input -> true peak detection (optional, rate-appropriate BS.1770 interpolation) -> gain computation (threshold, soft/hard knee) -> lookahead delay -> envelope with attack/release -> dual release (optional fast+slow) -> ISP correction feedback loop (optional) -> gain application with dry/wet mix.

**Key types:**

- `LimiterPlugin` -- Main plugin implementing `ParametricInPlacePlugin`. Uses `TruePeakDetector` and `DualRelease` from sotf-host.
- `LimiterPluginParams` -- Serde config with limiter parameters.
- `LimiterData` -- Real-time monitoring: gain reduction (dB), peak level, is_limiting flag, per-channel ISP dBTP.

## Key Public API

- `LimiterPlugin::new(channels, threshold_db) -> Self` (`lib.rs`)
- `LimiterPlugin::from_params(channels, params) -> Self` (`lib.rs`)
- Exposes `LimiterData` via `analyzer_data()` for UI monitoring
- Implements `ParametricInPlacePlugin` trait

**Parameters:** `threshold` (-24 to 0 dBFS), `release` (1-5000 ms), `lookahead` (0-10 ms), `soft` (bool, soft knee), `true_peak` (bool, BS.1770 detection at 4x/2x/1x according to input rate), `isp_mode` (bool, inter-sample peak correction), `dual_release` (bool, fast+slow envelope), `mix` (0-1), `feed_forward` (bool), `link_amount` (0-1, stereo link).

## Testing

```bash
cargo test -p sotf-plugin-limiter
```

## Important Notes

- True peak detection uses a 49-tap Hann-windowed sinc interpolator compatible with BS.1770 measurement requirements: 4x below 96 kHz, 2x below 192 kHz, and native sample peaks at 192 kHz and above. ISP lookahead covers its rate-dependent six-, twelve-, or zero-sample detector delay.
- ISP mode is predictive and requires hard mode, 100% wet mix, and enough lookahead to cover the rate-dependent detector delay. Output ISP feedback is supplementary verification/correction, not the primary detector.
- Dual release uses `DualRelease` from sotf-host with separate fast and slow time constants for natural-sounding gain recovery.
- Every nonzero-lookahead configuration is predictive. Sliding maxima use preallocated monotonic queues with amortized O(1) updates.
- Link amount controls stereo coupling: 0 = independent channels, 1 = fully linked (max of all channels used).
- Lookahead is structural after initialization because it changes host latency.
- Uses fast math (`fast_log10`, `fast_pow10`) from `math-dsp`.
- FTZ/DAZ enabled and denormals flushed post-processing.
