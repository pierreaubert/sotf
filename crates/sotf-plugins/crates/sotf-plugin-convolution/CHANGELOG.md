# 0.5.6

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- **CRITICAL**: Fixed UPC path dropping output samples when host buffer sizes are not exact multiples of `PARTITION_SIZE` (1024). Output is now buffered in a per-channel ring queue so that all computed samples are emitted.
- **CRITICAL**: Fixed `reset()` not clearing NUPC engines, overlap accumulators, input buffers, or parameter smoothers, which caused state leakage across playback passes.
- **MAJOR**: Fixed broken parameter smoothing in NUPC path — `mix` and `gain` are now advanced per sample instead of held constant across the whole buffer.
- **MAJOR**: Fixed broken parameter smoothing in UPC path — `mix` and `gain` now use a linear ramp across each 1024-sample partition instead of a single scalar value.
- **MAJOR**: Moved IR loading out of the real-time audio thread. `set_parameter("ir_file", ...)` now spawns a background worker thread and swaps in the loaded state asynchronously via `process_in_place`, eliminating file I/O, FFT planning, and heap allocations from the audio callback.

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Another round of parameters update
- Massive update to plugins, see individual markdown plan for details (wave 5)
