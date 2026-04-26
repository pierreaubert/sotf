# 0.5.21

- Removed the hot-path dry_buffer.resize() path entirely by mixing/delta-monitoring against the original sample right before overwrite.
- Added strict buffer-size validation so malformed host buffers return Err instead of panicking.
- Advanced threshold smoothing per STFT hop instead of jumping to the block-end value before the first hop in large blocks.
- Marked FFT size as structural/setup because changing it rebuilds the STFT state.
- Added regressions for buffer mismatch and mix=0 passthrough during latency fill.

