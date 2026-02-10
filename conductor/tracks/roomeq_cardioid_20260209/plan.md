# Plan: RoomEQ Cardioid Subwoofer Support

## Phase 1: Update Types
- [ ] Modify `crates/autoeq/src/roomeq/types.rs`:
    - [ ] Add `CardioidConfig`.
    - [ ] Add `Cardioid` to `SpeakerConfig`.

## Phase 2: Update Output Logic
- [ ] Modify `crates/autoeq/src/roomeq/output.rs`:
    - [ ] Add `build_cardioid_dsp_chain`.

## Phase 3: Update Optimizer Logic
- [ ] Modify `crates/autoeq/src/roomeq/optimize.rs`:
    - [ ] Implement `process_cardioid`.
    - [ ] Hook into `process_speaker_internal`.

## Phase 4: Schema & Docs
- [ ] Update `input_schema.json`.
- [ ] Update `INPUT_FORMAT.md`.

## Phase 5: Verify
- [ ] Compile.
