# 0.5.12

## Fixes

- Advertise mode, Exciter crossover, oversampling, DC blocker, and ADAA as
  structural controls, matching the initialized plugin's graph-rebuild-only
  setter contract.

# 0.5.11

## Fixes (2026-08-12 review closure)

- Moved oversampling ownership to the host graph. Saturation now advertises its
  requested factor, has zero internal latency, and processes dynamics and
  dry/wet mixing in the wrapper's single processing domain.
- Continuous drive, mix, and output controls now advance exactly once per
  source frame, making automation independent of callback partitioning.
- Topology-affecting live changes are rejected after initialization so the host
  can rebuild off the audio thread; continuous updates preserve cached metadata
  without allocation or topology reconstruction.
- Initialization now rejects invalid low-rate Exciter crossover configurations,
  processing checks initialization, context rate, and sample-count overflow,
  and scratch capacity follows an explicit maximum-block contract rather than
  scaling with sample rate.
- Documentation now describes Tube and Tape as static waveshapers instead of
  claiming physical analog emulation.
- Added exact-once host wrapping, latency-aligned dry/wet, all-mode/all-factor
  dynamic composition, callback partition, alias rejection, malformed input,
  reset, and allocation/QA regressions.

# 0.5.10

## Fixes (2026-08-12 review remediation)

- Bulk parameter updates now reject unknown mode and oversampling enum values
  before mutating any state, preventing silent topology changes and partial
  updates. Added an atomicity regression test.

# 0.5.9

## Fixes (2026-08-12 review remediation)

- Dynamic drive is now applied inside the selected oversampled nonlinearity,
  including the Exciter high-band path, instead of being ignored whenever the
  internal oversampler is enabled.
- Added a regression covering dynamic Exciter processing with 2x oversampling.

# 0.5.8

## Fixes (2026-08-12 review remediation)

- Oversampler processing failures now propagate to the host instead of being
  converted into successful full-block consumption. Added regression coverage
  for an injected oversampler error.

# 0.5.7

## Fixes (2026-08-12 review remediation)

- Dynamic drive modulation now feeds the selected direct/ADAA/Exciter topology instead of
  overwriting its output with a second memoryless pass. Exciter dynamics therefore preserve the
  split-band signal path.
- The plugin now reports internal oversampler latency and no longer requests a second layer of
  host oversampling.
- Reset settles drive, mix, and output smoothers at their current parameter targets.
- Added a fallible validated constructor and routed both public factories through it, rejecting
  unknown modes/factors, invalid channel counts, non-finite/out-of-range values, and zero sample
  rates. Exciter frequency is limited below Nyquist during initialization.
- Added regressions for dynamic Exciter topology, latency metadata, double-oversampling prevention,
  invalid configuration, and smoother reset.

## Deferred

- Oversampled dynamic-drive interpolation, latency-aligned dry/wet mixing, and realtime-safe
  topology switching require a larger host/oversampler control-path redesign.
- Exact host maximum-block sizing and pass fusion remain performance follow-ups.

# 0.5.6

## Fixes

- Short host buffers now return an explicit `Err` instead of relying on a debug-only assertion and
  later panicking on slice bounds.
- Added coverage for the exciter path with oversampling enabled.

# 0.5.5

## Fixes (from code review)

- **[Critical]** `low_buf` and `high_buf` were not resized together with `dry_buf` when a
  larger-than-expected block arrived in exciter mode, causing an index-out-of-bounds panic
  (`src/lib.rs` — buffer resize guard).
- **[Critical]** Drive, mix, and output-gain smoothers were block-constant: a single
  end-of-block value was applied to all frames, causing zipper noise on automation. All three
  now use a per-sample linear ramp (start + frame * step) (`src/lib.rs` — smoother ramp loop).
- **[Critical]** Tube ADAA path used `adaa_softclip` (which implements `f(x)=x/(1+|x|)`,
  i.e. `tone=1`) regardless of the `tone` parameter. When `tone != 1.0`, toggling ADAA silently
  changed the waveshaper. The Tube ADAA arm now delegates to direct `tube()` so harmonic
  character is always consistent (`src/lib.rs:909`).
- **[Major]** Dynamic saturation was post-distortion gain multiplication, acting as an expander
  rather than drive modulation. It now recomputes `saturate(dry, mode, dynamic_drive, tone)` with
  `dynamic_drive = drive * (1 + env * dyn_amount)` clamped to 20.0 (`src/lib.rs` — dynamic block).
- **[Major]** Dead LUFS auto-loudness code removed. `LufsTarget` was unconditionally disabled
  (`set_enabled(false)`) with no parameter to re-enable it, and it was measuring the mixed
  output rather than the wet signal. Struct field, constructor code, and import removed
  (`src/lib.rs`).
- **[Major]** Oversampler return value (frames actually written) was discarded with `let _ =`.
  During latency fill, the oversampler writes fewer than `nf` frames; unwritten frames are now
  zeroed rather than left with stale saturation state (`src/lib.rs` — oversampled paths).
- **[Minor]** `flush_denormals_inplace` was called on the entire `buffer` slice, potentially
  touching samples beyond `total = nf * nc`. Now called as `flush_denormals_inplace(&mut buffer[..total])`
  (`src/lib.rs:967`).
- **[Minor]** Added `debug_assert!(buffer.len() >= total)` to catch buggy-host buffer overruns
  in development builds (`src/lib.rs`).
- **[Minor]** Fixed incorrect doc comment on `tube()`: `f(x) = x/(1+|x|^n)` is an odd function
  (symmetric), not asymmetric as previously stated.
- **[Minor]** Fixed incorrect doc comment on `tape()`: the implementation is a memoryless
  exponential sigmoid, not a hysteresis model.
- **[Minor]** Fixed module-level comment that called Tube "asymmetric waveshaping" and Tape
  "simplified hysteresis approximation".

## New tests

- `test_exciter_large_block_no_panic` — regression for buffer resize bug
- `test_tube_adaa_matches_direct_when_tone_not_one` — regression for Tube ADAA mismatch
- `test_drive_smoother_ramps_across_block` — verifies per-sample drive ramp
- `test_dynamic_saturation_bounded_no_pumping` — verifies bounded output with dynamic=1.0
- `test_flush_denormals_limited_to_valid_samples` — verifies no writes beyond valid range
- `test_no_lufs_auto_gain_on_passthrough` — verifies LUFS dead code is gone

## Deferred

- **Performance 3.1** (seven buffer passes): Fusing all DSP stages into a single per-frame loop
  requires significant restructuring of the oversampled path and ADAA path. Deferred as a
  larger refactor.
- **Performance 3.3/3.4** (`parameters()` clones Vec, `rebuild_cached_parameters` on every
  `set_parameter`): Requires changing the `ParametricInPlacePlugin` trait signature to return `&[Parameter]`
  — a cross-crate change. Deferred.
- **Acoustics 1.5** (soft_clip minimum drive = 1.0 means no clean path): Behaviour is
  intentional per current design; documented in comment but not changed.
- **Acoustics 1.6** (DC blocker before final mix): Low risk for current use-cases (DC blocker
  targets saturation-induced offset; dry is clean). Deferred.

---

# 0.5.4

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added playlist support across the board
- Added a saturation plugin

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
