# Specification: Enhanced Plugin Test Infrastructure

## Overview
This track focuses on strengthening the testing, benchmarking, and quality assurance infrastructure for SOTF audio plugins. Instead of a separate crate, a unified test harness will be integrated directly into `crates/plugins`. This involves refactoring existing `qa-*.rs` scripts, benchmarks, and integration tests to provide a high-confidence environment that enforces real-time safety, validates complex loop logic, and eliminates undocumented "magic numbers."

## Functional Requirements
- **Unified Internal Test Utilities:** Create a `test_utils` module within `crates/plugins` providing:
  - Signal generators (sine, white noise, impulse, step).
  - Bit-accurate and RMS-based buffer comparison tools.
  - Utilities for systematic testing of varied buffer sizes and channel counts.
- **RT Safety Enforcement:** Integrate allocation and lock detection (e.g., via `assert_no_alloc`) into the standard test suite to ensure plugins remain real-time safe.
- **QA & Benchmark Refactoring:**
  - Standardize existing `qa-*.rs` validation logic to use shared utilities.
  - Refactor `crates/plugins/benches` to provide consistent performance reporting and automated latency (PDL) detection.
- **Parameter Automation & Smoothing:** Provide helpers to test sample-accurate parameter ramps and verify artifact-free transitions.
- **Magic Number & Logic Audit:**
  - Systematically identify and replace "magic numbers" in core plugins (Compressor, PEQ) with named constants or documented mathematical derivations.
  - Focus on rigorous testing of complex conditional logic in processing loops.

## Non-Functional Requirements
- **Confidence:** Achieve high code coverage (>80%) for core DSP logic.
- **Maintainability:** Ensure that adding new plugins requires minimal boilerplate for comprehensive testing.
- **CI Integration:** All tests and benchmarks must be compatible with `cargo test` and `cargo bench`.

## Acceptance Criteria
- [ ] `crates/plugins/src/test_utils.rs` (or similar) is implemented and used by tests.
- [ ] Existing `qa-*.rs` tests are refactored to use the new shared validation logic.
- [ ] RT safety violations are detectable within the standard test suite.
- [ ] Core plugins (Compressor, PEQ) are audited, magic numbers are removed, and loop logic is verified against varied buffer configurations.
- [ ] Performance benchmarks report block processing time and internal latency.

## Out of Scope
- Creating new top-level crates.
- Testing non-DSP related application logic.