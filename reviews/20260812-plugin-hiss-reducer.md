# Hiss Reducer plugin review — 2026-08-12

## Remediation status

Completed in `sotf-plugin-hiss-reducer 0.5.7` / `plugins-denoiser 0.5.11`:

- **Fixed P1 detector semantics:** the DSP now tracks fast/slow high-band power,
  requires 30 ms persistence, and uses hysteresis plus hold. Public docs and UI
  state explicitly that Threshold is an absolute dBFS level and that this
  zero-latency design is a high-band downward expander, not an SNR or spectral
  noise estimator. Tests separate persistent energy from sparse impulses and
  compare time-aligned behavior across sample rates.
- **Fixed P1 modulation/clicks:** reduction depth is continuous, gain uses
  sample-rate-derived attack/release, and live cutoff changes ramp the exact
  one-pole coefficient. Tests cover coefficient automation, bounded bypass
  transitions, callback partition invariance, and exact zero-strength nulling.
- **Fixed P1 construction/state:** fallible constructors and the factory reject
  zero channels and unusable rates; persisted values canonicalize through one
  path, unknown fields reject, and the visible cutoff is clamped to the active
  `0.45 × sample rate` limit. Runtime updates above that limit reject.
- **Fixed P1 sample-rate contract:** processing requires initialization and an
  exactly matching nonzero context rate. Focused tests cover uninitialized,
  zero/too-low, mismatched, and low-rate canonicalization cases.
- **Fixed P2 timing/bypass/split/metadata:** detector, persistence, hold, gain,
  cutoff, and bypass timing derive from sample rate; bypass stays warm and
  crossfades; the split uses exact exponential mapping and documents its
  intentional 6 dB/octave slope; metadata is stateful nonlinear IIR, zero
  latency, and non-channel-mixing. Response/timing/metadata tests enforce each
  contract.
- **Fixed P3 control/denormals:** cached schema validation plus direct setters
  eliminate realtime setter allocations, while process remains allocation-free.
  Filter/envelope state snaps before becoming subnormal; non-finite audio is
  sanitized. Allocation, long-tail, recovery, QA, and reset tests cover these
  paths.

All reported P1–P3 findings are remediated with regression coverage. The
separate STFT/minimum-statistics design remains an optional higher-latency
product, not a deferred correctness requirement for this explicitly documented
zero-latency expander.

Implemented in commit `Fix Hiss Reducer review findings`: validated initialization and context sample rate, canonicalized persisted/runtime parameters, corrected IIR scheduling metadata, reused cached parameter metadata, and replaced the instantaneous binary detector with sample-rate-derived fast/slow envelopes plus smoothed gain and denormal guards. Documentation now describes the threshold as a high-band level threshold rather than SNR. The broader STFT/minimum-statistics redesign remains deferred because it would change the plugin's zero-latency contract and requires a separate algorithm/latency product decision.

## Findings

### P1 — The documented SNR/stationarity detector measures neither SNR nor stationarity

The UI calls `threshold_db` an “SNR threshold” (`params.rs:9-19`), but the DSP converts it directly to an absolute full-scale amplitude and compares it only with the high-band envelope (`hiss.rs:74-78,96`). There is no desired-signal/reference power estimate, noise-floor estimator, spectral flatness, or time-variance test. The so-called stationarity ratio is `slow_envelope / instantaneous_absolute_sample > 0.25`; it is normally true except near transient peaks and becomes arbitrarily large at every zero crossing. Consequently the plugin is an absolute-level high-band downward gate, and its decision varies over each waveform cycle rather than identifying stationary hiss.

Rename and document the current control/algorithm honestly, or implement a real noise estimate: use sample-rate-aware fast/slow power envelopes (or a modest STFT), derive a noise likelihood from crest factor/spectral flatness and persistence, and compare signal/noise power in dB. Add calibrated tests separating stationary white/pink hiss, quiet sustained cymbals/strings, speech sibilants, impulses, and identical signals at different absolute levels.

### P1 — Binary per-sample attenuation creates modulation distortion and clicks

The residual gain jumps directly between `1.0` and `1.0 - strength` whenever either boolean decision crosses its boundary (`hiss.rs:76-85`). There is no hysteresis, hold time, attack/release smoothing, or wet/dry crossfade. Since `stationary_ratio` depends on instantaneous magnitude, even a steady sinusoid can toggle within a cycle; threshold crossings and parameter changes can introduce additional discontinuities. Preserving the IIR state in `apply_values` does not make those abrupt coefficient/gain changes click-free, despite the comment claiming it does (`lib.rs:154-164`).

Generate a continuous target reduction in dB, add hysteresis/hold to the classifier, and smooth gain with sample-rate-derived attack/release constants. Smooth cutoff changes with a stable coefficient ramp or parallel-filter crossfade. Test maximum sample-to-sample discontinuity, sidebands/THD+N on low-level tones, threshold sweeps, and live automation at every supported sample rate.

### P1 — Construction accepts invalid topology and unvalidated persisted parameters

The public constructors are infallible and accept zero channels (`lib.rs:60-84`); zero-channel processing then reports `context.num_frames` while consuming an empty buffer (`lib.rs:186-201`, `hiss.rs:62-65`). Factory deserialization passes saved `frequency_hz`, `threshold_db`, and `strength` straight to `from_params`, bypassing `ParamSpec` validation. The reducer clamps frequency and strength internally, but the plugin retains and reports the original out-of-range values (`lib.rs:73-74,87-93,124-142`; `hiss.rs:41-45`), so UI/state can disagree with audible DSP. Threshold is not clamped at all.

Make construction fallible, reject zero channels/sample rate, and canonicalize every factory value through one validation path before storing it. Clamp/reject against schema, finiteness, Nyquist, and cross-field constraints, then expose only canonical values. Add factory tests for zero channels, low sample rates, out-of-range persisted JSON, and runtime/factory parity.

### P1 — Runtime sample-rate consistency is neither required nor checked

The plugin intentionally processes before `initialize` using an assumed 48 kHz (`lib.rs:65-74`), records `initialized` but never reads it, and ignores `ProcessContext.sample_rate` during processing (`lib.rs:170-201`). A host that omits initialization, initializes at zero, or supplies a later context at another rate silently gets the wrong cutoff and envelope timing. `HissReducer::initialize(0)` even converts an invalid rate into 1 Hz (`hiss.rs:36-38`) rather than rejecting it.

Require successful nonzero initialization before processing and reject a context-rate mismatch, or make the graph contract enforce and test the invariant centrally. Remove the dead fields if the host owns the invariant. Cover uninitialized, zero-rate, rate-change, and mismatched-context cases.

### P2 — Detector timing changes with sample rate

The envelope coefficients are fixed at `0.999/0.001` per sample (`hiss.rs:74`). That is roughly a 20.8 ms time constant at 48 kHz, 10.4 ms at 96 kHz, and 45.4 ms at 22.05 kHz. Thus the classifier, transient rejection, startup behavior, and effective attenuation change with sample rate even though `initialize` recomputes only the crossover coefficient.

Specify envelope times in milliseconds and compute `exp(-1/(tau * sample_rate))`; preferably use asymmetric attack/release constants. Add response-equivalence tests across 22.05/44.1/48/96/192 kHz using time-aligned stimuli.

### P2 — Bypass is a hard switch that freezes stale state

When disabled, processing skips the reducer entirely (`lib.rs:198-200`). Re-enabling applies an unsmoothed wet transition using low-pass/envelope histories from before bypass, so a changed input can create a transient and a stale noise decision. Reset is also an immediate history clear. No test bounds transition discontinuity; existing bypass coverage checks only exact disabled transparency.

Define bypass semantics explicitly. For live bypass, keep the detector/filter state warm and ramp an equal-power wet mix; if freezing is intentional, reset/re-prime and crossfade on re-entry. Add on→off→on tests with unrelated signals and randomized callback partitions.

### P2 — The “frequency above which” control is only a shallow, warped first-order split

The split uses a backward-Euler one-pole low-pass (`hiss.rs:69-72,90-96`) and recombines `low + gain * (input-low)`. This is computationally cheap and reconstructs perfectly at unity residual gain, but at reduction it produces only a 6 dB/octave transition, so substantial wanted content below the nominal frequency is affected. Backward-Euler mapping also makes the displayed cutoff increasingly inaccurate toward Nyquist, while the internal `0.45 * sample_rate` clamp can silently differ from the stored/UI value.

Use an exact one-pole mapping (`1-exp(-2*pi*f/fs)`) if the shallow split is intentional, or a complementary higher-order crossover if stronger band isolation is desired. Clamp the visible value to a sample-rate-aware range and publish measured magnitude-response tolerances. Add sweep/impulse tests for cutoff accuracy, reconstruction at zero reduction, stop-band attenuation, and phase response.

### P2 — Metadata overstates cost by classifying a two-state IIR as FFT work

Both `cost_class` and compile metadata return `PluginCostClass::Fft` (`lib.rs:108-114`), although the reducer has no FFT, buffering, or lookahead and performs a small constant amount of scalar work per sample (`hiss.rs:53-57,67-86`). This can distort graph scheduling/barrier decisions and capacity estimates.

Use the appropriate light/IIR cost class and add a metadata snapshot test asserting nonlinear, stateful, non-channel-mixing, zero-latency behavior with the intended cost tier.

### P3 — Control-plane work is duplicated and allocates per updated entry

The plugin maintains a cached parameter vector but `parameter_schema` rebuilds another vector (`lib.rs:97-99,120-122`). `apply_values` rebuilds the full cache inside the loop after every entry (`lib.rs:145-166`), and each non-enabled entry recomputes all DSP coefficients even for an unchanged value. This is outside steady processing, but it becomes a realtime hazard if automation is delivered on the audio thread and makes multi-parameter updates unnecessarily expensive.

Apply and validate the set atomically, compute each dirty coefficient once, then rebuild/swap cached metadata once off the audio thread. Return or clone the existing cache consistently. Add allocation-counting tests for setters and a batched-update benchmark.

### P3 — Long decays can enter the denormal range

The one-pole and envelope states decay indefinitely through ordinary multiplication (`hiss.rs:69-75`) with no state snap-to-zero or explicit denormal guard. Very quiet tails can therefore incur architecture-dependent subnormal penalties even though the callback allocates nothing.

Snap sufficiently small states to zero at a low cadence or rely on a documented host-wide FTZ/DAZ contract. Add long-tail tests that inspect internal/output magnitudes and a benchmark with subnormal input.

## Algorithm assessment

The complementary residual construction is a sensible zero-latency, low-cost basis for a gentle high-frequency expander, but the current detector cannot support the stronger “stationary hiss” claim. A robust low-latency improvement would retain the time-domain split while estimating high-band fast/slow power, deriving a continuous probability/reduction target with hysteresis, and smoothing gain in dB. If transparent restoration is the goal, a small overlap-add spectral denoiser with minimum-statistics noise tracking would better distinguish broadband hiss from tonal/transient treble, at the cost of latency and CPU.

## Real-time allocation and performance assessment

Steady `process_in_place` is allocation-free, in-place, O(frames × channels), and uses only two preallocated state vectors. Focused allocation QA passes and measured CPU is excellent. The main realtime risks are control-path vector rebuilding if setters run on the callback, hard state/gain transitions, and possible denormals; the main optimization is accurate light/IIR scheduling metadata rather than micro-optimizing the already tiny loop.

## Scope reviewed

Read in full: the plugin's `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, QA binary, both source files, and both integration-test files; the complete shared `plugins-denoiser` documentation/manifest/module wiring and all of `src/hiss.rs`, including every inline test. Relevant catalog/factory aliases, bridge/FFI parameter registration, compile metadata, all-plugin benchmarks, realtime-allocation coverage, layout/factory/high-channel robustness tests, and the standard QA harness were inspected. No production code was changed.

## Strengths

- The audio loop is compact, bounded, allocation-free, and naturally supports interleaved multichannel audio with independent state.
- The complementary split guarantees exact reconstruction when reduction is zero and reports the correct zero algorithmic latency.
- Parameter registration is complete across schema/current/set paths, malformed buffer lengths use checked multiplication and return errors, and reset performs no allocation.
- Tests cover state preservation, exact bypass, malformed buffers, multichannel independence, reset, latency, parameter round trips, basic attenuation, factory smoke coverage, high-channel behavior, and zero allocation.

## Verification

- `rtk cargo test -p sotf-plugin-hiss-reducer` — 16 tests passed across four suites.
- `rtk cargo test -p plugins-denoiser hiss` — 12 hiss tests passed (41 unrelated tests filtered out).
- `rtk cargo test -p sotf-plugins --test realtime_allocation_tests test_hiss_reducer_zero_alloc` — focused zero-allocation test passed.
- `rtk cargo run -p sotf-plugin-hiss-reducer --features qa --bin qa-hiss-reducer` — zero latency and zero allocations passed; 5 seconds of stereo audio processed in 0.68 ms (~0.01% estimated CPU).
