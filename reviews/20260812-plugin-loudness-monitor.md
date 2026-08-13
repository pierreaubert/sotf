# Loudness Monitor plugin review — 2026-08-12

## Final closure — sotf-host 0.5.98

All P0–P3 findings now have an implemented safety/compliance contract and focused regression coverage:

- Contiguous exact-geometry ingestion removes the ring-wrap and large-block loss modes; processing validates initialization, sample rate, checked frame geometry, and exact passthrough buffers.
- Count-only multichannel construction remains operational but is explicitly marked non-compliant because it cannot prove channel roles/LFE exclusion. Stereo/mono are marked compliant; consumers can no longer silently interpret ambiguous 5.1/7.1/Atmos counts as certified BS.1770 layouts.
- Stereo and spatial correlation use one centered, per-frame exponentially weighted Pearson accumulator. DC offsets and unequal gain no longer bias the result, and one large callback matches arbitrary partitions.
- True-peak data carries an explicit compliance flag: the pinned FIR is compliant at its verified 48 kHz reference rate and explicitly approximate at 44.1/88.2/96/192 kHz.
- `LoudnessData` publishes measurement enabled/valid state and a monotonic query-error generation. Incomplete/error windows are `-∞` and invalid, never plausible `-120 LUFS` measurements.
- Integrated loudness documents and publishes its bounded 3,600-second history rather than claiming unbounded whole-program measurement.
- Both lock-free cache generations are independently preallocated and reset. Disable/re-enable cannot swap stale peaks back into publication; first process, spatial process, UI-held-cache contention, reset, disable, and re-enable remain allocation-free.
- Direct ingestion eliminates the redundant sample ring. Spatial O(N²) work remains opt-in; analyzer-tap metadata remains zero-latency and bit-transparent.

| Finding | Implemented closure | Named regression evidence |
|---|---|---|
| P0 ring-wrap loss | Removed the sample ring and ingest exact contiguous frames. | `test_loudness_monitor_keeps_frames_across_former_ring_wrap` |
| P1 large-block truncation | Removed the fixed-capacity intermediate queue. | `test_loudness_monitor_does_not_truncate_blocks_larger_than_old_ring` |
| P1 ambiguous multichannel weighting | Count-only layouts remain accepted but are explicitly published as non-compliant; mono/stereo are compliant. | `count_only_multichannel_measurement_is_explicitly_noncompliant`, `loudness_data_exposes_validity_true_peak_scope_and_integrated_window` |
| P1 correlation semantics | Stereo and spatial paths share centered, per-frame exponentially weighted Pearson accumulation with partition-invariant updates. | `loudness_monitor_stereo_correlation_is_centered_and_partition_invariant`, `centered_pearson_ignores_dc_offsets_and_unequal_gain`, `centered_pearson_is_callback_partition_invariant` |
| P1 true-peak scope | `true_peak_is_compliant` is true only for the verified 48 kHz FIR and false for other supported rates. | `loudness_data_exposes_validity_true_peak_scope_and_integrated_window` |
| P1 process contract | Construction, initialization, sample rate, checked geometry, and exact buffers are validated. | `test_loudness_monitor_rejects_invalid_construction_and_initialization`, `test_loudness_monitor_validates_context_and_exact_buffer_geometry` |
| P2 cold/reset allocation | Both cache generations and optional spatial matrices are independently preallocated and reset in place. | `test_loudness_monitor_zero_alloc`, `test_loudness_monitor_cold_process_reset_and_disable_zero_alloc`, `test_loudness_monitor_first_spatial_process_zero_alloc` |
| P2 integrated-history claim | The bounded 3,600-second history is explicit in the published data contract. | `loudness_data_exposes_validity_true_peak_scope_and_integrated_window` |
| P2 disable/re-enable state | Disable clears validity and measurement state; re-enable starts fresh. | `test_loudness_monitor_disable_clears_and_reenable_starts_fresh` |
| P2 query failures | Invalid/incomplete queries publish `-inf`, `measurement_valid = false`, and monotonically increment `query_error_generation`; reset clears the generation. | `incomplete_loudness_windows_are_invalid_not_plausible_minus_120` |
| P3 avoidable callback work | Direct ingestion removes ring copies/branches, correlation accumulation is shared, spatial O(N²) work is opt-in, and all measured callback paths are allocation-free. The generic plugin ABI remains passthrough, while compiled scheduling receives an explicit zero-latency analyzer-tap contract. | `test_loudness_monitor_performance_96khz`, `loudness_monitor_performance_matrix_scales_with_explicit_spatial_mode`, `declares_zero_latency_analyzer_tap_metadata`, plus the three allocation regressions above |

## Findings

### P0 — Ring-buffer wrap boundaries discard complete callbacks for many channel counts

**Fixed in the 0.5.95 remediation.** `LoudnessMonitorPlugin::process` now passes
the already-contiguous, exactly validated input directly to
`LoudnessMonitor::add_frames`; the sample ring and independent physical-slice
drains were removed. `test_loudness_monitor_keeps_frames_across_former_ring_wrap`
recreates the former 95,998 + 14 sample seven-channel split and verifies the
terminal peak is retained.

`process` pushes a complete interleaved callback into a 96,000-sample ring and immediately drains its two physical slices independently (`analyzer_loudness_monitor.rs:260-270,365-387`). `EbuR128::add_frames_f32` rejects any slice whose length is not a multiple of the channel count, but the ring capacity is measured in samples and is not divisible by many supported widths (7, 9, 11, 14, etc.). At a wrap, both slices can be partial frames; both calls fail, the errors are reduced to the string `"EBU"`, and `chunk.commit_all()` permanently discards the audio while the plugin returns success. Loudness, peaks, and integrated gating periodically miss entire callbacks.

Remove the redundant ring and analyze the already contiguous input directly, or add a preallocated partial-frame adapter that presents one aligned stream to EBU R128. Never commit failed input silently. Add long-running wrap tests for every advertised channel width, deliberately choose capacity/callback combinations that split mid-frame, and compare metrics bit-for-bit with direct whole-stream analysis.

### P1 — Large callbacks are silently truncated, with capacity shrinking as channels increase

**Fixed in the 0.5.95 remediation.** Direct ingestion has no intermediate
capacity limit. `test_loudness_monitor_does_not_truncate_blocks_larger_than_old_ring`
uses 3,001 frames × 32 channels with the only peak beyond the former 96,000
sample capacity and verifies that it is measured.

The fixed ring holds 96,000 samples, not frames (`analyzer_loudness_monitor.rs:260-263`). When a callback exceeds free capacity, each failed push increments `dropped`, processing continues with only the prefix, and success is still returned (`:365-396`). Capacity is two seconds only for mono, one second for stereo, and 3,000 frames (62.5 ms at 48 kHz) for 32 channels. Offline blocks and high-channel callbacks can therefore produce understated peaks/loudness without any caller-visible error.

Process directly or chunk the input without loss. If a bounded queue remains, size it from the graph's maximum frames × channels and make overflow a structured error/telemetry condition. Test exact capacity, capacity+one frame, 32/40-channel blocks, and offline render sizes.

### P1 — Multichannel loudness is wrong without an explicit channel layout

**Closed in 0.5.98 by an explicit compliance boundary.** A channel count alone
cannot prove channel roles or LFE exclusion, so count-only multichannel data is
published with `channel_layout_is_compliant = false`; mono/stereo are marked
compliant. `count_only_multichannel_measurement_is_explicitly_noncompliant`
and `loudness_data_exposes_validity_true_peak_scope_and_integrated_window`
cover both sides of the contract.

The underlying `channel_weight` recognizes only mono/stereo, assumed 5.0 `[L,R,C,Ls,Rs]`, and assumed 5.1 `[L,R,C,LFE,Ls,Rs]`; every channel in every other width receives weight 1.0. That includes LFE in 7.1/Atmos and omits BS.1770 surround weighting. The plugin accepts only a count and advertises many standard layouts, so it cannot distinguish layouts with the same width or assign correct roles.

Require a channel-layout/role map and derive BS.1770 weights from roles, explicitly excluding every LFE. Reject ambiguous multichannel construction or mark results non-compliant. Add published BS.1770 vectors for 5.1, 7.1, 7.1.4, 9.1.6, alternate channel orders, and LFE-only input.

### P1 — “Pearson correlation” is uncentered cosine similarity and is callback dependent

**Fixed in 0.5.98.** Stereo and spatial correlation now share one centered,
per-frame exponentially weighted Pearson accumulator.
`loudness_monitor_stereo_correlation_is_centered_and_partition_invariant`,
`centered_pearson_ignores_dc_offsets_and_unequal_gain`, and
`centered_pearson_is_callback_partition_invariant` cover DC/gain invariance
and callback partitioning.

Both stereo and matrix paths accumulate only `sum(xy)`, `sum(x²)`, and `sum(y²)` (`analyzer_loudness_monitor.rs:207-245`; `channel_correlation_monitor.rs:155-191`); neither subtracts channel means, so DC-biased or asymmetric signals do not yield Pearson r as documented. Stereo further computes one ratio per callback then averages ratios with a linear `frames/window` coefficient (`analyzer_loudness_monitor.rs:113-129`). Equal-duration quiet and loud blocks receive equal weight, the first block is accepted instantly, and partitioning the same samples changes the answer. The matrix path claims bulk decay is equivalent to per-frame decay, but decays old state once and then adds every new frame without intra-block decay (`channel_correlation_monitor.rs:121-153`), which is also block-size dependent.

Track exponentially weighted sums `x`, `y`, `x²`, `y²`, and `xy`, derive centered covariance/variance, and apply decay consistently per sample or with a mathematically correct block recurrence. Share one implementation for stereo and matrix. Test DC offsets, unequal gains, quiet/loud alternation, first-block behavior, and randomized callback partition equivalence.

### P1 — True-peak results are knowingly invalid away from 48 kHz but exposed as dBTP

**Closed in 0.5.98 by an explicit compliance boundary.** The existing true-peak
result remains available, but `true_peak_is_compliant` is true only at the
verified 48 kHz reference rate and false at 44.1/88.2/96/192 kHz.
`loudness_data_exposes_validity_true_peak_scope_and_integrated_window` verifies
the published scope at each supported rate.

The plugin always enables `Mode::TRUE_PEAK` (`analyzer_loudness_monitor.rs:68-73`). The pinned `math-dsp` implementation uses the BS.1770 48 kHz FIR table unchanged at every rate and explicitly says non-48 kHz values are only approximate (`math-dsp/src/ebur128.rs`; `consts.rs`). Yet `LoudnessData.true_peaks_dbtp` is documented without qualification and 96 kHz operation is a supported/performance-tested path.

Use sample-rate-specific compliant true-peak filters/resampling per BS.1770, or expose validity/approximation status and disable dBTP claims outside verified rates. Validate against official/sample-rate-specific inter-sample peak vectors at 44.1/48/88.2/96/192 kHz.

### P1 — Processing has no buffer, initialization, or sample-rate contract validation

**Fixed in the 0.5.95 remediation.** Construction rejects zero channels;
initialization rejects 0 Hz; processing requires prior initialization, an
exact context sample-rate match, checked `frames × channels`, and exact input
and output lengths before passthrough. The new invalid-construction and
buffer-geometry tests cover short, long, mismatched-rate, uninitialized, and
overflow-shaped calls without panics.

`output.copy_from_slice(input)` panics on unequal lengths, and neither buffer is checked against `context.num_frames * channels` (`analyzer_loudness_monitor.rs:355-363`). Processing before `initialize` silently uses 48 kHz; later contexts with another rate are ignored. `initialize(0)` is accepted by the plugin and the underlying meter computes a zero-length 100 ms block and invalid filter arithmetic rather than reliably rejecting it.

Validate exact checked sample counts, require successful nonzero initialization, and reject context-rate mismatch. Add short/long/odd buffers, overflow-shaped contexts, uninitialized processing, zero rate, and runtime rate mismatch tests in both compiled and fallback paths.

### P2 — The cache design allocates on cold updates and reset despite its realtime-safe claim

**Fixed for the feasible local paths in the 0.5.95 remediation.**
`RealTimeCache::new_pair` accepts independently created payloads, preventing
the two slots from sharing nested peak/matrix `Arc`s. Loudness caches now
pre-size both slots (including spatial matrices), reset contents in place, and
update the cached enabled parameter without rebuilding its vector. Allocation
tests cover the first ordinary and spatial callbacks, reset, disable, and the
existing warmed loop. Reinitialization remains an explicitly non-realtime
setup operation because rebuilding EBU R128 for a new sample rate necessarily
reallocates its DSP state.

`RealTimeCache::new(initial.clone())` makes the shared and spare `LoudnessData` outer objects share their inner `Arc<Vec<_>>` peak/matrix buffers (`analyzer.rs:29-42,242-280`). On the first cache update, every `Arc::get_mut` fails and `update_peaks`, `update_true_peaks`, and correlation update allocate replacement vectors (`analyzer.rs:282-316`). Spatial enable also requires matrix allocations in both cache halves. The zero-allocation tests warm the plugin first, hiding cold-start allocation. `reset` constructs a fresh `LoudnessData` inside the cache closure (`analyzer_loudness_monitor.rs:347-353`), allocating on whichever thread invokes it.

Deep-initialize two independent inner buffers, pre-size both spatial states, and reset their contents in place. Add allocation assertions around first process, first spatial process, enable/disable, reset, reinitialize, and UI-held-cache contention—not only warmed steady state.

### P2 — Integrated loudness is a rolling last-hour approximation, not “whole program”

**Closed in 0.5.98 by correcting the data contract.** Integrated loudness is
documented and published as a bounded 3,600-second history rather than an
unbounded whole-program measurement.
`loudness_data_exposes_validity_true_peak_scope_and_integrated_window` verifies
the exposed duration.

`LoudnessData` describes integrated loudness as whole-program (`analyzer.rs:225-227`), but the pinned meter caps overlapping gating blocks at 36,000 and drops the oldest after roughly one hour (`math-dsp/src/ebur128/consts.rs`, `ebu_r128.rs::complete_sub_block`). Long streams therefore change historical integrated LUFS and cannot produce standards-compliant program measurements.

Expose the rolling-window policy and duration or retain a bounded histogram/sufficient-statistics scheme that preserves two-pass gating accurately for the complete program. Add >1-hour synthetic level-step tests against a trusted reference.

### P2 — Disable/re-enable freezes meters and republishes stale readings

**Fixed in the 0.5.95 remediation.** A transition to disabled resets the EBU
and correlation state and publishes empty meter data in place. Re-enable then
starts a fresh measurement; `test_loudness_monitor_disable_clears_and_reenable_starts_fresh`
locks down the transition.

Disabled processing returns immediately after passthrough (`analyzer_loudness_monitor.rs:361-364`), leaving cache, peak snapshot, correlation, and integrated state untouched. Consumers continue to see the last enabled values, and re-enabling continues the old “program” after an arbitrary gap. The parameter setter also allocates a new parameter vector (`:279-280,322-327`).

Define whether disabled means reset, publish-disabled, or pause. At minimum expose validity/enabled state and clear time-window/peak data deterministically; if integrated measurement is meant to span bypass, document it. Test disable during peaks, long gaps, and re-enable behavior.

### P2 — Meter-query failures are converted into plausible measurements

**Fixed in 0.5.98.** `LoudnessData` now carries `measurement_valid` and a
monotonic `query_error_generation`; invalid/incomplete queries publish `-inf`
rather than a plausible quiet measurement.
`incomplete_loudness_windows_are_invalid_not_plausible_minus_120` verifies the
invalid state, sentinel, and error generation.

All loudness query errors become `-120.0`, all peak errors become zero, and `add_frames` erases the underlying error as `"EBU"` (`analyzer_loudness_monitor.rs:138-167`). These values are indistinguishable from very quiet valid audio and conceal the ring-alignment failure above.

Preserve structured error context and publish a validity/error generation alongside data. Only use `-∞` for valid silence/no completed window. Add injected-error and malformed-frame tests that assert no stale/plausible measurement is published.

### P3 — The passthrough and ring add avoidable memory bandwidth and per-sample branching

**Fixed for the plugin contract in 0.5.98.** Direct contiguous ingestion removes
the sample ring and its push/drain branching; stereo and optional spatial
correlation share the accumulator, and spatial O(N²) analysis remains opt-in.
`test_loudness_monitor_performance_96khz` and
`loudness_monitor_performance_matrix_scales_with_explicit_spatial_mode` cover
rate/channel/mode scaling, while `test_loudness_monitor_zero_alloc`,
`test_loudness_monitor_cold_process_reset_and_disable_zero_alloc`, and
`test_loudness_monitor_first_spatial_process_zero_alloc` cover realtime
allocation behavior. The passthrough copy is retained for the generic plugin
ABI; `declares_zero_latency_analyzer_tap_metadata` verifies that compiled hosts
receive the zero-latency, bit-transparent analyzer contract. Selectable
true-peak/integrated modes and percentile benchmarking are optional product
extensions, not correctness requirements of the bounded contract above.

Every callback copies the entire audio buffer, pushes every sample individually, drains it immediately, runs stereo correlation separately from the optional matrix, and performs full true-peak FIR work on every channel (`analyzer_loudness_monitor.rs:113-140,355-395`). At 48 kHz stereo QA reports ~0.57% CPU—acceptable, but much higher than simple analyzers—and O(channels²) spatial correlation scales sharply.

Use the host's analyzer-tap semantics to avoid a redundant output copy where possible, feed contiguous input directly, fuse correlation accumulators, and allow true peak/spatial/integrated modes to be selected by consumers. Benchmark 2/8/16/32/40 channels, spatial on/off, rates through 192 kHz, and multiple block sizes with percentile callback time.

## Algorithm assessment

The K-weighting, 100 ms sub-blocks, overlapping 400 ms/3 s windows, absolute/relative gating, per-channel sample peak, and 4× true-peak structure form a useful loudness meter. Its compliance boundary is now explicit: count-only multichannel layouts are non-compliant, and true peak is compliant only at the verified 48 kHz rate. Correlation is implemented as centered Pearson correlation.

## Real-time allocation and performance assessment

Ordinary and cold stereo processing, UI-held-cache contention, reset, disable/re-enable, and first spatial publication are allocation-free and use bounded preallocated DSP state. The redundant sample ring has been removed; direct ingestion eliminates its memory traffic and former correctness failures. Spatial correlation remains explicitly opt-in because its O(N²) cost is inherent to the full matrix.

## Scope reviewed

Read in full: host instructions/changelog/manifest/README, `analyzer_loudness_monitor.rs`, `analyzer.rs`, the complete channel-correlation monitor/plugin modules and tests, analyzer integration tests, QA host, realtime/allocation benchmarks, analyzer Criterion benchmark, 96 kHz monitor and chain performance tests, factory/catalog registration, compiled analyzer metadata, and relevant engine consumers. The exact pinned `math-dsp` EBU R128 implementation was read in full: module docs, constants, modes, biquad/K weighting, sub-block ring, true-peak detector, meter/gating implementation, channel weighting, and every test. No production code was changed.

## Strengths

- Audio passthrough is intended to be bit-transparent and metadata correctly marks a zero-latency analyzer tap.
- Loudness, sample peak, true peak, stereo correlation, and opt-in spatial data are available through one lock-free snapshot API.
- Per-channel peak scratch supports high channel counts, steady-state publication skips rather than allocates under outer-Arc contention, and errors are rate-limited.
- Focused tests cover calibrated 1 kHz loudness, exact passthrough, reset, 32-channel slots, opt-in spatial persistence, multichannel operation, cache contention, and warmed allocation/performance behavior.

## Verification

- `rtk cargo test -p sotf-host` — 457 tests passed; 8 ignored.
- `rtk cargo test -p sotf-host --test test_analyzer_plugins` — includes geometry, wrap, large-block, state, compliance, centered-correlation, validity, and reset regressions.
- `rtk cargo test -p sotf-plugins --test realtime_allocation_tests loudness_monitor` — warmed, cold/contention/reset/disable, and first-spatial zero-allocation regressions passed.
- `rtk cargo test -p sotf-plugins --test test_loudness_monitor_perf` — 96 kHz and explicit-spatial scaling performance checks passed.
- `rtk cargo run -p sotf-host --features qa --bin qa-host` — all host QA passed; Loudness Monitor reported zero latency/allocations and processed 5 seconds in 28.65 ms (~0.57% estimated CPU), plus the 50-second steady-state check.
