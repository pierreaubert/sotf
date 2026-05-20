# 0.5.5

## Fixes

- Documented that Haas delay is an intentional right-channel widening effect and is not included in
  host-reported latency. Added regression coverage so host latency remains the STFT latency only.

# 0.5.4

## Fixes

- **Bug #1** (`src/lib.rs:257-258`): `compute_freq_width_curve()` hardcoded `low_hz=300` and `high_hz=2000`; now uses `self.decor_low_hz` and `self.decor_high_hz`. Changing these parameters in the UI now affects the frequency-dependent width ramp.
- **Bug #2** (`src/lib.rs:232`): `generate_decorrelation_filter()` hardcoded the active band to `300..=15000 Hz`; now uses `self.decor_low_hz` as the lower bound (all bins at or above `decor_low_hz` receive a random phase). The upper bound is Nyquist so that bins above `decor_high_hz` — where the width curve is 1.0 — still receive decorrelation.
- **Bug #8** (`src/lib.rs:193-199`): Setting `decor_low_hz` or `decor_high_hz` via `set_parameter` now immediately calls `generate_decorrelation_filter()` so changes take effect without requiring a plugin reinitialisation.
- **Bug #3** (`src/params.rs:35-73`, `src/lib.rs:87-89`): `enable_comp_eq` and `comp_eq_depth_db` were stored, exposed in the UI, and serialised, but no compensation EQ was ever applied. Both parameters and their struct fields have been removed to avoid presenting non-functional controls. Old presets containing these fields are deserialized without error (serde ignores unknown fields). PARAMS now has 5 entries (was 7); LAYOUT's `config` panel is now empty.
- **Bug #5** (`src/lib.rs:467-476`): The `break` that could exit the output-fill loop while `output_pos < nf` has been replaced with a zero-fill fallback, preventing stale audio data from leaking into the tail of the output buffer.

## Deferred

- **Bug #4** (Haas delay L/R time offset): This is intentional psychoacoustic design (Haas effect). A "Time-Align Outputs" option was suggested but is a UX/feature addition — deferred.
- **Bug #6** (Random-phase tonal cancellation): Replacing the random-phase decorrelator with a Schroeder-style allpass is an algorithmic redesign — deferred.
- **Bug #7** (SIMD vectorisation of per-bin lerp): Profiling first required; deferred.

# 0.5.3

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
