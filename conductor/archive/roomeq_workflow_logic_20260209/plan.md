# Plan: RoomEQ Workflow Logic

## Phase 1: Logic & Helpers
- [x] Implement `align_levels_to_lowest` in `roomeq/optimize.rs`.
    - [x] Takes map of `channel -> curve` and map of `channel -> (min_f, max_f)`.
    - [x] Returns map of `channel -> gain_db` (negative only).
- [x] Implement `average_curves` in `roomeq/optimize.rs` or `types.rs`.
- [x] Implement `optimize_stereo_2_0` workflow.
- [x] Implement `optimize_stereo_2_1` workflow.

## Phase 2: Integration
- [x] Update `optimize_room` to detect system topology (Stereo 2.0 vs 2.1) and call the new workflows.
- [x] Retain legacy loop for `Custom` / unknown topologies.

## Phase 3: Verification
- [x] Create test case for 2.0 level alignment (verify gains are negative and match lowest).
- [x] Create test case for 2.1 pipeline (verify crossover optimization and post-EQ bounds).
