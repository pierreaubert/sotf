# PND plugin review — 2026-08-12

## Findings

## Remediation status — 2026-08-12

- P1 factory validation fixed in 0.5.7: `try_from_params` applies finite,
  schema-range, and non-zero-channel checks, and both workspace factories use
  the fallible constructor. Malformed factory regressions are covered.
- P1 structural realtime safety fixed in 0.5.7: analysis-window,
  multi-channel, and phase-vocoder changes are rejected after initialization;
  setup changes must be prepared by graph construction. This prevents live
  analyzer/FFT/vocoder allocation and topology/latency changes.
- P1 drift-smoothing semantics fixed in 0.5.7: the documented convention now
  holds (larger values are slower), with a deterministic monotonic regression.
- P2 consensus and duplicate-peak correctness fixed in 0.5.7: low-confidence
  channels are excluded, remaining observations use confidence-weighted
  consensus, and each previous peak can be matched once. The matching
  invariant has a focused regression test.
- P0 partially mitigated: bounded output-ring overflow now drops the oldest
  complete frames, and SRC underruns use corresponding input samples rather
  than zeros. This bounds memory and avoids silence, but it does not satisfy a
  strict no-drop fixed-frame clock-correction contract; that architecture-level
  redesign remains explicitly deferred.
- P1 fixed: processing now rejects a `ProcessContext` sample-rate mismatch.
- Verification after remediation: focused PND tests pass, including the new
  sustained-correction and sample-rate-contract regressions. The remaining
  findings below (reference-free drift identifiability, pitch-vocoder quality,
  level-robust peak/SNR modeling, exact queue transaction semantics, and
  broader algorithmic limits) remain open for a subsequent design batch.

### P0 — A persistent correction ratio cannot satisfy the fixed-frame plugin contract

The resampler consumes fixed 1024-frame chunks and produces a variable number of frames, appending them to a bounded output ring (`pnd_plugin.rs:393-527`). `process` must nevertheless return exactly the host's requested frame count (`pnd_plugin.rs:698-818`). For a sustained ratio above unity the ring gains frames until `process_one_chunk` returns “Output ring overflow”; below unity it loses frames and `process` fills the deficit with zeros (`pnd_plugin.rs:777-790`). Four chunks of buffering only postpones the failure. This makes sustained clock/varispeed correction impossible without dropouts or errors.

Define the actual system boundary. Clock-domain correction requires independent producer/consumer clocks plus a fill-level controller and an asynchronous SRC that always supplies the render callback; offline varispeed must be allowed to return a different frame count; a fixed-frame insert must use duration-preserving pitch correction. Do not expose the current variable-duration resampler as a general same-frame plugin. Add multi-minute tests at the minimum/maximum correction ratios and assert no underrun, zero insertion, overflow, or unbounded latency.

### P1 — The detector has no reference from which to infer absolute pitch or clock error

The analyzer matches spectral peaks only against the immediately previous FFT frame and records `current_frequency / previous_frequency` (`analysis.rs:146-171`). A steady 444.4 Hz sinusoid therefore has the same expected ratio (1.0) as a steady 440 Hz sinusoid; there is no score, reference pitch, pilot tone, timestamp, device-clock measurement, or long-term nominal baseline that says either frequency is wrong. Musical glissando, vibrato, chord changes, and beating create the same local ratios as wow/flutter. Yet the documentation and `test_pnd_known_drift_correction` claim that an arbitrary stable 444.4 Hz tone should be restored to 440 Hz (`tests/test_pnd_plugin.rs:87-166`). That inference is physically underdetermined; any apparent pass is an artifact of estimator/resampler/zero-crossing bias.

Require an observable reference: source/render clock timestamps for device drift, a known pilot/reference pitch, user-marked notes, or a carefully estimated slow baseline explicitly limited to wow removal (which cannot correct constant offset). Separate musical partial tracking from clock estimation. Replace the biased single-tone test with identifiability tests: 440 and 444.4 Hz without a reference must receive identical unity correction, while a known pilot or simulated timestamp drift must recover the injected error.

### P1 — `drift_smoothing` implements the reverse of the documented behavior

Status: fixed in 0.5.7 by `smooth_drift_ratio`; the documented larger-is-
slower convention is now tested. A sample-rate-aware time-constant redesign
remains deferred.

Both processing paths update `current_ratio = old * (1 - alpha) + target * alpha` (`pnd_plugin.rs:292-295,466-469`). Thus `alpha=1` jumps immediately and `alpha=0.001` is highly smoothed. `USAGE.md` says higher values are “more stable” and “slower to track,” while `test_drift_smoothing_slow_correction` labels 0.99 “very high smoothing” (`src/lib/tests.rs:9-38`). That test only sees a nearly-unity target, so it does not reveal the reversal.

Either rename the parameter to update coefficient and correct the docs/UI, or implement smoothing strength/time so larger means slower as promised. Prefer a time constant converted using actual analysis-hop duration, not a dimensionless block-dependent coefficient. Test a deterministic step target at multiple sample rates and callback partitions.

### P1 — Reported latency is callback-partition-dependent and incorrect for both paths

The resampler reports a fixed 1024 samples and the phase vocoder reports 2048+512 (`pnd_plugin.rs:844-851`). In the resampler path, a 1024-frame host block is accumulated, processed, and drained into that same callback, while two 512-frame callbacks make the first callback silent and drain on the second. Therefore the externally observed delay changes with block partitioning rather than being a fixed 1024 samples. In the vocoder path, processing begins as soon as `input_fill` reaches 2048 and output is drained during that same input iteration (`pnd_plugin.rs:301-330`); adding another hop to the reported latency is not derived from the actual queue positions. The existing latency test merely asserts the hard-coded formula (`src/lib/tests.rs:122-150`) rather than measuring an impulse.

Use sample-accurate input/output queues with a fixed startup prefill independent of callback size, include the SRC/filter group delay, and derive latency from queue state. Measure it with impulses for host blocks of 1, 64, 511, 512, 1024, and irregular partitions, then assert identical delay and stable metadata.

### P1 — The phase-vocoder pitch shifter is incomplete and will smear or misplace nontrivial spectra

`process_hop` retains every magnitude in its original FFT bin and only multiplies the phase advance by `pitch_shift` (`phase_vocoder_channel.rs:88-126`). Energy is never mapped to shifted bins, collisions/out-of-band bins are not handled, and there is no identity/peak phase locking or transient treatment. Small within-bin shifts may move a stationary partial, but larger corrections and polyphonic/transient content cannot produce a coherent spectral translation. The code processes the redundant negative half before overwriting it with conjugate symmetry, doubling much of the trigonometric/norm work. “Preserving formants” in the docs is unsupported; an ordinary uniform pitch shift generally moves formants unless a separate envelope estimator is used.

Implement a validated phase-vocoder pitch-shift design (often time-stretch plus high-quality resampling), or remap positive-bin energy with collision handling and phase locking; add transient reset/preservation and an explicit formant-envelope stage only if retaining that claim. Use real FFTs. Add sinusoidal frequency-accuracy, harmonic/polyphonic spectra, impulse/transient, stereo phase, formant, and alias-energy tests over the full ratio range.

### P1 — Structural parameter setters allocate and change topology/latency live

Status: the realtime safety portion is fixed in 0.5.7: structural setters now
reject changes after initialization. Off-thread rebuild/swap and
latency-aligned crossfade remain host-level architecture work and are deferred.

Although analysis window, multi-channel analysis, and phase vocoder are marked structural/setup, the public setter immediately resets analyzer histories, clears and repopulates analyzer vectors, creates FFT plans/buffers, and changes processing topology/latency (`pnd_plugin.rs:577-625`, `analysis.rs:247-259`). `reset` recreates the rubato resampler (`pnd_plugin.rs:821-840`), which also allocates. If these trait methods run on the audio thread they violate real-time safety and can click; switching back also exposes path states maintained on different timelines.

Enforce graph-rebuild-only application for structural settings and allocate/plan off-thread, then swap with a latency-aligned transition. Make reset allocation-free by resetting a reusable SRC or preparing a replacement outside the callback. Add allocator-counting and discontinuity tests for reset and every parameter path, not only steady-state processing.

### P1 — Factory parameters bypass schema validation

`from_params` assigns all floats directly (`pnd_plugin.rs:185-197`). NaN, infinity, out-of-range smoothing/strength/confidence, invalid analysis windows, zero channels, and zero sample rate can therefore reach FFT, ratio, division, capacity, and ring arithmetic despite runtime setter validation. Channel/sample-count products in initialization and parts of the phase-vocoder validation are also unchecked.

Validate construction through the same `ParamSpec` schema, reject unsupported channel/sample-rate configurations, and use checked size arithmetic throughout. Add malformed serde/factory tests for NaN/Inf, bounds, zero channels/rate, and allocation-size overflow.

### P2 — Multi-channel consensus can authorize the wrong correction

Status: fixed in 0.5.7 by confidence-weighted consensus with low-confidence
channel rejection. Full spatial/channel policy remains deferred.

The code takes an unweighted median drift but separately averages confidence (`pnd_plugin.rs:263-283,433-454`). A confident tonal channel plus silent/noisy channels can yield a median unity drift while the average crosses the threshold, or a valid dominant channel can be rejected by low-confidence channels. Even-sized medians use the upper middle. Peak matching is also many-to-one: multiple current peaks can select the same previous peak (`analysis.rs:146-176`), inflating matched-partial count and confidence (`analysis.rs:181-185`).

Form `(ratio, confidence)` observations together, discard low-confidence channels, compute a robust confidence-weighted consensus, and perform one-to-one partial assignment. Add silent+tonal, conflicting-channel, duplicated-peak, even-channel, noise, and channel-order permutation tests.

### P2 — Peak detection and confidence are not level- or spectrum-robust

Peak picking uses a fixed raw FFT magnitude threshold of 0.001 (`analysis.rs:117-143`), so behavior depends on input amplitude, FFT/window normalization, and channel gain. Confidence is simply matched peaks divided by all detected peaks; spectral leakage or noise increases the denominator, while many-to-one matching can increase the numerator. The “~−60 dB” comment is not tied to dBFS after window/FFT normalization.

Normalize magnitude, estimate a local/global noise floor, use prominence/SNR and frequency-dependent tolerances, and calibrate confidence against controlled datasets. Add amplitude-invariance sweeps, noise/SNR sweeps, low-frequency/bin-edge cases, silence-to-tone transitions, and real musical fixtures.

### P2 — Error paths can consume queued input before SRC success

`process_one_chunk` advances the input-ring read position and decrements its count before creating adapters and calling the resampler (`pnd_plugin.rs:476-504`). Any subsequent adapter/SRC/output-ring failure loses the input chunk, leaving state inconsistent if the host retries or continues.

Commit queue positions only after all fallible processing and capacity checks succeed, or explicitly enter a reset-required failed state. Test injected adapter/SRC failure and output-ring overflow for atomic queue behavior.

### P3 — Tests emphasize finiteness and implementation formulas rather than DSP contracts

The focused suite passes, but multiple phase-vocoder tests assert only no error/finite output (`src/lib/tests.rs:187-260`, `tests/integration.rs:89-123`). The smoother test explicitly admits it does not inspect smoothing (`src/lib/tests.rs:225-237`). The latency test checks constants, and the stable-tone correction test asserts an impossible reference-free outcome. No test establishes pitch-shift accuracy, callback-partition invariance, sustained ring stability, allocation-free reset/topology change, or response to musical pitch movement.

Build an oracle-based DSP suite around the contracts above and retain smoke tests only as a first layer.

## Algorithm assessment

The implementation has a clear experimental pipeline—real FFT peak tracking, robust temporal median, confidence gating, asynchronous cubic resampling, and an optional STFT path—but it conflates three distinct problems: clock-domain synchronization, wow/flutter estimation, and duration-preserving pitch shift. Each needs a different observable/reference and buffering contract. Split them before tuning thresholds or adding more heuristics.

## Real-time allocation and performance assessment

Normal prepared processing reuses FFT, ring, planar, median, and adapter storage and has a dedicated zero-allocation regression test. However, structural setters and reset allocate; multi-channel analysis runs an FFT per channel; the vocoder uses complex FFTs on real data and computes both spectral halves; non-stereo interleave/deinterleave is scalar; and median selection plus per-sample ring loops add cost. Correctness and topology should be fixed first, then benchmark by channels, block partitions, modes, and correction ratios.

## Scope reviewed

Read in full: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, `UI.md`, `USAGE.md`, `bin/qa_pnd.rs`, `examples/pnd_demo.rs`, all files under `src/` (including `analysis.rs`, configuration/parameter definitions, phase vocoder, plugin implementation, inline tests, and types), and both files under `tests/`. Relevant host/factory contracts reviewed include `Plugin`, compile metadata/latency, `RealTimeCache`, smoothing, parameter update modes/schema validation, SIMD interleave helpers, rubato adapters/SRC behavior, catalog registration, and the workspace real-time allocation coverage. No production code was changed.

## Strengths

- The code preallocates steady-state rings, planar buffers, FFT state, peak/median scratch, and per-channel monitoring data.
- Buffer sizes are validated before slicing in the public process paths, and oversized prepared-capacity failures are explicit.
- Drift history correctly handles circular wrap and caches its median until dirty.
- Runtime parameter validation rejects unknown, non-finite, out-of-range, and wrong-type values.
- Reset clears analyzer/vocoder/ring state, denormal/NaN smoke coverage is broad, and diagnostics are throttled.

## Verification

`rtk cargo test -p sotf-plugin-pnd` — 36 tests passed across four suites.
