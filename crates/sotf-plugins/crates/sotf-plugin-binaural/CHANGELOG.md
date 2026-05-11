# 0.5.18

## Fixes

- Fix CRITICAL reflection panning formula off by 45° (`room.rs`): replaced ad-hoc cosine panning with standard constant-power sine-law panning.
- Fix CRITICAL duplicate second-order ISM reflections (`room.rs`): deduplicate orthogonal wall-pair mirrors before creating Reflection objects.
- Fix MAJOR head tracking reloading SOFA from disk on every 0.5° change (`lib.rs`): reuse cached `SofaFile` from `BinauralState`.
- Fix MAJOR near-field shadowing severely underestimating attenuation (`hrtf.rs`): replaced weak ad-hoc model with Brown-Duda spherical-head shadowing filter, and corrected shadowed-ear selection.
- Fix MAJOR arbitrary -3 dB dual-mono attenuation on LFE path (`filter.rs`): removed `FRAC_1_SQRT_2` multiplier from `lfe_gain` and updated normalization accordingly.

# 0.5.17

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added spectral crossfade to binaural morphing

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fix Phase 4 review: FDN param corruption, rebuild_cached_parameters, spatial ordering

## Changes

- Complete Phase 4: adaptive threshold, denoiser DSP, FDN reverb, binaural preview
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
