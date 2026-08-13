# 0.5.27

## Improvements

- Removed the redundant planar full-chunk copy: the preallocated residual planes now feed rubato directly in both processing and drain paths.
- Added complete-stream rate, spectral-rejection, callback-partition invariance, dynamic-ratio allocation, and callback-deadline regression evidence.
- Documented why fixed-size input callbacks cannot imply the same fixed output frame count across different sample-rate clock domains; produced frame counts remain authoritative.

# 0.5.26

## Fixes

- Added object-safe allocation-free plugin/host draining and engine EOF propagation, including
  causal draining through downstream stateful plugins.
- Replaced one-chunk discard estimation with rubato's cumulative complete-stream procedure so
  leading delay and final sinc ringing are preserved exactly.
- Negotiated sample rates per host node after rate-changing plugins and reset pending state on stop.
- Added exhaustive residual-boundary/extreme-ratio/quality count tests, transactional multi-chunk
  retry, bridge choice roundtrips, unity non-finite policy, estimator channel matrix, and realtime
  ratio-automation allocation coverage.
- Clarified capacity, availability, latency-domain, unity, dynamic-ratio, quality, and drain APIs.

# 0.5.25

## Fixes

- Preserve the actual zero/burst output frame count of rate-changing plugins in `DawHost`; sub-chunk Resampler calls are no longer padded with input-rate silence.
- Reject host sample rates that disagree with the configured Resampler input clock.
- Validate process and flush destination capacity before consuming residual input or advancing rubato, making buffer-error retries transactional.
- Make equal-rate, non-dynamic conversion a bit-exact, zero-latency passthrough for arbitrary block partitions.
- Express latency entirely in output frames by converting the chunk priming interval through the active ratio.
- Expose quality as the canonical integer choice used by ParamBridge and reject live post-activation quality rebuilds that allocate and discard filter/residual state.
- Remove cached-parameter reconstruction from runtime ratio updates and always report `Some(0)` for known zero-frame output.
- Split immediately available output (`available_output_frames`) from conservative destination capacity and add an explicit `flush_output_frames_max` contract.
- Correct the crate guide/API documentation and expand QA/test coverage for these contracts.

## Remaining limitations

- The object-safe `Plugin`/engine EOF drain contract and exact complete sinc-tail trimming require a coordinated host/engine design; the concrete `flush()` helper still processes one padded partial chunk.
- The planar residual-to-rubato copy and scalar interleave/deinterleave loops remain candidates for profile-guided optimization.

# 0.5.24

## Fixes

- **QA binary:** The performance benchmark now appends each block's output into `bench_output`
  instead of reusing `bench_output[..rt_max_out * channels]` for every block. This keeps the QA
  benchmark's output layout correct if it is extended to validate rendered samples.

# 0.5.23

> Historical note: the concrete one-step flush and latency limitations below
> were superseded by the complete-stream drain and output-domain latency
> contracts in 0.5.25-0.5.26.

## Fixes

- **Critical – latency:** `latency_samples()` now returns `resampler.output_delay()` (rubato's
  exact FIR group delay including ring-buffer and polyphase offsets) instead of the stale
  `sinc_len / 2` heuristic. For a 44.1 → 48 kHz Medium-quality resampler the old value was 64;
  the correct value from rubato is higher. Hosts that use `latency_samples()` for delay
  compensation were placing resampled audio out of phase. (`src/lib.rs:599`)

- **Critical – data loss:** Added `ResamplerPlugin::flush(&mut self, output: &mut [f32]) -> Result<usize, String>`.
  Without it, any input frames buffered in the residual (i.e. the last `0 … chunk_size-1` frames
  of a stream) were silently discarded. Offline rendering pipelines and streaming deactivations
  must call `flush()` after the last `process()` call. (`src/lib.rs:316`)

- **High – stale residual after reset:** `reset()` now zeroes `residual_input` in addition to
  setting `residual_frames = 0`. Prevents future refactors from accidentally reading old audio
  data through unbounded slice access. (`src/lib.rs:514`)

- **High – RT allocation avoidance in `rebuild_resampler()`:** Quality-preset changes
  (`set_parameter("quality", …)`) no longer reallocate `output_buffer` or `residual_input`.
  `output_frames_max()` depends only on chunk_size and ratio (not sinc length), so the existing
  buffers remain correctly sized across quality transitions. The only remaining allocation is
  rubato's internal sinc-table build, which is inherent and documented. (`src/lib.rs:244`)

- **QA binary:** `initialize()` was called with `output_sr` (48000) instead of `input_sr`
  (44100), triggering a spurious warning on every QA run. (`bin/qa_resampler.rs:18`)

## Tests added

- `test_latency_uses_rubato_output_delay` — asserts the returned latency differs from the old
  `sinc_len/2` heuristic and is > 0.
- `test_flush_empty_residual` — flush on a clean resampler returns 0 frames.
- `test_flush_recovers_trailing_frames` — a sub-chunk block is buffered (0 output), then flush
  recovers it.
- `test_variable_block_size_small` — four 256-frame blocks filling a 1024-frame chunk.
- `test_variable_block_size_non_multiple` — 1500-frame block produces output for 1 full chunk,
  flush recovers the 476-frame residual.
- `test_zero_frame_block` — zero-frame `process()` succeeds and returns 0.
- `test_cumulative_frame_count` — 10 s of 44.1 → 48 kHz resampling stays within ±2 frames/chunk
  of the theoretical output count.

## Deferred

- **3.2 Double copy of input data** (`src/lib.rs:488–505`): `residual_input` is copied into
  `input_buffer` before each rubato call. Eliminating this copy requires passing `residual_input`
  directly as the rubato adapter, which needs mutable borrow restructuring. Deferred to avoid
  scope creep; documented with a TODO comment.
- **3.3 SequentialSliceOfVecs per-chunk construction**: The adapters cannot easily be stored as
  struct members (require `&mut` on construction). Deferred.
- **3.4 `planar_to_interleaved` loop order**: Acceptable for current channel counts (≤ 8 in
  practice). Deferred.
- **1.2 Chunking latency not reported**: The `chunk_size - 1` buffering latency is inherent to
  the chunked architecture and is now documented in `flush()` and `process()`. Adding it to
  `latency_samples()` would require a cross-crate API change to report both algorithmic and
  buffering latency separately. Deferred.

# 0.5.22

## Fixes

- Account for multi-chunk input in `output_frames_for_input()` so hosts allocate enough output space.
- Include pending residual frames when estimating output capacity.
- Add regression coverage for multi-chunk resampling estimates.
- **CRITICAL** Fix `latency_samples()` to use rubato's `output_delay()` instead of the `sinc_len/2` heuristic.
- **CRITICAL** Add `flush()` API to drain residual buffered frames and prevent silent loss of trailing audio.
- **CRITICAL** Zero `residual_input` buffers in `reset()` to prevent stale audio leakage.
- **MAJOR** Eliminate allocations in `rebuild_resampler()` by reusing pre-allocated buffers (real-time safety).
