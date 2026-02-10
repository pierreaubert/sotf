# Plan: RoomEQ Simplify Chain

## Phase 1: Update Workflow
- [ ] Modify `crates/autoeq/src/roomeq/workflows.rs`:
    - [ ] Update Step 6 to apply crossover to `aligned_curves` (ignoring delay).
    - [ ] Update Step 8 to construct simplified chain (no Pre-EQ, no Delay).

## Phase 2: Verify
- [ ] Compile.
- [ ] Run tests.
