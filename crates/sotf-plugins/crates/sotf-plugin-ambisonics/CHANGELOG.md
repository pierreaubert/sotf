# 0.5.9

## AllRAD/VBAP decoder

- Add a structural `algorithm` choice with serialized-compatible
  `mode_matching` and setup-time `allrad` modes.
- Implement deterministic virtual-sphere HOA decoding followed by physical
  2D-pair/3D-triangle VBAP remapping, composed into a fixed realtime matrix.
- Keep LFE rows silent and add factory, engine, catalog, QA, response, and
  allocation-free processing coverage.

# 0.5.8

## Review closure

- Replace hand-written normal-equation/Gauss-Jordan inversion with a
  rank-revealing SVD pseudoinverse. Regularization and rank thresholds scale
  with the largest singular value; each matrix exposes rank, condition,
  reconstruction-error, and peak-gain diagnostics and rejects unbounded gain.
- Support bounded underdetermined decoding, including TOA to current layouts,
  while explicitly reporting discarded spatial rank.
- Remove the fixed 8192-frame limit and approximately 1 MiB dual-band scratch
  reservation. Dual-band splitting and decoding now use two fixed 16-sample
  Ambisonics frames and remain allocation-free for any validated host block.
- Flush subnormal dual-band inputs before persistent IIR state and retain the
  block-wide NaN/Inf rejection policy.
- Advertise all supported ACN/SN3D input widths (4/9/16), enforce them in both
  factories, and add catalog, factory, bridge-choice, and exported-layout
  round-trip coverage.
- Consolidate serialized/runtime state onto `params::Params`, fix the first
  out-of-range ACN guard, add dense-direction and all-layout/order matrix
  quality tests, and expand benchmarks across every output layout plus
  single/dual-band worst-case TOA at 64/512/2048 frames. QA now gates both the
  default path and worst-case 16-in/16-out dual-band callback.

# 0.5.7

## Fixes (2026-08-12 follow-up)

- Keep the factory regression aligned with the FOA-only built-in catalog and
  verify that higher-order layouts remain explicit custom configurations.

# 0.5.6

## Fixes (2026-08-12 review follow-up)

- Align the built-in catalog and channel-count conformance checks with the
  factory's valid FOA (4-channel) configuration.
- Do not advertise SOA/TOA until matching built-in target layouts and output
  contracts are available; the current fixed 5.1 catalog default is FOA-only.

# 0.5.5

## Fixes (2026-08-12 review remediation)

- Correct max-rE degree weights for orders 1–3 using the exact Legendre-root
  definition, with per-ACN golden tests.
- Reject invalid orders instead of silently clamping them.
- Reject live structural changes and require host reconstruction, preventing
  stale channel topology and uninitialized scratch after parameter updates.
- Represent `target_layout` as the declared integer choice index and align the
  canonical parameter type key with `ambisonics_decoder`.
- Reject invalid dual-band sample rates, process-before-initialize, blocks over
  8192 frames, and non-finite input before persistent filter state is mutated.
- Correct product documentation: this decoder uses regularized mode matching,
  not AllRAD/VBAP.

# 0.5.4

## Fixes (from code review)

- **docs: dual-band latency semantics** (`src/lib.rs`): `latency_samples()` now
  documents that dual-band LR4 processing has frequency-dependent group delay near
  the 700 Hz crossover but no fixed linear-phase delay to report to the host.
  Regression test: `test_dual_band_reports_no_fixed_host_latency`.

- **fix: no-alloc dual-band scratch buffers** (`src/lib.rs`): `initialize()` now
  pre-allocates `lf_buffer` / `hf_buffer` to `MAX_BLOCK_FRAMES (8192) ×
  MAX_AMBI_CHANNELS (16)` so that the audio-thread hot path in `process()`
  never calls `Vec::resize()`.  The old code pre-allocated for 4096 frames but
  silently fell back to a heap allocation in `process()` for any larger block.
  The in-callback `resize()` calls are replaced by `debug_assert!` guards that
  catch oversized blocks early in debug builds.
  Regression test: `test_dual_band_large_block_no_alloc` (5000-frame block).

- **fix: per-speaker harmonic buffer reuse** (`src/spherical_harmonics.rs`, `src/decode_matrix.rs`):
  `spherical_harmonics_vector` now takes a mutable output slice (`&mut [f64]`) and
  writes in place. `DecodeMatrix::build` uses a reusable scratch buffer to populate
  each speaker's SH row, removing per-speaker temporary allocation during matrix
  build.
  Unit tests in `src/spherical_harmonics.rs` still cover first/second-order values
  and ACN ordering.

- **fix: improve `decode_frame` loop structure** (`src/decode_matrix.rs`):
  replaced iterator `take()` loops with direct row/input slice access and indexed
  accumulation in the small fixed-size dot product. This gives LLVM a simpler loop
  shape and avoids extra iterator overhead in the decode inner loop.

- **fix: remove crossover move in dual-band process** (`src/lib.rs`):
  dual-band processing now uses `self.crossover.as_mut()` directly instead of
  `take()` + restore. Crossover state remains in place for the call and no longer
  risks becoming `None` via panic during processing.

- **fix: `acn_to_degree_index` bounds guard** (`src/spherical_harmonics.rs`):
  Added `debug_assert!(acn <= channel_count(MAX_ORDER))`.  The
  floating-point `sqrt` truncation is correct for `acn ≤ 15` (MAX_ORDER = 3)
  but would silently produce wrong degree values for `acn ≥ 48`; the assert
  ensures a future increase to MAX_ORDER fails fast rather than producing
  incorrect harmonics.
  New test: `test_acn_to_degree_index_all_valid` verifies round-trip and range
  for every valid ACN index.

# 0.5.3

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Plugins implemented f2,3 7,8,9,10,11,12 and 13 see product features for details
