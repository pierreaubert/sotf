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
