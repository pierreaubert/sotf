# 0.5.4

## Fixes

- **Multiband feed gain cache** (`src/lib.rs`): Cached `mb_*_feed_db` as
  linear gains plus the wet normalization factor, avoiding repeated `fast_pow10`
  work in `process_mb`. Added `test_mb_feed_linear_cache_updates_on_parameter_change`.

- **Multiband gain headroom** (`src/lib.rs`): Normalized the multiband wet
  direct+crossfeed sum by the largest active feed amount so correlated mono
  material no longer receives the large default `direct + crossfeed` boost when
  auto-gain is disabled. Added `test_mb_mono_signal_is_headroom_normalized`.

- **Fractional/high-rate ITD delay line** (`src/lib.rs`): Replaced the fixed
  `[f32; 96]` integer-sample delay buffer with a dynamically sized `Vec<f32>`
  and fractional linear interpolation. The advertised 1 ms delay range now
  works at 96 kHz and 192 kHz, not just 48 kHz-ish rates. Added
  `test_delay_line_supports_fractional_and_high_sample_rate_delay`.

- **Filter update cleanup** (`src/lib.rs`): Removed duplicate Meier filter
  recreation inside `update_filters`; the filters are still rebuilt once per
  sample-rate/filter update.

# 0.5.3

## Fixes (from code review 2026-05-11)

- **Critical – Meier sample-rate bug** (`lib.rs:update_filters`): Meier LPF and allpass
  filters were initialized at 44100 Hz and never recomputed when `initialize()` was called
  with a different sample rate. Added Meier filter reinitialization to `update_filters()`.

- **Critical – Symmetric ITD model** (`lib.rs:compute_differential_itd_ms`): Renamed
  `compute_dynamic_itd_ms` to `compute_differential_itd_ms`. The function now returns
  `(delay_l, delay_r)` where the two crossfeed paths carry different delays proportional
  to `static_itd_ms / 2 ± dynamic`. Previously both paths received the same value, making
  head yaw have no differential effect. All callers updated (`initialize`,
  `process_in_place`). Updated `test_itd_delay_accuracy` to reflect the corrected
  acoustic model (12 samples per path at yaw=0 for 0.5ms ITD, not 24).

- **Critical – Multiband per-sample stack allocation** (`lib.rs:process_mb`): Replaced
  8 `[f32; 1]` stack arrays allocated per sample inside the processing loop with writes
  into the pre-allocated `mb_bands_l` / `mb_bands_r` Vec buffers. Used `split_at_mut`
  to satisfy the borrow checker for disjoint mutable slices of `[Vec<f32>; 3]`.

- **Major – Yaw smoother advances 1 step per block** (`lib.rs:process_in_place`):
  Changed `self.yaw_smoother.advance()` → `self.yaw_smoother.next_n(nf)` so the smoother
  advances by the actual block size, giving a correct ~10ms time constant at any sample
  rate and block size.

- **Major – Delay line double-discontinuity in setter** (`lib.rs:set_parameter`): Removed
  immediate `itd_delay_l/r.set_delay(...)` calls from the `head_yaw_deg` and
  `itd_delay_ms` parameter setters. `process_in_place` is now the sole owner of delay
  line updates (driven by the yaw smoother), preventing a jump-to-target followed by a
  revert-to-start discontinuity.

- **Major – Mix applied as block step** (`lib.rs:process_in_place`): Replaced block-level
  `let mix = self.mix_smoother.next_n(nf)` + uniform application with a linear ramp
  across the block (`mix_start` + per-sample linear interpolation to `mix_end`), avoiding
  zipper noise on mix parameter changes.

## Tests added

- `test_meier_filter_coefficients_correct_after_sample_rate_change`: verifies Meier
  crossfeed RMS is consistent across 44100 and 48000 Hz.
- `test_itd_yaw_asymmetry`: verifies positive yaw makes L→R crossfeed path longer than
  R→L, and yaw=0 produces symmetric delays.
- `test_mix_ramp_no_step_discontinuity`: verifies mix changes produce a ramp, not a step.

## Deferred (cross-crate or out of scope)

- **`parameters()` Vec clone** (review §3 major): Requires `ParametricInPlacePlugin` trait API change
  across all 30+ plugin crates. Deferred.
- **`Biquad<f64>` → `Biquad<f32>` for Bauer/Meier** (review §3 minor): Requires verifying
  no precision regression; deferred.
- **SIMD interleave** (review §3 major): Located in `math-dsp/simd.rs`, out of scope here.
- **`set_parameter` allocation** (review §3 minor): Architecture-level; deferred.

---

# 0.5.2

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin

## Changes

- Improve the UI a bit; will need to switch from Swift to GPUI soon
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 5)
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
