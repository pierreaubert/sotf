# Plan: RoomEQ Fix LFE Gain

## Phase 1: Update Workflow
- [ ] Modify `crates/autoeq/src/roomeq/workflows.rs`:
    - [ ] Update `ranges` construction in `optimize_stereo_2_1` to use `max_xo` for boundaries.

## Phase 2: Verify
- [ ] Compile.
