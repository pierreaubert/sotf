# 0.5.6

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Another round of parameters update
- Massive update to plugins, see individual markdown plan for details (wave 5)
