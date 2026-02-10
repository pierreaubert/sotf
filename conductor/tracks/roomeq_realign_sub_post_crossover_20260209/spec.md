# Track: RoomEQ Realign Sub Post Crossover

## 1. Overview
The user experienced a bug where LFE level was too low.
Level alignment before crossover application is good, but applying the Low-Pass filter significantly reduces the broadband energy of the subwoofer, potentially dropping its effective level below the mains (which are High-Passed but might retain more energy or were aligned differently).
The fix is to re-measure the average levels of the **filtered** curves (Mains HP vs Sub LP) and apply a correction gain to the subwoofer to match the mains.

## 2. Changes

### 2.1 Workflow (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_1`:
    *   After Step 6 (Apply Crossover to `l_post`, `r_post`, `sub_post`):
        *   Calculate `mean_main = avg(l_post)`.
        *   Calculate `mean_sub = avg(sub_post)`.
        *   `diff = mean_main - mean_sub`.
        *   `sub_gain_post += diff`.
        *   Update `sub_post` by adding `diff` to SPL.
        *   Proceed to Post-EQ.

## 3. Risks
*   Modifying gain after crossover optimization might affect the summation slightly (magnitude changes, phase is constant for gain). Since we are just matching levels, summation should actually *improve* or stay valid (crossover relies on level matching).

## 4. Verification
*   Compile check.
