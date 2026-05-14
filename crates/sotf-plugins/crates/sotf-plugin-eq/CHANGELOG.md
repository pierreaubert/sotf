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

- **Odd-order silent truncation:** Order 3 is silently rounded to 2 by `(order/2)*2`. A proper
  fix (returning an error) would be a breaking API change. The behavior is now documented in a
  comment in `from_params`. Deferred.

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
