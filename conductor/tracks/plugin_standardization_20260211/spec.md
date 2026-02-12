# Specification: Plugin Standardization

## Objective
To bring all plugins in the `sotf` workspace to a consistent, high-quality standard suitable for professional real-time audio processing.

## Core Requirements

### 1. Algorithmic Correctness
- **Invariants:** Enforce physical and mathematical invariants (e.g., energy conservation in upmixers, phase coherence in crossovers).
- **Stability:** Ensure filters (IIR/Biquad) remain stable during parameter sweeps.
- **Accuracy:** Validate against reference implementations or theoretical models.

### 2. Performance Optimization
- **Zero Allocation:** No heap allocations allowed in the `process` or `process_in_place` hot paths.
- **SIMD:** Leverage SIMD (Neon/SSE/AVX) for performance-critical operations (gain, mixing, biquads).
- **Fast Math:** Use optimized approximations for transcendental functions (`log10`, `powf`, `expf`) when applicable.
- **CPU Spikes:** Use `flush_denormals` to prevent performance degradation from denormal numbers.

### 3. Real-time Control
- **Parameter Smoothing:** All continuous parameters MUST be smoothed to prevent "zipper noise" or "pops".
- **Safety:** Use atomic values or lock-free structures for real-time parameter updates.
- **Configuration:** All relevant DSP parameters must be exposed via the `Plugin` trait's parameter system.

## Success Criteria
- All plugins pass performance benchmarks with zero allocations.
- No audible artifacts during rapid parameter changes.
- Mathematical validation of energy and phase for complex plugins.
