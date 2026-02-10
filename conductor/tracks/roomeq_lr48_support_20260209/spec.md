# Track: RoomEQ LR48 Support

## 1. Overview
Add `LinkwitzRiley8` (LR48) support to the `CrossoverType` enum and related parsing/generation functions.

## 2. Changes

### 2.1 Loss Logic (`crates/autoeq/src/loss.rs`)
*   Update `CrossoverType` enum: Add `LinkwitzRiley8`.
*   Update `build_crossover_filters_for_driver`: Handle `LinkwitzRiley8` case (order 8).

### 2.2 Crossover Parsing (`crates/autoeq/src/roomeq/crossover.rs`)
*   Update `parse_crossover_type`: Map "lr48", "lr8", "linkwitzriley48", "linkwitzriley8" to `LinkwitzRiley8`.
*   Update `crossover_type_to_string`: Handle `LinkwitzRiley8` -> "LR48".

## 3. Verification
*   Compile check.
*   Unit tests in `crossover.rs`.
