# Plan: RoomEQ System Configuration Refactor

## Phase 1: Schema Definition
- [ ] Define `SystemModel`, `SubwooferConfig`, `SystemConfig` structs in `roomeq/types.rs`.
- [ ] Add `system` field to `RoomConfig`.
- [ ] Verify serialization/deserialization with unit tests.

## Phase 2: Logic Implementation
- [ ] Update `optimize_room` in `roomeq/optimize.rs` to use `SystemConfig` if present.
    - [ ] Resolve logical channels from `system.speakers` mapping.
    - [ ] Implement subwoofer strategy selection based on `system.subwoofers.config`.
    - [ ] Implement subwoofer-to-main pairing logic for alignment.

## Phase 3: Ecosystem Update & Verification
- [ ] Update `autoeq-datagen` to optionally produce this new config format for testing.
- [ ] Create a regression test for a 2.1 system using the new schema.
