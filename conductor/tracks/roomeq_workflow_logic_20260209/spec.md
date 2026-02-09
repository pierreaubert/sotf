# Track: RoomEQ Workflow Logic: Stereo 2.0 and 2.1

## 1. Overview
This track implements specific optimization recipes for Stereo (2.0) and Stereo+Sub (2.1) systems as defined by the user. These recipes introduce strict sequencing of level alignment, pre-EQ, crossover optimization, and post-EQ.

## 2. Workflows

### 2.1 Stereo Case (No Subwoofer)
**Trigger:** `SystemModel::Stereo` AND `subwoofers` is None/Empty.

1.  **Level Measurement:** Calculate average SPL for Left and Right channels in range `[100Hz, 2kHz]`.
2.  **Normalization:** Identify the channel with the *lowest* average SPL. Normalize the *other* channel down (apply negative gain) so its average SPL matches the lowest one.
    *   *Constraint:* Do not boost the quiet channel; cut the loud one.
3.  **Optimization:** Find optimal EQ for Left and Right (independently) to match the target curve.

### 2.2 Stereo Case (With Subwoofer)
**Trigger:** `SystemModel::Stereo` AND `subwoofers` is Present.

1.  **Level Measurement:**
    *   Mains (L/R): Avg SPL in range `[min_xover_freq, 2kHz]`.
    *   Sub (LFE): Avg SPL in range `[20Hz, max_xover_freq]`.
2.  **Normalization:** Identify the channel (L, R, or LFE) with the *lowest* average SPL. Normalize the other two down to match this level.
3.  **Pre-EQ (Linearization):**
    *   Optimize EQ for L and R.
    *   *Constraint:* `min_freq` = `min_xover_freq`.
4.  **Crossover Optimization:**
    *   Create "Virtual Main" = Average(L, R).
    *   Optimize crossover between "Virtual Main" and "LFE".
    *   Output: Crossover Freq, Delays, Gains, Polarity.
5.  **Apply Crossover:**
    *   Apply HighPass + Delay + Gain to L and R.
    *   Apply LowPass + Delay + Gain to LFE.
    *   Result: `L_post`, `R_post`, `LFE_post`.
6.  **Post-EQ (Global):**
    *   Optimize EQ for `L_post` and `R_post`.
        *   *Constraint:* `min_freq` = `computed_xover_freq + 20Hz`.
    *   Optimize EQ for `LFE_post`.
        *   *Constraint:* `max_freq` = `computed_xover_freq - 20Hz`.

## 3. Implementation Details
*   **`optimize_room` Refactor:**
    *   Detect topology.
    *   Dispatch to `optimize_stereo_2_0` or `optimize_stereo_2_1` helper functions.
*   **Level Alignment:** New helper `align_channels_to_lowest(channels, frequency_ranges) -> gains`.
*   **Virtual Main:** New helper `average_curves(curves) -> Curve`.

## 4. Acceptance Criteria
*   **2.0:** L and R are level-matched to the quietest one before EQ. EQ is full range.
*   **2.1:** L, R, LFE are level-matched. Crossover is optimized between Avg(Mains) and Sub. Post-EQ respects frequency bounds relative to crossover point.
