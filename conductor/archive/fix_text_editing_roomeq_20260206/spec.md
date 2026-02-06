# Specification: Fix direct text editing for Room EQ form values

## Overview
The `NumberInput` component in `gpui-ui-kit`, used by the Room EQ configuration forms, currently fails to reliably enter or maintain text editing mode. While increment/decrement buttons work, clicking the numeric value does not consistently activate the text input field, and keyboard interaction is often blocked or ignored.

## Functional Requirements
- **Reliable Edit Activation:** Clicking the numeric value in a `NumberInput` must immediately and reliably transform the field into a text input with a visible cursor.
- **Stable Focus Management:** The component must maintain its editing state even during re-renders of the parent view, as long as it retains focus.
- **Keyboard Interaction:**
    - Support standard numeric input (digits, decimal point, minus sign).
    - Correctly handle `Enter` to commit, `Escape` to cancel, and `Backspace`/`Delete` for editing.
    - Ensure key events are captured by the input and do not trigger global application shortcuts.
- **Focus Loss Behavior:** Clicking outside the input field while editing should automatically commit the current value and exit editing mode.

## Technical Changes
- **`gpui-ui-kit/src/number_input.rs`**:
    - Add `window.refresh()` to the `on_mouse_down` handler for single clicks to ensure immediate UI feedback.
    - Call `cx.stop_propagation()` in the `on_key_down` handler to prevent events from bubbling up to parent containers or global shortcut handlers.
    - Refactor the focus loss logic in `render` to be more robust, ensuring the editing state is only cleared when focus is truly lost after being established.
    - Add `window.blur()` when confirming or cancelling an edit via keyboard to release focus.
    - Implement a `focus_handle()` method to allow providing a stable `FocusHandle` from parent components, matching the pattern used in the `Input` component.
- **`gpui-ui-kit/src/autoeq/mod.rs`**:
    - Update `AutoEqForm` to utilize the new `focus_handle()` capability if necessary, ensuring each `NumberInput` has a stable identity for focus tracking.

## Acceptance Criteria
- [ ] Clicking on any numeric parameter in the Room EQ "Configure" step activates a text input.
- [ ] Typing a new value and pressing `Enter` updates the parameter and closes the text input.
- [ ] Pressing `Escape` or clicking away cancels the edit without changing the value.
- [ ] Numeric input is correctly validated and clamped to the parameter's min/max bounds.
- [ ] Typing in the input does not trigger unrelated app actions (e.g., play/pause).
