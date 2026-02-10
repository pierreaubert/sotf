# Plan: RoomEQ Crossover Refactor

## Phase 1: Refactor Types
- [ ] Modify `crates/autoeq/src/roomeq/types.rs`:
    - [ ] Add `crossover` to `SubwooferSystemConfig`.
    - [ ] Remove `bass_management` from `RoomConfig`.
    - [ ] Remove `BassManagementConfig` struct.

## Phase 2: Update Workflows
- [ ] Modify `crates/autoeq/src/roomeq/workflows.rs`:
    - [ ] Update `optimize_stereo_2_1` to resolve crossover from `config.crossovers`.

## Phase 3: Cleanup Usages
- [ ] Fix `convert_recording.rs`.
- [ ] Fix tests (`workflow_test.rs`, `system_config_test.rs`).
- [ ] Fix other usages found via grep (e.g. `roomeq_config_gen.rs`, `multi_speaker.rs`).

## Phase 4: Schema & Docs
- [ ] Update `input_schema.json`.
- [ ] Update `INPUT_FORMAT.md`.
- [ ] Update `README.md` files.

## Phase 5: Verify
- [ ] Run validation script.
- [ ] Run tests.
