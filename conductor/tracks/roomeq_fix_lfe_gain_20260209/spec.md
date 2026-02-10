# Track: RoomEQ Fix LFE Gain

## 1. Overview
The user identified a bug in LFE gain calculation. The current normalization range uses `est_xo` (geometric mean of crossover range). The user implies it should cover up to `max range crossover` (or at least implies the current way is buggy).
Using `est_xo` might exclude significant subwoofer energy if the crossover range is wide. Using `max_xo` ensures we capture the full potential passband of the subwoofer during level estimation.

## 2. Changes

### 2.1 Workflow (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_1`:
    *   Change LFE normalization range upper bound from `est_xo` to `max_xo` (upper bound of crossover range).
    *   Ideally, Mains range should also start from `min_xo` or `est_xo`?
        *   If we measure Mains starting at `min_xo`, we might include roll-off energy if they can't handle it.
        *   If we measure Mains starting at `max_xo`, we ensure we measure their passband.
        *   Aligning `Sub(20-MaxXO)` to `Main(MaxXO-2k)` seems safer?
        *   Or stick to `est_xo` for Mains?
    *   Decision: Update LFE range to `(20.0, max_xo)`. Keep Mains at `(est_xo, 2000.0)` or change to `(max_xo, 2000.0)`?
        *   If Mains roll off at 80Hz and we measure from 40Hz (`est_xo` of 20-150 range?), the mean will be low -> Gain boost -> Bad.
        *   So Mains should be measured in their **safe** passband. `max_xo` is the safest lower bound for Mains.
        *   Sub should be measured in its **full** passband. `max_xo` is the upper bound of its usage.
    *   **New Strategy:**
        *   LFE: `20.0` to `max_xo`
        *   Mains: `max_xo` to `2000.0`
        *   This ensures we compare the "solid" operating regions of both.

## 3. Risks
*   If `max_xo` is very high (e.g. 200Hz) and sub struggles there, mean drops -> Boost. But `max_xo` comes from user config, so it should be valid.
*   If `max_xo` is high and mains are capable lower, we just ignore some main energy. That's fine for level setting.

## 4. Verification
*   Compile check.
