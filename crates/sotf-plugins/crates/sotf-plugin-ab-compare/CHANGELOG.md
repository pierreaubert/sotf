# 0.5.6

## Fixes

- Made path configurations and band-mask cutoffs structural parameters. Runtime writes now request
  an outer graph rebuild instead of parsing, allocating, changing latency, or replacing filters on
  the audio/control callback boundary.
- Injected the authoritative plugin factory before initial path construction in both the canonical
  facade and plugins bridge, including non-fallback plugin/rack regression coverage.
- Added the missing band-mask parameters to the canonical schema, engine settings/accessors and
  converter, and enforce exact runtime key/update-mode parity. Player A/B test sessions now retain
  explicit mask bounds, while the CLI constructor deliberately uses the canonical 20 Hz–20 kHz
  defaults.
- Replaced equal-power same-source blending with a unity-preserving linear crossfade: identical
  paths stay at unity at every mix position and inverted paths cancel at centre.
- Replaced callback-count diagnostic throttling with a 20 Hz elapsed-frame scheduler and reset it
  deterministically across initialization/reset.
- Stopped rebuilding heap-backed parameter metadata after realtime scalar changes.
- Expanded QA to exercise real plugin/rack paths, latency compensation, active band masking, and
  zero-allocation/performance checks at 1/2/6/12 channels.

# 0.5.5

## Fixes

- Feed every active audio block, and only its active slice, into the persistent
  EBU R128 loudness monitors while throttling diagnostic publication separately.
- Preserve the plugin's reported latency in bypass with a dedicated dry delay.
- Validate constructor state before creating smoothers, filters, nested hosts,
  or auto-gain state; invalid path changes now preserve the prior configuration.
- Use checked sample counts and fixed prepared buffers in the callback, returning
  an error above the 48,000-frame realtime maximum instead of allocating.
- Recreate band-mask filters on reset so pre-reset IIR history cannot leak into
  the next render.
- Align runtime gain/mix smoothing limits with the static parameter schema.

# 0.5.4

## Fixes

- Added `test_band_mask_reduces_out_of_band_energy`, covering the review gap for
  band-mask attenuation when the passband is narrowed.
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
- **Issue #6 (low):** Added a cached equal-power gain for the empty-path fast path
  (`empty_path_fast_gain`) and recompute points when mix target changes. The fast
  processing branch now reuses the cached gain and uses the short-circuit empty-path
  route when eligible (`src/lib.rs:162`, `src/lib.rs:255`, `src/lib.rs:493`),
  reducing repeated trig calls per block.
- **Issue #7 (low):** `validate_parameter()` now validates against cached parameter
  metadata directly instead of cloning the vector through the trait default, while
  `parameters()` remains a clone-returning API because that is part of the current
  `Plugin` trait contract.

## Deferred / Skipped

- **Issue #1** (rename `gain` to `mixed_gain` in the fast path): The referenced function
  `process_empty_path_fast` does not exist in this version's code. Review claim does not
  match the actual implementation — skipped.
- **Issue #7**: Full `Arc<Vec<Parameter>>` migration for zero-copy parameter list
  delivery is deferred; that would require trait/API changes across the plugin host.

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
