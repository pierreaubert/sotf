# Specification: Add AllPass Filter to math-iir-fir

## Overview
This track adds support for a second-order All-Pass biquad filter to the `math-iir-fir` crate. All-Pass filters are essential for phase manipulation without affecting the magnitude response, which is useful in applications like headphone crossfeed and phase alignment in speaker systems.

## Problem Statement
The `math-iir-fir` crate provides several standard biquad filter types (Low-pass, High-pass, Peak, etc.) but lacks an implementation for the All-Pass filter. This prevents the implementation of features like the Bauer crossfeed which relies on All-Pass sections for phase-accurate signal mixing.

## Functional Requirements
- **Coefficient Calculation:** Implement the standard coefficient formulas for a second-order All-Pass biquad filter.
- **Integration:** Add `AllPass` as a new variant to the `BiquadFilterType` enum.
- **Parameterization:** Support Center Frequency (Hz), Q-factor, and Sample Rate (Hz).
- **Processing:** Ensure the existing `process` and `process_block` methods correctly handle All-Pass coefficients.
- **Frequency Response:** Ensure `complex_response` and `result` (magnitude) correctly reflect the All-Pass characteristic (unity magnitude across all frequencies).

## Non-Functional Requirements
- **Performance:** Maintain the same level of performance as existing biquad filters.
- **Numerical Stability:** Ensure the filter remains stable across the full frequency range (up to Nyquist).
- **Zero-Allocation:** Coefficient recalculation must not involve heap allocations.

## Acceptance Criteria
- `BiquadFilterType::AllPass` exists and is documented.
- A unit test verifies that the magnitude response of an All-Pass filter is 0 dB (unity gain) across the audible spectrum.
- A unit test verifies the expected phase shift at the center frequency (exactly 180 degrees for a second-order All-Pass).
- The `math-iir-fir` crate compiles without warnings.

## Out of Scope
- First-order All-Pass filters (unless determined to be a trivial addition).
- UI implementation for the All-Pass filter.
- Integration into specific plugins (like Crossfeed) within this track.
