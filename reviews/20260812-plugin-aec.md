# AEC plugin code review — 2026-08-12

## Remediation status — 2026-08-12

### Final closure — 0.5.7

All P0–P3 findings in this review are now fixed and regression-tested.

| Finding | Closure and regression evidence |
|---|---|
| P1 spectral-domain post-filter input | The dedicated pre-IFFT estimate remains covered by `last_echo_estimate_is_frequency_domain_data`. |
| P1 circular/unconstrained PBFDAF | Previous/current overlap-save input and per-update causal projection remain covered by `adaptive_partitions_are_causal_after_frequency_update` and the delayed late-partition ERLE test. |
| P1 double-talk corruption / full-echo suppression model | Foreground is now a stable promoted path; background adaptation falls to 5% during double-talk. The suppressor learns a bounded residual-leakage factor and uses near-end onset plus rate-derived attack/release. `double_talk_freezes_foreground_and_recovers_afterwards`, `leakage_model_preserves_near_end_during_balanced_double_talk`, and `abrupt_echo_path_change_recovers_after_double_talk` cover gating, 0.25/1/2 near/far ratios, and recovery after a delayed-path change. |
| P1 callback-dependent latency | The bounded adapter still produces exactly 256 samples at callback sizes 1–1024; the existing segmentation/impulse regressions remain green. |
| P1 realtime parameter behavior | Tail and step size remain explicitly structural/state-preserving-by-rejection. Post-filter toggles update cached metadata in place, keep suppressor state current, and ramp over 10 ms; `post_filter_toggle_is_ramped_and_keeps_suppressor_state_current` plus QA allocation counting cover the realtime path. |
| P1 invalid persisted construction | Canonical construction rejects non-finite/out-of-range state before allocation in both factories; constructor and new factory/bridge conformance tests cover it. |
| P2 oversized callback allocation | One-block FIFO remains bounded; the above-old-capacity regression and standard QA allocation counter remain green. |
| P2 reinitialize stale state | Full rate-dependent rebuild/reset remains covered byte-for-byte against a fresh new-rate instance. |
| P2 duplicated two-path analysis | Reference FFT, FDL, and power analysis are now shared. `reference_fft_and_fdl_analysis_is_shared_by_both_paths` proves one reference transform per block; repeated standard QA runs improved from the original failing 5.42% to 2.98–3.22% CPU for its 48 kHz/100 ms-tail fixture. |
| P2 runtime/catalog/bridge drift | `params::Params` and `PARAMS` are now the runtime schema; duplicate f32 state files were removed, structural modes match, and main/bridge factories share validation and exact 2-input/1-output enforcement. Canonical schema and both factory tests cover parity. |
| P3 FFT/toggle/time constants/non-finite policy | The unused forward FFT is gone, post-filter reconstruction uses unique real-spectrum bins and `realfft`, dry mode evolves state and ramps, coefficients are sample-rate/block-duration derived, and NaN/±Inf are silenced before adaptation. Dedicated regressions cover each behavior. |

Final verification: `cargo test -p sotf-plugin-aec` (51 passed),
`cargo clippy -p sotf-plugin-aec --all-targets -- -W warnings` (no AEC warnings),
`cargo test -p plugins-bridge aec_bridge_enforces_canonical_bus_layout_and_ranges --offline`
(passed), `cargo check -p sotf-plugins --no-default-features --offline` (passed), and
`cargo run -p sotf-plugin-aec --features qa --bin qa-aec --offline` (zero allocations,
3.22% CPU on the final run, passed). The workspace-level focused allocation-test binary is currently blocked
before execution by an unrelated ABCompare `PARAMS`/field-count compile-time assertion.

### Follow-up — 0.5.6

- **Fixed:** initialization now uses an explicit post-filter rebuild path and
  regression coverage proves that changing sample rate after partial input and
  queued output produces byte-for-byte the same stream as a fresh plugin at the
  new rate.

- **Fixed:** the residual suppressor now receives a dedicated pre-IFFT echo spectrum, covered by
  a reconstruction regression.
- **Fixed:** after each PBFDAF frequency-domain update, partitions are projected onto their
  causal time-domain support before reuse; delayed-echo and non-causal-energy regressions cover
  the late-partition path.
- **Deferred:** robust coherence/Geigel double-talk adaptation gating and a calibrated residual
  leakage model require a dedicated DSP-quality change with speech/path-change fixtures.
- **Fixed:** output has an exact 256-sample delay across callback sizes 1 through 1024, and the
  bounded one-block queue cannot allocate for oversized callbacks.
- **Partial:** echo-tail and step-size are structural and rejected live; the boolean toggle is
  allocation-free. A wet/dry ramp and state-preserving realtime step-size smoothing remain deferred.
- **Fixed:** construction validates finite documented ranges and is fallible through both factories.
- **Fixed:** reinitialization rebuilds rate-dependent state and clears all streaming/post-filter state.
- **Deferred:** shared two-path FFT/FDL work, schema consolidation, sample-rate-derived smoothing,
  non-finite input policy, and real-FFT conversion remain performance/DSP follow-ups.

Verification after remediation: `cargo test -p sotf-plugin-aec` (39 passed), AEC realtime allocation
test (passed), and `cargo check -p plugins-bridge` (passed).

## Findings

No P0 issue was found. The default post-filter path does, however, contain a confirmed spectral-domain correctness failure, so the plugin should remain Alpha until the P1 items below are fixed and covered by signal-level regression tests.

### P1 — `last_echo_estimate_freq()` returns time-domain IFFT data to the default post-filter (confirmed)

**Evidence:** `Pbfdaf::process` accumulates the echo estimate in `output_buf`, then transforms that same buffer in place with the inverse FFT at `crates/sotf-plugins/crates/sotf-plugin-aec/src/pbfdaf.rs:114-127`. Nevertheless, `last_echo_estimate_freq()` documents the buffer as the *pre-IFFT* spectrum and returns it at `pbfdaf.rs:235-239`. The default-enabled plugin passes that value beside the actual error spectrum to `ResidualEchoSuppressor::process` at `src/lib/aec_plugin.rs:286-290`; the suppressor compares the two values bin by bin and as total spectral powers at `src/post_filter.rs:87-106,124-145`.

**Impact:** every default post-filter frame compares frequency bins of `FFT(error)` with unnormalised time-domain echo samples. DTD decisions and Wiener gains therefore have no spectral meaning and can produce frequency-dependent over/under-suppression, pumping, and near-end speech damage. Unit tests call the suppressor with two valid synthetic spectra, so they do not exercise this integration failure.

**Fix/test:** preserve the accumulated echo spectrum in a dedicated preallocated buffer before the IFFT (or IFFT a separate working buffer), and return that spectrum. Add an integration test that verifies `last_echo_estimate_freq()` against a direct FFT of the time-domain estimate, including conjugate symmetry and Parseval-scaled energy. Then compare post-filter enabled/disabled output on a deterministic delayed-white-noise echo and double-talk fixture.

### P1 — the advertised partitioned overlap-save convolution is circular and unconstrained (confirmed)

**Evidence:** each FDL entry is formed as `[current reference block, zeros]` at `src/pbfdaf.rs:96-112`, while output is taken from the *last* half of the circular IFFT at `pbfdaf.rs:125-134`. The frequency-domain weights are then updated directly at `pbfdaf.rs:155-179` without the standard time-domain gradient constraint (IFFT each partition, zero the noncausal half, FFT back). There is also no saved previous reference block.

**Impact:** with a causal length-`B` partition, `[current, 0]` does not supply the `[previous, current]` overlap required for the last `B` IFFT samples to represent linear convolution. Unconstrained weights can learn circular/negative-time aliases, giving non-unique echo paths and degraded convergence/tracking, especially for broadband paths crossing partition boundaries. The current convergence tests use sums of sinusoids and only require 5 dB ERLE (`pbfdaf.rs:294-351`), which can hide time aliasing.

**Fix/test:** implement a documented MDF/PBFDAF convention: retain the previous block and FFT `[previous, current]`, keep causal partition weights, and periodically or every update apply the time-domain causality constraint. Validate against direct FIR convolution using seeded white noise and impulse paths at delays `0`, `B-1`, `B`, `B+1`, and the configured tail end. Require streaming/offline agreement and ERLE across varied host block segmentation.

### P1 — neither adaptive path is protected during double-talk, and the residual suppressor models the cancelled echo as residual echo (confirmed)

**Evidence:** foreground and background filters both update on every block (`src/two_path.rs:71-78` and `src/pbfdaf.rs:155-179`). The only DTD is inside the post-filter, after adaptation. It calls the residual error “mic power” and bypasses only when residual power is more than 6 dB above the echo estimate (`src/post_filter.rs:87-106`). When not bypassed, it subtracts `beta * |full echo estimate|^2` from `|error|^2` at `post_filter.rs:130-145`; no misalignment/leakage estimate converts the cancelled echo estimate into residual-echo PSD.

**Impact:** near-end speech continues driving both adaptive filters and can corrupt the learned echo path. During ordinary double-talk where far-end echo is comparable to or louder than near-end speech, the 6 dB rule remains false and the full echo estimate drives gains toward the floor, suppressing speech. The existing DTD test uses the easy zero-reference case and tolerates up to 20 dB near-end loss (`src/lib/tests.rs:242-282`), so it does not validate real double-talk.

**Fix/test:** use a robust far-end/near-end coherence or Geigel-style detector to freeze or strongly reduce adaptation during double-talk; keep a truly stable foreground path while the background explores. Estimate residual echo as a bounded, smoothed leakage/misalignment factor times echo PSD, and use attack/release smoothing. Test far-end-only convergence, near-end-only transparency, double-talk at multiple near/far ratios, abrupt path changes, and nonstationary speech; report ERLE, near-end attenuation, convergence time, and recovery.

### P1 — reported latency is not the streaming latency and changes with host callback size (confirmed)

**Evidence:** input is accumulated until 256 samples, produced blocks are queued, and the queue is drained only after the complete host input callback has been consumed (`src/lib/aec_plugin.rs:271-340`). `latency_samples()` always reports 256 at `aec_plugin.rs:345-347`. For a 256- or 512-frame callback, freshly produced output is returned in the same callback (zero sample displacement); for a fixed callback `H < 256`, startup zero-fill and queue timing produce `256-H` samples of displacement. Thus the signal delay depends on callback partitioning, not the reported constant.

**Impact:** host latency compensation can overcompensate or misalign AEC output, and changing device buffer size changes alignment. Correct alignment is especially important because mic/reference timing directly bounds cancellable ERLE.

**Fix/test:** choose one contract and implement it explicitly: either queue exactly one full AEC block before emission and report 256, or expose a true zero-latency block transform and report zero where the host can supply full blocks. An internal frame adapter should produce identical sample placement for host chunks `1, 64, 128, 255, 256, 257, 480, 512, 1024`. Add impulse/sample-index tests; the workspace block-size matrix explicitly omits AEC (`crates/sotf-plugins/tests/plugin_high_channel_tests.rs:698-715`) and its latency test checks only nonzero metadata.

### P1 — parameters advertised as realtime allocate, plan FFTs, destroy state, and can drop out (confirmed)

**Evidence:** host `Parameter::new_*` defaults to `UpdateMode::Realtime` (`crates/sotf-plugins/crates/sotf-host/src/parameters.rs:157-177`), whose contract is “updated without rebuilding ... zero-dropout.” AEC does not override it (`src/lib/aec_plugin.rs:114-132`). Every setter first clones the cached parameter vector through validation, then rebuilds that vector; echo-tail and step-size changes additionally construct and drop two complete PBFDAFs, including FFT plans and all partition buffers (`aec_plugin.rs:197-211`, `104-111`). Rebuilding also discards the learned echo path.

**Impact:** live automation/control callbacks can allocate and deallocate megabytes, plan FFTs, and reset cancellation, causing audio-thread stalls and an abrupt return of echo. Even the boolean toggle allocates metadata. Process-only allocation tests do not cover setters.

**Fix/test:** mark echo-tail structural in both parameter systems and rebuild/swap it off the audio thread. Make step size a state-preserving coefficient update (with bounded smoothing), and make the boolean toggle allocation-free with a short wet/dry ramp. Avoid `parameters()` cloning in validation, or use static specs. Add counted-allocation and continuity tests for every parameter update; assert adaptive-state preservation where promised.

### P1 — configuration construction bypasses range validation and can cause instability or unbounded allocation (confirmed)

**Evidence:** `AecPluginParams` documents ranges but serde only deserializes fields (`src/lib/aec_plugin_params.rs:6-16`). `from_params` copies values without validation or clamping and immediately rebuilds (`src/lib/aec_plugin.rs:95-111`). Both factories call it directly (`crates/sotf-plugins/src/factory/create.rs:435-444`; `crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:315-318`). Only later `set_parameter` calls validate ranges.

**Impact:** a preset/config can select `mu` outside the stable intended range or request an enormous echo tail, leading to divergence or memory exhaustion during plugin creation. Values below the documented tail range silently become a one-partition filter. This is a boundary correctness and availability issue.

**Fix/test:** make `from_params` return `PluginResult<Self>` and validate finite values/ranges before allocation; reject rather than silently clamp persisted invalid state. Bound the derived sample count with checked arithmetic and a documented memory/tail limit. Exercise both factories with below/above-range, non-finite where the API permits it, extreme sample rates, and oversized tail values.

### P2 — valid large callbacks can allocate in `process()` (confirmed)

**Evidence:** the ring buffer is preallocated for 64 internal blocks, but `ensure_output_capacity` allocates and copies when it grows (`src/lib/aec_plugin.rs:139-155`) and is called inside the processing loop (`aec_plugin.rs:280-281`). Output is not drained until all `num_frames` have been ingested. The “large host block” test uses only 32 blocks and does not count allocations (`src/lib/tests.rs:123-148`).

**Impact:** callbacks above 16,384 frames at the fixed block size reallocate on the realtime path. The `Plugin::process` contract has no such maximum, and offline/aggregate hosts can legally exceed it.

**Fix/test:** drain into the caller output as blocks become available, retaining only the fixed adapter delay, or negotiate/preallocate a maximum block size outside processing. Add allocation assertions immediately below, at, and above the capacity boundary.

### P2 — reinitialization leaves old-rate adapter and post-filter state live (confirmed)

**Evidence:** `initialize` changes the sample rate and replaces only `aec` (`src/lib/aec_plugin.rs:230-233`). It does not clear a partially accumulated mic/reference block, queued output, or post-filter gains/DTD state. `reset` does clear those states (`aec_plugin.rs:236-244`) but is not called.

**Impact:** reinitializing after processing can combine old-rate samples with new-rate AEC state and emit old-rate queued audio; stale post-filter statistics then control the new stream. The existing sample-rate test initializes before meaningful audio and checks only that processing succeeds.

**Fix/test:** make initialize atomically rebuild and reset all streaming state (or reject live sample-rate changes). Test reinitialization with a half-full input block and nonempty output queue, then assert clean startup, correct latency, and no stale samples.

### P2 — two-path work duplicates the most reusable FFT/FDL computation (performance recommendation)

**Evidence:** `TwoPathAec` owns two complete `Pbfdaf` instances (`src/two_path.rs:42-56`) and calls each full process path per block (`two_path.rs:71-78`). Each independently FFTs the identical reference, stores an identical FDL, and recomputes identical per-bin FDL power before differing only in weights/step size (`src/pbfdaf.rs:96-112,146-153`). At the default post-filter setting, another inverse FFT is then performed.

**Impact:** the default path performs duplicate reference FFT, memory traffic, power summation, and FDL storage on every frame, reducing channel/block headroom. Cost grows linearly with echo-tail partitions.

**Fix/test:** share one reference FFT/FDL/power estimator between foreground and background, leaving separate weights, echo estimates, and errors. Flatten partition/bin storage if profiling supports it, use real FFTs for real signals, and benchmark CPU plus cache/memory bandwidth at 48/96/192 kHz and 50/200/500 ms tails before/after.

### P2 — runtime, catalog, and bridge contracts can drift (confirmed)

**Evidence:** runtime factories deserialize the separate f32 `AecPluginParams` (`src/lib/aec_plugin_params.rs:6-27`), while the catalog/layout/FFI expose the f64 `params::Params` and `PARAMS` (`src/params.rs:21-38,68-128`). This contradicts `params.rs`’s “single source of truth” claim. In addition, the main factory enforces exactly two inputs (`crates/sotf-plugins/src/factory/create.rs:435-440`), while the universal bridge factory accepts its `channels` argument but ignores it for AEC (`plugins-bridge/src/factory.rs:315-318`).

**Impact:** defaults, ranges, update modes, serialization versions, and validation can diverge between UI/FFI and actual construction; bridge formats can create an AEC for an invalid bus layout and fail later at render time.

**Fix/test:** derive runtime construction/state from one parameter definition and shared validation path. Apply the same exact 2-in/1-out check in every factory. Add a cross-factory parameter/default/range/channel-layout conformance test.

### P3 — cleanup and lower-priority DSP improvements (recommendations)

- `AecPlugin` plans and stores a forward FFT that is never used; only its inverse FFT is called (`src/lib/aec_plugin.rs:31-35,55-65,305-308`). Remove it and size scratch for the actual inverse plan.
- Post-filter-disabled time freezes gain/DTD state, so re-enabling can apply stale suppression immediately. Either keep estimates current while dry or reset/ramp on enable (`aec_plugin.rs:209-211,286-328`).
- Fixed per-block smoothing/leakage values (`src/two_path.rs:46-54`, `src/pbfdaf.rs:155-170`, `src/post_filter.rs:99-106,141-145`) change their time constants with sample rate. Derive coefficients from seconds and `block_size / sample_rate`.
- Weight sanitisation resets only non-finite coefficients after an unstable update (`pbfdaf.rs:168-177`) but lets non-finite input/error reach output. Define an input policy and add silence, denormal, NaN/Inf, full-scale step, and long-run finite/stability tests.

## DSP and streaming contract observed

- Input is fixed interleaved mono microphone + mono far-end reference; output is mono error. Main factory enforces 2 inputs, bridge factory currently does not.
- Internal adaptive block is fixed at 256 samples; complex FFT size is 512; configured tail is partitioned by 256 samples.
- Both foreground and background adapt continuously. Foreground output is selected; background state is copied after 25 consecutive smoothed blocks with at least 1 dB residual-power advantage.
- The process method overwrites exactly `context.num_frames` output samples and returns that count. Partial internal blocks are zero-filled through the output adapter.
- Advertised latency is 256, but actual sample placement is callback-size dependent as described above.
- Reset clears adaptive weights/FDLs, post-filter gains/DTD, adapter counters, and queued output. Reinitialize does not fully reset streaming state.
- There is no plugin-owned full bypass; host bypass behavior and adaptive-state evolution while externally bypassed are not tested. The post-filter toggle is a dry/processed mode switch without a ramp.
- Parameter/preset defaults currently agree (`200 ms`, `0.5`, post-filter on), but two independent state/schema structs own them.

## Test assessment and missing coverage

Strengths include exact input/output size rejection, reset smoke coverage, factory/channel metadata checks, a process-path zero-allocation test, foreground/background state-copy coverage, silence/finiteness smoke tests, and a basic synthetic echo-reduction test. All 34 AEC crate tests passed, and the workspace AEC allocation test passed.

Important gaps:

- no oracle for linear convolution, partition boundaries, spectral-domain identity, or post-filter integration;
- no callback-segmentation equivalence or measured latency test;
- no realistic double-talk/path-change corpus or near-end quality bound (the current allowance is 20 dB loss);
- no parameter-set allocation, state-continuity, invalid-constructor-range, or live reinitialize test;
- no test above the output queue’s 64-block capacity;
- `test_two_path_transfer_threshold_not_too_aggressive` ends without asserting transfer behavior (`src/lib/tests.rs:150-187`);
- no convergence/tracking thresholds across sample rates, tail lengths, reference spectra, or low-level signals;
- no external-bypass/resume behavior test.

## Scope reviewed

Read completely: plugin `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`; `src/lib.rs`, `src/lib/aec_plugin.rs`, `aec_plugin_params.rs`, `default.rs`, `misc.rs`, `src/params.rs`, `src/pbfdaf.rs`, `src/two_path.rs`, `src/post_filter.rs`; every inline/unit test, `tests/integration.rs`, and `bin/qa_aec.rs`.

Also inspected the directly relevant host `Plugin`/`ProcessContext` and parameter update-mode contracts, both plugin factories, catalog entry, engine AEC configuration conversion/settings, FFI parameter mapping discovery, channel/latency/block-size tests, factory/layout/parameter test discovery, realtime allocation test, and allocation benchmark. TokenSave was used first to map symbols, callers, factories, tests, and host dispatch before source reads.

## Verification performed

- `cargo test -p sotf-plugin-aec` — 34 passed.
- `cargo test -p sotf-plugins test_aec_zero_alloc` — 1 passed, 278 filtered out.

No code was changed and no full-workspace build was run.
