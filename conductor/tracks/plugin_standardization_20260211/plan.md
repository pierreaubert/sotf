# Implementation Plan: Plugin Standardization

This plan aims to bring all plugins to a consistent, high standard of correctness, performance, and real-time controllability.

## Phase 0: Infrastructure
- [x] **Fast Math Utilities:** Create `math_dsp::fast_math` with optimized approximations for `log10`, `pow`, and `exp`.
- [x] **Enhanced Smoothing:** Ensure `Smoother` is efficient and supports different curve types.
- [x] **SIMD Library:** Expand `plugins::simd` with common audio operations.
- [x] **Quality Benchmarks:** Automated tests verified for all core plugins.

## Phase 1: Basic Routing and Gain
- [x] `Gain`: Optimize per-channel mode and ensure SIMD usage.
- [x] `Matrix`: Ensure no allocations and efficient mapping.
- [x] `ChannelMuteSolo`, `Downmix`, `MonoToStereo`: Add smoothed transitions.

## Phase 2: Core Dynamics
- [x] `Compressor`: Optimize hot-path `log`/`pow` calls. Implement hard/soft knee accurately.
- [x] `Limiter`: Ensure look-ahead is efficient and transparent.
- [x] `Expander` & `Gate`: Similar optimizations as compressor.
- [x] `AutoGain`: Ensure stability and lack of artifacts. Refactored for background processing.

## Phase 3: Filters and Spectral Processing
- [x] `EQ`: Optimize Biquad cascade using SIMD.
- [x] `Crossover`: Verify phase-alignment (Linkwitz-Riley).
- [x] `Delay`: Efficient ring buffers and fractional delay interpolation.
- [x] `BandSplit` & `BandMerge`: Verify perfect reconstruction.

## Phase 4: Multi-band and Complex Dynamics
- [x] `MultibandCompressor` & `MultibandExpander`: Refactor to use optimized core dynamics.
- [x] `LoudnessCompensation` & `FletcherMunson`: Accurate and smoothed psychoacoustic curves.

## Phase 5: Advanced Spatial Processing
- [x] `Upmixer`: Audit for energy conservation and phase coherence. Optimize FFT path.
- [x] `Binaural`: Improve HRTF interpolation and performance. Ensure no allocations during SOFA loading.
- [x] `XTC` (Crosstalk Cancellation): Verify recursive filter stability.

## Phase 6: Infrastructure and Analyzers
- [x] `Resampler`: Optimize `rubato` integration for zero-alloc hot path.
- [x] `Convolution`: Efficient partitioned convolution implemented.
- [x] `SpectrumAnalyzer` & `LoudnessMonitor`: Optimize for low UI impact using lock-free rings.
