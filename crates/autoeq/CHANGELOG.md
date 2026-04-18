# 0.4.27

## Fixes

### Initial guess sign inversion for peaks/dips

- The smart initial guess generator (`initial_guess.rs`) had inverted
  magnitude signs: peaks in the deviation (measurement below target,
  needing boost) were seeded as cuts, and dips (measurement above target,
  needing cuts) were seeded as boosts. This caused the DE optimizer to
  start from a wrong initial population, slowing convergence and often
  missing obvious room modes — especially bass peaks below 100 Hz.

### F3 min_freq clamping skipped for stereo (no subwoofer)

- When target tilt was active, `process_single_speaker` clamped the
  optimizer's `min_freq` up to the speaker's F3 rolloff to prevent
  impossible bass boost. For stereo (2.0) setups without a subwoofer,
  the full-range speakers ARE the bass source — clamping prevented the
  optimizer from placing filters on bass room modes below F3. The
  clamping now only applies when the system has a subwoofer.

## Improvements

### Diagnostic logging for optimizer frequency range

- `prepare_single_channel_eq` now logs the configured, data, and
  effective frequency ranges plus the number of data points in range.
  Deviation values at key frequencies (30–300 Hz) are logged to help
  diagnose cases where filters are not placed in the expected region.
- `run_optimization_pass` logs per-filter frequency and gain bounds.

# 0.4.26

## Fixes

### `roomeq-qa-features` binary now works

- Fixed broken data directory path (`crates/autoeq/autoeq/bin/roomeq_qa_data`
  → `crates/autoeq/bin/roomeq_qa_data`). The binary was unusable before
  this fix.
- Replaced hardcoded `BROADBAND_STEP_INDEX` with per-step `changes_loss`
  flag on `FeatureStep`. Steps that change the loss function
  (`psychoacoustic`, `asymmetric_loss`, `broadband`) now correctly skip
  flat-score step-over-step regression instead of only broadband.
- Added EPA preference tracking: each step records the average EPA
  `preference` score (higher = better) across channels. After a
  loss-change boundary, validation checks that EPA preference does not
  drop below 95% of baseline instead of comparing flat scores.
- Output now shows `epa=X.XXX` per step and `epa vs baseline: +X.X%`
  for steps after baseline.
- Added `qa-roomeq-features` recipe to `crates/autoeq/Justfile` and
  wired it into the `qa-roomeq` aggregate target.

### EPA preference tracking in all roomeq QA binaries

- `roomeq-qa-coverage`, `roomeq-qa-quality`, and `roomeq-qa-synthetic`
  now track and display the average EPA `preference` score (higher =
  better) alongside flat-score metrics. EPA preference appears in both
  pass/fail output lines and failure summaries, giving visibility into
  perceptual quality across all QA runs.

## Features

### Measurement-derived target tilt (`TargetShape::FromMeasurement`)

- New `roomeq::slope` module with `estimate_slope_db_per_octave()`:
  OLS regression of SPL vs log₂(freq) within a configurable frequency
  window (default 200–10 kHz).
- New `FromMeasurement` variant on `TiltType` and `TargetShape` enums.
  When configured, the optimizer extracts the broadband slope from the
  input measurement curve at optimization time and uses it as the target
  tilt, preserving the speaker's natural response characteristic.
- `speaker_eq.rs` resolves `FromMeasurement` before building the target
  curve in both the `target_response` and legacy `target_tilt` paths.

# 0.4.25

## Features

### Measurement-driven Schroeder frequency from the recorded IR

- `DecomposedCorrectionSerdeConfig` now has an optional
  `room_dimensions: Option<RoomDimensions>` field. When it is provided
  together with `ssir_wav_path`, the optimizer derives the Schroeder
  frequency from the actual impulse response instead of using the
  config default:
  1. `roomeq::eq::try_ssir_analysis` now returns the mono IR and its
     sample rate alongside the `SsirResult` (it was previously dropped
     after SSIR analysis even though it was already in memory).
  2. `roomeq::eq::prepare_single_channel_eq` measures **bass-band**
     RT60 from that IR via `math_audio_dsp::analysis::compute_rt60_spectrum`
     at the 125 Hz and 250 Hz octave centres (Schroeder backward
     integration, −5 dB → −25 dB slope, ×3 extrapolation) and takes
     the longer of the two valid values. Bass RT60 — not broadband
     RT60 — is what the Schroeder formula `2000 · √(RT60/V)` is
     derived from, because it is what governs modal decay; typical
     bass RT60 is 1.5–2× mid RT60 in real rooms, so the broadband
     average systematically under-estimates Schroeder.
  3. The measured RT60 is plugged into
     `RoomDimensions::schroeder_frequency_with_rt60` using the
     user-supplied volume. The result overrides
     `dc_analysis_config.schroeder_freq` before
     `build_ssir_correction_weights` runs, so the modal / diffuse
     boundary and the downstream `restrict_boost_above_schroeder`
     cut-only bounds both use the measurement-driven number.
- **Plausibility clamp against malformed IRs.** The override is
  gated by `decide_schroeder_override`, a DSP-free helper that only
  accepts a measured Schroeder when it lands in the plausible band
  `[SCHROEDER_PLAUSIBLE_MIN_HZ, SCHROEDER_PLAUSIBLE_MAX_HZ]` =
  `[50 Hz, 800 Hz]`. Values outside trigger a `warn!` log and the
  optimizer falls back to the config value. This catches the two
  failure modes that would otherwise silently corrupt the modal-
  region bounds:
  - A raw sweep capture fed in instead of a deconvolved IR → very
    long apparent RT60 → Schroeder drops below 50 Hz → whole HF
    range suddenly gets cut-only bounds.
  - A truncated / contaminated IR → very short T20 slope → Schroeder
    rises above 800 Hz → mid-range filters get their upper gain
    bound pinned to 0 dB.
- **Refactor for testability.** The decision logic is split into two
  helpers in `roomeq::eq`:
  - `measure_bass_rt60(mono_ir, ir_sr) -> Option<f64>` wraps the
    bass-band `compute_rt60_spectrum` call.
  - `decide_schroeder_override(rt60, dc_config, current_schroeder_hz)
    -> Option<f64>` is a pure function — no file I/O, no DSP — that
    applies the three preconditions (RT60 > 0, dimensions present,
    result in plausible range) and logs each branch.
  Six new unit tests (`tests::decide_schroeder_override_*`) cover
  accepted overrides, out-of-range rejection on both ends,
  missing-dimensions fallback, and RT60-fit-failure fallback.
- **Noise-floor-aware IR truncation (Lundeby-lite).** Before running
  the bass-band RT60 fit, the IR is now passed through
  `trim_ir_length_to_noise_floor`, which cuts the late-noise tail
  so microphone self-noise, HVAC rumble, or ambient pickup can't
  flatten the Schroeder decay slope and inflate the measured RT60.
  Algorithm:
  1. Window the IR into 10 ms segments and compute per-segment
     mean-squared energy.
  2. Estimate the noise floor as the mean energy of the last 10 %
     of segments (assumed post-decay).
  3. Walk backward and find the latest segment whose energy still
     exceeds the noise floor by +10 dB — this is the last point
     where signal is cleanly above noise.
  4. Keep 3 segments (~30 ms) of headroom past that point so the
     T20 fit still sees some decay curvature at the crossover,
     and truncate there.
  The function is a no-op (returns the full length unchanged) for
  IRs shorter than 100 ms, IRs with fewer than 20 windows, IRs
  with a perfectly silent tail (noise_floor = 0), and pure-noise
  buffers where no segment exceeds the +10 dB threshold. Five new
  unit tests (`tests::trim_*`) cover each of those pass-through
  cases and assert that a 1 s synthetic IR with a clean 500 ms
  RT60 = 0.5 s decay followed by a 500 ms LCG-noise tail is
  truncated below 75 % of its length while still keeping the full
  T20 span (~170 ms for RT60 = 0.5 s).
- Fallback behaviour is unchanged end-to-end: if `room_dimensions`
  is absent, if the RT60 fit fails, or if `ssir_wav_path` is not
  set, the optimizer keeps using `dc_config.schroeder_freq`
  (default 250 Hz) exactly as before. The previous fix's
  `DEFAULT_LISTENING_ROOM_RT60_S = 0.4` guess is only reached when
  the caller invokes `RoomDimensions::schroeder_frequency()`
  without a measured RT60.
- New log lines make the decision transparent: the chosen bass
  RT60, the measured Schroeder value, and the config value it
  replaced are all emitted at `info` level per channel, alongside
  explicit `warn` notes when the measured value is outside the
  plausible range or when room dimensions are missing.

## Fixes

### Room-mode detection output is no longer ignored by the optimizer

- `roomeq::eq::prepare_single_channel_eq` previously captured SSIR /
  decomposed-correction room modes, logged them, and then discarded
  them. The DE optimizer's smart-initial-guess generator
  (`initial_guess::create_smart_initial_guesses`) ran its own
  `find_peaks` over the smoothed deviation and landed on different
  frequencies than the high-quality SSIR modes — leading to filters
  placed at invented centres (37 / 78 / 274 / 1012 Hz in one
  repro room) while real modes at 20.9 / 99.7 / 237.4 Hz went
  uncorrected.
- Now `prepare_single_channel_eq` threads the detected modes through
  a new `ObjectiveData.detected_problems: Vec<(f64, f64, f64)>` field
  (freq, Q, suggested gain — gain set to `-prominence_db` because a
  detected mode is by definition a peak that wants a cut). The DE
  wrapper `optim_de::optimize_filters_autoeq_with_callback` copies
  this list into a new `SmartInitConfig.pre_detected_problems`; when
  non-empty, `create_smart_initial_guesses` uses it verbatim as the
  "problems to correct" list instead of running its own naive
  peak-finder. Result on the repro room: filters land directly on the
  55 Hz, 130 Hz, 161 Hz modes with matched Q factors, and filter
  slots previously wasted on non-mode frequencies are freed.

### Boost filters are no longer generated in the modal region

- Below the Schroeder frequency the room is modal: peaks from
  constructive interference at the listening position *can* be cut by
  EQ, but nulls from destructive interference *cannot* be filled by
  EQ boost — the cancellation happens after the EQ, so adding more
  input energy just raises the direct wave and its anti-phase
  reflection by the same ratio, the null stays, and amplifier
  headroom is wasted. The DE optimizer previously had no knowledge of
  this physics and happily placed `+3 / +4 dB` boost filters at
  29 / 44 / 77 Hz valleys in the repro room.
- New `workflow::restrict_boost_above_schroeder(upper_bounds, args,
  schroeder_hz)` post-processes the per-filter parameter bounds
  produced by `setup_bounds` and clamps the gain upper bound to
  `0 dB` for any filter whose allowed frequency range sits entirely
  below Schroeder. Filters that straddle Schroeder keep symmetric
  bounds (they can still place above-Schroeder boosts where boosts
  are physically meaningful). Applied inside
  `run_optimization_pass` when the decomposed-correction analysis
  has produced a trustworthy `schroeder_freq`. With both fixes above
  landing in the repro room, every peak filter below 250 Hz is now a
  cut and the "boost a null" anti-pattern is gone.

### Schroeder frequency was being computed as 50 Hz on a 30 m³ room

- Two bugs piled up to give the same wrong answer in the SSIR path:
  - `impulse_analysis::build_ssir_correction_weights` derived the
    modal / diffuse boundary from `1 / T_mix` — a dimensionally wrong
    heuristic that equates a time-domain mixing time to a
    frequency-domain modal crossover. There is no physical law
    relating them that way. For a typical small listening room with
    `T_mix ≈ 38 ms` the heuristic returns ~26 Hz, which was then
    clamped up to a hard-coded **50 Hz floor**, so every SSIR-aware
    run on this room reported `boundary = 50 Hz` regardless of what
    the config asked for. The heuristic is removed; the function now
    trusts `config.schroeder_freq` directly (default 250 Hz, override
    per room in the JSON config).
  - `types::config::RoomDimensions::schroeder_frequency` used
    `11885 / √V`. That's Schroeder's formula `2000 · √(RT60 / V)`
    with an implicit `RT60 ≈ 35 s` — a concert-hall reverberation
    time, not a listening room. Applied to a 30 m³ living room the
    old formula would have returned ~2170 Hz, off in the opposite
    direction by ~10×. The function now uses the correct formula
    `2000 · √(RT60 / V)` with a default RT60 of **0.4 s**
    (exposed as a `DEFAULT_LISTENING_ROOM_RT60_S` constant). A new
    `schroeder_frequency_with_rt60(&self, rt60_seconds)` method is
    available for callers that have a measured reverberation time.
- For the same 30 m³ room (3 × 4 × 2.5 m, RT60 ≈ 0.4 s), both paths
  now produce ≈ 231 Hz, matching the published Schroeder calculation
  for a typical small listening room.

### Asymmetric loss is now ERB-aware and suppresses narrow nulls by design

- `loss::asymmetric::flat_loss_asymmetric` no longer uses its own 2-band
  RMS split (`err1 + err2/3`). It now builds per-sample asymmetric
  weights (peak vs. dip, smoothly blended across the 300 Hz transition)
  and hands a `sqrt(w) · error` vector to `enhanced_weights::
  combined_weighted_loss` at the same 70% ERB / 30% band blend used by
  `flat_loss`. The asymmetric loss therefore inherits the perceptually
  motivated ERB weighting instead of living in a parallel, non-perceptual
  regime — the file's old "peak/dip weighting is orthogonal to the ERB
  + band-weighted flat loss" caveat is gone. With every weight set to
  1.0 and no null mask, `weighted_mse_asymmetric` is numerically
  identical to `combined_weighted_loss(0.7, 0.3)` (new unit test
  `asymmetric_equals_combined_when_weights_are_unit`).
- `roomeq::impulse_analysis` gained a `detect_narrow_nulls` /
  `build_null_suppression_mask` pair that mirrors the existing
  `detect_room_modes` peak detector for the dip side. It finds local
  minima, computes `depth_db` against the same ±1 octave local baseline,
  estimates Q from the +3 dB bandwidth around the nadir, and — for any
  minimum that passes both `min_null_q = 3.0` and `min_null_depth_db =
  4.0` — drops a raised-cosine notch in a `mask[f]` array that starts
  at 1.0 everywhere. The mask is continuous (C⁰) so gradient-free
  optimizers do not see a step. Unlike room-mode peak detection it
  scans the full measurement band instead of stopping at Schroeder:
  narrow SBIR and crossover nulls above Schroeder are just as
  unfillable as modal nulls below.
- `roomeq::eq::prepare_single_channel_eq` now runs `detect_narrow_nulls`
  on the unsmoothed normalised curve whenever `asymmetric_loss = true`
  and plumbs the resulting mask through a new
  `ObjectiveData.null_suppression` field. The asymmetric-loss branch of
  `optim::compute_base_fitness` forwards that mask to
  `flat_loss_asymmetric` where it multiplies *only the dip branch* of
  the per-sample weights. Peaks at the same frequency are untouched —
  this matters at mode crossings where a narrow peak and a narrow null
  can overlap.
- `AsymmetricLossConfig::default().bass_dip_weight` changes from **0.2
  to 1.0**. The old near-ignore was a crude proxy for "don't fight
  acoustic nulls"; with explicit null-mask suppression in place broad
  bass dips (SBIR, baffle step, driver integration gaps) are
  legitimate correction targets and should be weighted like the
  mid/treble dip branch. This is a user-visible behaviour change for
  `LossType::SpeakerFlatAsymmetric` runs — the optimizer will now
  spend filter gain on broad bass dips that the old default let it
  ignore.
- The dead `DEFAULT_BASS_TREBLE_SPLIT_HZ = 3000.0` constant and
  `weighted_mse_asymmetric_with_split` helper are removed; nothing in
  the workspace still needs the 2-band shim now that the loss runs on
  `combined_weighted_loss`.

## Features

### EPA as a selectable loss + JSON output + calibration + tunability

- **`loss_type: "epa"`** is now documented and fully wired: selecting EPA
  from the CLI or the roomeq JSON config runs the psychoacoustic composite
  loss (flatness + sharpness + roughness + loudness-balance) via
  `compute_base_fitness`. The underlying module already existed but was
  unreachable by configuration.
- **Per-channel pre/post EPA scores in the JSON output.** Every roomeq run
  (regardless of `loss_type`) now writes an `epa_per_channel` block under
  `metadata` containing the full `EpaScore` (evaluation, potency, activity,
  preference, sharpness_acum, roughness, total_loudness_sone,
  loudness_balance) for both the initial and final frequency responses of
  every channel. See `OUTPUT_FORMAT.md` for the schema.
- **Calibrated loudness.** The Zwicker loudness model was silently
  discarding its `listening_level_phon` argument and comparing
  level-relative (mean-subtracted) curves against an absolute
  threshold-in-quiet table, giving nonsense loudness/balance values. New
  `compute_epa_normalized` / `epa_loss_normalized` helpers denormalize the
  input against `listening_level_phon` before evaluation. Both the JSON
  metrics path and the optimizer objective use the calibrated variant.
- **Tunable EPA via `OptimizerConfig.epa_config`.** Full `EpaConfig`
  (listening level, target sharpness, max roughness, E/P/A weights, plus
  new flatness ERB/band blend and `FrequencyBandWeights`) is now a first
  class field on `OptimizerConfig`, serde-defaulted so existing configs
  deserialize unchanged.

### `combined_weighted_loss` integration (flat + EPA)

- `flat.rs::flat_loss` no longer uses the old 2-band `err1 + err2/3` split.
  It now pre-filters to `[min_freq, max_freq]` and delegates to
  `enhanced_weights::combined_weighted_loss` with a fixed **70% ERB + 30%
  band** blend. ERB (Equivalent Rectangular Bandwidth) is a research-backed
  perceptual frequency scale that directly models cochlear filter
  bandwidth. **This is a deliberate behaviour change: absolute pre/post
  loss values reported for `speaker-flat`, `headphone-flat`, `drivers-flat`,
  and `multi-sub-flat` will differ numerically from previous versions.**
  Solution quality (filter placement, CEA2034 preference scores, perceived
  improvement) is preserved — only the loss surface's absolute scale
  changes. QA thresholds that hardcode expected pre/post numbers will need
  recalibration.
- EPA's flatness term uses the same `combined_weighted_loss` machinery via
  a new `epa_flatness` helper, but honors `epa_config.flatness_erb_weight`,
  `flatness_band_weight`, and `flatness_band_weights` instead of a fixed
  blend. Default EPA flatness is pure ERB (`1.0 / 0.0`) because the other
  EPA terms already carry band sensitivity.
- `enhanced_weights::FrequencyBandWeights` now derives `Serialize`,
  `Deserialize`, and `JsonSchema` so it can be configured via the roomeq
  JSON.

## Code changes

### Loss function module refactor

- Split the monolithic `src/loss.rs` (≈1.9k LOC) into focused submodules under
  `src/loss/`: `types.rs`, `flat.rs`, `asymmetric.rs`, `slope.rs`,
  `speaker.rs`, `headphone.rs`, `drivers.rs`, `multisub.rs`, plus the relocated
  `epa/` tree (`bark`, `cdt`, `loudness`, `roughness`, `sharpness`, `score`).
- `loss.rs` is now a 48-line re-export module preserving the full public API.
- Tests co-located with the source module they exercise.

## Docs

- `INPUT_FORMAT.md` — `loss_type` table now lists `epa`; new "EPA
  Configuration" section documents every `EpaConfig` field including the
  new flatness knobs.
- `OUTPUT_FORMAT.md` — new "EPA Per-Channel Metrics" section documenting
  the `epa_per_channel` block under `metadata`, including all eight
  `EpaScore` fields and the loudness calibration rationale.

## Fixes

- `roomeq::detect_passband_and_mean` now reports the true speaker passband
  for full-range recordings. The previous implementation used the raw
  median of the smoothed SPL as the reference level and searched only for
  the first threshold crossing from each end. On measurements with strong
  bass room modes or linearly sampled frequency grids the median was
  inflated enough that only the bass-mode region exceeded `median − 10 dB`,
  so the detected passband collapsed to a narrow window (e.g. a full-range
  left channel reported as `20.4 Hz – 38.5 Hz`). The reference is now the
  log-frequency weighted average of the 1-octave smoothed curve, and the
  passband edges are taken from the outermost samples above the threshold
  (with linear interpolation between neighbours), which is robust to
  interior dips and to curves that do not roll off within the measurement
  range.

# 0.4.24

## EPA scoring

- Sharpness-aware target curve — Instead of "flat" or "Harman tilt", compute the sharpness (weight
ed spectral centroid) of the corrected response and add a penalty when it deviates from a target sharpness value. This prevent the optimizer from creating a technically flat but perceptually harsh or dull result.
- Roughness penalty for close modes — Two room modes within a critical band create beating perceived asroughness. The optimizer detect mode pairs where |f1 - f2| < critical_bandwidth(f1) and prioritize correcting these over isolated modes, because the roughness they create is more annoying than the level error of a single mode.
- Loudness-weighted loss — Replace the current flat/asymmetric MSE with a loss weighted by ISO 226
  equal-loudness contours at the listening level. A 3dB error at 4kHz (where the ear is most sensitive) should cost more than a 3dB error at 50Hz.
- EPA scoring — Compute E, P, A scores from the corrected response and optimize to maximize Evaluation while preserving Potency. Implemented the psychoacoustic metric computations (Zwicker loudness, sharpness, roughness models).

## Taking care of CDT

The ear generates Cubic Distortion Tones (CDT) at 2*f1 - f2 when two tones f1, f2 are present. Over-correcting at these frequencies can strip perceived "warmth." We add a min_cut_envelope that limits how deep the optimizer can cut at any frequency, protecting CDT-sensitive regions. This mirrors the existing max_boost_envelope pattern exactly.

# 0.4.23

- Added Warped Biquad (Bark-scale resolution) and Kautz Filter (room-mode poles) support
- Temporal decay thresholds

# 0.4.22

- Frequency-dependent correction depth: max_boost_envelope field on OptimizerConfig with log-frequency interpolation. Applied in DE optimizer fitness evaluation.
- Decomposed correction as default:  decomposed_correction defaults to Some(enabled: true). Schroeder raised to 250Hz, steady-state weight lowered to 0.4. Falls back to freq-domain-only mode detection when no IR.
- Stronger bass assymetry: AsymmetricLossConfig extended with bass_peak_weight=5.0, bass_dip_weight=0.2, transition_freq=300Hz. Smooth sigmoid crossfade in loss computation.
- Channel matching priority: Threshold tightened 1.5→0.75dB, max_filters 3→5. Pre-pass computes shared mean SPL so all channels optimize toward same target.
- First-reflection cancellation: New reflection_cancel.rs module. Uses SSIR to identify first reflection, designs LP-filtered IIR echo subtraction (Johnston method) below 500Hz.
- Windowed measurement: direct/early/late windows using SSIR boundaries, computes per-window FR with smoothing.

# 0.4.21

- implemented proper delay detection and analysis (following AES presentation Acoustic and Psychoacoustic issues in Room Correction James D. (jj) Johnston and Serge Smirnov)


