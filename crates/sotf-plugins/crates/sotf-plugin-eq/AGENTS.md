# sotf-plugin-eq

Parametric equalizer plugin with biquad and SVF filter topologies, supporting cascaded high-order filters and coefficient interpolation.

## Architecture

```
src/
  lib.rs    -- EqPlugin (ParametricPlugin), BiquadFilterConfig, EqPluginParams, band transitions
  params.rs -- Centralized parameter specs
  ui.rs     -- GPUI UI (behind gpui-ui feature)
```

Data flow: `EqPluginParams` (JSON config with filter list) -> `EqPlugin::from_params()` -> per-channel, per-band, per-stage biquad filter banks -> sample-by-sample processing with coefficient interpolation.

**Key types:**

- `EqPlugin` -- Main plugin struct implementing `ParametricPlugin`. Holds `filters[channel][band][stage]` (3D Vec of Biquad).
- `EqPluginParams` -- Serde config: `filters` (global), `channel_filters` (per-channel override), `auto_gain` settings.
- `BiquadFilterConfig` -- Per-band config: filter_type, freq, q, db_gain, order (2/4/6/8).
- `BandTransition` -- Coefficient interpolation state for click-free parameter changes (~5ms crossfade).

**Filter topologies:**

- Topology 0 (default): Biquad (Direct Form I or Transposed Direct Form II via `use_tdf2`).
- Topology 1: SVF (zero-delay feedback) via `SvfFilter`. Single stage per band, no cascading.

**High-order filters:** Orders 4/6/8 cascade N/2 biquad stages with Butterworth Q staggering. Gain is split equally across stages.

**Oversampling:** Optional 2x or 4x oversampling via `Oversampler` for reduced cramping near Nyquist.

## Key Public API

- `EqPlugin::from_params(channels, params) -> Result<Self, String>` -- Construct from JSON config (`lib.rs`)
- `EqPlugin::new(channels, filters) -> Self` -- Construct from filter list (`lib.rs`)
- `BiquadFilterConfig` -- Filter definition struct (`lib.rs`)
- `EqPluginParams` -- Top-level config with optional per-channel filters and auto-gain (`lib.rs`)
- Implements `ParametricPlugin` trait (same channel count in/out)
- Exposes `AutoGainData` via `analyzer_data()` for UI monitoring

## Testing

```bash
cargo test -p sotf-plugin-eq
```

## Important Notes

- Filter parameter changes use ~5ms coefficient interpolation (`BandTransition`) to avoid clicks. During transition, old and new coefficients are linearly blended sample-by-sample.
- High-order filters (order > 2) use Butterworth Q staggering: `Q_k = 1 / (2 * cos(pi * (2k+1) / 2N))`. For peak filters, user Q is multiplied by Butterworth Q; for LP/HP/shelf, Butterworth Q is used directly.
- Per-channel filters (`channel_filters`) override the global `filters` list for specific channels. If `channel_filters` is set, each channel uses its own filter bank.
- SVF topology does not support cascading (always single stage per band regardless of order setting).
- Q bounds are per filter type, with two ceilings (`params.rs` is the single source of truth: `Q_MIN`/`Q_MAX_STANDARD`/`Q_MAX_OPTIMIZED`/`Q_MAX_NOTCH`, `q_max_for`/`q_max_ui`/`clamp_q`): validation/loading accepts 0.1–20 for non-notch types (matching the optimizers' `max_q` ceiling) and 0.1–40 for Notch; UI knobs/drags edit within 0.1–10 for non-notch (`q_max_ui`) and 0.1–40 for Notch. The DSP mirrors the validation ceiling in `lib/consts.rs` (`Q_MAX`/`Q_MAX_NOTCH`, f32). Switching a band away from Notch re-clamps Q to 20.
- Parameter IDs follow the pattern `band_{band}_freq`, `band_{band}_q`, `band_{band}_gain`, `band_{band}_filter_type`, `band_{band}_order`.
- The `gpui-ui` feature adds a GPUI-based UI component (optional dependency).
