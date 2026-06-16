# sotf-plugin-xtc

Crosstalk cancellation plugin for binaural-like playback over stereo speakers, using frequency-domain inverse filtering with physical head/speaker modeling.

## Architecture

```
src/
  lib.rs         -- XtcPlugin (Plugin), XtcData, main processing with overlap-add
  config.rs      -- XtcPluginParams, geometry and algorithm configuration
  filters.rs     -- HrtfTransferFunctions, XtcFilters, geometry cache, inverse filter computation
  reflections.rs -- Room reflection modeling (image source method and IR-based)
  validation.rs  -- Filter validation utilities
  params.rs      -- Centralized parameter specs
  tests.rs       -- Integration tests
```

**Algorithm pipeline:**
1. Stereo input deinterleaved and windowed (Hann, 75% overlap)
2. Forward FFT to frequency domain
3. Compute/apply XTC inverse filter matrix: models ipsilateral (direct) and contralateral (crosstalk) speaker-to-ear paths
4. Regularized inversion with frequency-dependent smoothing to avoid excessive boost
5. Optional room reflections (image source or IR-based)
6. Auto-gain normalization
7. Built-in limiter envelope to prevent clipping
8. Inverse FFT + overlap-add
9. Output as stereo (2 channels)

**Physical model:** Speaker geometry (distance, angle), head radius, head shadowing filter. Computes ipsilateral and contralateral path lengths and time delays for the inverse filter.

**Key types:**

- `XtcPlugin` -- Main plugin implementing `Plugin`. 2 input -> 2 output channels. Decomposed into cohesive sub-structs aligned with processing stages: `XtcFftConfig`, `XtcInputBuffers`, `XtcWorkBuffers`, `XtcFilterState`, `XtcOutputBuffers`, `XtcDynamics`, and `XtcDiagnostics`.
- `XtcPluginParams` -- Config: speaker distance/angle, head radius, regularization, HRTF options, room reflections.
- `XtcData` -- Monitoring data: auto-gain info, limiter envelope.
- `XtcFilters` -- Pre-computed frequency-domain XTC filter matrix (lock-free via `ArcSwap`).

## Key Public API

- `XtcPlugin::new(params) -> Self` (`lib.rs`)
- `XtcPlugin::from_params(params) -> Self` (`lib.rs`)
- Exposes `XtcData` via `analyzer_data()` for UI monitoring
- Implements `Plugin` trait: 2 input -> 2 output channels

**Parameters:** `distance_m`, `angle_deg`, `head_radius_m`, `regularization`, `low_shelf_gain`, `high_shelf_gain`, auto-gain settings, room reflection settings.

## Testing

```bash
cargo test -p sotf-plugin-xtc
```

Benchmarks:
```bash
cargo bench -p sotf-plugin-xtc --bench xtc-validation-benchmark
```

## Important Notes

- XTC filters are recomputed when geometry parameters change. Computation happens on a background thread; the audio thread picks up new filters via `ArcSwap` without blocking.
- Regularization prevents excessive filter boost at frequencies where the inverse is ill-conditioned. Higher values = safer but less cancellation.
- The built-in limiter envelope prevents output clipping that can occur with aggressive cancellation settings.
- SIMD operations: `complex_mul_simd`, `complex_mul_add_simd`, `deinterleave_stereo`, `window_mul_simd`.
- Head shadowing is modeled as a frequency-dependent lowpass filter based on head radius and angle.
- Room reflection support via two methods: image source (synthetic, from geometry) or IR-based (from measured impulse responses loaded via Symphonia).
- Uses a geometry cache (`compute_geometry_cache`) to avoid redundant trigonometric calculations when parameters haven't changed.
- FFT size 1024, 75% overlap (hop size 256) for the overlap-add processing.
