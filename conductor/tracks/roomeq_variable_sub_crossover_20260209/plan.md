# Plan: RoomEQ Variable Sub Crossover

## Phase 1: Update Workflow
- [ ] Modify `crates/autoeq/src/roomeq/workflows.rs`:
    - [ ] Update config resolution to handle `frequency_range`.
    - [ ] Update `optimize_crossover` call parameters.
    - [ ] Use optimized frequency for downstream steps.

## Phase 2: Verify
- [ ] Add unit test with frequency range in `crates/autoeq/tests/workflow_test.rs`.
- [ ] Run `cargo test -p autoeq`.
