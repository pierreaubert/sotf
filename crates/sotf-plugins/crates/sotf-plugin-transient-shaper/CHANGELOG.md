# 0.5.9

## Review remediation

- Smooth sensitivity thresholds and output gains as linear coefficients per sample,
  avoiding transcendental work in the callback loop, and verify rendered output is
  invariant to callback partitioning.
- Link envelope detection across channels so asymmetric transients retain their
  inter-channel gain ratio.
- Replace the callback-count meter throttle with a 30 Hz sample-derived window
  that accumulates extrema and reports attenuation-only shaping.
- Bound shaping gain and apply a linked soft peak ceiling only to plugin-generated
  boosts, without moving the stereo image or changing neutral overrange passthrough.
- Reuse the cached parameter schema storage during updates instead of rebuilding
  and allocating the full schema.
- Document the implemented shape detector and sensitivity gate accurately, and
  add exact oversized-buffer rejection before state mutation.

# 0.5.8

## Fixes (2026-08-12 review follow-up)

- Route the shared facade factory through the validated constructor so zero
  channels and non-finite or out-of-range preset values are rejected instead
  of silently clamped.

# 0.5.7

## Fixes (2026-08-12 review remediation)

- Attack shaping now uses only the positive fast-minus-slow envelope component, so the Attack
  control no longer applies inverted gain to decay tails.
- Gain monitoring now reports the largest absolute gain deviation, including attenuation-only
  blocks.
- Added fallible validated construction and routed the plugin bridge factory through it, rejecting
  zero channels and non-finite/out-of-range parameters.
- Initialization rejects zero sample rate; processing checks sample-count overflow and buffer
  length before advancing state and limits denormal flushing to the active region.

# 0.5.6

## Fixed

- `time_to_coeff` now handles zero/negative time constants and zero sample rates defensively by
  returning instant tracking (`1.0`) instead of producing infinities.

# 0.5.5

## Fixed

- **🔴 Sensitivity parameter was a no-op** (`lib.rs:385`): `sensitivity_lin`
  scaled both envelopes identically so the computed ratios were unaffected.
  Reimplemented as a threshold gate — gain modulation is only active when the
  slow envelope exceeds `10^(sensitivity_db/20) × 1e-3` (linear).  Positive
  values raise the threshold (only loud transients are shaped); negative values
  lower it.  A new test `test_sensitivity_affects_audio_output` verifies the
  fix.

- **🟠 Output gain applied pre-mix instead of post-mix** (`lib.rs:449`): The
  old code multiplied `output_gain_lin` into the wet signal before the dry/wet
  crossfade, so its effect scaled with `mix`.  Now applied to the final mixed
  result, matching the expected "makeup gain" behaviour at all mix settings.
  Verified by `test_output_gain_post_mix`.

- **🟠 `reset()` did not reset smoothers** (`lib.rs:362–367`): After a
  transport loop, the attack/sustain/mix smoothers kept their mid-ramp state,
  causing the first shaped transients after reset to use unexpected amounts.
  `reset()` now calls `smoother.reset(target)` for all three smoothers.
  `cache_counter` is also zeroed.  Verified by `test_reset_resets_smoothers`.

- **🟡 Monitoring `last_gain` reported only the last-iterated channel**
  (`lib.rs:454`): Changed accumulator from overwrite to `max()`, consistent
  with `peak_transient`/`peak_sustain`.

- **🟡 Envelope states not flushed to zero during silence** (`lib.rs:458–463`):
  Added explicit flush (`< 1e-30 → 0.0`) on `fast_env` and `slow_env` after
  each block, preventing CPU denormal penalties on subsequent non-silent blocks.

- **🟡 Passthrough test had an overly loose tolerance and misleading comment**
  (`lib.rs`: `test_transient_shaper_passthrough`): With `attack=0, sustain=0`
  the gain is exactly `1.0` for every sample regardless of envelope state.
  Tolerance tightened from `0.05` to `1e-5`; comment corrected.

## Added

- New tests: `test_sensitivity_affects_audio_output`, `test_silence_produces_no_nan_inf`,
  `test_single_impulse_fast_envelope_responds`, `test_reset_resets_smoothers`,
  `test_output_gain_post_mix`.

## Deferred

- **🟡 Stereo envelope linking** (review §1.3): cross-crate feature, no
  existing `link` parameter infrastructure.  Noted for a future PR.
- **🟡 Rectified-sine ripple / RMS detector** (review §1.4): design trade-off
  inherent to the SPL approach; addressing it requires a larger DSP change.
- **🟡 Hardcoded clamp ranges duplicated from PARAMS** (review §2.3): low
  divergence risk with current range values; deferred to avoid cross-crate churn.
- **🟢 `time_to_coeff` defensive guard** (review §2.4): constants are
  hardcoded positive and the function is crate-private; deferred.
- **🟢 `rebuild_cached_parameters` allocates inside `set_parameter`** (review
  §3.4): host contract guarantees `set_parameter` is called off the RT thread;
  no audio-safety issue in practice.
- **🟢 QA binary assertion tolerance** (review §5.4): out of scope for this
  crate (QA binary is `required-features = ["qa"]`).

# 0.5.4

## New

- Added missing qa_*.rs files for some plugins
- Added a transient shaper plugin (Differential envelope detector)

## Fixed

- **CRITICAL**: `Sensitivity` parameter is now a detection threshold instead of a mathematical no-op. The parameter previously scaled the rectified input before both envelope followers equally, causing the scaling factor to cancel out in the ratio-based gain computation. It now gates gain modulation when the slow envelope falls below a threshold derived from `sensitivity_db`.
- **MAJOR**: `reset()` now resets parameter smoothers (`attack_smoother`, `sustain_smoother`, `mix_smoother`) to their targets, preventing stale smoother states after transport loops or host resets.
- **MAJOR**: `Output` gain is now applied to the final mixed output instead of the wet path only. Previously, makeup gain was partially or fully absent at low `mix` values; it now correctly scales the overall plugin output.
