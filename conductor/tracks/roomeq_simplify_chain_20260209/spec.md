# Track: RoomEQ Simplify Chain

## 1. Overview
The user requested simplification of the Stereo 2.1 DSP chain:
1.  **Remove Delay:** Do not output delay plugins (IIR-only context).
2.  **Remove Pre-EQ:** Pre-EQ was used for finding optimal crossover, but final chain should not include it. Post-EQ should be optimized on the original aligned curves (with crossover applied) to handle all correction.

## 2. Changes

### 2.1 Workflow (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_1`:
    *   Steps 1-5 remain (find `final_xo_freq`, gains, inversions) using linearization.
    *   **Step 6 (Apply Crossover):** Apply crossover filters + gains to `aligned_curves` (NOT `linearized_curves`).
        *   Ignore `xo_delays`.
    *   **Step 7 (Post-EQ):** Optimize on these new curves.
    *   **Step 8 (Output):** Build chain: `AlignGain` -> `Crossover` -> `MainGain` (with inversion) -> `PostEQ`.
        *   Remove `pre_eq_filters`.
        *   Remove `output::create_delay_plugin`.

## 3. Risks
*   Ignoring delay might degrade summation at crossover if drivers were time-misaligned. But user explicitly requested removal.
*   Post-EQ must do "heavy lifting" if drivers are non-flat. This is usually fine for room correction EQ.

## 4. Verification
*   Compile check.
*   Verify output chain structure logic.
