# 0.5.4

## Fixes

- **Issue #5 (high):** Removed hard 4096-frame cap on internal processing buffers (`src/lib.rs`).
  Buffers now grow dynamically when the host block size exceeds the pre-allocated capacity,
  allowing offline renderers and non-standard hosts that use large blocks to work without error.
  The common real-time path (blocks ≤ 4096) remains allocation-free.
- **Issue #2 (high):** `update_latency_compensation` now returns `Result<(), String>` and
  propagates `DawHost::build()` failures (`src/lib.rs`). On error, both delay lines are zeroed
  so the plugin stays audible while compensation is disabled. Callers (`rebuild_path_a`,
  `rebuild_path_b`) propagate the error with `?`. Existing test helpers updated accordingly.
- **Issue #3 (medium):** `process()` now guards the `mix_smoother.set_target()` call with a
  comparison against the current target, skipping the call when the value has not changed
  (`src/lib.rs`). Eliminates redundant work on every block when the mix is settled.
- **Issue #4 (medium):** Replaced the magic constants `20.5` and `19999.5` in
  `band_mask_active()` with named associated constants `BAND_MASK_MIN_HZ`,
  `BAND_MASK_MAX_HZ`, and `BAND_MASK_EDGE_EPSILON` with explanatory doc-comments
  (`src/lib.rs`). Behaviour is unchanged.

## Deferred / Skipped

- **Issue #1** (rename `gain` to `mixed_gain` in the fast path): The referenced function
  `process_empty_path_fast` does not exist in this version's code. Review claim does not
  match the actual implementation — skipped.
- **Issue #6** (cache trig computation in fast path): Same as #1 — `process_empty_path_fast`
  and `can_use_empty_path_fast_path` are absent from this codebase. Skipped.
- **Issue #7** (avoid `Vec` clone in `parameters()`): Cosmetic / speculative optimization
  requiring cross-crate `Arc<Vec<Parameter>>` changes. Deferred.

# 0.5.3

## Fixes

- Preserve preallocated A/B processing buffers across reset instead of clearing their lengths.
- Avoid hot-path buffer resizing during processing; blocks beyond the prepared capacity now return a clear error.
- Pass only the active block slices into the nested A and B hosts.

# 0.5.2

## Fixes

### `test_auto_gain_runtime_enable_disable` no longer pins stale cache data

- `RealTimeCache` (used for `ABCompareData`) is a two-slot double-buffer:
  when `update()` cannot take `Arc::get_mut()` on the current spare, the
  write is silently skipped. This is the intended RT-safe behaviour — a
  one-frame-stale analyzer value is imperceptible.
- The test grabbed three successive snapshots via
  `let data = plugin.get_data().unwrap();`. Rust's `let` shadowing does
  **not** drop the old binding at the shadow point; each previous `data`
  kept its `Arc` alive until function return. After the first snapshot,
  that Arc became the next spare (via the swap path), permanently stuck
  at `strong_count >= 2`, so every subsequent measurement update was
  dropped. The final assertion read the first enabled-phase value
  (`-5.896887 dB`) instead of the post-disable `0.0`.
- Fix: wrap the intermediate cache reads in explicit `{ ... }` blocks so
  each `Arc` is released before the next audio-thread write. This matches
  real UI polling (poll → display → drop), which is the pattern
  `RealTimeCache` is designed for.
- No change to plugin behaviour: `AutoGain::current_gain_db()` already
  returns `0.0` when disabled, and the audio-path compensation already
  short-circuits to unity gain. Only the test's observation pattern was
  broken.
