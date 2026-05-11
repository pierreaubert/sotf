# 0.5.4

## New

- Added missing qa_*.rs files for some plugins
- Added a transient shaper plugin (Differential envelope detector)

## Fixed

- **CRITICAL**: `Sensitivity` parameter is now a detection threshold instead of a mathematical no-op. The parameter previously scaled the rectified input before both envelope followers equally, causing the scaling factor to cancel out in the ratio-based gain computation. It now gates gain modulation when the slow envelope falls below a threshold derived from `sensitivity_db`.
- **MAJOR**: `reset()` now resets parameter smoothers (`attack_smoother`, `sustain_smoother`, `mix_smoother`) to their targets, preventing stale smoother states after transport loops or host resets.
- **MAJOR**: `Output` gain is now applied to the final mixed output instead of the wet path only. Previously, makeup gain was partially or fully absent at low `mix` values; it now correctly scales the overall plugin output.
