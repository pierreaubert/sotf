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
