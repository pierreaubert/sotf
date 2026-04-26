# 0.5.36

- Async filter recompute now builds room/HRTF/filter data off-thread, publishes a pending generation, and starts the crossfade only when process() adopts the completed update.
- Large process calls are chunked through preallocated scratch buffers, so the hot path no longer resizes for offline-sized blocks.
- Unwritten output tail is zeroed before return.
- Brown-Duda now returns a complex head-shadowing gain and applies its phase in both symmetric and asymmetric plant paths.
- Added coverage proving Brown-Duda contributes a phase term.
