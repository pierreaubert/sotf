# Implementation Plan: Enhanced Plugin Test Infrastructure

## Phase 1: Internal Test Infrastructure (crates/plugins)
- [x] Task: Create `crates/plugins/src/test_utils.rs` 8a96084
    - [x] Create `test_utils` module (as `cfg(test)`)
    - [x] Define shared `SignalGenerator` and `BufferComparison` utilities
- [x] Task: Signal & IO Utilities (TDD) f05e008
    - [x] Write tests for sine, noise, and impulse generators
    - [x] Implement `SignalGen` module
    - [x] Write tests for RMS and bit-accurate comparison
    - [x] Implement `BufferComparison` utilities
- [ ] Task: Conductor - User Manual Verification 'Internal Test Infrastructure' (Protocol in workflow.md)

## Phase 2: RT Safety & QA Refactoring
- [ ] Task: RT Safety Enforcement (TDD)
    - [ ] Write a test that fails when a heap allocation occurs in `crates/plugins`
    - [ ] Integrate `assert_no_alloc` or similar into the standard `cargo test` suite
- [ ] Task: Refactor QA Suites (TDD)
    - [ ] Identify and refactor common validation patterns from `qa-*.rs` files
    - [ ] Migrate `qa-*.rs` logic into unified testing utilities within `crates/plugins`
- [ ] Task: Conductor - User Manual Verification 'RT Safety & QA Refactoring' (Protocol in workflow.md)

## Phase 3: Benchmarking & Performance Refinement
- [ ] Task: Benchmark Refactoring (TDD)
    - [ ] Refactor existing benchmarks in `crates/plugins/benches` to use unified profiling utilities
    - [ ] Add automated latency (PDL) detection to benchmarks
- [ ] Task: Performance & Profiling (TDD)
    - [ ] Implement performance report generator for plugins
- [ ] Task: Conductor - User Manual Verification 'Benchmarking & Performance Refinement' (Protocol in workflow.md)

## Phase 4: Integration & Magic Number Audit
- [ ] Task: Compressor Audit & Migration (TDD)
    - [ ] Refactor `Compressor` unit tests to use `test_utils`
    - [ ] Audit `crates/plugins/src/dynamics/compressor.rs` for magic numbers and complex loop edge cases
    - [ ] Replace identified magic numbers with constants or derivations
- [ ] Task: PEQ Audit & Migration (TDD)
    - [ ] Refactor `PEQ` unit tests to use `test_utils`
    - [ ] Audit `crates/plugins/src/eq/peq.rs` for magic numbers and complex loop edge cases
    - [ ] Replace identified magic numbers with constants or derivations
- [ ] Task: Parameter Automation Tests (TDD)
    - [ ] Implement automated parameter ramp tests for `Compressor` and `PEQ`
    - [ ] Verify artifact-free transitions and smoothing
- [ ] Task: Conductor - User Manual Verification 'Integration & Magic Number Audit' (Protocol in workflow.md)