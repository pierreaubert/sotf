# Implementation Plan: Fix Room EQ Components and Plots in `app-gpui`

This plan addresses the misalignment between the GPUI Room EQ interface and the `autoeq/roomeq` backend, ensuring forms are functional and plots are normalized.

## Phase 1: Investigation and Data Mapping [checkpoint: 786a02f]
- [x] Task: Audit `autoeq` backend for required Room EQ parameters. 7f8a1b2
- [x] Task: Identify missing or incorrect fields in the current `app-gpui` Room EQ components. 7f8a1b2
- [x] Task: Locate the plot rendering logic and normalization points in `app-gpui/ui/screens/room_eq`. 7f8a1b2
- [x] Task: Conductor - User Manual Verification 'Investigation and Data Mapping' (Protocol in workflow.md) 786a02f

## Phase 2: Form Alignment and Validation [checkpoint: fb52e6e]
- [x] Task: Update Room EQ form models to include missing parameters (e.g., target curve, freq range, gain limits). 7f8a1b2
- [x] Task: Implement validation logic for form inputs based on backend constraints. 7f8a1b2
- [x] Task: Write Tests: Verify form data serialization matches `roomeq` input format. fb52e6e
- [x] Task: Implement: Refactor Room EQ UI components to display the updated form fields. fb52e6e
- [x] Task: Conductor - User Manual Verification 'Form Alignment and Validation' (Protocol in workflow.md) fb52e6e

## Phase 3: Plot Normalization and Visualization [checkpoint: fa8d093]
- [x] Task: Write Tests: Implement unit tests for a normalization utility function that aligns SPL and Target curves. fb52e6e
- [x] Task: Implement: Create/update a utility to calculate the offset needed to normalize SPL data to 0dB relative to the target. fb52e6e
- [x] Task: Implement: Apply normalization logic to the `RoomEq` plot components. fb52e6e
- [x] Task: Verify: Ensure the Target curve and SPL response are visually aligned in the UI. fb52e6e
- [x] Task: Conductor - User Manual Verification 'Plot Normalization and Visualization' (Protocol in workflow.md) fa8d093

## Phase 4: Integration and Final Verification
- [ ] Task: Perform end-to-end test of the Room EQ flow (Form Input -> Backend Call -> Plot Result).
- [ ] Task: Verify that all acceptance criteria in `spec.md` are met.
- [ ] Task: Conductor - User Manual Verification 'Integration and Final Verification' (Protocol in workflow.md)
