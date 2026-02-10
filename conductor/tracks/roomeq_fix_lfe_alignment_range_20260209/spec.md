# Track: RoomEQ Fix LFE Alignment Range

## 1. Overview
User reports LFE level is too low (25dB vs 35dB) in 2.1 optimization.
This is likely due to `align_channels_to_lowest` using a wide range (20Hz-MaxXo) for LFE, which includes deep bass peaks (room modes) that inflate the average level, causing the algorithm to apply excessive attenuation.
The fix is to narrow the LFE alignment range to the octave below the crossover (`MaxXo * 0.5` to `MaxXo`), ensuring we align the levels *at the crossover* where summation matters most.

## 2. Changes

### 2.1 Workflow (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_1`:
    *   Change LFE range in `ranges` map: `(20.0, max_xo)` -> `(max_xo * 0.5, max_xo)`.
    *   (Optional) Check if `max_xo * 0.5` < 20.0? Unlikely for typical subs, but `max` with 20.0 is safe.

## 3. Risks
*   If the sub has a dip in the crossover region, we might BOOST it too much. But `optimize_crossover` should handle integration. Better to be level-matched at crossover than mismatched.

## 4. Verification
*   Compile check.
