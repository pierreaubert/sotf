# 0.5.6

## Review remediation

- Smooth mono-bass enable/disable over the same time-based transition as the
  width controls, preventing a block-edge side-channel step.
- Advance crossover targets at audio rate while redesigning LR4 coefficients at
  a bounded control cadence instead of performing trigonometric design per sample.
- Remove the fixed callback-sized dry scratch allocation and mix directly from
  each input sample, eliminating both the 512 KiB reservation and arbitrary
  65,536-frame callback cap.
- Add deterministic regressions for transition continuity, bounded crossover
  redesign work, and mixed processing of a 70,000-frame callback.

# 0.5.5

## Review remediation

- Align the factory catalog with the plugin's stereo-only constructor: Stereo
  Imager is advertised for exactly two input channels, so unsupported layouts
  are rejected before instantiation rather than reported as supported.

# 0.5.4

## Review remediation

- Preserve the untouched M/S reference and apply only filtered side-band width
  corrections, making neutral width transparent and eliminating dry/wet comb filtering.
- Add a neutral fast path that is sample-identical and skips crossover work.
- Add fallible construction validation for stereo-only layout, finite/ranged
  values, crossover ordering, unknown serialized fields, and Nyquist at initialization.
- Reject crossing frequency targets rather than swapping their runtime identity.
- Validate checked buffer length and scratch bounds before every processing path.
- Retain deterministic crossover and smoother reset behavior.

# 0.5.3

## Fixes

- **Smooth crossover frequency automation** — `low_mid_freq` and
  `mid_high_freq` now retarget frequency smoothers in `set_parameter()` and
  retune the LR4 crossovers gradually per frame during processing, avoiding
  large instantaneous coefficient jumps. Added
  `test_crossover_frequency_changes_are_smoothed`.

# 0.5.2

## Fixes

- Completed the mono-bass hot-loop cleanup: `mono_bass` now becomes a scalar side-gain outside the
  per-sample loop, removing the remaining branch from the inner processing path.

# 0.5.1

## Fixes

- **[lib.rs]** Eliminated heap allocation on every `set_parameter()` call: replaced
  `rebuild_cached_parameters()` (which allocates a new `Vec<Parameter>`) with
  in-place mutation of `cached_parameters[i].default_value`. Audio-thread safe.
  (Review issue #2 — critical)

- **[lib.rs]** Overrode `validate_parameter()` to validate against the static
  `PARAMS` table instead of calling `self.parameters()` (which clones the cached
  `Vec<Parameter>`). Eliminates the allocation that occurred on every
  `set_parameter()` call via the trait default.
  (Review issue #2 — critical)

- **[lib.rs]** `reset()` now snaps all five parameter smoothers to their current
  target values via `Smoother::reset()`. Previously, a smoother mid-transition
  would resume the ramp after reset instead of jumping to the target.
  (Review issue #5 — medium)

- **[lib.rs]** Hoisted the `mono_bass` bool out of the per-sample loop. The
  branch now lives outside the hot path, reducing branch-prediction pressure.
  (Review issue #7 — medium)

- **[lib.rs]** Increased `dry_buf` pre-allocation in `initialize()` from
  `8192 * 2` to `65536 * 2` samples, covering virtually all real-world buffer
  sizes and preventing the in-callback `resize()` fallback from firing.
  (Review issue #3 — medium)

- **[lib.rs]** Added fast path in `process_in_place()`: when mix is fully wet
  (smoother target and current both ≥ 1.0), skip the `dry_buf` copy and the
  per-sample dry/wet blend entirely, saving memory bandwidth and a multiply-add
  per sample at the default mix=1.0 setting.
  (Review issue #6 — medium)

- **[lib.rs]** Added a full-dry early return in `process_in_place()` when both
  `mix_smoother.target()` and `mix_smoother.current()` are zero, bypassing all DSP
  and avoiding unnecessary per-block allocations/copies. This addresses issue #13
  (`mix=0` fast path).
  (Review issue #13 — medium)

- **[lib.rs]** `PluginInfo` now reports `env!("CARGO_PKG_VERSION")`
  so the plugin version matches crate `Cargo.toml`, fixing the previously
  stale `1.0.0` hardcoded value.
  (Review issue #11 — advisory)

- **[lib.rs]** `process_in_place()` now reinitializes plugin internals when
  `context.sample_rate != self.sample_rate`, so if a host calls process without a
  prior `initialize()` or with a late sample-rate change, crossover coefficients
  and smoothers are corrected from the context sample rate.
  (Review issue #9 — algorithmic, high)

## Deferred (cross-crate, noted for follow-up)

- **Unsmoothed crossover frequency changes (issue #1 — critical):** Adding
  per-sample IIR coefficient interpolation requires new API on
  `Lr4Crossover` in `math-iir-fir` (`process_with_coefficients`, target
  coefficient set, interpolation step). This is a cross-crate change.
  Defer to a dedicated `math-iir-fir` task.

- **`flush_denormals_inplace` incorrect threshold (issue #4 — medium):**
  The threshold `1e-30` in `math-dsp/src/simd.rs` is too large (not the
  subnormal boundary). Fix belongs in `math-dsp`. Defer.

- **Minor issues #8–#15:** Nits (crossover ordering validation, latency
  reporting, block processing, output gain). Skipped per review scope
  constraints.

# 0.5.0

## New

- Added missing qa_*.rs files for some plugins
- Added a stereo image plugin (Multi-band M/S width control), reuse the multi-band compressor
