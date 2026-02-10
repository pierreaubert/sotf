# Plan: RoomEQ Cardioid Scenario

## Phase 1: Update Scenarios
- [ ] Modify `crates/autoeq-datagen/src/scenarios.rs`:
    - [ ] Rename `scenario_03_small_multi_sub`.
    - [ ] Add `scenario_03_small_stereo_2_2_cardioid`.
    - [ ] Update `all_scenarios` list.

## Phase 2: Update Config Gen
- [ ] Modify `crates/autoeq-datagen/src/roomeq_config_gen.rs`:
    - [ ] Add `CardioidConfig` imports.
    - [ ] Add logic to detect `sub_bottom`/`sub_top` and generate Cardioid config.

## Phase 3: Verify
- [ ] Compile.
