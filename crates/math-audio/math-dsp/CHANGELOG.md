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
