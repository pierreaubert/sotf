# Plan: Add AllPass Filter to math-iir-fir

## Phase 1: Red Phase (Verification & Testing)
- [ ] Task: Write failing unit tests for All-Pass filter behavior
    - [ ] Create `test_allpass_response` to verify magnitude is exactly 0 dB (1.0 linear) across the spectrum.
    - [ ] Create `test_allpass_phase` to verify phase shift is 180 degrees at the center frequency.
    - [ ] Add tests for All-Pass parameter validation in `try_new`.
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Red Phase' (Protocol in workflow.md)

## Phase 2: Green Phase (Implementation & Integration)
- [ ] Task: Verify and Refine Biquad All-Pass implementation
    - [ ] Double-check RBJ coefficient formulas in `compute_coeffs` for numerical stability.
    - [ ] Ensure `AllPass` is correctly handled in all `BiquadFilterType` match arms (e.g., `result`, `complex_response`).
- [ ] Task: Add high-level All-Pass helpers
    - [ ] Implement a `peq_allpass` or similar helper if beneficial for PEQ chain construction.
- [ ] Task: Integration Check
    - [ ] Verify that the `AllPass` filter can be correctly used in the `crates/plugins` crossfeed implementation.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Green Phase' (Protocol in workflow.md)

## Phase 3: Validation & Quality Gates
- [ ] Task: Final Validation
    - [ ] Run full test suite for `math-iir-fir`.
    - [ ] Verify >80% code coverage for the new tests.
    - [ ] Run `cargo clippy` and ensure zero warnings.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Validation' (Protocol in workflow.md)
