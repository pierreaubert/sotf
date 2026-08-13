# 0.5.11

- Added allocation-free normalized spectral-flux transient detection and
  identity phase locking around remapped spectral peaks. Synthesis peak phases
  initialize from analysis phase, reset on attacks, and preserve independent
  channels' relative phase rather than starting every channel at zero.
- Preserve each detected attack's within-frame time origin when remapping
  transient phase ramps. Offset-spanning regressions at both correction bounds
  keep the output peak at input offset plus the fixed 2047-frame latency. New-
  hop novelty is crest-gated against sustained programme, armed before Hann
  edge attenuation, and carried through overlapping frames; a harmonic-bed
  residual regression covers short attacks over already-audible material.
- Replaced raw fixed-magnitude peak gating with a frame-relative spectral-noise
  floor and energy-weighted temporal confidence. Reference confidence now
  combines local prominence, proximity, and broadband SNR evidence.
- Made multi-channel consensus require a coherent confidence-weighted majority
  cluster. Silent/noisy channels remain excluded, while contradictory tonal
  estimates fail closed and channel-order permutations are deterministic.
- Reject non-finite input before mutating analyzer, phase-vocoder, or cache
  state, so a repaired callback is retry-equivalent to uninterrupted processing.
- Added objective shifted-transient localization, repeated percussive attack,
  voiced harmonic-stack concentration, inter-channel phase, harmonic-level,
  colored/white-noise, SNR, spatial-consensus, transaction, reset,
  cache-generation, and realtime allocation regressions.
- Formant preservation remains explicitly unsupported: no formant-envelope
  model or misleading control is exposed. Reference-free constant-offset
  identifiability and the fixed-frame duration-preserving boundary are unchanged.
- The objective quality path has a measured bounded CPU cost: five identical
  5-second stereo QA runs had a 50.25 ms median (1.00% realtime CPU), versus
  38.53 ms (0.77%) for 0.5.10. Both runs were allocation-free; the ~30.4%
  relative DSP increase is retained for transient and spatial-phase correctness.

# 0.5.10

## Breaking semantic correction

- Redefined PND as a fixed-frame, duration-preserving correction insert and removed the
  unsustainable variable-duration rubato SRC rings, oldest-frame dropping, and dry underrun
  fallback. Device-clock correction/SRC now explicitly belongs outside the plugin.
- Made the preallocated 2048-point/512-hop phase vocoder the sole correction engine. Every
  successful callback overwrites and returns exactly the requested frame count.
- Added a fixed 1536-frame causal prefill and report the measured 2047-frame WOLA latency,
  invariant for 1/64/511/512/1024 and irregular callback partitions.
- Bumped the canonical parameter schema to v2. Legacy `phase_vocoder: false` and `true` states
  both migrate explicitly to the duration-preserving engine; new state omits the retired toggle.
- Made analyzer decisions and correction-strength smoothing sample-clock based, so onset/step
  output and ratio state are identical across callback partitions.
- Release referenced correction toward unity on every completed low-confidence hop after pilot
  authority disappears. Reset is allocation-free and immediately publishes default diagnostics,
  including while readers hold the two preceding cache generations.
- Documented the shared-ratio/per-channel multichannel policy and the absence of formant
  preservation and transient phase locking.
- Added multi-minute correction-extreme, transactional error, detector amplitude/SNR/bin-edge/
  silence-tone-motion, controlled 40/20/10 dB SNR sweep, unity amplitude/SNR, transient
  localization, multichannel, callback-partition, process, and reset allocation evidence.

# 0.5.9

- Correct the absolute-reference resampler direction and add end-to-end
  444.4-to-440 Hz regressions for both resampler and phase-vocoder paths.
- Use local guard-band pilot prominence rather than whole-program magnitude,
  and return reference changes toward unity without leaving stale correction
  latched.

# 0.5.8

- Replace the incomplete phase-only vocoder transform with positive-frequency
  instantaneous-frequency analysis, magnitude/frequency bin remapping,
  collision accumulation, and target-bin synthesis-phase propagation.
- Add numeric frequency-oracle regressions for octave-up, octave-down, and
  unity pitch processing. These fail if spectral energy remains in its source
  bins and cover both shift directions independently of the drift detector.
- Define `drift_smoothing` as a 1–1000 ms sample-clock time constant. Ratio
  updates occur only after a new analysis hop, not once per host callback;
  regressions prove equal elapsed time at 48/96 kHz and split callbacks yields
  the same one-pole state.
- Add an optional `reference_frequency_hz` pilot/note reference. Stable
  programme without a reference remains change-only. Live and persisted values
  are finite/range/Nyquist validated.

# 0.5.7

- Reject malformed factory parameters (non-finite values, out-of-range values,
  and zero channels) through the same schema bounds used by live updates.
- Enforce graph-rebuild-only application for structural analysis/topology
  parameters after initialization, avoiding live FFT/vocoder allocations and
  latency changes.
- Correct drift smoothing semantics so larger smoothing values track more
  slowly, and use confidence-weighted channel consensus with one-to-one peak
  matching to prevent duplicate or silent channels from authorizing corrections.

# 0.5.6

- Clarify that bounded-ring overflow handling is only a mitigation. The full
  no-drop fixed-frame clock-correction contract remains deferred to a host/SRC
  architecture change and is not represented as fully fixed.

# 0.5.5

- Fixed sustained variable-rate correction overflowing the bounded output ring
  under the fixed-frame host contract. Oldest queued frames are dropped to keep
  latency bounded, and temporary SRC underruns preserve corresponding input
  samples instead of inserting silence.
- Reject processing contexts whose sample rate differs from the initialized
  rate; added regression coverage for long-running correction and mismatch
  handling.

# 0.5.4

- Fixed the phase-vocoder path rejecting host blocks larger than its 1024-frame planar scratch
  buffer. Oversized blocks are now split into prepared-capacity chunks and processed without
  allocating in the hot path. Added `test_phase_vocoder_accepts_blocks_larger_than_planar_capacity`.

# 0.5.3

- Fixed `current_drift_estimate()` reading the circular drift-history buffer as
  if it were linear: after the buffer wrapped, stale values from the start of
  the array were used instead of the most-recent entries. The fix correctly
  unrolls the ring starting from `drift_write_pos`. (analysis.rs:~220)
- Fixed `latency_samples()` always returning `RESAMPLER_CHUNK_SIZE` (1024) even
  when the phase vocoder is active. It now returns `PV_FFT_SIZE + PV_HOP_SIZE`
  (2560) when `phase_vocoder` is true, so hosts can align the signal
  correctly. (lib.rs:~1022)
- Fixed `reset()` not flushing rubato's internal interpolation delay lines.
  `init_resampler()` is now called on reset to re-create the resampler from
  scratch, preventing clicks or phase discontinuities after a transport
  stop/start. (lib.rs:~1002)
- Fixed `process_phase_vocoder()` reading `correction_strength` directly instead
  of advancing `correction_strength_smoother`, which caused zipper noise on
  rapid parameter changes in the phase-vocoder path. (lib.rs:~481)
- Fixed phase-vocoder diagnostic cache zeroing `matched_partials` and
  `total_peaks` instead of forwarding the real analyzer values, making
  monitoring inconsistent between the two processing paths. (lib.rs:~519)
- Tightened `test_pnd_known_drift_correction`: removed the no-op `if` branch
  that let the test pass even when drift was not corrected; the test now
  asserts `output_error < input_error` unconditionally.
- Added `test_drift_history_wraps_correctly` to verify circular-buffer median
  correctness after wrap.
- Added `test_latency_samples_reports_pv_latency_when_vocoder_active`.
- Added `test_reset_reinitializes_resampler`.
- Added `test_pv_path_uses_correction_strength_smoother`.

## Deferred (noted for future work)

- Phase vocoder does not map energy to shifted bins (§3.1 in review): full
  architectural redesign required; deferred.
- Complex FFT on real data in phase vocoder (§3.2): requires integrating
  `realfft`; deferred.
- Phase locking for vocoder (§3.3): enhancement, not a bug; deferred.
- Block-size-dependent drift smoothing in vocoder path (§4.2): requires a
  sample counter; deferred.
- Phase vocoder arbitrary block size via ring buffer (§4.3): deferred.
- One-to-many partial matching in analyzer (§4.1): medium complexity; deferred.
- Performance improvements (§5.x: block-based push, per-sample loops, FFT plan
  sharing): deferred.
- Duplicate `PndPluginParams` / `Params` structs (§6.1): cross-crate refactor;
  deferred.

# 0.5.2

- Added input/output buffer validation in both resampler and phase-vocoder paths, so bad host buffers return Err instead of slicing/panicking.
- Added prepared-capacity checks for oversized blocks and output-ring overflow instead of relying on debug assertions or ring overruns.
- Removed phase-vocoder hot-path resize() by rejecting blocks larger than prepared capacity.
- Fixed PndAnalyzer::reset() leaving median_scratch length zero, which could panic after the next valid drift estimate.
- Preallocated analyzer peak/ratio scratch and max drift-history storage so analysis-window changes do not reallocate.
- Marked analysis-window, multi-channel analysis, and phase-vocoder controls as structural/setup.
- Fixed from_params() returning stale cached parameter values.
