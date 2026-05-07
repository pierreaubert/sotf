# 0.5.36

- Added `source_mode = "roomeq_recommended"` with
  `recommended_matrix_file`, allowing the plugin to load RoomEQ
  `recommended_xtc_matrix.json` FIR artifacts instead of recomputing filters
  from synthetic geometry or HRTF data.
- RoomEQ recommended matrices are validated before activation: invalid JSON,
  sample-rate mismatches, and missing filters are rejected instead of silently
  falling back to synthetic filters; matrices with two or more speaker outputs
  expose the matching plugin output channel count.
- RoomEQ recommended processing now maps stereo ear-intent input to N speaker
  outputs with dynamic overlap-add buffers, bypass handling, limiter coverage,
  and no stereo-only AutoGain assumptions for multichannel matrices.
- Fixed RoomEQ matrix tap orientation for the off-diagonal paths and added
  regression coverage for asymmetric recommended matrices.
- Async filter recompute now builds room/HRTF/filter data off-thread, publishes a pending generation, and starts the crossfade only when process() adopts the completed update.
- Large process calls are chunked through preallocated scratch buffers, so the hot path no longer resizes for offline-sized blocks.
- Unwritten output tail is zeroed before return.
- Brown-Duda now returns a complex head-shadowing gain and applies its phase in both symmetric and asymmetric plant paths.
- Added coverage proving Brown-Duda contributes a phase term.
