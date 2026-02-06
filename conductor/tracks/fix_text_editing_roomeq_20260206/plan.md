# Implementation Plan: Fix direct text editing for Room EQ form values

This plan addresses the unreliability of text editing in the `NumberInput` component and ensures consistent behavior across all interactive form elements.

## Phase 1: Component Audit and Robustness
- [x] Task: Audit all interactive components in `gpui-ui-kit` (`Select`, `Potentiometer`, `VerticalSlider`, `VolumeKnob`, `Slider`) for event propagation control and reliable focus acquisition. 7f8a1b2
- [x] Task: Implement `focus_handle()` builder in `NumberInput` to support external stable handles, matching the `Input` component pattern. 7f8a1b2
- [x] Task: Add `cx.stop_propagation()` to `on_key_down`, `on_mouse_down`, and `on_scroll_wheel` in `NumberInput` to prevent bubbling. 7f8a1b2
- [x] Task: Ensure `window.refresh()` is called on every state change (click, key down, focus loss) in `NumberInput`. 7f8a1b2
- [x] Task: Refactor focus loss handling in `NumberInput::render` to be more robust and prevent premature exit of editing mode. 7f8a1b2
- [x] Task: Add `window.blur()` on edit confirmation/cancellation in `NumberInput`. 7f8a1b2
- [x] Task: Conductor - User Manual Verification 'Component Audit and Robustness' (Protocol in workflow.md) 7f8a1b2

## Phase 2: Automated Regression Testing (TDD)
- [ ] Task: Write failing tests: Create a suite in `gpui-ui-kit/tests/number_input_tests.rs` that simulates:
    - Single click activating edit mode.
    - Keyboard input updating the internal text buffer.
    - `Enter` key triggering `on_change` with the parsed value.
    - `Escape` key cancelling the edit.
- [ ] Task: **Verify "Red" Phase**: Confirm that these tests fail on the current implementation.
- [ ] Task: Implement fixes to pass all tests ("Green" Phase).
- [ ] Task: Conductor - User Manual Verification 'Automated Regression Testing' (Protocol in workflow.md)

## Phase 3: Form Integration and Global Verification
- [ ] Task: Update `AutoEqForm` to ensure it passes unique, stable identities or handles to its child inputs.
- [ ] Task: Verify in `app-gpui` that typing in Room EQ fields (e.g., Min/Max Freq) does not trigger global shortcuts like play/pause.
- [ ] Task: Conductor - User Manual Verification 'Form Integration and Global Verification' (Protocol in workflow.md)

## Phase 4: Final Verification
- [ ] Task: Perform a final end-to-end check of all numeric fields in the Room EQ "Configure" step one by one.
- [ ] Task: Conductor - User Manual Verification 'Final Verification' (Protocol in workflow.md)
