# 0.5.8

## Testing

- Strengthen the two-path transfer-threshold regression to prove that ten
  consecutive candidate blocks do not promote the background path. This
  exercises the behavior that the previous five-block threshold got wrong
  instead of merely checking the configured threshold and process success.

# 0.5.7

## Correctness and DSP

- Keep the foreground echo path stable and freeze it through double-talk; only
  the background path explores, with adaptation reduced to 5% while double-talk
  is active, before sustained improvements are promoted. Regression coverage
  includes balanced double-talk and recovery after an abrupt echo-path delay change.
- Replace full-echo spectral subtraction with a bounded, smoothed residual
  leakage estimate and near-end-aware attack/release behavior. Balanced
  double-talk fixtures at multiple near/far ratios now limit near-end loss.
- Derive power, detector, gain, leakage, transfer, and wet/dry ramp coefficients
  from sample rate and block duration instead of fixed per-block constants.
- Silence non-finite microphone/reference samples at ingress and guarantee they
  cannot poison adaptive state or output.
- Keep residual-suppressor state current while dry and crossfade post-filter
  toggles over 10 ms, preventing stale-state clicks on re-enable.

## Performance and integration

- Share the reference FFT, frequency-domain delay line, and power analysis
  between foreground and background paths. Repeated AEC QA runs fell from the
  original failing 5.42% to 2.98–3.22% CPU for the standard 48 kHz / 100 ms-tail
  fixture while remaining zero-allocation.
- Use a real inverse FFT for post-filter reconstruction and remove the unused
  plugin-level forward complex FFT and full redundant spectrum.
- Make `params::Params` and `params::PARAMS` the runtime/factory schema, remove
  the duplicate f32 state type, mark structural fields consistently, and add
  main-factory/bridge layout, range, and round-trip conformance tests.

# 0.5.6

- Project each adaptive frequency-domain partition onto its causal time-domain
  support after updates, preventing non-causal circular alias energy.

# 0.5.5

## Fixes

- Centralize post-filter reconstruction during `initialize()` so rate changes
  cannot retain suppressor gains or double-talk history from the previous stream.
- Add a regression comparing reinitialized output with a fresh plugin after old
  stream input and queued output have both been exercised.

# 0.5.4

## Fixes

- Preserve the pre-IFFT echo-estimate spectrum used by residual echo suppression.
- Use previous/current reference blocks for the PBFDAF overlap-save input convention.
- Make the streaming adapter provide an exact, callback-size-independent 256-sample latency.
- Bound the realtime output FIFO to one block, removing callback-time growth for oversized buffers.
- Validate persisted sample rate, echo-tail, and step-size values before allocating adaptive state.
- Advertise echo-tail and step-size as structural parameters and reject live state-destroying changes.
- Fully clear adaptive, post-filter, partial-input, and queued-output state on reinitialization.
- Make AEC construction fallible in both plugin factories and the bridge.

## Tests

- Added spectral identity, callback segmentation/latency, invalid configuration, structural metadata,
  clean reinitialization, and above-old-capacity streaming regressions.

# 0.5.2

## Fixes

- **Issue #7 (PBFDAF hot-loop shape)** `src/pbfdaf.rs`: Rewrote echo accumulation,
  power summing, and weight updates with slice/iterator zips. This keeps the existing
  FDL layout but gives LLVM cleaner contiguous inner loops. Existing echo-cancellation
  regression coverage continues to exercise the path.

- **Issue #1 (Post-filter double-talk suppression)** `src/post_filter.rs`: Added power-ratio
  double-talk detector (DTD) to `ResidualEchoSuppressor`. When smoothed mic power exceeds
  smoothed echo-estimate power by 6 dB (factor 4), the Wiener suppression is bypassed and
  gains are guided back toward 1.0. Prevents near-end speech from sounding muffled during
  double-talk. New test: `test_post_filter_dtd_preserves_near_end_speech`.

- **Issue #2 (Negligible leakage factor)** `src/pbfdaf.rs:154`: Increased leakage constant from
  `1e-5` to `1e-3` per block. The old value produced a time constant of ≈530 s (essentially
  no leakage). The new value gives ≈5.3 s — practical weight decay when the echo path
  disappears. New test: `test_pbfdaf_leakage_factor_is_meaningful`.

- **Issue #3 (Callback-time allocation in `ensure_output_capacity`)** `src/lib.rs:124`:
  Pre-allocated the output ring buffer to `block_size * 64` (16 384 samples) at construction
  time instead of `block_size * 16`. This covers host callbacks up to ≈341 ms at 48 kHz
  without any runtime reallocation. `ensure_output_capacity` is preserved as a safety fallback
  for unusual configurations. New test: `test_output_buffer_no_alloc_on_large_host_blocks`.

- **Issue #4 (Callback-time `resize()` for post_filter_ifft_buf)** `src/lib.rs:336`:
  Replaced the conditional `resize()` inside `process()` with `debug_assert_eq!`. The buffer
  size equals `fft_size` at construction and never changes. New test:
  `test_post_filter_ifft_buf_size_never_changes`.

- **Issue #5 (Two-path criterion too aggressive)** `src/two_path.rs:50,81`: Raised
  `transfer_threshold` from 5 to 25 blocks (≈27 ms → ≈133 ms at 48 kHz / 256-sample blocks).
  Tightened the power-advantage margin from 5 % (ratio 0.95) to 1 dB (ratio 0.794).  The old
  settings caused rapid foreground/background ping-pong on non-stationary signals. New test:
  `test_two_path_transfer_threshold_not_too_aggressive`.

## Deferred

- **Issue #6** (`fft_scratch` reuse for forward/inverse FFT): Reviewed — using `max()` of the
  two scratch lengths is correct per rustfft's contract. Not a bug; no action needed.
- **Issue #7** (full PBFDAF SIMD / flat FDL layout): Cross-crate DSP optimization; deferred
  to a dedicated performance PR after the iterator hot-loop cleanup above.
- **Issue #8** (Post-filter real FFT instead of complex IFFT): Minor optimization; deferred.

# 0.5.1

## Fixes

- Added copy_adaptive_state_from, so adaptive weights/FDL/error state can be promoted.
- Transfer_bg_to_fg() now copies background state into foreground instead of resetting it.
- Added input/output buffer-size validation and made the output queue track length explicitly, resizing only for oversized host-buffer edge cases so unread output cannot be overwritten.

## New tests:

- Background-to-foreground transfer preserves adaptive state
- Malformed input/output buffers return errors instead of panicking
- Large host blocks preserve every produced output sample
