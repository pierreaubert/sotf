# Plan: RoomEQ Docs & Schema Update

## Phase 1: Schema Update
- [ ] Update `crates/autoeq/bin/roomeq/input_schema.json`.
    - [ ] Add `system` definition.
    - [ ] Handle `subwoofers` flattened map schema.

## Phase 2: Documentation Update
- [ ] Update `crates/autoeq/bin/roomeq/INPUT_FORMAT.md`.
    - [ ] Add System Configuration section.
    - [ ] Add 2.1 example.
- [ ] Check/Update `crates/autoeq/bin/roomeq/README.md`.

## Phase 3: Verification
- [ ] Validate the 2.1 example from `INPUT_FORMAT.md` against the new schema using a temporary test script or online validator logic simulation.
