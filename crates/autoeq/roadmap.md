# Features for 0.4.43

## Cross-talk cancellation (CTC) / binaural-aware stereo correction

  grep -rni "crosstalk\|bacch\|hrtf\|binaural\|interaural\|itd\|ild" returns zero hits.
  Every channel today is treated independently — but for a stereo or surround setup the
  listener's two ears each receive both speakers' signals through different acoustic paths.
   Per-channel magnitude EQ cannot fix the resulting comb-filter imaging error. State of
  the art (Atal-Schroeder, BACCH-SP, Smyth Realiser, Trinnov 3D Remapping, Apple
  Personalized Spatial Audio for speakers) inverts the 2×2 (or N×N) HRTF transfer matrix at
   the listener's head to deliver each input signal to the intended ear only.

  Concretely roomeq is missing:
  - A 2×N (or N×N) acoustic transfer matrix model — measured in-room or synthesised from a
  generic / personalised HRTF.
  - An optimisation backend that solves min ‖H · F − I_target‖ for a filter matrix F
  (per-channel FIR + cross-channel taps) under a regularised inverse (Kirkeby-style is
  already in fir.rs, but only single-channel).
  - Sweet-spot robustness: solve for the worst-case head position / a Gaussian
  head-position prior, not a single point.
  - Output as a sparse-matrix plugin chain (the infrastructure for per-channel matrix
  routing is already there: output.rs:270 create_sparse_matrix_plugin).

  This would slot cleanly above the existing per-channel pipeline — optimize_room produces
  the per-channel chain, then a CTC pass adds the off-diagonal taps. The existing
  voice_of_god (timbre matching across channels) is the closest neighbour but addresses a
  magnitude-domain problem, not the cross-talk inverse.

## Bayesian / surrogate-model optimisation for expensive losses

  Current backends — DE, CMA-ES, ISRES, COBYLA, NSGA-II/III, MH variants — are all
  evaluation-hungry: DE needs ~10⁶ fitness calls, CMA-ES ~10⁴–10⁵. Today that's fine
  because flat_loss is sub-millisecond. But the loss menu is heading the other way:
  - LossType::Epa already does Bark + Zwicker sharpness/roughness/loudness per evaluation.
  - multi_objective runs the full loss N times (one per measurement / seat).
  - A future CTC objective (#1) would convolve full HRTFs per evaluation.
  - A future listener-preference loss (audiogram / A-B trained) would invoke a learned
  model.

  grep -rni "bayesian\|gaussian.process\|surrogate\|expected.improvement" → zero hits. The
  natural addition is a sample-efficient backend in crates/math-audio/math-optimisation/
  exposed as autoeq:bo:

  - Gaussian-process surrogate (Matérn-5/2 kernel, marginal-likelihood ARD lengthscales)
  over the same (lower, upper) continuous bounds the existing backends use — register
  through optim/registry.rs next to cmaes.
  - Acquisition: Expected Improvement plus real Monte-Carlo q-EI over the joint GP
  posterior so the existing parallel-evaluation infrastructure (ParallelConfig) still
  pays off.
  - Hot-start from the smart-init / Sobol pool that DE already builds (init_sobol) — same
  OptimParams stays the entry point.
  - Hand-off to a local refiner (autoeq:cobyla) once the GP's posterior std drops below a
  threshold — mirrors the existing --refine flow.
  - For multi-objective, a Monte-Carlo qEHVI (expected hypervolume improvement) variant
  that complements nsga2/nsga3 for ≤ ~500 evaluations.

  Target use case: any EPA / multi-seat / future-CTC objective where a single evaluation is
   ≥ 10 ms. Empirically BO converges in ~10²–10³ evals on 10–30D PEQ problems, a 100–1000×
  wall-clock improvement over DE on those losses, and complements (rather than replaces)
  CMA-ES for cheap-evaluation flat-loss runs.

## Delayed to next version

### Driver-physics-derived dynamic boost envelopes (distortion-aware EQ)

  Current driver protection is conservative-static:
  - excursion.rs → F3 detection → fixed HPF.
  - optim.rs:154-162 → max_boost_envelope / min_cut_envelope are user-supplied or
  CDT-derived.
  - auto_tune.rs clamps MAX_MAIN_MAX_BOOST_DB = 4.0 etc. as flat caps.

  State of the art (Genelec GLM, Klippel-driven calibration, Dirac with measured THD,
  Trinnov) instead derives the boost envelope from actual driver behaviour at a target SPL:

  - Measured THD / IMD curves vs. frequency and level (REW supports this; Klippel exports
  it).
  - Predicted SPL at the listening position given the current target and program crest
  factor.
  - A frequency-dependent maximum boost = the boost at which the driver's predicted
  distortion at the program level remains under a configurable threshold (e.g. 1 % THD).

  Concretely missing:
  - A DriverDistortionProfile { thd_curve: Curve, imd_curve: Option<Curve>, max_spl_db: f64
   } ingest path alongside the existing CSV measurement loader.
  - A function in roomeq/excursion.rs (or a sibling roomeq/distortion.rs) that converts a
  (target_spl, program_crest_db) pair plus a DriverDistortionProfile into an Vec<(f64,
  f64)> boost envelope, fed straight into the existing ObjectiveData.max_boost_envelope.
  - An optional signal_level_db_per_decade mode that recognises typical music spectra
  (pink-ish above 200 Hz, hotter in the bass) so the envelope tracks real program demand
  instead of a flat sine-tone assumption.
  - A lightweight reverse mode for the cut side: limit cuts to the depth where the residual
   dip is still below the masked threshold of the next octave, using the existing EPA
  Bark/loudness machinery.

  This is a high-ROI gap because Genelec / Trinnov use it as a marketing differentiator and
   the building blocks (envelopes, CDT mask, EPA loudness) are all already present.

### listener-preference / audiogram personalisation (Sonarworks / Mimi style — there's no audiogram, hearing_loss,
  or preference-learning hook in the tree);

### measurement-uncertainty-aware robust optimisation (spatial_robustness.rs is magnitude-domain only — no bootstrap or minimax-over-uncertainty);

### a continuous listening-area optimisation prior (instead of the discrete seats array in multiseat.rs).


# Guides

## Measuring an N×N Acoustic Transfer Matrix In-Room

  What you actually need

  For a stereo CTC system the matrix is 2×2:

              ┌                              ┐
              │ H_LL(f)   H_RL(f)            │
  H(f)   =    │ H_LR(f)   H_RR(f)            │
              └                              ┘

  H_ij(f) = complex transfer from speaker i to ear j, including the listener's head shadow,
   pinna filtering, torso scatter, and the room. For 5.1 → 2 ears it becomes 5×2; for full
  7.1.4 → 2 ears it's 11×2.

  You measure one column at a time — for each speaker, capture both ears simultaneously.

  Hardware setup

  Mic options, in order of accuracy:

  1. In-ear microphones on a real listener. DPA 4060/4560 or Sound Professionals MS-TFB-2
  blocked-meatus mics. Captures the user's own HRTF + room. Best result, hardest to keep
  stable (sub-mm head movement during a 20 s sweep wrecks high-frequency phase).
  2. HATS / mannequin. B&K Type 4128, Neumann KU100, GRAS KEMAR. Repeatable, immune to
  listener fidgeting, but generic anatomy — sub-1 dB / a few µs different from any
  individual head, which is audible at the highest CTC frequencies (>6 kHz).
  3. Pseudo-binaural mic pair. Two omnis spaced 17.5 cm with a foam baffle between them.
  Cheap, useable for low-mid CTC (≤ ~3 kHz), inadequate above where pinna effects dominate.

  Other equipment:

  - Audio interface: N+1 outs, M+1 ins (the +1 is a loopback for time alignment).
  - Loudspeakers in their final installed positions — the matrix is geometry-specific.
  - Optical distance meter or laser cross to log mic-to-speaker geometry per take.
  - Quiet room (NC-20 or better) for adequate contralateral SNR at HF.

  Excitation signal

  Exponential (log) sine sweep, Farina-style. Standard now. 10–20 s, 20 Hz → 24 kHz, –6
  dBFS peak, 1–2 s of silence head/tail. Reasons:

  - Best SNR per unit time of any deterministic excitation.
  - Inverse-filter deconvolution recovers the linear IR while separating harmonic
  distortion into time-shifted copies you can window out — important because in-ear mics
  see the speaker hard, and HF distortion will contaminate IR if not separated.
  - Tolerant of low-frequency time-variance (HVAC, distant traffic).

  Avoid MLS (worse SNR, distortion folds into linear IR), and avoid white-noise correlation
   (poor LF SNR).

  Measurement procedure (single sweet spot)

  1. Position HATS or listener at the primary seat. For a real listener: bite block, eyes
  closed, neck rest if possible. Tell them not to swallow.
  2. Set levels: ~85 dB SPL average during sweep at ear, no clipping anywhere in the chain,
   in-ear preamp gain locked.
  3. Wire loopback: split the test signal to one interface input → ground-truth time
  reference for every take.
  4. For each speaker i ∈ {1..N}:
    - Mute all other speakers.
    - Play sweep through speaker i.
    - Capture both ear channels and the loopback simultaneously.
    - Repeat 3–5 takes; time-align by the loopback before averaging (don't trust interface
  buffering to be sample-stable across takes).
  5. Move on to the next speaker without disturbing the listener / HATS.
  6. Total time for stereo CTC at one head position: ~3–5 minutes.

  Post-processing

  For each take:

  1. Deconvolve the captured ear channel against the inverse sweep
  (logarithmic-pre-emphasis aware) → impulse response h_ij(t).
  2. Anchor t=0 using the loopback IR — gives a common time reference across speakers and
  across head positions.
  3. Strip harmonic distortion residues. They appear as separate IR copies at known
  negative time offsets; window them out before computing magnitude.
  4. Window the IR to the part you actually want to invert. Two regimes:
    - CTC-only window: rectangular + ½-cosine fade, 3–8 ms long, starting from the direct
  sound. Captures direct + the first 1–2 reflections. Inverting longer windows is unstable
  because the late reverb varies wildly with head position.
    - Joint CTC+magnitude window: use the existing FDW machinery in impulse_analysis.rs —
  long at LF (≥ 200 ms), short at HF (~3 ms). This produces a frequency-dependent matrix
  that is well-behaved to invert at every band.
  5. FFT the windowed IR → complex H_ij(f). Use the same log-spaced grid as the rest of
  roomeq (e.g. 200 bins 20 Hz → 24 kHz).
  6. Phase unwrap with the existing phase_utils.rs helpers.
  7. Stack into the matrix H(f) ∈ ℂ^{M×N} per frequency bin.

  Sweet-spot robustness — measure a head-position ensemble

  A single-position inverse delivers near-perfect imaging in a head-sized bubble and
  aggressive comb filtering everywhere else. To get a usable sweet zone you measure an
  ensemble and solve for the inverse that's robust over the cloud:

  - Translation grid: ±2 cm in x, y, z (7 points: centre + 6 face-centred).
  - Yaw rotation: ±5° (3 points: ±5°, 0°). Pitch and roll matter less — humans don't rock
  side to side at the listening seat.
  - Total: ~9–21 head-position takes per speaker. With KEMAR on a precision tripod head
  this is tractable; with a real listener it's a 30-minute session.

  The ensemble feeds the regularised inverse — minimise Σ_p ‖H_p · F − I_target‖² over the
  position cloud, optionally with a worst-case (minimax) variant for "stable" rather than
  "best-average".

  Synthesised alternative — room × generic HRTF

  If you can't put mics in ears, you can synthesise the matrix:

  H_ij(f) ≈ SpeakerToHeadCentre_i(f)  ·  HRTF_ij(f, θ_i, φ_i)

  - SpeakerToHeadCentre_i = a normal omni-mic measurement at the head-centre point — the
  existing roomeq measurement workflow gives you this for free.
  - HRTF_ij = lookup from a published database (CIPIC, MIT KEMAR, SADIE, RIEC) at the
  angular position θ, φ of speaker i relative to head-forward.

  It's what Trinnov's 3D Remapping does for non-personalised installs and what Apple's
  spatial audio does at scale. Loses ~3–8 dB of CTC depth above 5 kHz vs. measured per-head
   HRTF, but it costs zero extra measurement time and integrates into the existing
  magnitude pipeline.

  A middle ground: personalised HRTF synthesis from photos (Apple Vision Pro head-scan,
  Sony 360 Reality Audio, Genelec Aural ID). The user uploads ear photos; a model returns a
   personalised HRTF; you convolve as above.

  Common gotchas

  - Loudspeaker EQ already applied or not? Decide once. Recommended: measure raw (no
  per-channel correction), let CTC handle both magnitude and cross-talk in one pass; or
  measure post-EQ and let CTC handle only the cross-talk geometry. Don't mix-and-match
  between speakers.
  - Sample-rate consistency. Lock the interface to 48 kHz throughout — log it in the
  measurement metadata. Resampling IRs after the fact changes the loopback time reference.
  - Head shadow at HF. Contralateral path (left speaker → right ear) is 10–25 dB down above
   1.5 kHz. Push sweep level to the maximum the ipsilateral ear can take; otherwise the
  contralateral SNR is the limiting factor of the entire CTC bandwidth.
  - Phase reference per take. Always recompute relative arrivals from the loopback IR.
  Interface buffering can vary by tens of samples between takes even on the same hardware.
  - Causality of the inverse. H^{-1} is generally non-causal; you bake in a bulk delay τ ≈
  N_FIR / 2 so the realisable filter is H^{-1}(f) · e^{-jωτ}. The modelling delay must be
  flagged downstream because lipsync.
  - Regularisation. H(f) is near-singular at LF (path-length differences shrink → matrix is
   poorly conditioned) and at room-mode nulls. Use a Kirkeby-style β(f) — the existing
  fir.rs::Kirkeby already implements this for the scalar case; the matrix version is F =
  (H^H H + β(f) I)^{-1} H^H · I_target.

  What roomeq already has that you can reuse

  - read::read_curve_from_csv and the recording artefact ingest path for IR/sweep files.
  - time_align::detect_delays_multi_channel for per-speaker arrival from the loopback.
  - phase_utils for multi-wrap unwrap.
  - mic_phase_calibration.rs is a natural home for HATS-vs-blocked-meatus correction
  tables.
  - impulse_analysis.rs FDW gating already produces frequency-dependent windowed magnitudes
   — extend it to also emit the windowed complex IR per channel pair.
  - fir.rs::Kirkeby for the regularised inverse (scalar today, would need an N×M matrix
  variant).
  - synthetic.rs already builds synthetic IRs for tests — extend with a synthetic 2×2 head
  model (delay + simple shadow) for unit-testing the CTC inverse.

  Suggested config schema (mirrors existing patterns)

  {
    "ctc": {
      "enabled": true,
      "matrix_source": "measured",
      "measurements": {
        "speakers": ["L", "R"],
        "mics": ["left_ear", "right_ear"],
        "head_positions": [
          { "id": "primary",  "x": 0.000, "y": 0.000, "z": 0.000, "yaw_deg":  0.0 },
          { "id": "left_2cm", "x":-0.020, "y": 0.000, "z": 0.000, "yaw_deg":  0.0 },
          { "id": "yaw_p5",   "x": 0.000, "y": 0.000, "z": 0.000, "yaw_deg":  5.0 }
        ],
        "files": [
          { "head_position": "primary", "speaker": "L", "ir": "irs/L_primary.wav" },
          { "head_position": "primary", "speaker": "R", "ir": "irs/R_primary.wav" }
        ]
      },
      "window": { "type": "fdw", "min_ms": 3.0, "max_ms": 30.0 },
      "regularization": { "beta_lf_db": -20.0, "beta_hf_db": -40.0 },
      "robustness": "minimax",
      "fir_taps": 4096
    }
  }

  For the synthesised path, replace matrix_source: "measured" with "hrtf_database" plus a
  HRTF SOFA path and per-speaker (azimuth, elevation, distance).


## Interactions with XTC

A Posteriori XTC Plugin vs. Joint Optimisation in roomEQ

  The existing sotf-plugin-xtc is a synthetic-geometry, runtime XTC: distance/angle/head-radius parameters drive a
  2×2 frequency-domain inverse with Kirkeby regularisation, plus optional image-source or IR reflections, plus
  auto-gain + limiter. It runs at audio rate after whatever else is in the chain.

  So the chain is one of:

  A:   [per-channel roomEQ]  →  [XTC plugin (synthetic geometry)]
  B:   [per-channel roomEQ]  →  [XTC plugin (loaded measured IR)]
  C:   [joint roomEQ-with-binaural-matrix]  → [thin XTC matrix runtime]

  A and B are what you have. C is what feeding the measured N×M matrix back into roomEQ buys you. The differences
  between A/B and C are not cosmetic — most of them are arithmetic errors that the a posteriori chain cannot detect.

  1. The a posteriori chain optimises for inputs the speakers will never see

  Most important point. Per-channel roomEQ fits F_L(f) to flatten the response of speaker L driven by L alone at the
   listening position. But once XTC is inserted downstream, speaker L is driven by α(f)·L + β(f)·R. The signal the
  speaker actually radiates is no longer the one the per-channel optimiser targeted, so the per-channel correction
  is wrong on the wrong input. Joint optimisation fits F for the actual speaker-input signals and the actual
  binaural-output target — the single global minimum.

  2. Regularisation β(f) is set blindly in the a posteriori chain

  The matrix H(f) is near-singular below ~150 Hz (head spacing ≪ wavelength → ipsi/contra paths almost identical)
  and at every room-mode null. Kirkeby's β(f) trades cancellation depth for filter sanity — but the XTC plugin
  doesn't know which frequencies roomEQ has already nulled, where the spatial-robustness coherence is poor, or where
   the room-position ensemble is incoherent. So it regularises uniformly, leaving 6–12 dB of cancellation depth on
  the table everywhere coherence is high, and over-regularising everywhere it's low. Joint design lets β(f) be
  driven by the same spatial-robustness / FDW coherence the room measurements already produced —
  frequency-by-frequency. Empirically this is the single biggest wide-band cancellation-depth gain.

  3. Driver protection is uncoordinated

  XTC produces 5–25 dB boosts on the contralateral channel at HF and 10+ dB at LF where the condition number
  explodes. That boost lands on top of the roomEQ-corrected signal. If your roomEQ already gave +3 dB at 60 Hz to
  fill a modal dip and was sitting 1 dB below the woofer's excursion limit, XTC's +6 dB on the cross-tap at 60 Hz
  blows the driver. Neither stage sees the total electrical signal at the speaker terminal. Joint optimisation
  evaluates max_boost_envelope / CDT cut envelope / auto_tune excursion bounds on the summed signal — α·L + β·R —
  and refuses to invert paths that would clip or damage. The existing distortion-aware envelope work (your
  honourable-mention #2 from earlier) only pays off in the joint formulation.

  4. Two regularisers, two smoothness budgets, no shared TV²

  The new tv2_weight smoothness penalty operates on peq_spl per channel. XTC's own regularisation operates on the
  matrix inverse. They produce wiggle on opposite sides of each other — XTC adds a sharp HF feature to invert head
  shadow, roomEQ's smoothness penalty doesn't see it, optimiser converges to a "smooth" per-channel curve that ends
  up rough at the ear after XTC. Joint design applies one TV² penalty to the delivered binaural curve, so what you
  penalise is what the listener hears.

  5. Calibration sources drift apart

  The XTC plugin's geometry comes from user parameters (distance, angle, head radius). RoomEQ's per-channel
  correction comes from a measurement that already encodes the actual speaker arrival times, the actual HF rolloff
  of the head/torso into the mic, and the actual room. The two cannot disagree if they're the same data — and they
  always disagree when they're not. With a measured 2×N matrix as input to roomEQ, there's one source of truth and
  no second knob to mistune.

  6. Latency and phase budget

  The XTC plugin runs at FFT 1024 / 75 % overlap → ~10.7 ms bulk delay at 48 kHz, plus its non-causal inverse.
  Mixed-phase roomEQ has its own latency budget. Two separate non-causal corrections in series mean two pre-ringing
  windows, declared independently. Joint design produces one matrix filter with a single declared latency — same
  total ms, but the pre-ringing constraint (mixed_phase's pre_ringing_threshold_db) operates on the binaural sum,
  not on each stage in isolation.

  7. Sweet-spot intersection vs. union

  A posteriori: per-channel roomEQ is robust across the spatial-robustness position cloud, XTC is tuned for one
  geometry. The chain's robustness is the intersection of the two — usually a smaller bubble than either alone,
  because the XTC sweet spot moves out from under the roomEQ ensemble. Joint design uses the same position ensemble
  for both, so the sweet zone stays as wide as the cloud you measured.

  8. What you score is not what you ship

  pre_score / post_score / EPA / Harman are computed on the per-channel magnitude at the listening position —
  without XTC active. Once XTC is enabled in playback, the binaural balance is different and the score is no longer
  reliable. Joint optimisation scores the actual binaural reconstruction — the shipped output is the scored output.

  9. Net runtime cost can be lower with joint design

  Counterintuitive but real. In the a posteriori chain both stages over-correct in the same band — per-channel EQ
  does 6 dB at 4 kHz to flatten a tweeter peak, XTC does another 4 dB at 4 kHz on the contralateral inverse to track
   the same head-shadow signature. Joint design lets the optimiser put the work in whichever filter is cheapest —
  usually the per-channel biquad chain, leaving the runtime XTC matrix as a thinner FIR. Same depth, fewer taps,
  lower CPU.

  When the a posteriori chain is still the right answer

  - You don't have measured matrix data — only generic geometry. The XTC plugin is exactly the right tool there;
  joint design has no extra information to use.
  - The user wants to enable/disable XTC at runtime without re-running roomEQ. Joint design bakes the cross-talk
  inverse into the per-channel chain in a way that doesn't unhook cleanly.
  - Real-time geometry tweaking (sliders for distance/angle in the UI) — joint design freezes the matrix at
  calibration time.

  Recommended architecture (hybrid, not either/or)

  The cleanest answer for SoTF given the existing plugin:

  1. roomEQ optionally ingests the measured N×M matrix when it's available.
  2. In the joint mode, roomEQ produces two coordinated outputs: the per-channel DSP chain (as today) plus a
  recommended_xtc_matrix (FIR taps + recommended β(f)) as a sibling JSON artifact.
  3. The runtime XTC plugin gains a third source mode alongside synthetic geometry and loaded IR: roomeq_recommended
   — load the matrix and β(f) profile that roomEQ co-designed.
  4. The user can still flip XTC off, override geometry, etc. — it just isn't the coordinated solution anymore, and
  you log a warning that the per-channel chain assumed XTC was active.

  This way you keep the runtime tunability of the existing plugin, but get the global-minimum, driver-safe,
  sweet-spot-aligned, score-consistent result whenever the calibration data supports it.
