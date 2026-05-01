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
