# Implementation Plan: FFT Plugins Sound Quality & Correctness

## Phase 1: Foundational STFT & XTC Stabilization
- [x] Task: Audit and Fix STFT Overlap-Add Normalization
    - [x] Create failing test cases for 50% and 75% overlap Hann windows to verify unity gain.
    - [x] Correct scaling factors in `sotf-plugins` shared FFT utilities or individual plugin implementations.
- [x] Task: Resolve XTC Volume Saturation
    - [x] Write a test reproducing the saturation behavior with high-amplitude input.
    - [x] Adjust internal gain offsets and regularization parameters ($\beta$) to keep coefficients within safe limits.
    - [x] Implement frequency-domain soft-limiting for aggressive cancellation peaks.
- [ ] Task: Conductor - User Manual Verification 'Foundational STFT & XTC Stabilization' (Protocol in workflow.md)

## Phase 2: Upmixer Steering & Phase Alignment
- [x] Task: Refine Upmixer Direct/Ambient Decomposition
    - [x] Create tests using vocal material to detect "leakage" in surround/height channels.
    - [x] Adjust the coherence-based steering logic to improve center-focus for correlated voice signals.
- [x] Task: Verify and Correct Multi-channel Phase Alignment
    - [x] Audit all output paths (Direct, Ambient, Height) for consistent group delay and phase response.
    - [x] Implement phase correction filters if mismatches are found between the paths.
- [ ] Task: Conductor - User Manual Verification 'Upmixer Steering & Phase Alignment' (Protocol in workflow.md)

## Phase 3: Validation & Performance Verification
- [x] Task: Final Audio Quality Validation
    - [x] Run existing `xtc_validation` and `upmixer` benchmarks.
    - [x] Verify spectral flateness and spatial imaging results.
- [x] Task: Performance & Memory Safety Check
    - [x] Execute `engine_allocation_tests` to ensure no new allocations were introduced.
    - [x] Verify CPU usage baseline on macOS (HAL and cpal paths).
- [ ] Task: Conductor - User Manual Verification 'Validation & Performance Verification' (Protocol in workflow.md)
