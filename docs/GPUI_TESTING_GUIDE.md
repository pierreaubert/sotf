# GPUI Application Testing Guide

This document outlines a comprehensive strategy for testing GPUI-based applications, covering state management, UI rendering, user interaction, and performance. It is designed to be used by AI agents and developers to implement robust testing suites.

## 1. Testing Strategy Overview

To ensure high quality and stability, we employ a multi-layered testing approach:

| Layer | Focus | Tools/Techniques |
|-------|-------|------------------|
| **Unit (Logic)** | State transitions, data binding, business logic | `#[test]`, Pure Rust structs |
| **Component (UI)** | Rendering, prop propagation, lifecycle hooks | `gpui::TestAppContext`, Builder verification |
| **Interaction** | Event handling, input validation, focus management | Simulated events, Action dispatch |
| **Visual (Golden)** | Pixel-perfect rendering, layout stability | Snapshot/Golden file comparison |
| **Integration** | Cross-component flows, full app state cycles | End-to-End scenarios |

## 2. Implementation Guide

### 2.1 State Management & Data Binding
**Goal:** Verify that the application state (`AppState`, Models) transitions correctly in response to updates, independent of the UI.

*   **Technique:** Test the underlying `Model<T>` or `struct` logic directly.
*   **Example:**
    ```rust
    #[test]
    fn test_volume_clamping() {
        let mut app_state = AppState::default();
        app_state.set_volume(1.5); // Should clamp to 1.0
        assert_eq!(app_state.volume, 1.0);
        
        app_state.set_volume(-0.5); // Should clamp to 0.0
        assert_eq!(app_state.volume, 0.0);
    }
    ```

### 2.2 Component Rendering & Props
**Goal:** Ensure components render the correct elements based on their configuration (props).

*   **Technique:** Use `gpui::TestAppContext` to instantiate views and inspect the view hierarchy.
*   **Key Aspects:**
    *   **Prop Propagation:** Verify builder methods (e.g., `.disabled(true)`) actually affect the internal state/rendering.
    *   **Conditional Rendering:** Check if elements appear/disappear based on state.
*   **Example:**
    ```rust
    #[gpui::test]
    async fn test_button_rendering(cx: &mut TestAppContext) {
        let window = cx.add_window(|cx| {
            Button::new("btn", "Click Me").variant(ButtonVariant::Primary)
        });
        
        // Inspect the window content to find the button and verify its style
        // (Pseudocode: GPUI testing utilities usually allow finding elements)
        window.update(cx, |view, _| {
            assert!(view.is_primary()); 
        });
    }
    ```

### 2.3 Event Handling & User Interaction
**Goal:** Verify that UI elements respond to user input (clicks, keys) and dispatch the correct actions.

*   **Technique:** Simulate events using `cx.dispatch_action` or specific handler invocation.
*   **Key Aspects:**
    *   **Action Dispatch:** Clicking a button should trigger a specific `Action`.
    *   **Input Validation:** Text inputs should reject invalid characters or format data.
    *   **Focus Management:** Tab navigation should move focus logically.
*   **Example:**
    ```rust
    #[gpui::test]
    async fn test_playback_toggle(cx: &mut TestAppContext) {
        let state = cx.new_model(|_| AppState::default());
        let view = cx.add_window(|cx| PlayerView::new(state.clone(), cx));
        
        // Simulate "Space" key or PlayPause action
        cx.dispatch_action(PlayPause);
        
        // Assert state changed
        state.read_with(cx, |s, _| {
            assert!(s.is_playing);
        });
    }
    ```

### 2.4 Lifecycle Hooks
**Goal:** Ensure initialization (`new`), updates, and cleanup (drop) occur as expected.

*   **Technique:** Use counters or flags in `Model` or `View` to track lifecycle events.
*   **Example:**
    *   Verify that `cx.observe` callbacks are triggered when the observed model updates.
    *   Verify that background tasks (timers) are cancelled when the view is dropped.

### 2.5 Visual Regression (Golden Tests)
**Goal:** Detect unintended visual changes.

*   **Technique:** Render a component/view to an image or a structured JSON representation (like `gpui-d3rs` does) and compare it against a "golden" master file.
*   **Tools:**
    *   **JSON Snapshots:** Good for checking structure/logic (e.g., checking calculated layout positions or graph data points).
    *   **Image Snapshots:** Good for checking actual pixels (colors, anti-aliasing).

### 2.6 Performance Metrics
**Goal:** Prevent performance regressions.

*   **Technique:** Measure execution time of critical update loops.
*   **Example:**
    *   Measure the time it takes for `PlayerView::render` to complete.
    *   Measure the time from "Action Dispatched" to "UI Updated".

## 3. Checklist for AI Implementation

When implementing tests for a GPUI feature, ensure the following are covered:

- [ ] **State Logic:** Is the business logic tested in isolation?
- [ ] **Render Output:** Does the component render correctly for all variants/states?
- [ ] **Interactivity:** Do clicks, hovers, and key presses trigger the right actions?
- [ ] **Edge Cases:** What happens with empty data, long strings, or rapid inputs?
- [ ] **Cross-Component:** Does interaction in Component A correctly update Component B?
- [ ] **Lifecycle:** Are resources cleaned up? Are subscriptions established?

## 4. Reference Implementations

*   **Component Structure:** See `gpui-ui-kit/tests/component_tests.rs`.
*   **Interaction Tests:** See `gpui-ui-kit/tests/interaction_tests.rs`.
*   **Golden Tests:** See `gpui-d3rs/tests/golden_tests.rs`.
*   **App Logic:** See `sotf-audio-player/app-gpui/src/ui.rs` (logic within `PlayerView`).

## 5. Next Steps

1.  **Refactor `PlayerView`:** Extract logic from `PlayerView` into a testable `PlayerModel` or `ViewModel` to make unit testing easier.
2.  **Add Test Helpers:** Create a `TestApp` wrapper in `tests/common` that sets up the `TestAppContext` with standard dependencies (Theme, Settings, etc.).
3.  **Implement Golden Tests:** Add a `tests/golden` directory for visual snapshots of complex components (e.g., Graphs, EQ curves).
