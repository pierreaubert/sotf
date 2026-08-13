# 0.5.6

## Fixes

- Preserved canonical AutoGain position, headroom policy, and SPL calibration
  state in CLI rack/traditional builders; Fletcher-Munson compatibility now
  explicitly enters calibrated Auto mode.
- Marked the legacy `auto_gain_enabled` parameter as a hidden layout
  compatibility alias while keeping `auto_gain_position` as the sole visible,
  canonical AutoGain control.

# 0.5.5

## Fixes

- Replaced seven independently sampled ISO sections with a jointly optimized 20-section bank and response-error tests over all standard points and 20–90 phon extremes.
- Made preservation of the 1 kHz reference the default; headroom normalization is now an explicit visible policy based on the realized full-band cascade peak.
- Added a 256-sample old/new-bank crossfade for coefficient and mode changes, and moved all Auto coefficient preparation out of `process_in_place`.
- Auto mode now requires an explicit measured SPL-at-0-dB calibration. AutoGain position is the canonical typed runtime state; the old enabled boolean is a compatibility alias only.
- Filter design is safe at 16–192 kHz, reset is in-place and allocation-free, and standalone processing no longer changes the audio thread floating-point mode.
- Completed Fletcher–Munson field migration, canonical schema/UI wiring, high-channel QA, tail-latency benchmarks, and focused regressions for every review finding.

# 0.5.4

## Fixes

- Headroom compensation now responds only to positive cascade gain; cuts no longer cause additional broadband attenuation, and identical ISO response scans run once rather than once per channel.
- Construction rejects unsupported per-channel curves, invalid channel counts, unknown AutoGain positions, non-finite/out-of-range fields, and invalid modes instead of silently accepting them.
- Initialization validates sample-rate-safe filter frequencies and propagates sample-rate changes into AutoGain monitors and time constants.
- Pre AutoGain now measures the final post-EQ buffer, reset clears AutoGain and compensation smoother state, and processing requires exact buffer dimensions plus the initialized sample rate.

# 0.5.3

## Fixes

- Documented the manual-mode shelf cascade as an intentional approximation: each low/high shelf is
  implemented as two half-gain cascaded shelves for a steeper transition, so the corner-region
  response is not an exact additive gain curve. Added regression coverage for the passband gain.

# 0.5.2

## Fixes

- **Bug #1 (critical)**: `update_comp_gain_smoother` now evaluates the combined
  frequency response of all ISO 226 filter bands on a 128-point log-spaced grid
  (20 Hz – 20 kHz) instead of sampling only the 7 band-centre frequencies.
  The old approach missed constructive interference (ripple peaks) between centres,
  underestimating the true peak gain by several dB and allowing potential clipping
  when all bands had large gains (e.g. playback=40 dB SPL vs reference=83 dB SPL).
  Applies to `src/lib.rs: update_comp_gain_smoother`.

- **Bug #2 (high)**: Auto-gain measurement (`ag.measure_input` / `ag.measure_output`)
  now happens every process block instead of every 10 blocks.  The old 10-block
  throttle caused up to ~107 ms of stale loudness data at 512-sample / 48 kHz,
  producing audible level jumps on rapid loudness changes.  The `cache_update_counter`
  field has been removed as it is no longer needed.
  Applies to `src/lib.rs: process_in_place`.

- **Bug #3 (high)**: In Post mode, `ag.apply_compensation` is now called BEFORE
  `ag.measure_output`.  Previously, output was measured before the AutoGain's own
  gain was applied; the feedback loop therefore never saw the actual output level,
  causing potential positive feedback divergence when the AutoGain was boosting.
  Applies to `src/lib.rs: process_in_place (AutoGainPosition::Post branch)`.

- **Bug #5 (medium)**: `rebuild_iso_filters()` is no longer called when
  `playback_level_db` or `reference_level_db` changes while `mode_index == 0`
  (Manual mode).  ISO filters are irrelevant in Manual mode; rebuilding them was
  harmless but wasteful.
  Applies to `src/lib.rs: set_parameter`.

- **Bug #7 (medium)**: `maybe_rebuild_auto_filters()` is now guarded by
  `if self.mode_index == 2` at the call site in `process_in_place`, avoiding an
  unconditional per-block function call in Manual and ISO modes.  The function
  itself already had an early return, but the call overhead is now eliminated.
  Applies to `src/lib.rs: process_in_place`.

## Deferred

- Review #4 (manual mode shelf cascade approximation): cascading two shelves each
  at `gain/2` is acknowledged as a sub-dB approximation, not an exact sum.  The
  existing behaviour is intentional for a steeper slope; documenting this as a
  known approximation is deferred.
- Review #6 (per-sample scalar biquad processing): SIMD optimisation of the biquad
  chain is deferred; current throughput (16 ch × 7 bands × 48 kHz) is within budget.

# 0.5.1

## New

- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed iso226 implementation in loudness compensation plugin

## Changes

- Merged loudness compensation and FM into 1 plugin
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 5)
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
