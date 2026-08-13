# 0.5.123

## Verification

- Define a latency-aligned, steady-state reconstruction-energy policy for correlated,
  anti-correlated, quadrature, and independent stereo in 5.1 and 9.1.6 with multi-source
  extraction enabled.
- Verify multi-source rendering is numerically equivalent across irregular host block partitions
  and remains allocation-free at 9.1.6 / FFT 4096. Exercise callback-thread FTZ/DAZ with true
  IEEE-754 subnormal bit patterns instead of merely low-amplitude normal samples.
- Add an end-to-end multi-source processing benchmark matrix covering every advertised layout and
  FFT sizes 512–4096. A local exploratory branch-hoisted candidate was not retained.

# 0.5.122

## Verification

- Stamp published ML probabilities with their transport generation and double-check the generation
  on read so reset/publication races cannot expose an old inference result.
- Add a concurrent 100,000-publication/20,000-reset stress regression covering the worker, reset,
  and reader ordering contract.

# 0.5.121

## Review remediation

- Replace the non-physical reactive-intensity front/back axis with a stereo-valid lateral cue;
  quadrature and anti-phase material are now spatially uncertain rather than falsely localized.
- Align empty factory defaults with every catalog parameter and test the real getter surface.
- Make ML reset invalidate queued and in-flight inference generations.
- Reject non-finite input before persistent analysis state can be poisoned.
- Derive critical frequency-analysis smoothing coefficients from hop duration and sample rate.
- Set FTZ/DAZ on the actual processing thread and remove redundant synthesized-buffer scans.
- Add multi-source energy-budget scenarios and initialize the worst-case 9.1.6/FFT-4096 timing test.

# 0.5.120

## Bug fixes

- Publish ML vocal-detector probabilities with Release/Acquire ordering so the
  audio thread cannot observe the ready flag with stale probability bits.
- Validate that inference returns exactly one f32 output with canonical shape
  `[1, 1]`; reject extra, empty, or malformed output tensors instead of using
  an arbitrary first element.

# 0.5.119

## Verification

- Add a non-hop-aligned bypass round-trip regression covering queued-state discard and the
  zero/FFT latency contract.

# 0.5.118

## Bug fixes (from code review 2026-08-12)

- Rebuild all channel-dependent main/HR rings, output buffers, routing caches, and per-speaker
  decorrelators on every speaker-layout change, including equal-width role changes.
- Initialize diffuseness smoothing state on normal startup instead of falling back to raw block
  diffuseness until a resolution toggle.
- Validate the LFE/upmix crossover pair transactionally; malformed factory presets are clamped to a
  one-Hz minimum transition instead of reaching the asserting constructor.
- Make diagnostic hard bypass explicitly structural, reset queued streaming state on both edges,
  and report its actual zero latency rather than the inactive FFT latency.
- Reset dialogue, subharmonic, height-transient, and decorrelation oscillator histories so transport
  reset is deterministic.
- Wire the previously dead crate-local test module into the test build and repair stale field paths.
- Mark spectral-table controls setup-only to keep table regeneration out of realtime automation.

# 0.5.117

## Bug fixes (from code review 2026-05-16)

- `frequency_domain.rs` now clamps height-band gains below `cached_bandpass_bin` back to
  `HEIGHT_MASK_FLOOR` before and after spectral smoothing, preventing low-frequency height leakage
  from neighboring-bin smoothing.
- Added regression coverage that verifies bins below the height bandpass remain at the height mask
  floor after processing diffuse high-frequency input.

# 0.5.116

## Bug fixes (from code review 2026-05-11)

- `hr_processing.rs:78-86` — Replaced brick-wall HR highpass with an 8-bin raised-cosine transition band to eliminate Gibbs pre/post ringing around transients (review §1.1).
- `frequency_domain.rs:561-585` — Extended decorrelation crossfade from 5 blocks (~107 ms) to 25 blocks (~535 ms) and switched to a cosine crossfade shape to prevent audible swish/click on decorrelation mode transitions (review §1.2).
- `lib.rs:286-288` — Removed dead `energy_correction_per_bin/temp/prev` Vec fields (never read or written in the processing path); also removed all initialization/reset sites (~12 KB heap savings per instance) (review §2.1).
- `lib.rs:440` / `process.rs:38,43` — Renamed `prev_magnitude_spectrum` to `prev_power_spectrum` throughout; the field stores squared magnitude (power), not magnitude (review §2.2).
- `lib.rs:1990` — Added `debug_assert!` to guard against main output-accumulator ring-buffer overflow before the WOLA write loop (review §2.3).
- `lib.rs:1155` — `from_params()` now clamps `params.fft_size` to the nearest power of two via `next_power_of_two()`, preventing a panic in `new()` when a malformed preset provides a non-power-of-two size (review §2.5).
- `decorrelation.rs:207` — Normalized the per-bin LFO phase offset (`0.37 * i`) by `TAU * (i / half)` so the decorrelation spatial pattern is FFT-size invariant (review §2.6).

## Deferred

- §1.3 Height `height_direct_leak` low-frequency bleed: requires cross-module change touching `panning.rs` and `frequency_domain.rs`; defer to dedicated PR.
- §1.4 VBAP normalization double-pass review: requires psychoacoustic validation of intended front/rear balance; defer to dedicated PR.
- §1.5 LR4 group-delay documentation: documentation-only; added to backlog.
- §2.4 Smoother per-block advance: minor for typical buffer sizes; defer.
- §3.1 Scalar panning inner-loop vectorization: cross-module refactor; defer to audio-optimizer sprint.
- §3.2 `flush_denormals_inplace` removal: requires confirming FTZ/DAZ flags are always active before removing the safety net; defer.
- §3.3 Decorrelation `make_input_vec` allocation: only triggered on parameter changes, not in audio callback; low priority, defer.

# 0.5.115

- Canonicalized frequency-resolution choices so `ERB`, `Fine ERB`, and `Per Bin` map reliably to the analyzer modes `erb`, `fine_erb`, and `per_bin`.
- Reset per-band covariance, coherence, DOA, and decorrelation state when frequency resolution changes to avoid stale analysis history across band layouts.
- Smoothed high-latency and narrow-band analysis updates more conservatively to reduce covariance/coherence/DOA modulation artifacts in ERB, Fine ERB, and Per Bin modes.
- Smoothed per-band diffuseness before it modulates ambient gain and height suitability, reducing another block-rate analysis control path.
- Switched the main and HR FFT paths to sqrt-Hann WOLA analysis/synthesis so modified IFFT blocks are tapered at hop boundaries before overlap-add.
- Fixed `bypass_all_processing` so bypass passes stereo only to FL/FR and no longer synthesizes center-channel energy.
- Made `binaural_preview` a true 2-channel output mode, including HR-path fold-down and engine channel-flow reporting.
- Fixed app/GPUI channel accounting so toggling `binaural_preview` resizes the player graph, Matrix, meters, and workflow ports to stereo instead of retaining the previous surround layout.
- Added regressions for binaural-preview stereo output, bypass center silence, canonical frequency-resolution modes, and prime-sized host blocks across 5.0 through 9.1.6 layouts.
- Added `qa-upmixer isolate` to run controlled artifact-bisection variants on a track, report peak/step/hop-boundary/second-difference metrics, emit per-block diagnostics, optionally write comparison WAVs, and accept FLAC or other inputs through an `ffmpeg` QA fallback.
- Extended `qa-upmixer diagnose` with `--frequency-resolution` so ERB, Fine ERB, and Per Bin can be measured directly on the same input.
- Added `qa-upmixer diagnose` mode for block-by-block CSV diagnostics of output peaks, control deltas, dialogue detection, decorrelation strength, height gains, height flux gate, coherence, and per-channel levels.
- Added smoothed `dialogue_spatial_control` for spatial decomposition and panning so raw dialogue-probability jitter no longer directly modulates ambient gain, effective coherence, decorrelation strength, surround bleed, or height direct leak.
- Slew-limited the height flux gate and final height-band gain updates to reduce frame-to-frame mask chatter that can sound like grain or scratchiness.
- Initialized height mask state at the height floor to avoid startup/reinitialization jumps in height gain diagnostics.
- Extended diagnostics with `dialogue_spatial_control` and its delta for easier comparison against raw dialogue-probability movement.

# 0.5.114

- Corrected diffuseness/DOA analysis to use the full active-intensity vector, not just the real axis.
- Removed hot-path Vec allocations from smoothed LFE crossover table refresh.
- Moved input/output buffer validation before bypass processing so bypass returns clean errors instead of panicking.
- Marked FFT/decorrelation rebuilding controls as structural/setup so hosts don’t treat them as realtime automation targets.
- Added regressions for quadrature intensity classification and bypass buffer mismatch handling.
