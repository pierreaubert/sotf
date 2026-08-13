# sotf-plugin-limiter

Peak limiter plugin with true peak detection, lookahead, dual release, ISP mode, and feed-forward/feedback topology.

## Architecture

```
src/
  lib.rs    -- LimiterPlugin (ParametricInPlacePlugin), LimiterPluginParams, LimiterData
  params.rs -- Centralized parameter specs
```

Data flow: Input -> true peak detection (optional, 4x oversampled) -> gain computation (threshold, soft/hard knee) -> lookahead delay -> envelope with attack/release -> dual release (optional fast+slow) -> ISP correction feedback loop (optional) -> gain application with dry/wet mix.

**Key types:**

- `LimiterPlugin` -- Main plugin implementing `ParametricInPlacePlugin`. Uses `TruePeakDetector` and `DualRelease` from sotf-host.
- `LimiterPluginParams` -- Serde config with limiter parameters.
- `LimiterData` -- Real-time monitoring: gain reduction (dB), peak level, is_limiting flag, per-channel ISP dBTP.

## Key Public API

- `LimiterPlugin::new(channels, threshold_db) -> Self` (`lib.rs`)
- `LimiterPlugin::from_params(channels, params) -> Self` (`lib.rs`)
- Exposes `LimiterData` via `analyzer_data()` for UI monitoring
- Implements `ParametricInPlacePlugin` trait

**Parameters:** `threshold` (-24 to 0 dBFS), `release` (1-5000 ms), `lookahead` (0-10 ms), `soft` (bool, soft knee), `true_peak` (bool, 4x oversampled detection), `isp_mode` (bool, inter-sample peak correction), `dual_release` (bool, fast+slow envelope), `mix` (0-1), `feed_forward` (bool), `link_amount` (0-1, stereo link).

## Testing

```bash
cargo test -p sotf-plugin-limiter
```

## Important Notes

- True peak detection uses the ITU-R BS.1770 Table-2 4x polyphase FIR. Its six-sample detector delay is covered by ISP lookahead.
- ISP mode is predictive and requires hard mode, 100% wet mix, and at least six lookahead samples. Output ISP feedback is supplementary verification/correction, not the primary detector.
- Dual release uses `DualRelease` from sotf-host with separate fast and slow time constants for natural-sounding gain recovery.
- Every nonzero-lookahead configuration is predictive. Sliding maxima use preallocated monotonic queues with amortized O(1) updates.
- Link amount controls stereo coupling: 0 = independent channels, 1 = fully linked (max of all channels used).
- Lookahead is structural after initialization because it changes host latency.
- Uses fast math (`fast_log10`, `fast_pow10`) from `math-dsp`.
- FTZ/DAZ enabled and denormals flushed post-processing.
