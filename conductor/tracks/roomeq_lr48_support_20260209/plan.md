# Plan: RoomEQ LR48 Support

## Phase 1: Update Loss Logic
- [ ] Modify `crates/autoeq/src/loss.rs`:
    - [ ] Add `LinkwitzRiley8` to `CrossoverType`.
    - [ ] Update `build_crossover_filters_for_driver`.

## Phase 2: Update Parsing
- [ ] Modify `crates/autoeq/src/roomeq/crossover.rs`:
    - [ ] Update `parse_crossover_type`.
    - [ ] Update `crossover_type_to_string`.
    - [ ] Update tests.

## Phase 3: Verify
- [ ] Compile.
- [ ] Run tests.
