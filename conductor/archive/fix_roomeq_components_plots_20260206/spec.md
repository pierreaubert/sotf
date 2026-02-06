# Specification: Fix Room EQ Components and Plots in `app-gpui`

## Overview
The current implementation of the Room EQ interface in `app-gpui` contains functional discrepancies:
1. **Form Misalignment:** Input forms for Room EQ are missing critical parameters or contain non-useful fields that do not align with the `autoeq/roomeq` backend expectations.
2. **Plot Inaccuracy:** Visualization plots are not normalized, leading to a visual mismatch where the Target curve sits at 0dB while the SPL (Sound Pressure Level) response resides between -20dB and -30dB.

## Functional Requirements
- **Form Refactoring:**
    - Update `room_eq` forms to include all necessary parameters required by the `roomeq` solver (e.g., target curve selection, frequency range, gain limits, smoothing).
    - Ensure form validation logic matches the constraints and formats expected by the `autoeq` backend.
- **Plot Normalization:**
    - Implement automatic normalization of measured SPL data in the UI plots.
    - Align the measured response and the target curve visually (typically centered around 0dB) to allow for meaningful comparison.

## Non-Functional Requirements
- **Consistency:** Form layout and interaction should remain consistent with the established `gpui-ui-kit` style.
- **Performance:** Normalization calculations should be efficient to ensure smooth UI transitions and real-time plot updates.

## Acceptance Criteria
- [ ] Room EQ forms display all fields necessary for a successful optimization run.
- [ ] Forms prevent submission of data that doesn't conform to `autoeq` requirements.
- [ ] Optimization plots show the SPL response and Target curve visually overlapping/aligned near 0dB.
- [ ] Verification that the optimization parameters sent to the backend are correct.

## Out of Scope
- Major architectural changes to the `autoeq` or `roomeq` solvers.
- Redesign of the entire Room EQ workflow beyond these specific components.
