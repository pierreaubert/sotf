# Convolution audio plugin code review — 2026-08-12

## Remediation status — 2026-08-12

Fixed in `0.5.9`:

- NUPC now schedules every FFT level at its absolute IR offset; zero-latency
  heads use an exact tail block size and restore the removed head offset.
  Sparse boundary/tail cases are checked against direct time-domain convolution.
- The dormant nested DSP suite is wired correctly and runs as part of the crate.
- Factory/direct defaults, construction bounds, smoother initialization, cached
  host values, successful synchronous/async IR path publication, clear
  cancellation, last-known-good failure behavior, and full state reset on IR
  replacement are corrected.
- The UPC callback no longer dispatches work through Rayon's global pool.
- Compile metadata is conservative about time-varying processing.

Fixed in `0.5.10`:

- Rubato cross-rate IR conversion now uses its whole-clip flush and exact
  startup-delay trimming, preserving impulse timing and the final response
  tail. Regression tests cover 44.1↔48, 48→44.1, and 96→48 kHz impulses.

Fixed in `0.5.11`:

- Async requests carry monotonic generations and publish idle/loading/ready/failed status. Stale,
  cleared, rate-reload, and failed completions cannot replace the last-known-good IR.
- Callback installation atomically swaps prebuilt state without allocating. Large old backends and
  failure strings are transferred through bounded queues for background reclamation.
- Empty/loading/failed/cleared output preserves configured latency. Replacement and clear use a
  bounded 128-sample discontinuity transition and reset incompatible streaming history.
- Only the selected UPC or NUPC backend is built. NUPC channels mapped to the same IR channel share
  immutable spectra, plans, and head taps; duration/channel/memory limits are enforced pre-planning.
- UPC stores sample-exact smoother envelopes beside the matching delayed partition.
- Missing sample rates and invalid construction values are rejected. QA/fuzzing load real IRs and
  exercise active UPC/NUPC configurations.
- README, architecture, usage, UI schema, formats, limits, latency, and status documentation agree
  with the implementation.

Verification after remediation:

- `cargo test -p sotf-plugin-convolution` — 48 passed across 3 suites.
- `cargo test -p plugins-spatial` — 9 NUPC tests passed.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_convolution_zero_alloc` — passed.
- `cargo clippy -p sotf-plugin-convolution -p plugins-spatial --all-targets --all-features
  --no-deps -- -D warnings` — passed with no warnings.

## Findings

### P1 — Confirmed: NUPC loses the absolute time offsets of later IR partition levels

`plan_partitions` records an absolute `offset` for every progressively larger level (`crates/sotf-plugins/crates/plugins-spatial/src/nupc/types.rs:24-60`), and `PartitionLevel::new` uses that offset only to select the corresponding IR slice (`crates/sotf-plugins/crates/plugins-spatial/src/nupc/partition_level.rs:45-57`). Each level then independently emits its result after its own one-block queue latency (`partition_level.rs:81-98,144-149`); neither `PartitionLevel` nor `NupcEngine::process_sample` delays that contribution by the level's absolute IR offset (`nupc_engine.rs:78-92`). For example, with `min_block=1024`, an impulse at IR sample 4096 belongs to a 2048-sample level but is emitted around sample 2048 instead of sample `4096 + 1024`. Later reverb energy therefore moves earlier in time, producing pre-echo and a fundamentally different room/filter response. The zero-latency-head variant has the same problem: it slices the tail at `head_len` and starts the FFT plan at relative offset zero, but never restores the removed `head_len` delay (`nupc_engine.rs:49-75`). This is the default factory backend because `use_nupc` defaults true (`crates/sotf-plugins/crates/sotf-plugin-convolution/src/params.rs:31-33`; `src/lib/types.rs:24-30`).

Implement a scheduled NUPC output accumulator: every level contribution must land at its absolute IR offset plus the declared common latency (or at its exact IR offset for the zero-latency composite). Validate the entire streaming engine against direct time-domain convolution for sparse impulses at every partition boundary and inside every level, long random IRs, varied callback sizes, and both head modes. `test_nupc_vs_upc_simple` does not compare NUPC with UPC or an oracle; it only checks finite/nonzero output (`plugins-spatial/src/nupc/tests.rs:69-97`).

### P1 — Confirmed: resampled IRs retain Rubato startup delay and discard the corresponding tail

`resample_ir` repeatedly calls `process_into_buffer`, appends its raw output, then truncates from the end to the nominal resampled length (`crates/sotf-plugins/crates/sotf-plugin-convolution/src/lib/convolution_plugin.rs:481-542`). Rubato's synchronous FFT resampler has a nonzero `output_delay()` (`fft_size_out / 2`) and its clip-oriented `process_all_into_buffer` explicitly trims that startup silence. The current code does neither: an IR impulse is shifted later by the resampler delay, while the final response energy is removed by end truncation. Any 44.1/48/96 kHz mismatch therefore changes phase/delay and shortens reverb or correction tails.

Use Rubato's whole-clip API, or explicitly flush enough zero input, remove exactly `output_delay()` leading frames, and retain exactly the rounded target length after that removal. Add delta tests at sample 0 and near the final source sample for 44.1→48, 48→44.1, and 96→48 kHz, plus a direct/reference resampling comparison and energy/tail preservation checks.

### P1 — Confirmed: factory-created instances report stale parameter and IR values

`new` builds `cached_parameters` immediately (`convolution_plugin.rs:126-158`). `from_params` then changes `use_nupc`, head mode/taps, synchronously loads the IR, and changes mix/gain, but returns without rebuilding that cache (`convolution_plugin.rs:207-225`). Consequently the factory path (`crates/sotf-plugins/src/factory/create.rs:129-134`) can process one configuration while `parameters()`, `get_parameter()`, presets, and UI state report constructor defaults and an empty IR path. The public synchronous `load_ir` has the same problem because `apply_ir_state` updates `ir_file` without refreshing the cache (`convolution_plugin.rs:320-344`). The current factory/integration tests never assert the values supplied to `from_params` (`tests/integration.rs:54-68`).

Rebuild the cache after all construction/load assignments, or remove the mutable duplicate cache and derive current values directly from canonical fields. Initialize smoothers to the configured values rather than ramping from constructor defaults on first playback. Add a factory round-trip test with a real IR and non-default values for every field, asserting both the host-visible values and DSP behavior.

### P1 — Confirmed: clearing an IR does not cancel an in-flight load, so the cleared IR can reinstall itself

The empty-path branch clears active state and buffers but leaves `ir_load_result_rx` intact (`convolution_plugin.rs:581-609`). A previously queued worker result is still polled at the next callback and installed unconditionally (`convolution_plugin.rs:736-743`). Thus “load A, immediately clear” can later activate A, and a sample-rate-triggered reload can likewise resurrect an IR after clear. The UI and serialized path can say empty while convolution is active.

Give requests monotonically increasing generations and install a result only if it matches the current desired generation/path. Clearing must invalidate/drop the receiver immediately. Test load→clear before completion, A→B with reversed completion order, sample-rate reload→clear, and worker failure after a newer request.

### P1 — Confirmed: an asynchronous IR install allocates and can destroy large states on the audio thread

The callback receives an owned `IrLoadResult` and calls `apply_ir_state` (`convolution_plugin.rs:736-743`). That function allocates a new `Arc` for `ArcSwap::store`, then replaces `nupc_engines`, the potentially huge FDL, FFT scratch, and Rayon accumulator vectors (`convolution_plugin.rs:320-327`). Replacing these fields can deallocate the old IR spectra/FDLs/NUPC engines on the callback. The failure branch also logs/formats and destroys active state in the callback (`convolution_plugin.rs:743-748`). The steady-state allocation tests warm through the one install before starting their counter (`src/lib/tests.rs:225-242`; `crates/sotf-plugins/tests/realtime_allocation_tests/tests.rs:249-266`), so they do not cover the transition.

Prepare one immutable, fully owned backend state off-thread, atomically exchange a prebuilt `Arc` in the callback without allocation, and retire the old state through a background reclamation queue. Keep error reporting/status transfer allocation-free on the callback. Extend realtime instrumentation to count both allocations and deallocations during repeated successful swaps, failed swaps, clears, and crossfade completion.

### P1 — Confirmed: IR replacement reuses old UPC queues/overlap and has no click-safe transition

`apply_ir_state` replaces spectra/engines and clears only the NUPC dry-delay buffer (`convolution_plugin.rs:320-331`). It does not reset `input_fill`, partially collected input, `output_accum`, or the completed UPC `output_ring` (`convolution_plugin.rs:90-103`). After a UPC change, up to one block of the previous IR remains queued, the old overlap tail is summed with the new response, and pre-change input can be transformed by the new IR. The NUPC engine starts clean but switches transfer functions discontinuously. Sample-rate reinitialization continues running the old-rate response until the worker finishes (`convolution_plugin.rs:653-672`). These transitions can click, smear, leak the old room, and briefly run a frequency-shifted response.

Treat replacement as an explicit transition: either crossfade old/new complete engines with aligned latency, or reset all streaming state at a documented silence boundary. Never combine buffers belonging to different IR generations. Add mid-partition A→B impulse/reference tests, long-tail replacement tests, rate-change tests, and bounded-discontinuity tests.

### P1 — Confirmed: the UPC hot path waits on Rayon workers

For IRs of eight or more partitions, every channel dispatches `rayon_accum_pool.par_iter_mut()` from `process_in_place` and waits for its fold tasks before the IFFT (`convolution_plugin.rs:885-924`). Rayon work stealing, worker wake-up, scheduling, and joins are not realtime bounded; contention with any other global Rayon work can overrun the audio deadline even if the loop itself allocates nothing. Dispatch repeats once per channel per 1024 input frames. The catalog nevertheless labels the plugin as having zero-allocation realtime evidence (`crates/sotf-plugins/src/factory/catalog.rs:556-573`).

Use a dedicated audio-DSP worker topology with bounded lock-free handoff and an explicit deadline/fallback, or keep the callback single-threaded and optimize/cache-vectorize the partition loop. Benchmark worst-case callback duration and jitter under saturated global Rayon load for realistic long IRs and 2/8/12 channels; average throughput is not sufficient evidence.

### P1 — Confirmed: reported latency disagrees with the no-IR/bypass signal path

With no state, processing returns immediately and leaves the buffer undelayed (`convolution_plugin.rs:752-756`). `latency_samples`, however, reports 1024 samples for UPC and normal NUPC even before any IR is loaded (`convolution_plugin.rs:707-720`; `src/lib/tests.rs:41-45`). Clearing or failing an IR restores immediate passthrough without changing that report. Conversely, asynchronous activation suddenly changes the actual signal delay without forcing a host render-plan rebuild. Parallel host paths can therefore be misaligned during empty, loading, failed, cleared, and UI-bypassed states. The UI documents a bypass toggle but the plugin has no local latency-preserving bypass contract (`UI.md:6-12`).

Choose one invariant: either always delay dry/bypassed audio by the configured backend latency, or report zero while inactive and synchronously notify/rebuild the host when activation changes latency. A bypass of a latency-bearing effect normally needs a latency-matched dry path. Test reported latency against an impulse for no IR, loading, active, mix=0, clear, failure, and bypass, including parallel-branch alignment.

### P1 — Confirmed test defect: the 724-line DSP regression suite is not compiled

The crate root loads the test module through `#[path = "lib/tests.rs"]` (`crates/sotf-plugins/crates/sotf-plugin-convolution/src/lib.rs:8-10`). Inside that path-attributed module, plain `mod misc;` resolves to the production `src/lib/misc.rs`, not `src/lib/tests/misc.rs` (`src/lib/tests.rs:13`). As a result, every test in `src/lib/tests/misc.rs`—unity/Dirac behavior, partial blocks, long IR, channel mapping, reset, tiny samples, and the claimed parallel-path check—is dormant. `cargo test -- --list` contains none of them, while the changelog and catalog cite them as evidence (`CHANGELOG.md:7-30`; `factory/catalog.rs:571-573`).

Use an explicit path for the nested test module or move tests to an integration module with unambiguous ownership. Add a CI assertion/snapshot of critical test names so a module-resolution regression cannot silently remove the DSP suite. Once activated, repair the helper to initialize every production buffer (notably `rayon_accum_pool`) and replace energy/finite assertions with offline convolution oracles.

### P2 — Confirmed: compile metadata declares a block-invariant transfer while parameters ramp within the block

`compile_metadata` always returns `PluginCompileMetadata::linear_transform` (`convolution_plugin.rs:558-566`), whose host contract sets `time_invariant_for_block=true` and permits gain movement (`sotf-host/src/plugin/types.rs:105-125`). NUPC advances mix/gain for every sample (`convolution_plugin.rs:770-785`), and UPC interpolates them across each completed partition (`convolution_plugin.rs:848-855,945-959`). The transfer is therefore explicitly time-varying during automation; state can also change at callback entry after an async load.

Return conservative boundary metadata while smoothing or transitioning, and advertise block invariance only after all coefficients/IR state are stable. Add compiled-plan versus unfused output comparisons during mix/gain ramps and IR transitions.

### P2 — Confirmed: default NUPC construction duplicates both backends and copies immutable IR data per output channel

`build_ir_state` always constructs all UPC spectra and a channel-expanded UPC FDL (`convolution_plugin.rs:251-283,307-315`), even when `use_nupc=true`. It then constructs a complete independent `NupcEngine` for every output channel (`convolution_plugin.rs:285-305`); mono IR spectra and FFT plans are therefore duplicated across every mapped channel. There is no IR duration, decoded-frame, channel-count, or memory-budget limit. A long mono IR on 7.1.4 can consume hundreds of megabytes, then double memory again during asynchronous replacement.

Represent the active backend as an enum and build only that backend. Share immutable IR spectra/plans across channels while keeping only FDL/history mutable per channel. Enforce configurable decoded-size/duration/channel/memory limits before planning and report estimated memory/CPU to the caller. Benchmark peak resident memory during A→B replacement.

### P2 — Confirmed: missing source sample rate is silently treated as already correct

The loader converts absent codec sample rate to zero (`convolution_plugin.rs:396-401`). `build_ir_state` resamples only when the rate is nonzero and different (`convolution_plugin.rs:237-248`), so an unknown-rate IR is accepted as if it matched the engine. Its spectral features and timing can be arbitrarily wrong.

Reject missing/zero sample rate with a user-visible error. Add malformed/metadata-free fixture tests and verify no previous active IR is destroyed by the failed request.

### P2 — Confirmed: construction bypasses the declared parameter bounds and uses a different default backend

The live schema limits mix to 0–1, gain to ±20 dB, and head taps to 32–512 (`src/params.rs:21-40`). `ConvolutionPluginParams` exposes those fields without equivalent validation (`src/lib/types.rs:19-30`), and `from_params` assigns them directly (`convolution_plugin.rs:212-224`). Factory JSON can therefore create negative/over-unity mixes, extreme gain, and an arbitrarily expensive time-domain head. Separately, `PARAMS` says `use_nupc=true`, while `ConvolutionPlugin::new` sets it false (`convolution_plugin.rs:149-151`), so direct and factory construction select different algorithms and performance/latency behavior.

Use one validated, versioned construction type derived from `PARAMS`; clamp or reject values before allocating and make `new` honor the declared defaults. Add factory tests at/beyond every bound and assert direct/factory default parity.

### P2 — Confirmed: async load failure leaves host-visible state stale and discards the previous working IR

The file setter reports success after only an existence check (`convolution_plugin.rs:610-629`). Decode/resample errors arrive later in `process_in_place`, where the plugin logs, clears active state and `ir_file`, but does not rebuild cached parameters or expose an error/status object (`convolution_plugin.rs:741-748,975-976`). `get_parameter("ir_file")` can continue reporting the failed path while audio becomes dry, and a bad replacement destroys a valid previous IR instead of preserving it.

Keep the last known-good state until a replacement is fully valid, publish generation-tagged load status/error through a control-safe API, and refresh canonical parameter state only upon successful commit (or explicitly represent desired versus active path). Test valid A→invalid B and assert A continues audibly with consistent UI/status.

### P2 — Confirmed: QA and fuzz coverage measure the identity path, not convolution

The QA binary constructs no IR and benchmarks passthrough (`bin/qa_convolution.rs:13-31`); its reported zero allocations and near-zero CPU do not exercise either convolution backend. The fuzzer allocates a random IR vector but never passes it to the plugin, then creates an empty-path instance (`crates/sotf-plugins/bin/plugin_fuzzer/convolution_fuzzer.rs:13-35`). The steady-state allocation test uses `new`, which selects UPC, so it does not cover default-factory NUPC (`realtime_allocation_tests/tests.rs:229-266`). The active NUPC unit tests test only a leading Dirac or finite/nonzero output (`plugins-spatial/src/nupc/tests.rs:40-97`).

Load deterministic short and long IR fixtures in QA/fuzzing, exercise both backends/head modes, vary host block size and channel count, and compare with direct convolution. Report callback p50/p99/max plus allocations/deallocations during state transitions, not only a passthrough throughput number.

### P3 — Recommendation: make UPC automation sample-consistent and latency-aware

UPC advances each smoother by 1024 samples only when an input partition completes, then linearly reconstructs the ramp with `t=i/PARTITION_SIZE` (`convolution_plugin.rs:848-855,949-958`). The last sample never reaches the smoother's stored end value, creating a small discontinuity at the next partition. A parameter change partway through an input partition is also applied retroactively across samples captured before the change, and the ramp is attached to input time rather than the delayed output time.

Store per-output-sample gain/mix envelopes alongside the delayed dry block, or advance smoothers once per incoming sample into a preallocated control ring. Test automation at every callback/partition boundary and compare output across block sizes 1, 63, 64, 127, 1024, and 1536.

### P3 — Recommendation: reconcile documentation and generated schema

`USAGE.md` says only F32 WAV is accepted and multi-channel IRs must match the input (`USAGE.md:108-117`), while the loader accepts integer WAV, AIFF, and FLAC (`convolution_plugin.rs:347-367,422-461`) and channel mapping cycles with modulo (`convolution_plugin.rs:288-299,878-883`). `UI.md` documents only the first three controls (`UI.md:45-73`), while the live layout exposes NUPC/head controls in an Advanced tab (`src/params.rs:47-70`). The local `AGENTS.md` describes an in-crate `nupc.rs` that no longer exists and claims lower latency although the plugin passes the same 1024-sample minimum block used by UPC (`convolution_plugin.rs:291-297`).

Generate user docs from `PARAMS/LAYOUT` where possible and document formats, cyclic channel mapping, backend-specific latency, head semantics, loading/error state, replacement behavior, bypass policy, and memory/IR limits. Clarify whether `gain_db` trims only wet signal—the implementation does (`convolution_plugin.rs:785,954-958`)—or the full mixed output, since the UI calls it “Output level trim” (`src/params.rs:28-30`).

## DSP and streaming contracts observed

- Input/output: equal-width interleaved channels; the plugin overwrites exactly `context.num_frames * channels` when an IR is active and otherwise leaves the input unchanged.
- UPC: 1024-sample partitions, 2048-point complex FFTs, inverse scale `1/2048`, FDL ring, overlap-add, and a 1024-sample output queue. IR channels map cyclically to output channels.
- NUPC: one independent engine per output channel; progressively larger FFT levels plus an optional direct-FIR head. The plugin currently configures `min_block=1024`, so normal NUPC does not reduce latency below UPC.
- Mix/gain: 20 ms smoothers; dry is latency-aligned only while an active backend runs. Gain is applied only to the wet term.
- State: file changes normally build off-thread, but install/reclamation and error handling occur in the callback. Synchronous `from_params/load_ir` build on the caller thread.
- Reset: explicitly clears UPC FDL/input/overlap/output queue, NUPC engine state, dry delay, and smoother history. It keeps the active IR and configuration.
- Latency: active UPC and normal NUPC report/use 1024 samples; NUPC with a nonempty direct head reports zero. Empty/failed/cleared state is immediate passthrough despite the configured report.
- Channels: mono or fewer-channel IRs repeat cyclically; there is no cross-channel matrix convolution.
- Bypass: no plugin-local bypass flag. No-IR behaves as an immediate bypass; mix=0 with an active IR is a latency-matched dry path.

## Scope reviewed

Read completely: repository and nested `AGENTS.md`; crate `README.md`, `CHANGELOG.md`, `UI.md`, `USAGE.md`, `Cargo.toml` and `qa` feature; every convolution crate source file; all inline, dormant nested, and integration tests; QA binary; realtime allocation coverage; plugin fuzzer; facade exports; factory create/catalog wiring; engine settings/defaults; FFI/bridge parameter wiring; host parametric adapter and compile-metadata contracts; and every directly used `plugins-spatial::nupc` module and test. The Rubato delay contract was checked against the vendored dependency implementation. No plugin source was skipped and no production code was changed.

Verification run:

- `cargo test -p sotf-plugin-convolution` — 23 passed across the active suites; the dormant `src/lib/tests/misc.rs` tests were separately confirmed absent from `cargo test -- --list`.
- `cargo test -p plugins-spatial` — 6 passed.
- `cargo run -p sotf-plugin-convolution --features qa --bin qa-convolution` — passed, but exercised only no-IR passthrough as noted above.

No full-workspace build was run.

## Test strengths and priority gaps

The active suite has useful parameter-type/structural-change checks, reported-latency versus leading-Dirac checks for the three configured modes, async-load steady-state allocation coverage, factory/API smoke tests, buffer-length validation, and basic reset handling. Setup work does preplan FFTs and preallocate steady-state buffers; per-sample NUPC processing itself allocates no memory.

The first acceptance gate should be an offline direct-convolution oracle over arbitrary sparse and random IRs, followed by corrected cross-rate IR tests. Then activate the dormant suite and cover async request ordering, state-swap allocation/deallocation, click-safe A/B replacement, latency-preserving bypass, long-IR/channel memory bounds, and worst-case realtime jitter under worker contention. Finite, nonzero, or approximate-energy assertions cannot establish convolution correctness.
