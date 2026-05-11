# 0.5.22

## Fixes

- Account for multi-chunk input in `output_frames_for_input()` so hosts allocate enough output space.
- Include pending residual frames when estimating output capacity.
- Add regression coverage for multi-chunk resampling estimates.
- **CRITICAL** Fix `latency_samples()` to use rubato's `output_delay()` instead of the `sinc_len/2` heuristic.
- **CRITICAL** Add `flush()` API to drain residual buffered frames and prevent silent loss of trailing audio.
- **CRITICAL** Zero `residual_input` buffers in `reset()` to prevent stale audio leakage.
- **MAJOR** Eliminate allocations in `rebuild_resampler()` by reusing pre-allocated buffers (real-time safety).
