# 0.5.73

## Fixes

- Keep the five-millisecond coefficient transition measured in source audio time at Off, 2x,
  and 4x oversampling, including oversampler FIR priming callbacks.
- Add real-audio Q, gain, and filter-type transition tests plus callback-partition invariance
  coverage at every oversampling factor.
- Verify processed stereo frequency, Q, gain, and type transitions sample-for-sample against two
  independently stateful mono references at Off/2x/4x and whole, single-frame, and irregular
  callback partitions.

# 0.5.72

## Fixes (2026-08-12 review completion)

- Preserve the advertised user Q for high-order peak, notch, bandpass, and all-pass
  cascades, including parameter round-trips and runtime order/Q edits.
- Make filter-bank replacement a fallible transactional control-thread operation that
  validates every channel, normalizes the runtime sample rate and realization, rebuilds
  SVF/schema state, and leaves the previous bank intact on failure.
- Enforce the oversampling initialization contract at 4096 frames and prove active 4x
  processing, including coefficient transitions, performs no callback allocation.
- Align the custom/AU UI with the five per-band fields and render standard-biquad previews
  using the actual order and supplied sample rate.
- Carry topology, TDF-II, auto gain, and oversampling through engine and plugin factories.
- Use the crate version in plugin metadata and tighten direct in-place buffer validation.

# 0.5.71

## Fixes (2026-08-12 review remediation)

- Keep the engine-facing EQ settings schema aligned with all five global EQ
  controls, including auto-gain and oversampling. These controls now survive
  settings round-trips and the accessor count check cannot regress silently.

# 0.5.69

## Fixes

- Recompute the automatic Bark warped-biquad lambda when the plugin is reinitialized at a new sample rate; explicit lambda values remain fixed.

## Fixes (2026-08-12 review remediation)

- Preserve each channel's filter configuration when building SVF banks and when capturing
  coefficient-transition endpoints for smoothed biquad edits.
- Reject SVF plus internal oversampling, so reported latency always matches the route executed.
- Validate standard filter construction consistently: nonzero channels/sample rate, finite and
  documented frequency/Q/gain ranges, and frequency below Nyquist.
- Process only the host-declared active buffer region and return errors for undersized or
  overflowing ordinary process buffers.
- Expose Auto Gain and Oversampling through schema/current-value snapshots.

# 0.5.68

## Fixes

- **Restore oversampler across unwinds** — the oversampling path now restores
  the temporarily moved `Oversampler` before resuming a panic, and returns a
  clear error if the oversampler is missing while oversampling is enabled.
  `test_oversampling_2x_processes_audio` now asserts the oversampler is restored.

# 0.5.67

## Fixes

- **Odd filter orders are rejected** (`src/lib.rs`): `from_params` and the
  runtime `band_N_order` parameter now return a clear error for odd orders
  instead of silently rounding down. Added `test_from_params_rejects_odd_filter_order`
  and `test_set_parameter_rejects_odd_filter_order`.
- **Reset preserves coefficients** (`src/lib.rs`): `reset()` now calls
  `Biquad::reset()` to clear filter state without reconstructing coefficients.
  Added `test_reset_preserves_biquad_coefficients`.

# 0.5.66

## New

- Added config-driven advanced EQ filter families:
  - `topology: "warped_biquad"` uses `math-iir-fir::WarpedBiquad` with optional `lambda`
    and Bark-scale lambda by default.
  - `topology: "kautz_filter"` uses `math-iir-fir::KautzFilter` as a dry-plus-correction
    modal filter bank with `kautz_sections`.

## Fixes

- **Multi-stage interpolation (lib.rs ~82, ~510, ~935):** `BandTransition` now stores
  per-stage `old_coeffs_per_stage`/`new_coeffs_per_stage` vectors instead of a single
  primary-stage coefficient pair. During a parameter change on a high-order band (order 4/6/8),
  all N/2 biquad stages now linearly interpolate their coefficients simultaneously, eliminating
  the glitch where stage 0 morphed while stages 1+ snapped instantly to new Butterworth-Q values.

- **AllPass missing from BAND_TEMPLATE (params.rs ~56):** Added `"AllPass"` to the filter-type
  choice list in `BAND_TEMPLATE`. The EQ plugin has always been able to process allpass filters
  (tested by existing `test_eq_allpass_filter_type_parses`), but the param-spec omission meant
  users could not create allpass bands through the standard UI parameter bridge.

## Deferred

- **`reset()` uses `Biquad::new` instead of a `reset_state()` method:** Review suggested a
  dedicated `Biquad::reset_state()` that zeroes z1/z2 without reallocating coefficients. This
  requires a cross-crate change to `math-iir-fir`. Deferred.

- **Oversampler `take()`/`put-back` panic safety:** Review suggested `catch_unwind` to restore
  `oversampler` on panic. In practice the closure body cannot panic (it calls well-tested DSP),
  and adding `catch_unwind` would suppress legitimate bugs. Deferred pending a redesign of
  `Oversampler::process` to take `&mut self` (cross-crate).

- **Cache-unfriendly interleaved processing / SIMD:** Performance improvements for the hot path.
  Out of scope for this fix pass.

- **Butterworth Q multiplication for peaking filters:** The current behavior (multiplying user Q
  by Butterworth Q for each stage) is a valid design choice. Added documentation comment in
  `create_band_stages`. Deferred from "fix" to "document".

# 0.5.65

## New

- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed tutorial not working on windows, preference panel: remove unused plugin panel, added a misc one, added a parameter for #cpu core

## Changes

- Listening + bug hunting session on plugins
- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Road the working AU plugins
- Cleanup: another round of clippy
- Another round of parameters update
- Massive update to plugins, see individual markdown plan for details (wave 5)
- Massive update to plugins, see individual markdown plan for details (wave 2)
