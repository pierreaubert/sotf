# Plan: RoomEQ Classical Source

## Phase 1: Refactor Directivity in math-xem-common
- [ ] Rename `DirectivityPattern` to `DirectivityGrid`.
- [ ] Create `enum Directivity`.
- [ ] Update `Source` to use `Directivity`.
- [ ] Implement `Directivity::amplitude`.

## Phase 2: Implement Classical Pattern
- [ ] Implement `Classical` logic in `amplitude` method with frequency dependence.

## Phase 3: Update DataGen
- [ ] Modify `crates/autoeq-datagen/src/scenarios.rs` to use `Source::classical` for mains.

## Phase 4: Verify
- [ ] Run tests.
