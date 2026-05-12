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

## Deferred (cross-crate, noted for follow-up)

- **Unsmoothed crossover frequency changes (issue #1 — critical):** Adding
  per-sample IIR coefficient interpolation requires new API on
  `Lr4Crossover` in `math-iir-fir` (`process_with_coefficients`, target
  coefficient set, interpolation step). This is a cross-crate change.
  Defer to a dedicated `math-iir-fir` task.

- **`flush_denormals_inplace` incorrect threshold (issue #4 — medium):**
  The threshold `1e-30` in `math-dsp/src/simd.rs` is too large (not the
  subnormal boundary). Fix belongs in `math-dsp`. Defer.

- **Minor issues #8–#15:** Nits (crossover ordering validation, sample-rate
  in constructor, version mismatch in PluginInfo, latency reporting, block
  processing, output gain). Skipped per review scope constraints.

# 0.5.0


