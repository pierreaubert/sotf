# Plan: RoomEQ Output Curves & Phase

## Phase 1: Update Types
- [ ] Modify `crates/autoeq/src/roomeq/types.rs`:
    - [ ] Add `phase` to `CurveData`.
    - [ ] Update `From` implementations.

## Phase 2: Update Workflows
- [ ] Modify `crates/autoeq/src/roomeq/workflows.rs`:
    - [ ] In `optimize_stereo_2_0`, compute and set `final_curve`.
    - [ ] In `optimize_stereo_2_1`, compute and set `final_curve` for all channels.

## Phase 3: Verify
- [ ] Compile.
- [ ] Check `optimize.rs` to ensure it propagates phase (it uses `CurveData::from(&final_curve)` so it should be automatic once `From` is updated).
