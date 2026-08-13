# Upmixer plugin code review

## Remediation status — 2026-08-12

Implemented in `sotf-plugin-upmixer` 0.5.118:

- Speaker-layout changes now rebuild and clear both main and HR channel-width-dependent storage,
  routing caches, and role-dependent decorrelators, including equal-width layout changes.
- Normal initialization now allocates diffuseness smoothing state for every ERB band.
- Runtime crossover targets are pair-validated before mutation; factory presets clamp malformed
  overlapping pairs to a documented one-Hz minimum transition.
- Diagnostic hard bypass is setup-only, resets streaming state on both edges, and reports zero
  latency while active; normal processing continues to report `fft_size` latency.
- Reset now clears dialogue, subharmonic, height-transient, and decorrelation oscillator histories.
- Frequency-table controls are marked setup-only, preventing hosts from advertising them as
  realtime automation. The previously dead crate-local suite is compiled and run.

Implemented in `sotf-plugin-upmixer` 0.5.120:

- **Fixed:** ML result publication now uses Release/Acquire ordering between
  the probability bits and readiness flag.
- **Fixed:** ML inference requires one f32 output tensor with shape `[1, 1]`;
  empty, multiple, non-f32, and differently shaped outputs are rejected.

Implemented in `sotf-plugin-upmixer` 0.5.121:

- **Fixed:** stereo direction now uses only observable lateral level imbalance and positive real
  correlation. Reactive intensity is no longer treated as a front/back axis; scenario tests cover
  mono, anti-phase, quadrature, independent, and level-panned material.
- **Fixed:** empty factory defaults match every catalog parameter through the real getter surface.
- **Fixed:** ML reset advances a generation and rejects queued/in-flight results from older
  transports. Non-finite input is rejected before it can poison persistent state.
- **Fixed:** critical frequency-analysis smoothers are hop-time/sample-rate based, with equivalent
  step-response tests. FTZ/DAZ is enabled on the actual callback thread and redundant full-buffer
  denormal scans were removed.
- **Verified:** correlated, anti-correlated, quadrature, and independent multi-source scenarios have
  finite, non-silent, bounded reconstruction energy. The realtime timing test now initializes the
  production DSP and exercises 9.1.6 at FFT 4096.

Implemented in `sotf-plugin-upmixer` 0.5.122:

- **Fixed and stress-tested:** ML results carry an explicit transport-generation stamp, and readers
  double-check it around the probability load. The focused
  `concurrent_publication_and_reset_never_expose_a_stale_generation` regression runs a publisher,
  resetter, and reader concurrently for 100,000 publications and 20,000 resets.

Verified in `sotf-plugin-upmixer` 0.5.123:

- **Reference policy:** latency-aligned steady-state total output energy stays within -4 to +2 dB
  of stereo input energy for correlated, anti-correlated, quadrature, and deterministic independent
  signals in 5.1 and worst-case 9.1.6 with multi-source extraction enabled. The same render is
  numerically equivalent across monolithic and irregular host block partitions.
- **Realtime safety:** the allocation test now exercises multi-source extraction at the worst-case
  advertised 9.1.6 / FFT 4096 configuration. The denormal regression now constructs true IEEE-754
  subnormal inputs by bit pattern and verifies callback-thread FTZ/DAZ produces only zero or normal
  output.
- **End-to-end multi-source profile:** Criterion now covers every advertised speaker layout at FFT
  sizes 512, 1024, 2048, and 4096 with four analysis/panning/synthesis hops per iteration. On an
  Apple M4 Pro (macOS 26.5.2, rustc 1.97.1), the local 9.1.6 / FFT 4096 baseline was 1.200–1.220 ms.
  A temporary loop-unswitch of the block-invariant `multi_source_extraction` condition measured
  1.218–1.241 ms; the intervals overlap and the exploratory candidate was not retained. The final
  unchanged kernel remeasured at 1.207–1.219 ms. Command:
  `cargo bench -p sotf-plugin-upmixer --bench upmixer-benchmark -- upmixer_multi_source_processing_matrix/9.1.6/4096`.

**Review date:** 2026-08-12  
**Crate:** `crates/sotf-plugins/crates/sotf-plugin-upmixer`  
**Status:** remediated through 0.5.123; every confirmed P0-P3 finding has focused regression
coverage, and the retained panning/energy recommendations now have reference and benchmark evidence.

## Confirmed defects

### P1 — Growing the speaker layout leaves the HR ring at the old channel width

`change_speaker_config()` reallocates the main time/output buffers and the main accumulator when the channel count changes, but not `hr_output_accumulator`; it also does not regenerate `decorrelation_filters` (`src/setup.rs:54-78`). The HR writer subsequently calculates `acc_base = write_idx * self.core.num_output_channels` and indexes the old allocation using the new width (`src/hr_processing.rs:200-227`). A supported change such as 5.1 (6 channels) to 7.1.4 (12 channels), followed by enough audio to move the ring cursor, can therefore index beyond an allocation sized for six channels. The fallback for missing per-channel decorrelators avoids one panic but silently reuses only the shared left/right filter (`src/frequency_domain/consts.rs:403-419`), losing the promised unique filter per surround/height speaker.

The same-channel-count fast path is also incomplete: it only swaps the layout and recalculates panning (`src/setup.rs:46-51`). For example, changing between layouts with the same width can leave queued audio in the old channel order and retain decorrelators classified for the old speaker roles.

**Impact:** host-visible structural parameter changes can panic the audio process or emit channels with stale routing/filter state.

**Fix:** centralize every channel-dependent allocation in one rebuild routine. Reallocate and clear both main and HR accumulators, both time-output arrays, the output block, blended and per-channel decorrelators, and all cursors/fill counts; regenerate role-dependent filters even when the channel count is unchanged. Prefer returning a rebuild request to the host rather than allocating in `set_parameter()`.

**Regression test:** initialize 5.1, process past latency, set `speaker_config` to every larger and smaller supported layout, process at least `4 * fft_size + latency` frames after each change, and assert no panic, correct output width/order, finite output, and cleared latency. Repeat for equal-width layouts. The existing test only checks metadata immediately after the change and never processes audio (`src/test/tests.rs:637-654`); that file is also not compiled, as noted below.

### P1 — Diffuseness smoothing is disabled after normal initialization

`initialize()` sizes PCA covariance, coherence, and history vectors, but never sizes `smoothed_diffuseness` or `diffuseness_initialized` (`src/lib/upmixer_plugin.rs:1863-1885`). The processing path detects the missing vectors and falls back to raw block-level diffuseness (`src/frequency_domain/consts.rs:175-195`). Those vectors are only created if the frequency-resolution parameter is later changed through `reset_analysis_state_for_current_resolution()` (`src/lib/upmixer_plugin.rs:1155-1180`).

**Impact:** the default startup path bypasses the smoother described in the changelog. Raw diffuseness directly modulates ambient gain and height suitability (`src/frequency_domain/consts.rs:250-256,376-387`), increasing block-rate image/level modulation and making initial behavior differ from behavior after an unrelated resolution toggle.

**Fix:** allocate and reset both vectors in `initialize()` alongside `smoothed_coherence`, and share that initialization with the resolution-change path.

**Regression test:** after `initialize()`, assert both vectors match the ERB band count; feed alternating coherent/diffuse blocks and verify the production path respects `DIFFUSENESS_MAX_STEP`. Also compare a fresh instance with one toggled ERB → Fine ERB → ERB.

### P1 — Public crossover controls can enter a state that the constructor forbids

`new()` asserts `bandpass_hz > lfe_cutoff_hz` (`src/lib/upmixer_plugin.rs:514-526`), but the independently valid host ranges overlap: LFE cutoff is 20–180 Hz and upmix crossover is 150–350 Hz (`src/params/consts.rs:78-90,177-188`). `set_param_value()` accepts either target without enforcing their relationship (`src/lib/upmixer_plugin.rs:1322-1336`). With, for example, LFE = 180 Hz and bandpass = 150 Hz, `transition_end` is below the LFE boundary; the later full-upmix loop revisits bins already classified as LFE and clears their LFE value (`src/frequency_domain/consts.rs:209-224,227-294,342-362`). The flat factory also calls the asserting constructor, so a syntactically valid preset with this pair can panic rather than return a configuration error (`src/lib/upmixer_plugin.rs:1401-1423`; `crates/sotf-plugins/src/factory/create.rs:137-144`).

**Impact:** accepted automation can corrupt the bass/upmix split; accepted JSON can abort plugin construction.

**Fix:** validate the pair atomically and return `PluginResult` from construction. Either constrain one target relative to the other or define a safe minimum transition gap. Never use `assert!` for user/preset values.

**Regression test:** cover both parameter-set orders and flat JSON with LFE 180 / bandpass 150; require an explicit error or a documented clamped pair, then verify every FFT bin belongs to exactly one intended crossover region.

### P1 — `bypass_all_processing` changes latency and freezes stale streaming state

The plugin reports `fft_size` latency (`src/lib/upmixer_plugin.rs:2469-2471`), but diagnostic bypass immediately copies the current input to FL/FR and returns without advancing or clearing either STFT/HR ring (`src/lib/upmixer_plugin.rs:2210-2228`). Re-enabling processing resumes the old partial input, accumulators, HR delay, and detector state. Thus bypass changes effective latency from `fft_size` to zero and a bypass round trip can emit audio captured before bypass next to new input.

**Impact:** uncompensated timing shifts, stale-audio bursts, and discontinuities when the diagnostic control is toggled. The current tests only check static always-bypassed fidelity (`src/test/tests.rs:1862-1934`; `tests/integration.rs:173-199`).

**Fix:** choose and document one contract: latency-preserving bypass should run/advance the delay and output only delayed FL/FR; a diagnostic hard bypass should be structural and reset/prime all streaming state on both edges. It must not silently retain queued audio.

**Regression test:** process an impulse and tone, toggle bypass on and off at non-hop-aligned host blocks, and assert constant reported/measured latency, no pre-bypass samples after the transition, and bounded first/second differences.

### P1 — The stereo “DOA” model treats reactive phase as a physical front/back axis

The code defines active intensity as complex `P * V*`, accumulates both its real and imaginary components, and obtains a 2-D angle with `atan2(im, re)` (`src/frequency_domain/diffuseness_and_doa.rs:21-55`). `bin_intensity_doa()` repeats the same mapping (`src/frequency_domain/consts.rs:55-66`). In acoustics, active intensity is the real part; the imaginary part is reactive intensity, not an orthogonal front/back spatial component. Ordinary two-channel stereo also does not contain a front/back velocity axis. Consequently a 90-degree L/R pair is classified as almost fully directional with a ±90-degree “DOA”; the test explicitly codifies that behavior (`src/frequency_domain/tests.rs:13-35`). This angle also steers secondary-source energy toward physical speakers (`src/panning.rs:192-215`).

**Impact:** phase-shifted ambience, all-pass decorrelation, and stereo effects can be treated as localized sources and steered to an invented direction, undermining direct/ambient separation and image stability.

**Fix:** do not call or use `Im(PV*)` as a front/back direction. For stereo, use an explicitly defined lateral cue (level difference plus signed real correlation/phase consistency) and treat reactive/large phase-offset energy as uncertain or diffuse. A real 2-D/3-D active-intensity vector requires appropriate coincident pressure/velocity channels, not L/R playback channels.

**Regression test:** include in-phase mono, anti-phase side, 90-degree phase-shifted equal-level stereo, independent noise, and level-panned sources. State the desired directness and lateral sign for each; quadrature must not invent a physical rear/front DOA.

### P1 — Parameter metadata, factory defaults, and documentation are different sources of truth

The catalog advertises `params::PARAMS` as the schema (`crates/sotf-plugins/src/factory/catalog.rs:575-595`), while both factories deserialize the separate flattened `UpmixerPluginParams` from `config.rs` (`crates/sotf-plugins/src/factory/create.rs:137-144`; `crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:165-168`). Despite `params.rs` claiming to be the single source of truth (`src/params.rs:1-11`), defaults differ materially:

| Key | Factory/config default | Advertised `PARAMS` default |
|---|---:|---:|
| `height_gain` | 0.5 (`src/config.rs:74-76`) | 1.0 (`src/params/consts.rs:61-72`) |
| `ambient_boost` | 1.0 (`src/config.rs:150-153`) | 1.2 (`src/params/consts.rs:207-219`) |
| `height_direct_leak` | 0.05 (`src/config.rs:133-135`) | 0.15 (`src/params/consts.rs:303-315`) |
| `surround_direct_bleed` | 0.15 (`src/config.rs:137-140`) | 0.5 (`src/params/consts.rs:317-329`) |
| `rear_ambient_boost` | 1.0 (`src/config.rs:142-144`) | 1.5 (`src/params/consts.rs:330-342`) |
| `safety_cap_db` | 0.0 (`src/config.rs:94-96`) | 3.0 (`src/params/consts.rs:435-448`) |

UI.md also specifies a safety range of -12 to +6 dB, while `PARAMS` exposes only 0 to +3 dB and `from_params()` clamps negative values to zero (`UI.md:84`; `src/lib/upmixer_plugin.rs:1438-1443`). USAGE.md describes yet another subset/default set.

**Impact:** an empty factory preset, a generated host preset, and the documented preset do not produce the same sound; UI reset-to-default can audibly increase height/surround bleed and relax clipping protection.

**Fix:** delete the duplicate parameter model or generate both serialization and runtime fields from one definition. Add a single compatibility migration before changing existing preset meaning.

**Regression test:** deserialize `{}` through the real factory type and compare every exposed parameter with catalog metadata and `get_parameter()`. Verify all documentation examples deserialize to their stated values.

### P2 — `reset()` does not reset several audible/history states

The reset clears the primary FFT/ring/PCA buffers, but leaves `subharmonic_phase`, `subharmonic_envelope`, `subharmonic_amp_envelope`, `decor_lfo_phase`, all dialogue detector history (`dialogue_spectral_centroid`, `dialogue_envelope_variance`, `dialogue_prev_rms`, `dialogue_probability`), and the ML worker's published result/queue unchanged (`src/lib/upmixer_plugin.rs:1999-2105`). It even seeds `dialogue_spatial_control` from the stale probability (`src/lib/upmixer_plugin.rs:2075`). The existing reset test merely verifies that output eventually has nonzero energy again (`tests/integration.rs:218-253`), not that a reset instance equals a fresh instance.

**Impact:** seeks, transport restarts, or graph resets inherit prior content classification and subharmonic/decorrelation phase, so output is not deterministic from the same post-reset input.

**Fix:** reset every stateful detector, envelope, oscillator, crossfade, and ML context/result. Preserve parameter targets, but set enable envelopes deterministically from those targets. If the ML worker cannot clear its queue/result safely, restart it on the control thread.

**Regression test:** drive all states with loud dialogue/bass, reset, then compare the complete output and diagnostics against a freshly initialized plugin for silence, impulse, and tone.

### P2 — Automating crossover/HF controls performs full spectral-table work on the audio thread

Every host callback advances four smoothers. While a target is moving, tiny changes trigger a full LR4 table rebuild (`src/lib/upmixer_plugin.rs:2159-2188`), whose per-bin `complex_response()` work covers the whole spectrum (`src/bass.rs:25-66`). Bandpass and height-cap movement similarly recompute every height bin with `powf(0.7)` (`src/setup.rs:187-210`). These controls are not marked structural/setup in metadata (`src/params/consts.rs:78-90,177-188,277-289`).

**Impact:** a normal automation gesture creates several callbacks of O(FFT-size) transcendental work on the realtime thread. This is most expensive at larger FFT sizes/sample rates and is absent from the steady-state benchmark.

**Fix:** build target tables on the control/setup side and crossfade preallocated current/next tables, or use coefficient-domain interpolation with a bounded update cadence. If that cannot be made realtime-safe, mark these controls setup-only.

**Regression test:** benchmark worst-case continuous automation at FFT 4096 / 9.1.6 and measure callback p99/max, not just steady state. Assert no allocations and an explicit CPU budget.

### P2 — Most crate-local tests are dead, and the timing test does not initialize the DSP

`src/test.rs` points at `src/test/tests.rs`, but `src/lib.rs:1-25` never declares `mod test`. The roughly 2,000-line suite containing configuration, energy, continuity, HR, panning, bypass, and parameter tests is therefore not compiled or run. The live no-default-features command reports only 62 tests. Separately, the workspace timing test constructs the upmixer but never calls `initialize()` (`crates/sotf-plugins/tests/realtime_tests.rs:148-168`); ERB/cache state is empty, so the measurement does not exercise the production analysis path.

**Impact:** regressions appear covered in source but provide no CI protection, and published timing evidence understates production cost.

**Fix:** wire the module with `#[cfg(test)] mod test;`, repair any newly exposed failures, and initialize/warm the realtime benchmark. Avoid tests that silently return when assets/models are missing; make asset availability an explicit CI feature.

**Regression test:** add a CI assertion/list check for the expected test count or a sentinel test from `src/test/tests.rs`; make the timing test assert initialized diagnostics (nonzero sample rate and nonempty coherence state).

### P3 — ML publication ordering and model output validation are incomplete

The worker stores the probability and `has_result` with relaxed ordering, while the reader loads both relaxed (`src/ml_inference.rs:113-119,233-242`). This does not establish a happens-before relationship on weakly ordered CPUs. Model validation checks only input type/shape (`src/ml_inference.rs:137-171`); inference accepts the first f32 value from any output shape (`src/ml_inference.rs:254-270`).

**Impact:** a reader can theoretically observe the result flag without the corresponding probability, and a wrong-output model can load successfully and silently use an arbitrary first value.

**Fix:** publish `has_result` with Release and read it with Acquire (the probability bits may remain Relaxed behind that barrier), and validate one f32 output with shape `[1,1]` or a precisely documented equivalent.

**Regression test:** validate wrong datum type, zero/multiple outputs, and wrong output shapes; add a concurrent publication stress test (preferably under Loom if available).

**Remediation:** fixed in 0.5.120. The worker stores probability bits before a
Release store of `has_result`; readers Acquire the flag before loading the bits.
`run_inference()` now requires exactly one f32 output tensor with shape `[1, 1]`.
Unit tests cover the publication ordering contract, reject malformed output
shapes, and stress concurrent publication/reset/read generation consistency in
`concurrent_publication_and_reset_never_expose_a_stale_generation`.

## Recommendations (not confirmed defects)

### P2 — Replace the scalar channel × bin panning kernel

`apply_vbap_panning_and_inverse_fft()` performs several scalar complex loops per output channel (`src/panning.rs:15-245`). At 9.1.6 this is the dominant O(channels × bins) routing stage. Hoist every channel classification/scalar once, split front/surround/height kernels to remove branches, then consider bin-major/SIMD processing or batched channel mixing. Benchmark the refactor against 5.1 and 9.1.6 and retain bit/metric tolerances.

### P2 — Validate the PCA/secondary-source energy budget end to end

The principal component, residual ambient, center subtraction, optional second eigenvector, user boosts, and normalized speaker gains are combined in separate stages (`src/frequency_domain/consts.rs:246-388`; `src/panning.rs:15-230`; `src/setup.rs:167-184`). There is no live test proving bounded reconstruction energy for correlated, anti-correlated, quadrature, and independent inputs with `multi_source_extraction` on. Establish a reference energy/reconstruction policy before changing the extra normalization; the May review's VBAP double-normalization concern remains unresolved.

### P3 — Remove redundant full-buffer denormal scans after verifying thread FTZ/DAZ

Initialization enables FTZ/DAZ (`src/lib/upmixer_plugin.rs:1967-1970`), yet every synthesized channel and the final output are scanned for denormals (`src/output.rs:35-50`; `src/lib/upmixer_plugin.rs:2465`). Confirm FTZ/DAZ is enabled on the actual processing thread on every supported platform, then remove or debug-gate the scans to save memory bandwidth. Do not remove them solely on assumption: initialization and processing can occur on different host threads.

### P3 — Make smoothing constants time based

Dialogue, coherence, DOA, diffuseness, height, transient, and safety followers mostly use per-FFT-block constants. Their effective time constants therefore change with FFT size, hop size, and sample rate. Derive alphas from seconds and the actual hop duration, then test equivalent step responses at 44.1/48/96/192 kHz and low-latency/default modes.

## DSP and streaming contract audit

- **Input/output:** stereo interleaved input; configured interleaved speaker order; binaural preview reports two output channels. Buffer length validation occurs before bypass and returns errors rather than panicking.
- **STFT:** main path uses periodic sqrt-Hann analysis/synthesis, 50% overlap, inverse scale `sqrt(2)/N`, and reports `fft_size` samples of latency. The streamed impulse regression covers the normal path.
- **Overwrite/accumulate:** per-channel IFFT buffers are overwritten; main and HR ring slots are accumulated then cleared on drain. The layout-change defect violates the HR ring-width invariant.
- **Latency:** normal-path measured/reported latency is coherent; `bypass_all_processing` violates it. Low-latency mode reallocates/plans FFTs and resets/prime state.
- **Channels:** LFE is identified from speaker metadata; front/height/center flags are cached. Layout changes are structural but the direct setter currently performs an incomplete in-place rebuild.
- **Reset:** main overlap state is cleared, but detector/oscillator/ML state is incomplete as described above.
- **Non-finite input:** no explicit policy or test was found. NaNs can poison covariance, detector histories, spectra, and output; define whether the host sanitizes them or add finite guards.
- **Parameters/presets:** cached parameter get/set coverage is broad, including the three-way registration bridge, but two independent state models/default sets make the effective schema inconsistent. `ml_model_path` is manually get/settable but absent from advertised `PARAMS`.
- **Realtime allocation:** steady-state process uses preallocated buffers and `mem::take` reuse. Structural setters, model changes, and initialization allocate by design. The steady-state allocation benchmark exists, but automated spectral-table updates and ML lifecycle transitions need separate RT tests.
- **ML:** MFCC/context extraction is preallocated on the audio thread; ONNX inference and its tensor allocations are on a dedicated worker. Loading, optimization, thread spawn, and old-worker join happen synchronously in the parameter side-effect path, so hosts must keep that path off the audio thread or the parameter must be setup-only.

## Scope reviewed

Read in full: nested `AGENTS.md`; `Cargo.toml`; README, USAGE, UI, changelog, and ML training/contract documentation; every file under `src/` (including frequency-domain, ML, parameter, and test submodules); every integration/regression/quality test; the benchmark; and all `qa-upmixer` modules. Also reviewed the upmixer factory/catalog and bridge wiring, the `Plugin` buffer/latency/reset contract, workspace realtime/allocation tests, STFT normalization/distortion callers surfaced by TokenSave, and the archived May 2026 review to distinguish fixed items from current defects.

TokenSave was used first for file inventory, symbols, callers, test mapping/risk, panic sites, and factory impact. It saved approximately 40k tokens across the recorded queries.

## Verification

- `cargo test -p sotf-plugin-upmixer --no-default-features` — **passed: 63 tests across 6 suites**.
- `cargo test -p sotf-plugin-upmixer --features onnx --lib` — passed: 44 tests in 1 suite.
- Added `upmixer_bypass_round_trip_discards_queued_audio_and_restores_latency`, which covers
  a non-hop-aligned bypass interval after queued STFT output and verifies zero residual output
  after re-enable with silence, plus the 0/`fft_size` latency contract.

No production code was changed by this review.

Follow-up verification in 0.5.119 covers the previously fixed diagnostic-bypass contract with a
non-hop-aligned round trip and confirms that queued streaming state does not leak after re-enable.

## Strengths

- The main and HR FFT plans/buffers are created outside steady-state processing, and the WOLA normalization/latency path has focused live regression coverage.
- Main/HR accumulator slots are cleared after drain; DC/Nyquist imaginary parts are forced to zero before real IFFT.
- The LR4 tables retain complex response and have a unity-sum test.
- Height masks are clamped below the upmix band before and after smoothing, closing the previously reported low-frequency leakage issue.
- Decorrelation transitions use a longer cosine/phase crossfade, and adaptive filter blends are normalized to all-pass magnitude.
- Parameter side effects are centralized by index and most smoothed controls have consistent set/get/cached wiring.
- The QA tool provides useful block diagnostics, artifact deltas, isolation variants, and optional WAV/CSV output; it is substantially more useful than peak-only tests for tracking modulation artifacts.
