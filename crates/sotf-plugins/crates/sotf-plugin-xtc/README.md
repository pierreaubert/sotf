# sotf-plugin-xtc

SOTF XTC plugin for crosstalk cancellation.

Removes acoustic crosstalk to deliver a binaural-like listening experience from stereo speakers. Uses FFT-based processing with overlap-add for real-time operation.

Source topology is construction-time state. `source_mode`, `hrtf_file`,
`room_ir_file`, `recommended_matrix_file`, and `fft_size` must be changed by
rebuilding the plugin/graph; the callback only adopts same-width filter updates.

Rapid geometry and head-tracking changes are coalesced through one latest-only
worker per plugin instance. Completed filters are published lock-free; stale
requests are replaced before computation and no file loading occurs in the
audio callback.

Measured room IRs may be mono or stereo PCM WAV in integer or floating-point
encoding. The explicit `fft_size` early-reflection window rejects longer IRs
with a trimming/windowing error rather than silently losing their decay tail.
