# 0.5.18

## Bug fixes

- `analysis::compute_rt60_broadband` now uses Schroeder backward integration
  with least-squares T30/T20 extrapolation and fit-quality rejection instead
  of first-crossing timing. This makes octave-band RT60 estimates less prone
  to inflated values from noisy or flattened decay tails.
- `analysis::compute_rt60_spectrum` now trims late steady-state noise on each
  band-filtered impulse before fitting RT60 and logs the selected fit method,
  `r²`, and fit window for easier diagnosis.

# 0.5.17

## Bug fixes

- `ebur128`: `gating_blocks` changed from `Vec` to `VecDeque` to eliminate
  O(n) `remove(0)` shifts on the audio hot path once the 1-hour cap is
  reached (#1).
- `instantaneous_frequency`: phase unwrapping now uses `rem_euclid` instead
  of `%` for robust wrap-to-π behavior with negative differences (#2).
- `audio_features::utils::geometric_mean` now asserts that the input length
  is a multiple of 8.  Previously `chunks_exact(8)` silently dropped the
  remainder, producing wrong results for non-multiple-of-8 slices (#3).
- `audio_features::spectral`: spectral flatness no longer hardcodes 256
  bins; it uses the largest multiple of 8 `<= norms.len()` (#4).
- `audio_features::chroma`: `pip_track` now returns empty pitch/mag vectors
  instead of erroring when the frequency mask is empty (e.g. very low
  sample rates where Nyquist < fmin) (#5).
- `analysis::compute_thd_from_ir`: harmonic extraction window minimum is now
  frequency-dependent (`3 * sample_rate / (harmonic_order * start_freq)`)
  instead of a fixed 256 samples, preventing windows that are too short for
  low fundamentals or too long for low sample rates (#6).
- `analysis::compute_coherence_from_realizations`: now returns `Err` for
  `N < 4` instead of silently returning γ² = 1, which is statistically
  meaningless (#7).
- `fast_exp2`: documented the silent `[-126, 126]` clamp (#9).
- `fdn`: documented the rationale for the `±4` safety clamp (#10).
- Synchronized version strings in `README.md` and `CLAUDE.md` with
  `Cargo.toml` (#8).

# 0.5.16

## New features

- Added reusable binaural transfer-matrix DSP primitives for RoomEQ CTC:
  regularized and weighted matrix inverse solves, approximate minimax
  worst-position reweighting, per-position reconstruction errors, FIR synthesis
  from half-spectra, sweep deconvolution, loopback/direct-peak alignment,
  harmonic residue suppression, direct-peak windowing, and complex
  frequency-dependent windowing.
- Harmonic residue suppression now keys residue positions from the detected
  direct peak, so delayed acoustic arrivals are handled correctly after
  loopback alignment.
- Added reusable psychoacoustic DSP primitives for expensive perceptual losses:
  Bark-scale conversion and aggregation, Zwicker-style specific/total loudness,
  sharpness, listening-level calibration, pairwise sensory roughness, cached
  feature extraction, and stereo HRTF/CTC convolution helpers.
- Added reusable frequency-response helpers for linear DSP modeling: complex
  biquad response, FIR response, and LR4 low/high crossover response. RoomEQ
  uses these to align CTC transfer-matrix solving with exported runtime DSP.

## Performance

- Added parallelisation in FDW computation

## AI model

- Note: not synced here. Need to copy the python code or port it to rust

# 0.5.15

## New features

- Added Frequency-Dependent Windowing (FDW) analysis for impulse responses,
  including Morlet-style frequency-dependent gates, FDW-gated magnitude, and
  direct/total time-frequency energy ratios for correction-depth consumers.

# 0.5.14

- Added new signals: Dirac and MLS

# 0.5.13

## Switch to oxiblas-ndarray for BLAS operations

Replaced ndarray's built-in dot product and matrix multiplication with
oxiblas-ndarray's pure-Rust BLAS implementation for better performance
on all platforms without requiring external BLAS libraries (OpenBLAS,
Accelerate, MKL).

- `chroma.rs`: vector dot products now use `dot_view()`, matrix
  multiplication uses `matmul()` from oxiblas-ndarray.
- Added `oxiblas-ndarray` dependency.

Cargo version 0.5.12 -> 0.5.13.

# 0.5.12

## GD-Opt v2 — Phase GD-1c primitives: multi-sweep coherence + noise-floor

Adds the three pure DSP primitives the sotf-engine multi-sweep
capture path needs to populate the `coherence` and `noise_floor_db`
columns on `autoeq::Curve` that GD-1g's confidence gate consumes
(`docs/gd_opt_v2_plan.md` §2.2 / §2.3 / §2.4).

- `compute_coherence_from_realizations(realizations) -> Vec<f32>` —
  per-bin γ² = |H̄|² / ⟨|H|²⟩ across N complex spectra. Returns 1.0
  for N=1 (trivial self-consistency), 0.0 when realizations have
  canceling phases, any value in [0, 1] for partial correlation.
  Rejects mismatched bin counts.
- `deconvolve_sweep(recording, reference, sample_rate) -> Vec<Complex<f32>>`
  — inverse-filter deconvolution of one recorded log sweep:
  H = (Y · X*) / (|X|² + ε²) with ε set 60 dB below the sweep's
  peak spectral bin to bound out-of-band division-by-zero. Returns
  the half-spectrum `[0, fft_size/2]`.
- `estimate_noise_floor_db_from_silence(silence, sample_rate) -> Vec<f32>`
  — per-bin dB over a Hann-windowed FFT of the pre-silence window.
  Amplitude normalisation is `4/N` so a pure sinusoid at a bin
  centre reports its dBFS within ±1 dB. Pure silence reports
  ≤ -200 dB.

9 new tests in `analysis::gd_1c_tests`, all passing.

Cargo version 0.5.11 → 0.5.12.

# 0.5.11

- added FCMLA instruction in simd (ARM8.3+ works in Apple ARM)

# 0.5.10

- added proper signal recording specialized on delays detection with narrowband probe
