# Band Split plugin code review — 2026-08-12

## Remediation status — 2026-08-13

All P1–P2 findings are closed; this review reported no P0 or P3 finding.

Engine integration follow-up: Band Split retains channel topology in its settings
and graph contract but no longer emits `channels` into the plugin's strict runtime
parameter object. A public-factory regression covers the default config.

- Construction, initialization, factory state, and live automation share finite,
  range, Nyquist, strict-order, channel-count, and crossover-type validation.
  Invalid changes are rejected transactionally with focused neighbor-crossing,
  low-rate, non-finite, overflow, zero-channel, and strict-JSON tests.
- Checked exact input/output sizing prevents malformed-buffer indexing. Processing
  additionally requires initialization and a context sample rate matching the
  initialized rate; zero/huge/wrong-length cases return errors before DSP state.
- LR24/LR48 remains structural after initialization and no longer rebuilds or
  resets live DSP. Parameter metadata and UI declare frequency realtime and type
  structural; actual changes reject without mutation while exact no-op writes succeed.
- Frequency smoothing advances every sample, while expensive coefficient design
  occurs only when needed at a persistent eight-sample/6 kHz control interval. A
  12-channel, four-band LR48 test proves exact callback-partition invariance and
  bounds redesign count; QA records p50/p95/p99/max callback timing under repeated
  automation. An audio-rate reference comparison requires under 2% relative RMS
  error and under 0.01 RMS zipper residual. Successful frequency/gain setters and
  processing are allocation-free.
- Reset snaps gain/frequency smoothers and applied coefficients to their targets.
  Regression coverage compares reset during a ramp with a fresh equivalent.
- An independent signal-response harness now covers LR24/LR48 analytical slopes
  and complementarity, impulse reconstruction magnitude and phase, deterministic
  white-noise split/sum gain and correlation, two through four bands, 32–192 kHz,
  and 12-channel isolation. Documentation explicitly distinguishes two-band
  magnitude complementarity from cascaded multiband phase/group-delay behavior.
- Public factory coverage rejects zero channels, invalid/descending/duplicate
  topologies, unsupported crossover choices, and unknown persisted fields.
  Plugin version and conservative compile metadata are tied to runtime state by tests.

Exact closure verification is recorded in the task handoff for the remediation
commit.

## Findings

### P1 — Crossover frequencies are accepted without finite, range, Nyquist, or ordering validation

`BandSplitPlugin::new_multiband` only checks that the frequency list is non-empty and produces at most four bands, then casts every `f64` to `f32` and constructs the filters (`crates/sotf-plugins/crates/sotf-plugin-band-split/src/lib/band_split_plugin.rs:47-100`). The underlying multiband crossover documents that frequencies must be ascending but does not enforce it. `from_params` forwards explicit lists unchanged, while its legacy defaults cap at 20 kHz rather than at the initialized sample rate's Nyquist limit (`band_split_plugin.rs:128-155`). Thus duplicate, descending, negative, non-finite, or above-Nyquist frequencies can reach coefficient design; the legacy 20 kHz default is already invalid at 32 kHz.

Reject non-finite values, require strict ascending order, and validate every split against a sample-rate-derived safe interval (for example, `0 < f < 0.49 * sample_rate`). Since construction precedes `initialize`, either defer filter construction until sample rate is known or revalidate there. Add constructor/factory tests for NaN, infinity, negative, zero, duplicate, descending, and low-sample-rate configurations.

### P1 — Dynamic frequency parameters bypass the declared parameter bounds and can cross adjacent splits

The static bridge declares 20–20,000 Hz, but dynamic `frequency_N` handling accepts any finite float and directly targets its smoother (`band_split_plugin.rs:307-323`). It neither rejects/clamps the advertised bounds nor preserves strict ordering relative to neighboring splits. A successful return for an invalid topology leaves UI metadata claiming a valid value while the DSP moves toward an invalid one.

Centralize topology validation for static, dynamic, preset, and constructor paths. Prefer rejection with an actionable error; if automation must remain continuous, clamp each target to a sample-rate-aware interval separated from its neighbors by a documented minimum ratio. Test automation that attempts to cross every adjacent pair and verify block-partition-invariant, finite output.

### P1 — Changing LR24/LR48 rebuilds and resets the filter graph on the parameter-control path

Changing `crossover_type` calls `CrossoverMode::reinit` immediately (`band_split_plugin.rs:254-271`), and `reinit` replaces the whole crossover with newly allocated vectors and zero state (`src/lib/crossover_mode.rs:46-48`). This is both an allocation hazard if setters execute on the audio thread and an audible discontinuity for live changes. The schema correctly labels the choice structural (`src/params.rs:31-33`), but the plugin implementation still performs the mutation in place.

Require graph rebuild/replacement for structural changes, or prepare the replacement off-thread and crossfade old/new graphs with explicit latency/state policy. Add an allocation-guarded setter test and a non-silent steady-state LR24↔LR48 transition test.

### P1 — `process` trusts host buffer lengths and can panic

For every requested frame, `process` slices `input[in_off..in_off + in_ch]` and indexes the calculated output position without first checking `input.len()` or `output.len()` (`band_split_plugin.rs:370-414`). The inner crossover uses only debug assertions for shape. A malformed context or graph buffer therefore panics rather than returning `PluginResult`, and channel/frame multiplication is not checked for overflow.

Use checked multiplication, validate exact required lengths before touching DSP state, and return a descriptive error. Test short input/output, zero channels, huge frame counts, and mismatched context buffers without `catch_unwind` masking the contract.

### P2 — Frequency automation recomputes many biquad coefficients in the sample loop

Every frame advances every frequency smoother and calls `set_frequency` (`band_split_plugin.rs:381-389`). The LR4 implementation suppresses only changes below 0.001 Hz and otherwise updates four biquads per channel per split; LR48 suppresses changes below 0.1 Hz and updates eight. During a 20 ms ramp this puts trigonometric coefficient design in the hottest loop, with cost proportional to channels, splits, and filter order. The threshold mismatch also makes LR24 and LR48 automation trajectories materially different.

Use a control-rate coefficient update cadence or coefficient interpolation designed for stable IIR automation, skip settled smoothers before dispatch, and benchmark worst-case four-band LR48 automation at 12 channels. Verify response error, stability, zipper energy, and partition invariance against the current per-sample reference.

### P2 — Reset leaves frequency smoothers untouched

`reset` clears crossover state and resets band-gain smoothers, but does not reset the frequency smoothers (`band_split_plugin.rs:358-364`). If reset occurs during automation, the freshly cleared filters resume an old in-flight ramp rather than starting from a defined current/target frequency. This differs from gain behavior and makes transport/reset semantics dependent on when reset landed.

Define reset semantics explicitly. Usually, snap each frequency smoother to its target and ensure the crossover coefficients match that target before clearing state. Add a test that resets at multiple points in a ramp and compares the next block with a freshly initialized equivalent.

### P2 — Non-finite dynamic values are silently reported as success

Both band-gain and dynamic-frequency handlers return `Ok(())` after detecting the parameter ID even when `v.is_finite()` is false and no state changes (`band_split_plugin.rs:280-325`). Band gain additionally clamps out-of-range values despite the rest of the parameter API generally validating ranges. Silent success makes automation/preset failures hard to diagnose and diverges from the static bridge contract.

Return an error for non-finite and out-of-range input consistently across all paths, or document a single deliberate clamping policy. Assert the returned error as well as unchanged state in tests; the existing NaN test only protects state.

### P2 — The verification suite does not substantiate broadband reconstruction and phase claims

The README/usage material presents Linkwitz–Riley splitting as flat/reconstructing, while the dependency itself warns that cascaded multiband outputs have unequal group delays and are not phase-perfect. Plugin tests emphasize finiteness, dimensions, DC unity, and loose split/merge checks (`src/lib/tests.rs`, `tests/integration.rs`, `tests/property_tests.rs`). They do not measure impulse reconstruction, broadband magnitude ripple, phase/group delay, actual LR24/LR48 slopes, stop-band attenuation, multichannel isolation, live automation, or callback partition invariance.

Qualify the documentation for cascaded multiband phase behavior. Add swept-sine/FFT and impulse tests across 2–4 bands and supported sample rates, plus white-noise split→sum error/correlation tests after settling. Set numerical limits from an independently computed response rather than merely checking finiteness.

## Realtime allocation and performance assessment

The normal steady-state `process` path uses preallocated crossover scratch and `band_flat`; it performs no explicit heap allocation, locking, logging, or I/O, returns `context.num_frames`, reports zero latency, enables denormal handling, and flushes output denormals. The dedicated allocation coverage is a useful strength. Lifecycle methods intentionally allocate, but the type-change setter also allocates and therefore must be kept off the audio thread. The dominant steady-state risk is coefficient redesign during frequency ramps, not buffer allocation.

The sample loop is otherwise straightforward: roughly O(frames × splits × channels × filter order), followed by O(frames × bands × channels) gain/interleave work. Cached parameter IDs/names avoid repeated formatting, and per-band gain smoothing is correctly sample-based rather than block-based.

## Algorithm and host-contract assessment

The channel-changing boundary is declared in compile metadata, and output layout consistently uses band-major interleaving. LR24/LR48 selection, per-band gain smoothing, sample-rate reinitialization, cached parameter registration, and the three required parameter surfaces are all present. However, the frequency topology is a structural invariant and currently has no authoritative validation layer. That is the main correctness issue; reset/type-change semantics and high-rate coefficient updates are the main realtime issues.

## Scope reviewed

Read every plugin-owned file without omission: `README.md`, `USAGE.md`, `UI.md`, `CHANGELOG.md`, `AGENTS.md`, `Cargo.toml`, every source module under `src/`, all unit/integration/property tests, and `bin/qa_band_split.rs`. Also checked facade exports, factory creation, catalog/schema registration, realtime allocation tests/benchmark, Band Merge integration assumptions, host smoothing/compile metadata, and the complete LR4/LR8 crossover implementations in the `math-audio` dependency. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-band-split`: 56 tests passed across four suites.
- TokenSave context, file mapping, callers, and test-risk results were used before direct reads; the cross-project `math-audio` graph was stale, so its two known crossover source files were read directly.

## Suggested verification after fixes

- Run `cargo test -p sotf-plugin-band-split` and the realtime allocation suite.
- Run the QA binary at 32, 44.1, 48, 96, and 192 kHz for mono through 12 channels.
- Add criterion coverage for static and continuously automated LR24/LR48 at maximum bands/channels.
- Compare frequency/phase response and split→sum error against an independent double-precision reference.
