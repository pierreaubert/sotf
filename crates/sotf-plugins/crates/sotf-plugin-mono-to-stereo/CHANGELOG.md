# 0.5.4

## Fixes

- Start `from_params` at the requested stereo width instead of smoothing from the default width.
- Make decorrelation phase generation deterministic so QA is not dependent on random filter phases.
- Update mono-to-stereo QA energy checks to measure settled, representative audio regions.

# 0.5.3

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
# Unreleased

## Fixes
- Fixed hardcoded decorrelation frequency range: `generate_decorrelation_filter()` now uses `self.decor_low_hz` and `self.decor_high_hz` instead of hardcoded 300 Hz and 15000 Hz.
- Fixed hardcoded width curve frequencies: `compute_freq_width_curve()` now uses `self.decor_low_hz` and `self.decor_high_hz` instead of hardcoded 300 Hz and 2000 Hz.
- Removed dead `enable_comp_eq` and `comp_eq_depth_db` parameters from UI layout to avoid misleading users.

