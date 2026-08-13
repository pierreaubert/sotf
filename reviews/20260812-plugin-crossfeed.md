# Crossfeed plugin review — 2026-08-12

Retained quality follow-up implemented in 0.5.14: a conservative parametric
HRTF mode uses a 700 Hz head shadow, 0.25 ms base ITD, and -9 dB cross-ear
path. Equal near-ear subtraction preserves mono fold exactly and the dry path
keeps reported latency at zero. Hard-pan, anti-phase, mono-fold, partition,
latency, and allocation tests define the contract. This mode is explicitly not
a personalized or measured SOFA renderer.

## Remediation status — 2026-08-12

Implemented in `sotf-plugin-crossfeed` 0.5.11:

- Yaw-derived differential ITD now runs with zero static ITD in all three algorithms.
- Zero-delay operation continuously advances delay history, preventing stale sample resurrection.
- The public preset parameter now applies the complete selected preset.
- Unrelated setters no longer reconstruct/reset every filter; only affected Bauer or Multiband
  coefficient state is rebuilt.
- Construction and initialization validate finite values, authoritative ranges, crossover ordering,
  Nyquist constraints, and nonzero sample rate. Non-finite yaw is rejected.
- Processing validates the exact interleaved stereo buffer length before both active and bypass paths.
- Documentation now matches actual defaults, ranges, gain units, and Bauer topology.
- Auto Gain now applies the serialized `autogain_target_lufs` through the shared
  helper; target changes produce distinct converged compensation, and meter
  errors are propagated instead of discarded.

Filter-frequency automation is resolved for the exposed Bauer cutoff/feed controls with a
128-sample coefficient interpolation that preserves biquad history; Multiband crossover
frequency updates now use state-preserving in-place LR4 coefficient updates. Regression coverage
is `test_bauer_frequency_automation_is_click_free`.

Multiband feed controls now expose a finite -60 dB Off endpoint that maps to exactly zero
crossfeed. Wet gain uses independent per-band constant-power normalization
`1/sqrt(1 + feed^2)`, so changing one band no longer changes the level of unrelated bands.
Regression coverage is `test_mb_feed_has_true_off_endpoint_and_per_band_constant_power_norm`.

All P0-P3 findings are closed in 0.5.13. Bypass and mode transitions use the documented
deterministic reset policy. Bauer frequency changes interpolate coefficients over 128 samples and
Multiband changes preserve LR4 histories. Single-parameter setters and reset are allocation-free,
including preset and Auto Gain changes. `max_block_frames` is now the setup-time graph contract:
four scratch buffers are allocated at exactly that capacity instead of ten unconditional
65,536-frame buffers. The Multiband hot loop uses direct scalar LR4 stages rather than invoking the
general slice dispatcher twice per sample.

Additional re-audit fixes make parameter batches transactional, reject pre-initialization and
sample-rate-mismatched callbacks, sanitize non-finite input before filter state, make serialized
state strict, report the crate version in metadata, and ensure reset precedes preset coefficient
transitions so the requested preset is not silently cancelled.

Finding-to-regression evidence:

- yaw/static ITD and delay history: `test_yaw_only_itd_advances_delay_for_every_algorithm`,
  `test_yaw_itd_automation_is_partition_invariant`, `test_zero_delay_still_advances_delay_history`;
- complete presets and filter continuity: `test_public_preset_selection_applies_complete_preset`,
  `public_presets_converge_to_fresh_reference_audio`,
  `test_unrelated_parameter_update_preserves_filter_history`,
  `test_bauer_frequency_automation_is_click_free`,
  `test_multiband_frequency_automation_preserves_crossover_state`;
- Auto Gain target/error behavior: `autogain_target_lufs_changes_compensation`,
  `autogain_target_lufs_updates_the_helper`;
- construction, lifecycle, buffers, and finite state:
  `test_invalid_construction_and_sample_rate_are_rejected`,
  `process_requires_initialized_matching_sample_rate`,
  `test_process_rejects_non_exact_stereo_buffer_lengths`,
  `non_finite_audio_is_sanitized_before_dsp_state`, `serialized_state_rejects_unknown_fields`;
- Multiband feed semantics: `test_mb_feed_has_true_off_endpoint_and_per_band_constant_power_norm`;
- deterministic transitions/reset: `disabled_crossfeed_resets_state_before_reentry`,
  `mode_transition_resets_inactive_filter_state`, `test_reset_clears_all_filter_state`;
- realtime allocation and configured memory: `realtime_parameter_updates_and_reset_do_not_allocate`,
  `scratch_capacity_matches_setup_contract`, `test_process_does_not_resize_buffers`,
  `test_oversized_block_returns_error`;
- schema/docs/default parity: `parameters_include_all_public_params`, `param_index_coverage`,
  `deserialize_empty_json_uses_defaults`, and `default_plugin_has_expected_metadata`.

The delay-automation finding is resolved in 0.5.7: head-yaw smoothing and both fractional ITD
paths now advance once per sample inside every algorithm. `DelayLine::set_delay` performs no
allocation after initialization, and a varied-callback-size regression test proves partition-
invariant output during a yaw ramp.

## Findings

### P1 — Head yaw is inaudible when static ITD is zero

`process_in_place` computes yaw-derived differential delays whenever yaw or static ITD is nonzero (`crossfeed_plugin.rs:602-611`), but all three algorithms call the delay lines only when `params.itd_delay_ms > 0` (`crossfeed_plugin.rs:314-327,336-348,356-411`). The default static ITD is zero, so moving the exposed Head Yaw control updates delay values that are never applied. Existing yaw tests set `itd_delay_ms = 0.5`, masking the default-use defect.

Base `has_itd` on the effective per-path delay (or always process the tiny delay lines) and define a coherent head-tracking model. Add yaw-only impulse tests with static ITD zero for both signs and all three algorithms, including live smoothed motion.

### P1 — Selecting a preset does not apply that preset

The public `crossfeed_preset` parameter maps only to `params.preset` (`crossfeed_plugin.rs:190-205`); `apply_values` never calls `CrossfeedPluginParams::from_preset` (`crossfeed_plugin.rs:470-515`). The selector therefore changes its displayed value without changing mode, cutoff, feed, or any audio. Presets work only when external code directly invokes `from_preset` (`crossfeed_plugin_params.rs:102-137`).

On preset selection, apply the complete preset atomically, update all dependent smoothers/caches, and decide whether subsequent manual edits set a “Custom” state. Add public parameter tests that select every preset and assert both parameter values and characteristic audio output.

### P1 — Every parameter update reconstructs all filters and resets their histories

After any update—including mix, yaw, enabled, feed cache, or auto-gain settings—`apply_values` unconditionally calls `update_filters` (`crossfeed_plugin.rs:470-494`). That replaces Bauer and Meier biquads and reinitializes both LR4 crossover banks (`crossfeed_plugin.rs:258-311`), discarding filter state. Automation therefore clicks and causes unnecessary coefficient design even when no filter-related parameter changed. Filter/crossover frequency changes themselves also hard-reset without coefficient smoothing or a parallel crossfade.

Track dirty categories and update only affected state. For live filter changes, interpolate stable coefficients where valid or build a parallel path and crossfade. Add constant-input/sine automation tests that bound discontinuities for every parameter and assert unrelated setters preserve filter state/output continuity.

### P1 — Auto-gain target LUFS is a dead control

`autogain_target_lufs` is serialized, exposed, returned, and writable (`crossfeed_plugin.rs:181,223`; `params/consts.rs:130-144`), but `AutoGainParams` construction omits it and the update path changes only maximum gain and smoothing (`crossfeed_plugin.rs:140-151,495-511`). The control has no effect on audio. Moreover, input/output measurement errors are discarded (`crossfeed_plugin.rs:599-601,644-647`), concealing invalid state.

Either remove/rename target LUFS to match what `AutoGain` actually supports, or extend/configure the helper with the target. Propagate or explicitly handle measurement failures. Add tests showing two target settings converge to measurably different compensation and error injection is deterministic.

### P1 — Construction bypasses validation and can build invalid filters/topologies

`new` accepts `CrossfeedPluginParams` directly and immediately constructs biquads/crossovers/delays (`crossfeed_plugin.rs:62-153`) without schema validation. NaN/Inf, reversed crossover frequencies, frequencies above Nyquist, invalid mix/feed/auto-gain values, and zero/invalid sample rate can enter DSP state. Runtime validation does not repair malformed factory state, and `head_yaw_deg` validation accepts non-finite floats then silently ignores them (`crossfeed_plugin.rs:519-532`).

Validate construction and initialization through the same canonical schema, add cross-field/Nyquist constraints, and reject non-finite yaw. Add malformed JSON/factory tests, low-sample-rate tests, reversed crossovers, and NaN/Inf coverage.

### P1 — Processing does not validate the interleaved stereo buffer length

The method checks only `num_frames` against scratch capacity, then deinterleaves `buffer` as `nf` stereo frames (`crossfeed_plugin.rs:581-614`). A short or oddly sized buffer can panic or cause helper-level out-of-bounds behavior rather than returning `PluginResult::Err`; surplus samples create an ambiguous contract. `nf * 2` is not checked for overflow.

Use checked multiplication and require the exact stereo sample count before measuring or deinterleaving. Test short, long, odd, zero-frame, and overflow-shaped contexts in every bypass/active mode.

### P2 — Multiband feed units contradict the documentation and cannot represent “no crossfeed” — fixed in 0.5.11

The three feed dB values are converted as ordinary gains (`crossfeed_plugin.rs:245-255`) and multiplied directly into the opposite-channel bands (`crossfeed_plugin.rs:397-418`). Thus 0 dB means unity crossfeed, not zero crossfeed. Yet `USAGE.md` says “MB Low Feed 0 dB” means bass remains wide/no crossfeed, while the schema allows low feed only −20..0 dB (`params/consts.rs:74-87`), so true zero is not representable. The global normalization further changes direct level based on the largest feed, making independent band controls interact.

The controls now use a finite -60 dB endpoint mapped to zero crossfeed, while preserving dB gain semantics above that endpoint. Each band applies constant-power normalization `1/sqrt(1 + feed^2)` to its direct and crossfeed terms; this removes the previous `1/(1+max_feed)` coupling between independent bands. Regression coverage verifies the endpoint, cache, and normalization factors. Broader hard-pan/anti-phase listening coverage remains a follow-up.

### P2 — Delay automation is block-rate, callback-partition-dependent, and can resurrect stale samples

Yaw smoothing advances to one end-of-block value, and each delay line is set once for the entire block (`crossfeed_plugin.rs:602-611`). A large callback therefore jumps directly to a later delay than a small callback, producing different Doppler/interpolation artifacts. `DelayLine::process` returns immediately at zero delay without writing or advancing its ring (`delay_line.rs:47-61`); after a zero-delay interval, re-enabling delay can read samples retained from an earlier period.

Interpolate delay per sample (or at bounded control intervals with a click-free fractional-delay transition), continue advancing/writing state at zero delay, and test randomized callback partition equivalence plus delay on→off→on with distinct impulses.

### P2 — Mode/enabled bypass transitions are hard switches with frozen state

Disabled/Off returns before smoothers, filters, delay lines, or AutoGain advance (`crossfeed_plugin.rs:581-590`). Re-enabling exposes stale histories and a mix/yaw smoother whose real-time trajectory paused. Mode changes also switch unrelated filter states immediately; the “structural/setup” annotation is not enforced by this API.

Choose either graph-rebuild semantics or a live, equal-power crossfade while maintaining relevant state. Ensure bypass policy is explicit (freeze versus continuously process) and tested for bounded discontinuity and deterministic reset.

### P2 — Parameter updates allocate and do excessive non-real-time work

The setter rebuilds the parameter `Vec`, may allocate/drop `AutoGain`, reconstructs filters, reinitializes crossovers, and can resize delay storage through later processing (`crossfeed_plugin.rs:470-515,225-240`; `delay_line.rs:21-29`). These operations are unsafe if the host applies automation on the audio thread. Normal processing avoids scratch growth, but the multiband path invokes a general `process_frame` with tiny one-sample slice arrays for every sample (`crossfeed_plugin.rs:364-393`), limiting optimizer/SIMD opportunities.

Apply control changes on a non-realtime owner or use preallocated command/state swaps. Add an allocation-counting setter/reset suite. Extend the crossover API with a scalar or block method and benchmark Bauer/Meier/MB with and without ITD/AutoGain over realistic block sizes.

### P2 — Documentation and schemas disagree on defaults, ranges, and algorithms

`CrossfeedPluginParams::default` selects Multiband (`crossfeed_plugin_params.rs:77-99`), while the parameter schema defaults mode index 3 but user documentation says Bauer is default; the enum's derived default is Off. `USAGE.md` describes Bauer as HPF in one place and the implementation as a low-shelf-on-difference elsewhere; UI ranges (e.g. Bauer 300–2000, MB feeds −12..12) differ from `ParamSpec` (400–1000 and asymmetric −20..0/0..15). `AGENTS.md` names a nonexistent trait. These inconsistencies make presets, saved state, UI, and review expectations unreliable.

Generate docs/UI tables from `ParamSpec`, establish one default source, and document the actual transfer functions and gain conventions. Add metadata snapshot tests covering labels, defaults, ranges, serialized defaults, and constructor defaults.

### P3 — Large fixed scratch reservation is disconnected from the host block contract

The plugin allocates ten 65,536-frame buffers (four main plus six band buffers) regardless of actual maximum block size (`crossfeed_plugin.rs:119-127,550-560`), roughly 2.5 MiB of `f32` scratch per instance. This prevents callback allocation but wastes memory for normal 64–2048-frame hosts and still rejects larger offline blocks.

Accept a graph-build maximum block size and allocate exactly that capacity. Reuse buffers with non-overlapping lifetimes, and expose a block crossover API that can write directly into final scratch.

## Algorithm assessment

Bauer's mid/side low-shelf formulation preserves mono and reduces low-frequency stereo difference; Meier supplies a simple frequency/phase-shaped opposite-ear path; Multiband offers flexible LR4 splitting. These are useful pragmatic crossfeed algorithms, but they are not a complete virtual-loudspeaker model: ITD, ILD, pinna/torso filtering, head orientation, source geometry, and room/direct-field cues are only loosely represented. Describe that limitation, then consider a compact measured/parametric HRTF crossfeed mode if speaker externalization is a quality goal.

## Real-time allocation and performance assessment

The steady audio path uses preallocated scratch, cached multiband gains, f32 LR4 processing, stereo de/interleave helpers, and denormal control. The main real-time risks are allocation/state reconstruction in setters, dynamic delay-buffer resize, and ignored AutoGain errors. Performance opportunities are block crossover processing, pass fusion (deinterleave/process/mix/interleave), f32 Bauer/Meier filters after accuracy checks, and skipping inactive algorithm state without compromising transition continuity.

## Scope reviewed

Read in full: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, `UI.md`, `USAGE.md`, `bin/qa_crossfeed.rs`, every file under `src/` including all 881 lines of inline DSP tests and parameter tests, and all 463 lines of `tests/integration.rs`. Relevant host/factory wiring reviewed includes catalog/factory registration and parameter conversion, `ParametricInPlacePlugin` adapter/validation, compile metadata, `AutoGain`, smoothing, SIMD stereo helpers, LR4 crossover, biquad implementation use, workspace robustness and realtime-allocation coverage. No production code was changed.

## Strengths

- The recent Bauer mid/side shelf formulation preserves mono and has useful low/high response regression coverage.
- The LR4 three-band path uses persistent crossover state and cached linear gains.
- Fractional ITD storage is sample-rate-sized and supports high rates.
- Normal audio processing rejects oversized scratch requests rather than resizing.
- Reset covers filter, crossover, delay, smoother, and AutoGain state; tests exercise all algorithms, sample-rate changes, reset, ITD, yaw-with-static-ITD, mix ramps, and public parameter round trips.

## Verification

`rtk cargo test -p sotf-plugin-crossfeed` — 57 tests passed across three suites.
# Follow-up remediation (0.5.12)

- Fixed the responsive render-plan contract by labeling the overflowable mode
  selector group (`MODE`); the plugin layout test now validates successfully.
