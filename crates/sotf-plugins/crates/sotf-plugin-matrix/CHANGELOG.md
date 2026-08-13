# 0.5.93

## Fixes

- Mark preset selection as structural in host-visible metadata, matching its
  routing-table rebuild contract.

# 0.5.92

## Fixes

- Smooth explicit phase inversion through zero and prune connections after zero-gain fades settle.
- Make realtime gain, phase, mute, solo, and dim edits allocation-free; remove obsolete block-sized scratch and bypass unity channel-state work.
- Add the missing `solo_N` controls and require exact channel-state widths atomically.
- Make presets atomic and truthful: stereo input passes through, 5.1 downmix uses a headroom-normalized SMPTE law, and 5.1 remap converts SMPTE/WAVE order to AAC order.
- Snap smoothing state on reset/reinitialization, reject a zero initialization rate, publish live parameter values, validate exact process buffers/sample rate, and report the crate version from package metadata.

# 0.5.91

## Fixes

- Apply and smooth the global gain control, and publish current matrix/channel-state values.
- Reject malformed or out-of-range dynamic IDs, sparse matrix dimensions, invalid preset indices, and channel-state widths without panics.
- Validate process buffers and avoid growing block-sized scratch during processing; reset now settles routing smoothers.
- Preserve configured nonsquare Matrix state in the plugins bridge factory.

# 0.5.90

## Fixes

- Removed the per-block `ch_gains_buffer.fill(1.0)` pass. The channel-gain scratch buffer now writes
  every frame/channel slot exactly once, using smoother output when available and `1.0` otherwise.

# 0.5.89

## Fixes

- **Bug #3 (critical): Cache-destroying loop order in `process()`** (`src/lib.rs:681`)
  Swapped from connections-outer/frames-inner to frames-outer/connections-inner.
  The previous order scanned the entire input buffer once per active connection,
  causing O(N×F) cache misses for an N-connection matrix over F frames.
  The fixed order keeps each frame's interleaved samples hot in L1 cache while
  iterating the much smaller active-connections list.

- **Bug #4 (high): `update_active_connections()` called every process block** (`src/lib.rs:704`)
  Removed the call from `process()`. The function is now only called by parameter
  mutators (`set_gain`, `set_matrix`, `set_phase_invert`, `apply_preset`) when the
  matrix actually changes. The previous code performed a full O(N²) matrix scan
  every block regardless of whether any parameter had changed.

- **Bug #7 (medium): Per-sample `phase_sign` branch** (`src/lib.rs:693`)
  Pre-resolved `(phys_in, phys_out, phase_sign)` per active connection in
  `update_active_connections()` (stored in `connection_phys`). The `phase_invert`
  branch and channel-map lookups are now done once at parameter-change time, not
  once per sample per connection.

## Deferred

- **Bug #1 (acoustic naming):** `stereo_downmix` preset is actually a stereo-blend
  (crosstalk mix), not a channel-count reduction. Renaming requires a cross-crate
  API change (preset strings are part of the serialized parameter format used by
  the engine and TUI). Deferred to a dedicated cross-crate refactor.

- **Bug #2 (documentation):** MS encode/decode amplitude relationship note. No
  code change required; documentation-only improvement deferred.

- **Bug #6 (minor perf):** `ch_gains_buffer.fill(1.0)` could be avoided by writing
  all channel entries in the smoother loop. Low impact relative to the loop-order
  fix; deferred.

# 0.5.88

## New

- Added missing qa_*.rs files for some plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
