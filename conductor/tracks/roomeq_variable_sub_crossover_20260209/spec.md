# Track: RoomEQ Variable Sub Crossover

## 1. Overview
Allow `system.subwoofers` crossover configuration to specify a `frequency_range` instead of a fixed `frequency`. The `optimize_stereo_2_1` workflow must adapt to optimize the crossover frequency within this range.

## 2. Changes

### 2.1 Workflow (`crates/autoeq/src/roomeq/workflows.rs`)
*   Update `optimize_stereo_2_1`:
    *   Logic to handle `xover_config.frequency_range`.
    *   Calculate `est_xo` (geometric mean or fixed) for initial Alignment/Pre-EQ.
    *   Set Pre-EQ `min_freq` to `min_xo`.
    *   Pass range to `optimize_crossover`.
    *   Use optimized frequency result for Post-EQ setup.

## 3. Risks
*   Pre-EQ might correct too low if `min_xo` is very low (e.g. 40Hz). But `optimize_channel_eq` handles this.
*   Alignment might be slightly off if `est_xo` is far from optimal. But level alignment is robust.

## 4. Verification
*   Create a test case with a frequency range (e.g. 60-100Hz) and verify it runs and picks a frequency.
