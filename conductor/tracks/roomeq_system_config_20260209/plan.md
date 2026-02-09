# Plan: RoomEQ System Configuration Refactor

## Phase 1: Schema Definition
- [x] Define `SystemModel`, `SubwooferConfig`, `SystemConfig` structs in `roomeq/types.rs`.
- [x] Add `system` field to `RoomConfig`.
- [x] Verify serialization/deserialization with unit tests.

## Phase 2: Logic Implementation
- [x] Update `optimize_room` in `roomeq/optimize.rs` to use `SystemConfig` if present.
    - [x] Resolve logical channels from `system.speakers` mapping.
    - [x] Implement subwoofer strategy selection based on `system.subwoofers.config`.
    - [x] Implement subwoofer-to-main pairing logic for alignment.

## Phase 3: Ecosystem Update & Verification
- [x] Update `autoeq-datagen` to optionally produce this new config format for testing.
- [x] Create a regression test for a 2.1 system using the new schema.
