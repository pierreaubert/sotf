# Implementation Plan: Enhanced Plugin Test Infrastructure

## Phase 1: Internal Test Infrastructure (crates/plugins) [checkpoint: a7ceb28]
- [x] Task: Create `crates/plugins/src/test_utils.rs` 8a96084
    - [x] Create `test_utils` module (as `cfg(test)`)
    - [x] Define shared `SignalGenerator` and `BufferComparison` utilities
- [x] Task: Signal & IO Utilities (TDD) f05e008
    - [x] Write tests for sine, noise, and impulse generators
    - [x] Implement `SignalGen` module
    - [x] Write tests for RMS and bit-accurate comparison
    - [x] Implement `BufferComparison` utilities
- [x] Task: Conductor - User Manual Verification 'Internal Test Infrastructure' (Protocol in workflow.md) a7ceb28

## Phase 2: RT Safety & QA Refactoring [checkpoint: 32b31e6]
- [x] Task: RT Safety Enforcement (TDD) 4731c44
    - [x] Write a test that fails when a heap allocation occurs in `crates/plugins`
    - [x] Integrate `assert_no_alloc` or similar into the standard `cargo test` suite
- [x] Task: Refactor QA Suites (TDD) f727666
    - [x] Identify and refactor common validation patterns from `qa-*.rs` files
    - [x] Migrate `qa-*.rs` logic into unified testing utilities within `crates/plugins`
- [x] Task: Conductor - User Manual Verification 'RT Safety & QA Refactoring' (Protocol in workflow.md) 32b31e6

## Phase 3: Benchmarking & Performance Refinement [checkpoint: a80369b]
- [x] Task: Benchmark Refactoring (TDD) 6574a33
    - [x] Refactor existing benchmarks in `crates/plugins/benches` to use unified profiling utilities
    - [x] Add automated latency (PDL) detection to benchmarks
- [x] Task: Performance & Profiling (TDD) 926f7a6
    - [x] Implement performance report generator for plugins
- [x] Task: Conductor - User Manual Verification 'Benchmarking & Performance Refinement' (Protocol in workflow.md) a80369b

## Phase 4: Integration & Magic Number Audit [checkpoint: a50edf5]
- [x] Task: Compressor Audit & Migration (TDD) 68b4197
    - [x] Refactor `Compressor` unit tests to use `test_utils`
    - [x] Audit `crates/plugins/src/dynamics/compressor.rs` for magic numbers and complex loop edge cases
    - [x] Replace identified magic numbers with constants or derivations
- [x] Task: PEQ Audit & Migration (TDD) 68857c9
    - [x] Refactor `PEQ` unit tests to use `test_utils`
    - [x] Audit `crates/plugins/src/eq/peq.rs` for magic numbers and complex loop edge cases
    - [x] Replace identified magic numbers with constants or derivations
- [x] Task: Parameter Automation Tests (TDD) d6b9a0c
    - [x] Implement automated parameter ramp tests for `Compressor` and `PEQ`
    - [x] Verify artifact-free transitions and smoothing
- [x] Task: Conductor - User Manual Verification 'Integration & Magic Number Audit' (Protocol in workflow.md) a50edf5

## Phase: Review Fixes
- [x] Task: Apply review suggestions ffc7dc1