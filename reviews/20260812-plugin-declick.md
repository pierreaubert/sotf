# Declick plugin review — 2026-08-12

## Remediation status — 2026-08-12

Fixed in 0.5.6/shared transient suppressor: startup/reset history priming,
clean-only bounded slope learning during rejected bursts, finite/schema-bounded
sensitivity, zero-channel/rate and context-rate safety, local recovery from
non-finite samples, exact existing buffer validation, deterministic reset, and
accurate Dynamics cost metadata.

Fixed in 0.5.7 / `plugins-denoiser` 0.5.10:

- The causal derivative clamp is replaced by an eight-sample-lookahead robust
  pre/post median and MAD detector. Short marked regions are reconstructed from
  channel-specific surrounding context, with fixed reported latency.
- Adjacent channel pairs share detection decisions by default; the persisted
  `link_channels` parameter selects linked or fully independent processing.
- Bypass remains latency matched, detector history stays warm, and enabled and
  sensitivity automation use sample-rate-aware 5 ms smoothing.
- Processing is directly frame-major and allocation-free for arbitrary callback
  sizes. The unused Rayon dependency and false parallel-processing claim are
  removed.
- Construction is fallible for zero channels/rates, factories forward sample
  rate, persisted values use schema defaults/bounds, and successful single-value
  updates modify the parameter cache without allocation.
- Deterministic clean/corrupt tests cover isolated and multi-sample clicks,
  two seconds of repeated clicks, onset/step preservation, 12 kHz and square
  wave false positives, linked and independent stereo behavior, non-finite
  recovery, callback partitioning, bypass re-entry, control smoothing, reset,
  factory parity, compile metadata, and realtime allocation.

## Findings

### P1 — The first legitimate attack after construction/reset is always treated as a click

All history starts at zero with a `1e-6` slope envelope (`transient.rs:26-50,54-61`). A normal nonzero first sample therefore exceeds an approximately `2e-5` default threshold and has curvature ratio 1, so it is clamped almost to zero (`:178-203`). The test explicitly accepts this destruction (`single_sample_mono`, `:579-587`). Transport starts, seeks, discontinuous block entry, and reset can erase the leading attack of wanted audio.

Prime history without output modification for a short startup window, or use lookahead/context to distinguish a persistent onset from an isolated impulse. Add onset vectors for speech consonants, drums, piano, and arbitrary DC starting values after new/reset/seek.

### P1 — The suppression branch rapidly teaches the detector that a click burst is normal

During suppression, the envelope is updated toward `threshold = envelope * sensitivity + epsilon` (`transient.rs:184-200`). With sensitivity >1 this is positive feedback: at the default 10 and 48 kHz, the envelope grows by roughly 0.94% per suppressed sample and doubles in about 74 samples, quickly raising the threshold until corrupt audio passes. The change log describes this as adapting to bursts, but it weakens detection precisely while contamination persists.

Estimate the clean/background slope independently from rejected samples; freeze it, use a bounded robust update, or learn only from accepted neighborhoods. Test long crackle/buzz bursts, repeated vinyl clicks, sensitivity extremes, and maximum leaked amplitude over seconds—not just five two-sample impulses.

### P1 — A slew limiter is not click reconstruction and can replace corruption with new discontinuities

Detected samples are clamped to `last_output ± threshold`, while hidden `last_output` drifts toward the corrupt input (`transient.rs:192-203`). As soon as classification stops, output jumps directly to the current input (`:204-212`). With no lookahead, median context, AR model, or crossfade, the plugin cannot infer the missing sample and may produce a bounded step followed by a recovery click. Zero declared latency is therefore achieved by accepting restoration artifacts.

For transparent repair, detect first and replace a short marked region using pre/post interpolation, local AR prediction, or a robust median/polynomial model; expose the resulting lookahead latency. At minimum smooth entry/exit and bound derivative/curvature. Compare against corrupted/clean reference clips with residual energy, click salience, crest factor, and listening tests.

### P1 — Independent per-channel decisions can damage stereo image and surround coherence

Every channel has a separate envelope/classifier and is processed independently (`transient.rs:11-22,117-157`). A common physical click with different channel gains may be repaired in only some channels; a legitimate transient in one channel may be clamped while its correlated partner passes. This changes inter-channel level, phase, and spatial localization.

Offer linked detection from max/robust aggregate curvature while performing channel-specific interpolation, with selectable independent/link groups for multichannel layouts. Test common-mode, one-sided, unequal-gain, delayed, and surround-correlated impulses for image/correlation preservation.

### P1 — Malformed construction and persisted parameters create split-brain state

The infallible constructor accepts zero channels and then reports successful frame processing on an empty buffer (`lib.rs:43-61,135-156`; `transient.rs:79-84`). Factory JSON bypasses schema validation: stored `sensitivity` can be out of range/non-finite while the suppressor applies only a minimum clamp (`lib.rs:46-56`; `transient.rs:75-77`). The plugin can therefore report a different value from its active DSP, and infinity disables detection.

Make construction fallible, reject zero channels, and canonicalize factory/runtime values through the same finite 1–100 schema before storing or applying them. Add zero-channel, NaN/Inf, out-of-range JSON, and factory/runtime parity tests.

### P1 — Sample-rate initialization is optional and runtime consistency is unchecked

The suppressor assumes 48 kHz at construction, `initialize(0)` silently becomes 1 Hz, and `process_in_place` ignores `ProcessContext.sample_rate` (`transient.rs:45-50,64-68`; `lib.rs:130-156`). Processing before initialization or with a mismatched context changes the nominal 20 ms detector timing without error.

Require successful nonzero initialization and reject context-rate mismatch, or establish/test that invariant in the host adapter. Cover uninitialized, zero-rate, rate-change, and mismatched-context processing.

### P2 — The curvature heuristic confuses legitimate high-frequency/alternating content with clicks

Detection uses only first difference, second difference, and a fixed ratio `curvature/abs_delta > 0.4` (`transient.rs:178-192`). High-frequency tones, cymbals, clipped/limited waveforms, square waves, and sharp musical attacks naturally have high curvature. Conversely broad/slow clicks or clicks landing on a steep wanted slope can evade it. No frequency, duration, robust local median, or psychoacoustic evidence supports the fixed 0.4 threshold.

Calibrate detection on a labeled clean/corrupt corpus; use a robust prediction residual normalized by local MAD/energy and require an isolated temporal shape. Add false-positive/false-negative ROC tests across frequency, level, crest factor, sample rate, and material class.

### P2 — Non-finite audio permanently poisons channel state

NaN/Inf is not checked. A NaN falls through comparisons, is written to the envelope/output/input histories, and subsequent samples continue producing NaN thresholds/deltas (`transient.rs:178-215`). One bad sample can therefore contaminate the rest of that channel.

Define a non-finite policy: sanitize the current sample and reset/repair affected state, or return an error before mutation. Add NaN/±Inf injection at startup, steady signal, and multichannel boundaries, asserting finite recovery.

### P2 — Bypass freezes stale histories and re-entry is unsmoothed

Disabled processing skips the suppressor (`lib.rs:148-153`), so input can change arbitrarily while detector/output histories remain frozen. Re-enabling compares the new signal with the pre-bypass sample and can manufacture a click classification; bypass itself is a hard switch.

Keep detector history warm while bypassed and crossfade repaired/dry output, or reset/re-prime deterministically on enable. Test on→off→on with level/phase/content changes and bound discontinuities.

### P2 — Documentation claims parallel multichannel processing, but the implementation is sequential

The shared change log says multichannel channels are processed “in parallel (process uses Rayon),” and `rayon` remains a dependency, but the active loop is an ordinary sequential `for ch` (`transient.rs:117-157`) with no Rayon call. It pays two full deinterleave/interleave passes and a fixed 1,024-frame-per-channel scratch allocation without receiving the claimed parallel speedup. Recursive chunking repeats that copying for larger callbacks.

Correct the documentation and benchmark before choosing a design. For normal small channel counts, a frame-major scalar loop may be faster and use less memory; for very high counts/large offline blocks, explicit non-realtime parallelism may help but should not dispatch worker tasks from an audio callback. Benchmark channel counts 1–40 and block sizes 16–65,536.

### P2 — FFT cost metadata is materially wrong

Both cost methods classify this scalar, zero-lookahead time-domain loop as `PluginCostClass::Fft` (`lib.rs:76-82`). That can distort graph scheduling and capacity estimates; the host also contains name-based cost heuristics for declick, compounding inconsistent classification.

Assign the measured light/nonlinear time-domain class and add compile-metadata snapshot tests for stateful, nonlinear, same-channel, zero-latency behavior.

### P3 — Control-path cache rebuilding allocates and sensitivity automation is abrupt

Every `apply_values` call rebuilds the parameter vector (`lib.rs:103-125`), and sensitivity changes immediately alter the threshold multiplier without hysteresis or smoothing. This is outside the steady audio loop but is unsafe if automation reaches the callback and can change classification discontinuously.

Apply validated batches off-thread, rebuild cache once only when values change, and smooth detector-control changes or restrict them to setup boundaries. Add setter allocation and automation discontinuity tests.

## Algorithm assessment

The remediated implementation is a short-region interpolator rather than a
slew limiter. Eight samples of future and past context provide deterministic,
low-latency repair for isolated clicks and short crackle while the pre/post
return test protects persistent musical onsets. Robust medians and MAD/local
slope normalization materially reduce false positives compared with the fixed
curvature ratio. It is intentionally bounded and is not a substitute for an
offline spectral restoration tool on long dropouts or dense continuous damage.

## Real-time allocation and performance assessment

The steady path is allocation-free, lock-free, and bounded. State consists of a
17-frame interleaved ring and per-channel scalar analysis arrays allocated at
construction. Every callback is processed directly frame-major; there is no
deinterleave/interleave copy, recursive chunking, resizing, or worker dispatch.
Reset fills existing storage. Valid single-parameter updates change cached
values in place and sensitivity/bypass targets are smoothed in the callback.

## Scope reviewed

Read in full: all Declick `AGENTS.md`, changelog, manifest, README, QA binary, source/parameter files, and both test files; all shared `plugins-denoiser` docs/manifest/module wiring and every line/test of `transient.rs`. Factory/catalog aliases, bridge/FFI/NIH registration, compile/host scheduling metadata, example use, allocation tests/benchmarks, all-plugin benchmark, high-channel/robustness coverage, and standard QA were inspected. No production code was changed.

## Strengths

- The processing loop is bounded, zero-allocation, sample-rate-aware, stateful
  across callbacks, and invariant to callback partitioning.
- Active and bypassed paths share exact eight-sample latency; bypass remains
  warm and transitions are smoothed.
- Linked pair decisions preserve spatial coherence while channel-specific
  interpolation retains each channel's local level; independent mode remains
  available.
- Parameter schema/current/apply/single-setter/factory/engine conversion paths
  include every control and preserve older presets through serde defaults.
- Tests use aligned clean references and explicit false-positive material in
  addition to safety, reset, malformed-buffer, allocation, and performance
  checks.

## Verification after remediation

- `rtk cargo test --offline -p sotf-plugin-declick` — 14 passed across four suites.
- `rtk cargo test --offline -p plugins-denoiser transient` — 13 transient
  tests passed (39 unrelated tests filtered out).
- `rtk cargo test --offline -p sotf-plugins --test realtime_allocation_tests
  test_declick_zero_alloc` — passed.
- `rtk cargo test --offline -p sotf-plugins --test factory_integration_tests
  declick_factory_validates_construction_and_preserves_all_parameters` — passed.
- `rtk cargo clippy --offline -p plugins-denoiser -p sotf-plugin-declick
  --all-targets --all-features --no-deps -- -D warnings` — passed.
- `rtk cargo run --offline -p sotf-plugin-declick --features qa --bin
  qa-declick` — zero allocations; 5 seconds in 37.74 ms (0.75%); active
  1/2/8/40-channel callbacks at 16/257/1024 frames all met their deadlines.
- `rtk cargo check --offline -p sotf-engine --lib` — passed (one dependency
  future-incompatibility notice only).
