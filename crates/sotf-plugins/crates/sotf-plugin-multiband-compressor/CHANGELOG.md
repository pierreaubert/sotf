# 0.5.16

## Review closure (2026-08-12)

- Add an explicit one-band broadband Compressor mode and make the canonical factory expose its matching identity and runtime schema.
- Make lookahead and band count structural after initialization so host latency/topology metadata cannot become stale; preserve exact dry, bypass, passive, and M/S alignment.
- Reject serialized legacy sidechain controls that have no DSP implementation and keep them out of the runtime schema.
- Keep engine-generated default Compressor configs aligned with that strict runtime schema by omitting the rejected legacy sidechain fields.
- Stop advertising the legacy boolean `link_channels` field as a live engine accessor; broadband Compressor exposes the canonical continuous `link_amount` control while still accepting the boolean during preset construction for compatibility.
- Make continuous channel linking canonical, smooth crossover/dynamics/link/tilt/makeup automation sample-by-sample, and preserve preallocated tilt-filter state across writes and reset.
- Process oversized host blocks in allocation-free 4096-frame chunks and avoid schema/string allocations in valid realtime setters.
- Document the cascaded LR4 phase/group-delay limitation and add factory broadband, partition-equivalence, bounded-jump, swept transfer/phase/null, latency, analyzer, invalid-preset, and allocation regressions.

# 0.5.15

## Fixes (from code review 2026-08-12)

- Validate factory-supplied compressor parameters before constructing crossover and detector
  state. Non-finite or out-of-range dynamics values, invalid sidechain choices, malformed band
  overrides, and non-ascending or non-representable crossover frequencies are now rejected with
  an actionable error instead of reaching IIR coefficient construction.
- Route the main, bridge, and A/B Compare factory paths through the fallible constructor while
  retaining the legacy one-band bridge compatibility mode.

# 0.5.14

## Fixes

- Aligned the catalog's `compressor` entry with the runtime it constructs: this factory name is a
  multiband-compressor compatibility alias, exposes the dynamic global/per-band schema, and uses
  the custom multiband UI metadata. It is no longer advertised as a standalone single-band layout.

# 0.5.13

## Fixes (from code review 2026-08-12)

- Report configured lookahead latency and phase-align dry, bypassed, and passive paths with the
  delayed wet bands. The implementation now honors the advertised 20 ms maximum.
- Decode the processed M/S signal before dry/wet mixing so `mix = 0` remains exact L/R passthrough.
- Make `num_bands` structural after initialization. Live changes now fail transactionally instead
  of reallocating state and risking stale per-band audio storage.
- Publish live analyzer values by storing owned vectors in each realtime-cache snapshot instead of
  nested shared `Arc`s that prevented updates.

# 0.5.12

## Fixes (from code review 2026-05-16)

- **Medium (2.3):** Muted bands now report the same `-120.0 dB` silence floor as constructors,
  data snapshots, and `num_bands` resize paths. Added a regression test covering solo-induced band
  muting so meter state remains consistent.

# 0.5.11

## Fixes (from code review 2026-05-11)

- **Critical (2.1):** Removed `Vec::resize` inside `process_in_place`. Buffers are pre-allocated in
  `initialize()` to 4096 frames. Oversized blocks now trigger `debug_assert!` instead of silently
  allocating on the audio thread. Existing tests that passed >4096-frame blocks have been fixed to
  split into smaller blocks (`src/lib.rs:1083`, `tests/test_multiband.rs`).

- **High (1.3):** `MeasuredMakeup::update()` was called once per channel per frame, making the EMA
  time constant 2x too fast on stereo. Moved outside the `ch` loop; updates once per frame using the
  max envelope across channels. `makeup_linear()` hoisted out of the inner loop as a side-effect
  (`src/lib.rs:1219`).

- **High (1.5):** Per-band `knee_db` parameter was defined in `BAND_TEMPLATE` but never registered
  in `rebuild_cached_parameters`, `set_parameter`, or `get_parameter`. Added `band_{i}_knee` to all
  three locations (`src/lib.rs:515, 895, 987`).

- **High (2.6):** `reset()` did not clear `sidechain_tilt_biquads` state. Added call to
  `rebuild_sidechain_tilt()` at end of `reset()` to reinitialize filter state (`src/lib.rs:1047`).

- **High (2.7):** `sidechain_tilt_biquads` not rebuilt when `num_bands` increases via
  `set_parameter`. Added `rebuild_sidechain_tilt()` call inside the `num_bands` change handler
  (`src/lib.rs:693`).

- **High (2.4):** `set_parameter` for `lookahead_ms` alias and global `per_band_lookahead_ms` did
  not clamp to [0, 10] ms, while the constructor did. Applied `.clamp(0.0, 10.0)` in both setters
  (`src/lib.rs:375, 795`).

- **Medium (2.2):** `set_param_value(0, value)` used `value as usize` (truncation) for `num_bands`.
  Changed to `value.round() as usize`. Note: `param_bridge` Float→Int coercion in `sotf-host` still
  truncates before reaching this setter — full fix deferred to `sotf-host` (cross-crate).

- **Medium (2.3):** `band_levels_db` silence floor was inconsistent: `0.0` in constructor, `-120.0`
  in `MultibandCompressorData::new()`, `-100.0` in `set_parameter` resize. Unified to `-120.0`
  everywhere (`src/lib.rs:315, 677`).

- **Medium (2.8):** Cache update throttle was block-count based (fires every 10 blocks), causing
  choppy UI at large block sizes and fast UI at small block sizes. Replaced with a sample counter
  that fires every ~50 ms worth of samples (`src/lib.rs:1021, 1275`).

## Deferred (cross-crate or speculative)

- **1.1 / 1.2 (High):** Sidechain tilt uses a high-shelf instead of a true tilt filter, and is only
  applied consistently when `link >= 1.0`. Full tilt filter design requires a simultaneous
  low-shelf + high-shelf pair. Deferred to a follow-up DSP pass.

- **1.4 (Medium):** Per-band threshold, ratio, and knee have no parameter smoothers (zipper noise
  during automation). Requires adding `Vec<Smoother>` per parameter. Deferred.

- **1.6 (Medium):** LR4 crossover 0.1 Hz stall threshold is in `math-iir-fir/lr4_crossover.rs`
  (different crate). Deferred.

- **2.2 (partial):** `param_bridge` Float→Int coercion truncates instead of rounds in `sotf-host`.
  Deferred to a `sotf-host` patch.

- **2.5 (Medium):** Stub features (`sidechain_hpf`, `detection_mode`, `program_dependent_release`,
  `sidechain_external`) are exposed to users but have no DSP implementation. Deferred until
  implemented.

- **3.2 / 3.4 (Low):** Cache-unfriendly access patterns and missing SIMD paths. Speculative
  micro-optimizations; not profiled. Deferred.

# 0.5.10

## New

- Added missing parameters for new plugins

## Fixes

- Fix same regression in expander and compressor
- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- Refactor:  merged compressor and multi-band compressor, merged expander and multi-band expander
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
# 0.5.17

- Mark latency-changing lookahead controls structural in host-visible metadata, matching their rebuild-only setters.
