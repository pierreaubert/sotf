# Plan: Add AllPass Filter to math-iir-fir

## Phase 1: Red Phase (Verification & Testing) [checkpoint: d6ed25a]
- [x] Task: Write failing unit tests for All-Pass filter behavior [d6ed25a]
    - [x] Create `test_allpass_response` to verify magnitude is exactly 0 dB (1.0 linear) across the spectrum.
    - [x] Create `test_allpass_phase` to verify phase shift is 180 degrees at the center frequency.
    - [x] Add tests for All-Pass parameter validation in `try_new`.
- [x] Task: Conductor - User Manual Verification 'Phase 1: Red Phase' (Protocol in workflow.md) [d6ed25a]

## Phase 2: Green Phase (Implementation & Integration) [checkpoint: d6ed25a]
- [x] Task: Verify and Refine Biquad All-Pass implementation [d6ed25a]
    - [x] Double-check RBJ coefficient formulas in `compute_coeffs` for numerical stability.
    - [x] Ensure `AllPass` is correctly handled in all `BiquadFilterType` match arms (e.g., `result`, `complex_response`).
- [x] Task: Add high-level All-Pass helpers [d6ed25a]
    - [x] Implement a `peq_allpass` or similar helper if beneficial for PEQ chain construction.
- [x] Task: Integration Check [d6ed25a]
    - [x] Verify that the `AllPass` filter can be correctly used in the `crates/plugins` crossfeed implementation.
- [x] Task: Conductor - User Manual Verification 'Phase 2: Green Phase' (Protocol in workflow.md) [d6ed25a]

## Phase 3: Validation & Quality Gates [checkpoint: d6ed25a]
- [x] Task: Final Validation [d6ed25a]
    - [x] Run full test suite for `math-iir-fir`.
    - [x] Verify >80% code coverage for the new tests.
    - [x] Run `cargo clippy` and ensure zero warnings.
- [x] Task: Conductor - User Manual Verification 'Phase 3: Validation' (Protocol in workflow.md) [d6ed25a]
