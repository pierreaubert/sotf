# 0.5.62

## Fixes

- **[2.2] params.rs now uses `f32` for runtime parameters** (`src/params.rs`):
  `Params.dim_gain_db` and `Params.fade_ms` now use `f32` to match DSP storage.
  `PluginParamDef` still exposes `f64` through casting in `param_value`/`set_param_value`.
  Added regression assertions in `param` serde tests.

- **[3.3] set_channel_states accepts a borrowed slice** (`src/lib.rs`):
  Replaced `Vec<ChannelState>` by `&[ChannelState]` in `set_channel_states` to avoid
  unnecessary ownership transfer and allocation opportunities.
  Added `test_set_channel_states_accepts_slice`.

- **[4.4] qa_channel_mute_solo now checks allocator usage** (`bin/qa_channel_mute_solo.rs`):
  Wrapped the hot-path `process_in_place` call in `assert_no_allocs` so QA explicitly
  verifies zero allocations in the checked block.

- **[4.5] enabled docstring now documents bypass behavior** (`src/lib.rs`):
  Clarified that disabled state bypasses per-channel state and fades channels to unity gain.

# 0.5.61

## Fixes

- **[2.1] params.rs fade_ms default aligned to 5.0 ms** (`src/params.rs:37`): PARAMS spec had
  `10.0`, lib.rs DSP constant had `5.0`. Mismatch would cause UI/preset recall inconsistency.
  Fixed to `5.0` in PARAMS; all three sources (`param_specs.rs`, `params.rs`, `lib.rs`) now agree.

- **[2.3] from_params silently discarded mismatched channel_states** (`src/lib.rs:131-135`):
  When a preset contained fewer or more channel states than the configured channel count, the
  entire state was silently dropped and reset to defaults. Now: truncate to `channels` if too
  many, pad with `ChannelState::default()` if too few. Tests:
  `test_from_params_fewer_channel_states_pads_defaults`,
  `test_from_params_more_channel_states_truncates`.

- **[3.1] Block-based smoothing replaces per-sample smoother ticking** (`src/lib.rs:451-480`):
  The inner loop called `Smoother::advance()` O(num_frames × channels) times per callback.
  Replaced with one `Smoother::next_n(num_frames)` call per channel per block, plus a
  per-frame linear ramp between start and end gains. Linear-ramp error vs. true exponential
  is <0.3% for a 512-sample block at 5 ms tau / 48 kHz — inaudible. Speedup is proportional
  to block size.

- **[3.2] Lazy cached_parameters rebuild with dirty flag** (`src/lib.rs`):
  Every per-channel toggle (`mute_N`, `solo_N`, `dim_N`) previously caused an immediate
  `serde_json::to_string` on the channel_states vec. Now uses `Cell<bool>`/`RefCell` interior
  mutability: all mutation methods call `mark_params_dirty()`; `parameters()` rebuilds lazily.
  JSON serialization cost is deferred to the `parameters()` call.

- **[3.2 followup] Per-channel set_parameter (mute_N/solo_N/dim_N) now works** (`src/lib.rs`):
  `set_parameter` previously called `validate_parameter` before pattern-matching per-channel
  IDs, causing "Unknown parameter" errors for `mute_0`, `solo_0`, `dim_0`. Per-channel
  parameters are now dispatched before the registered-param validator. Test:
  `test_per_channel_set_parameter_mute_works`.

- **[2.4] debug_assert for buffer length in process_in_place** (`src/lib.rs:432-438`):
  Added `debug_assert_eq!(buffer.len(), num_frames * channels)` to catch host bugs early in
  debug builds.

## Deferred

- **[2.5] SIMD only for stereo**: `apply_per_channel_gain_simd` in `math-dsp/simd.rs` only has
  AVX2/NEON paths for `channels == 2`. Cross-crate fix needed. Deferred.

- **[4.1] Fast "all unity" bypass**: improvement, not a bug. Deferred.

- **[4.3] param_specs.rs dead code**: module is not `mod`-declared in lib.rs. It defines the
  correct constants (FADE_MS_DEFAULT = 5.0) that params.rs had wrong. Now that the default
  is fixed in params.rs, param_specs.rs remains standalone. Cross-crate clean-up deferred.

# 0.5.60

## New

- Added missing qa_*.rs files for some plugins
- Debugging of the new plugin features

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed 95% of the 5k tests in the repo
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
