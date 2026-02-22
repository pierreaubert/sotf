# Implementation Plan: FFT Plugins Sound Quality & Correctness

## Phase 1: Foundational STFT & XTC Stabilization
- [ ] Task: Audit and Fix STFT Overlap-Add Normalization
    - [ ] Create failing test cases for 50% and 75% overlap Hann windows to verify unity gain.
    - [ ] Correct scaling factors in `sotf-plugins` shared FFT utilities or individual plugin implementations.
- [ ] Task: Resolve XTC Volume Saturation
    - [ ] Write a test reproducing the saturation behavior with high-amplitude input.
    - [ ] Adjust internal gain offsets and regularization parameters ($\beta$) to keep coefficients within safe limits.
    - [ ] Implement frequency-domain soft-limiting for aggressive cancellation peaks.
- [ ] Task: Conductor - User Manual Verification 'Foundational STFT & XTC Stabilization' (Protocol in workflow.md)

## Phase 2: Upmixer Steering & Phase Alignment
- [ ] Task: Refine Upmixer Direct/Ambient Decomposition
    - [ ] Create tests using vocal material to detect "leakage" in surround/height channels.
    - [ ] Adjust the coherence-based steering logic to improve center-focus for correlated voice signals.
- [ ] Task: Verify and Correct Multi-channel Phase Alignment
    - [ ] Audit all output paths (Direct, Ambient, Height) for consistent group delay and phase response.
    - [ ] Implement phase correction filters if mismatches are found between the paths.
- [ ] Task: Conductor - User Manual Verification 'Upmixer Steering & Phase Alignment' (Protocol in workflow.md)

## Phase 3: Validation & Performance Verification
- [ ] Task: Final Audio Quality Validation
    - [ ] Run existing `xtc_validation` and `upmixer` benchmarks.
    - [ ] Verify spectral flateness and spatial imaging results.
- [ ] Task: Performance & Memory Safety Check
    - [ ] Execute `engine_allocation_tests` to ensure no new allocations were introduced.
    - [ ] Verify CPU usage baseline on macOS (HAL and cpal paths).
- [ ] Task: Conductor - User Manual Verification 'Validation & Performance Verification' (Protocol in workflow.md)
