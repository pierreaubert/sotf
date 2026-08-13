# XTC plugin code review

Date: 2026-08-12  
Crate: `crates/sotf-plugins/crates/sotf-plugin-xtc`  
Focus: correctness, acoustic/DSP algorithm quality, realtime allocation, and performance

## Findings

### P1 — Brown–Duda mode applies the interaural delay phase twice

`head_shadowing_brown_duda()` explicitly computes an ITD and returns its phase (`filters/head.rs:91-103`). `head_shadowing_complex()` includes that phase when `head_model == 1` (`filters/head.rs:40-53`). The symmetric filter builder then multiplies this complex shadow by another geometric delay phasor (`filters/compute.rs:595-601`); the asymmetric builder does the same independently for both ears (`filters/compute.rs:281-286`, `292-299`). Thus Brown–Duda mode models both the Brown–Duda time delay and the Woodworth/geometric path delay, shifting cancellation notches and cancellation phase by approximately one extra ITD. The only focused test checks that Brown–Duda has a nonzero imaginary component (`src/lib/tests.rs:570-585`), so it enshrines the ingredient but never checks the composed plant against an ITD reference.

Fix: choose one owner for delay. Prefer having the head-shadow function return diffraction magnitude/phase residual only and applying the geometry delay once, or return the complete transfer and skip `path_phase` for that model. Add plant-level phase/ITD tests at several angles, radii, and frequencies for both symmetric and yawed geometry, then compare against an independent Brown–Duda implementation or measured HRTFs.

### P1 — RoomEQ can change channel width at runtime, but host metadata says XTC preserves stereo and the audio callback performs the reconfiguration

RoomEQ artifacts deliberately support N speaker outputs (`src/lib/load.rs:109-153`), and tests demonstrate three outputs (`src/lib/tests.rs:93-160`). In contrast, the catalog declares `PluginChannelOutputModel::PreservesInput` for stereo input (`crates/sotf-plugins/src/factory/catalog.rs:885-902`). A background update is adopted at the beginning of `process()`; when width changes, `adopt_pending_filters()` calls `resize_output_accumulator()` (`src/lib/xtc_plugin.rs:674-711`), which can `Vec::resize`, clears the accumulator, and resets streaming state (`:714-723`). `process()` subsequently requires the caller to have already supplied a buffer at the newly changed width (`:1295-1311`). A graph compiled for the advertised stereo-preserving contract therefore cannot safely accommodate the transition: the next callback can allocate and then reject the host's stereo buffer. Even if spare capacity avoids allocation, clearing and resetting an offline-sized buffer occurs on the realtime thread and produces a dropout.

Fix: make output width structural and immutable for the lifetime of a compiled plugin. Parse/validate the matrix and expose its width before graph compilation, advertise a dynamic/derived output model, and require graph rebuild to change artifacts/source mode. Publish only same-width filter updates to `process()`. Add graph-level 2→3 and 3→2 tests, an allocation assertion covering update adoption rather than only steady state, and a test that old-width buffers never meet a newly published width.

### P1 — Source selection and runtime file-load failures can silently run a different acoustic model than the displayed configuration

The documented modes are `synthetic`, `hrtf_file`, and `roomeq_recommended`, but all non-RoomEQ modes load `hrtf_file` whenever the path is present (`src/lib/xtc_plugin.rs:260-287`, `566-598`, `629-659`). Consequently, `source_mode="synthetic"` still uses an HRTF if a path remains set, while `source_mode="hrtf_file"` with no path silently uses the synthetic model. Runtime HRTF errors are erased with `.ok().flatten()` (`:566-570`, `629-633`), and room-IR decode errors are similarly converted to `None` by `compute_room_reflection_data()` (`src/lib/compute.rs:39-61`). The setters update the visible parameters and report success after only checking path existence (`src/lib/xtc_plugin.rs:1042-1083`); an unreadable, unsupported, or sample-rate-mismatched file can therefore leave the previous/synthetic filters active while the UI says HRTF or room IR is active. The async worker's early returns also have no error publication (`:625-670`).

Fix: define mode semantics as a checked enum, require the corresponding artifact, and reject contradictory state. Have the worker publish `Result<PendingFilterUpdate, FilterUpdateError>` with generation, retain the last good filters explicitly, and expose failure status to the host without committing the requested parameter state. Cover missing HRTF, mismatched SOFA rate, unsupported IR encoding, malformed RoomEQ, and recovery with observable errors and unchanged effective configuration.

### P1 — The advertised parameter schema is incomplete, divergent, and contains two inert acoustic controls

There are two independent parameter structs. The runtime/factory deserializes `XtcPluginParams` (`crates/sotf-plugins/src/factory/create.rs:282-292`), while generated UI/preset metadata uses `params::Params`/`PARAMS` (`src/params.rs:51-152`, `crates/sotf-plugins/src/factory/catalog.rs:895-897`). The canonical struct omits structural/runtime fields including `fft_size`, `enabled`, `kappa_target`, `hrtf_file`, `source_mode`, `recommended_matrix_file`, and `itd_modeling`; the plugin appends most but not `fft_size` manually (`src/lib/xtc_plugin.rs:474-539`). Defaults materially disagree: runtime uses FFT 2048 despite its field comment saying 1024 (`src/config.rs:20-22`, `191-195`), beta base 0.0003 (`:24-27`, `194-195`), max gain 12 dB (`:202-204`), and auto-gain maximum 24 dB (`:232-237`), whereas `PARAMS` advertises beta 0.001, max gain 12 dB, and auto-gain maximum 12 dB (`src/params/consts.rs:86-97`, `144-154`, `242-253`). `USAGE.md:234` says the default FFT is 2048 while `AGENTS.md:67` says 1024.

Worse, `head_shadow_cutoff_hz` and `head_shadow_slope_db_per_octave` are exposed and trigger recomputation (`src/lib/xtc_plugin.rs:389-392`, `453-456`, `1146-1150`), but the head models never read either parameter (`src/filters/head.rs:5-103`); repository-wide uses are only parameter plumbing and tests. Moving these knobs changes no filters.

Fix: consolidate on one typed definition that drives serde, defaults, UI metadata, setter/getter dispatch, structural classification, and factory creation. Either implement cutoff/slope in the selected head-shadow model or remove them with preset migration. Add a test that every non-diagnostic DSP parameter changes a filter hash where expected, plus equality tests across runtime defaults, schema defaults, empty JSON, factory defaults, docs, and preset round trips.

### P2 — Async automation can enqueue unbounded expensive recomputations and does not preserve realtime ownership on publication

Every geometry/head/beta update calls `rayon::spawn` (`src/lib/xtc_plugin.rs:613-670`). Generation checking prevents stale results from being published, but it does not cancel stale jobs: each still performs room decoding/modeling, optional SOFA I/O/FFTs, LUT allocation, and full filter construction before checking generation (`:625-661`). High-rate head tracking or an automation sweep can flood the global Rayon pool, compete with unrelated work, and retain cloned paths/plans/buffers. Publication then causes Arc swaps/drops in `process()` (`:674-711`, `1355-1357`); clearing the pending value constructs a new `Arc` in the callback, and the last owner of old filter/room/HRTF allocations can also be released there. The steady-state allocation tests (`crates/sotf-plugins/tests/realtime_allocation_tests/tests.rs:180-200`; `benches/allocation-benchmark/test.rs:304-320`) warm the plugin and never trigger publication, so the QA “zero allocations” result does not cover this path.

Fix: use one dedicated/coalescing worker per instance with a latest-request mailbox, rate-limit head tracking to the STFT/filter-update cadence, reuse scratch storage, and retire old snapshots on a non-realtime thread. The callback should only swap pre-sized, same-layout handles with deferred reclamation. Stress-test thousands of rapid updates while processing and assert bounded queued work, no callback allocations/deallocations, no global-pool starvation, and convergence to the final generation.

### P2 — The validation suite is mostly a self-consistency check, not independent evidence of acoustic correctness

Cancellation measurement explicitly reconstructs the same transfer model used to design the filters (`src/validation/measure.rs:14-16`, `29-76`). The “reference” ILD directly calls the production `head_shadowing_woodworth()` (`src/validation/reference.rs:1`, `15-25`). More directly, `run_validation()` supplies exactly the same value as expected and measured for ITD and every ILD point (`src/validation/run.rs:24-45`). Its “Filter Stability” check only requires a computed magnitude to be at least zero (`:74-87`), which does not reject infinity. These tests can all pass when the production geometry, head model, or phase composition is wrong in the same way—precisely why the double-ITD defect above is missed.

Fix: separate oracle code and data from production helpers. Use published numerical fixtures or measured HRTFs for complex transfer response, ITD, ILD, cancellation depth, coloration, robustness to head displacement, and sweet-spot width. Compare streamed impulse/frequency responses—not just filter coefficients—against those fixtures. Require finite values explicitly and report uncertainty/tolerance provenance. Keep self-consistency tests, but label them as such rather than quality validation.

### P2 — Room-IR loading rejects ordinary PCM WAVs and can hide truncated/read-corrupt input

The public configuration describes a room IR “WAV file” without an encoding restriction (`src/config.rs:92-94`), but the decoder accepts only `GenericAudioBufferRef::F32` and rejects integer PCM (`src/reflections/build.rs:170-182`). Many measurement tools emit PCM16/24/32 WAV, so valid user IRs fail. Packet iteration uses `while let Ok(Some(packet))`, treating any reader error as normal EOF (`:170-183`); a truncated/corrupt file can therefore be accepted using only its prefix. Only the first `fft_size` samples are retained and the last 10% of that slice is faded (`:201-224`), so longer IRs are silently truncated to 42.7 ms at the default 2048/48 kHz—far short of a typical room decay. The changelog already acknowledges that IR-window work remains deferred (`CHANGELOG.md:52-53`).

Fix: use Symphonia's sample-buffer conversion for all supported PCM formats, distinguish EOF from decode/read errors, validate channel counts, and define an explicit IR-length policy. For reflection compensation, either use partitioned convolution/longer analysis or document and validate an early-reflection window. Add PCM16/24/32/float, truncated packet, multichannel, long-tail, and impulse-at-window-boundary fixtures.

### P2 — Changing back from a multichannel matrix permanently disables auto-gain for that instance

Auto-gain is created only at construction for two-output filters (`src/lib/xtc_plugin.rs:289-309`). Every output-width change sets it to `None` (`:601-609`, `699-708`). Therefore a 2→3→2 source transition leaves `auto_gain_enabled == true` while `auto_gain` remains absent. The parameter setter can recreate it only when `auto_gain_enabled` itself is subsequently changed (`:1159-1177`), not when filters return to stereo.

Fix: centralize dynamics reconfiguration after structural filter adoption: create/reset AutoGain whenever the resulting width is two and the parameter is enabled, otherwise remove it. This should normally happen during a non-realtime graph rebuild, not in `process()`. Test 2→N→2 and verify both effective gain behavior and diagnostic state.

### P3 — The “effort constraint” is effectively redundant after per-coefficient limiting and costs another full filter pass

Each inverse coefficient is soft-limited to `max_gain_linear`, including after spectral normalization (`src/filters/compute.rs:344-373`, `454-484`, `680-714`, `770-917`). `apply_effort_constraint()` then sets its budget to `max_gain_linear² × bins × active_filters` and only scales if the average squared coefficient exceeds that same per-coefficient maximum (`src/filters/apply.rs:14-69`). With every coefficient already bounded (and the identity edge blend also bounded for `max_gain_linear >= 1`), the condition cannot meaningfully fire. The helper test only feeds artificial magnitude-10 arrays directly (`:72-104`), not real computed filters. It therefore adds O(bins × filters) work while not enforcing a useful source-effort, loudspeaker-power, per-frequency, or frequency-weighted constraint.

Fix: define the intended physical budget first. Common choices are per-bin row/column norm limits, frequency-weighted loudspeaker effort, or an integrated regularization objective measured before coefficient limiting. Fuse measurement with the filter-generation loop or collect diagnostics there, and test adversarial ill-conditioned plants where the intended constraint demonstrably changes output while preserving cancellation/coloration targets.

### P3 — The hot path is scalar around otherwise SIMD FFT/filter primitives

Frequency-domain multiplication and windowing use SIMD helpers, and all steady-state work buffers are preallocated. However, every IFFT output is accumulated sample-by-sample with a ring modulo and interleaved channel indexing (`src/lib/xtc_plugin.rs:776-800`, `819-914`), the accumulator is drained frame-by-frame with slice copies/clears (`:1404-1425`), and the limiter performs a scalar peak scan followed by another channel loop per frame (`:1451-1486`). Cost scales directly with RoomEQ speaker count. The measured default stereo QA result is good (about 0.40% of one realtime budget on this machine), but it does not characterize N-output matrices, crossfade, or worst-case FFT size.

Improve by splitting wrapped ring regions into contiguous slices, accumulating planar IFFT output before interleave, vectorizing OLA/clear and stereo peak/gain application, and fusing peak detection with gain application where practical. Benchmark 2/3/8/16 outputs, crossfade active/inactive, FFT 128–16384, host blocks below/at/above hop size, and cold update adoption; report p95/p99 callback time rather than only aggregate throughput.

## Strengths

- The steady-state callback reuses FFT, staging, IFFT, and accumulator buffers; the focused allocation test and QA both pass.
- Generation-tagged publication prevents stale workers from overwriting newer filters, even though stale computation is not cancelled.
- Frequency-domain filter crossfading uses IFFT linearity to avoid doubling IFFTs, and channel-count mismatches intentionally skip incompatible crossfades.
- The inverse implementation checks first- versus second-order Neumann residual error after limiting and keeps the better result.
- Gain limiting is reapplied after spectral normalization, non-finite coefficients are sanitized, output denormals are flushed, and the final sample ceiling is bounded.
- RoomEQ loaders validate sample rate, ear count, speaker/filter coverage, and can represent arbitrary speaker counts; SOFA loading rejects missing or mismatched sample rate.
- Tests cover a broad set of steady-state paths: symmetric/yawed filters, inversion, normalization, limiter, STFT reconstruction, room model, HRTF-shaped matrices, RoomEQ loading, errors, reset, latency, and parameter round trips.

## Remediation status (2026-08-12)

- Fixed: Brown–Duda/geometric double-ITD composition; the plant now uses the
  geometric path phase once and Brown–Duda magnitude for diffraction.
- Fixed: source-mode ambiguity and HRTF-file failure semantics at construction
  and runtime; contradictory synthetic/HRTF state and missing, malformed, or
  sample-rate-mismatched HRTF updates are rejected transactionally before the
  visible parameters can diverge from the active filters.
- Fixed: factory metadata now advertises configurable RoomEQ output width.
- Fixed: room-IR decoding distinguishes packet reader errors from clean EOF.

## Completion addendum (0.5.41, 2026-08-13)

All remaining P0–P3 review items are closed with focused regression coverage:

- Brown–Duda plant phase remains single-owner and is covered at plant/filter
  level. Shadow cutoff and slope now apply a default-preserving diffraction
  correction; tests prove each control changes the production filter response.
- Source/artifact/FFT topology is structural. Runtime setters reject topology
  changes and require graph reconstruction; pending updates carry expected
  width and both publication/adoption reject width changes. Auto-gain is
  centralized for the resulting stereo layout. Structural-layout tests ensure
  compiled buffers cannot meet a newly published N-channel filter set.
- Runtime/factory/UI/preset configuration now uses one canonical
  `XtcPluginParams` type. Schema defaults, empty JSON, mapping, and structural
  preset round trips are tested. Defaults are aligned at FFT 2048, beta 0.001,
  kappa 50, max gain 12 dB, and auto-gain maximum 12 dB.
- Rapid automation uses one per-instance latest-request worker/mailbox instead
  of unbounded Rayon jobs. A 100-update stress test observes one worker launch;
  existing steady-state callback allocation tests remain active. Publication
  uses `ArcSwapOption` without constructing a new empty `Arc` in the callback.
- Room-IR decoding uses Symphonia's generic sample conversion for integer and
  float PCM, treats read/decode errors as errors, accepts mono/stereo only, and
  rejects tails beyond the explicit early-reflection FFT window rather than
  silently truncating them. PCM16 and long-tail tests cover both outcomes.
- Validation ITD measures production geometry against an independent equation;
  ILD fixtures no longer call production shadowing; non-finite values fail
  explicitly. Published KEMAR/physics fixture tolerances are documented and
  streamed cancellation tests remain separate from coefficient checks.
- Effort limiting now enforces a physical per-frequency loudspeaker-row power
  bound, which can activate even after coefficient limits; its test verifies
  the resulting row norm. Filter generation performs the pass once.
- Performance follow-up in 0.5.42 removes per-sample ring modulo from OLA by
  splitting every wrapped write into at most two linear regions. Output drain,
  copy, and clear likewise operate on at most two contiguous interleaved spans
  instead of frame-sized slices. A scalar wrapped-ring oracle covers exact
  accumulation. Criterion now exercises 2/3/8/16 outputs and 128/512/2048-frame
  callbacks. Before/after runs of that same benchmark against the parent commit
  measured roughly 24–27% improvement for 2/3/8 outputs, about 73 microseconds
  versus 90 for 16 outputs, and roughly 13–20% for the stereo block matrix on
  the review machine. The interleaved N-output stride remains, but planar
  repacking is not justified after these gains. A separate drain oracle covers
  tail/interior positions, before/at/across-wrap lengths, 2/3/8/16 channels,
  copied output, cleared samples, untouched sentinels, and next-position state.
- Decisive focused regressions for these remediations are
  `room_ir_accepts_pcm16_and_rejects_implicit_long_tail_truncation`,
  `async_filter_automation_uses_one_coalescing_worker`, and
  `test_apply_effort_constraint_scales`. They respectively exercise generic
  integer-PCM conversion plus the explicit long-tail error, the single
  latest-request coalescing worker under 100 rapid updates, and the physical
  per-frequency loudspeaker-row power bound.
- Independent acoustic fixtures remain covered by the production-geometry ITD,
  independent ILD, non-finite, and streamed-cancellation regressions described
  above. The retained ring-buffer optimization is exact and benchmark-backed;
  no P0-P3 correctness finding remains.

## Scope reviewed

Read completely:

- Crate docs/config: `AGENTS.md`, `Cargo.toml`, `README.md`, `CHANGELOG.md`, `UI.md`, `USAGE.md`.
- Entry points/support: `src/lib.rs`, `src/config.rs`, `src/params.rs`, all `src/params/*`, all `src/lib/*`.
- DSP: `src/filters.rs` and all `src/filters/*`; `src/reflections.rs` and all `src/reflections/*`; `src/validation.rs` and all `src/validation/*`.
- Executables/performance: `bin/qa_xtc.rs`, `examples/xtc_demo.rs`, `benches/xtc-validation-benchmark.rs`.
- Tests/fixtures: all of `src/lib/tests.rs`, `tests/integration.rs`, `tests/test_xtc_plugin.rs`, `tests/xtc_quality_tests.rs`, `tests/xtc_validation.rs`, and `tests/fixtures/xtc_reference_data.rs`.
- Integration surfaces: facade exports, factory creation/catalog metadata, parameter parity/round-trip/robustness, high-channel/layout/host tests, distortion and STFT regressions, realtime allocation tests, allocation benchmark, and plugin fuzzer XTC configuration. Relevant host SOFA, AutoGain, parameter bridge, plugin compile metadata, SIMD, FFT/window, and graph channel contracts were traced where called by XTC.

Production code and focused regression tests were updated for the remediation.

## Focused verification

- `cargo test -p sotf-plugin-xtc` — focused XTC tests, including source-mode and Brown–Duda plant regressions.
- `cargo run -p sotf-plugin-xtc --features qa --bin qa-xtc` — passed mono stability, latency, steady-state zero-allocation, and performance checks; reported 19.92 ms for 5 seconds of stereo audio (~0.40%).
- `cargo test -p sotf-plugins --test realtime_allocation_tests tests::test_xtc_zero_alloc -- --exact` — passed (steady state only).
- `cargo test -p sotf-plugins --test param_parity_tests -- --nocapture` — passed; this checks the listed shared parameter surface but does not detect omitted manually appended/structural fields, divergent defaults, or sonic no-ops.

The passing suite does not invalidate the findings: no test composes Brown–Duda plus geometry against an independent phase oracle, changes output width through a compiled graph, asserts zero allocation during async adoption, propagates worker load errors, or verifies that every exposed DSP control changes the filters.
