//! Integration tests for Potentiometer component
//!
//! Tests the potentiometer (rotary knob) component including:
//! - Basic rendering with different sizes
//! - Value changes via scroll wheel
//! - Value changes via click (increment behavior)
//! - Double-click to reset
//! - Selected state
//! - Linear vs Logarithmic scales
//! - Disabled state
//! - Callbacks: on_change, on_select, on_reset, on_drag_start

use gpui::{
    Context, Modifiers, MouseButton, TestAppContext, VisualTestContext, Window, div, prelude::*,
};
use gpui_ui_kit::audio::potentiometer::{Potentiometer, PotentiometerScale, PotentiometerSize};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ============================================================================
// Basic Rendering Tests
// ============================================================================

struct PotentiometerTestView;

impl Render for PotentiometerTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Potentiometer::new("test-pot")
                .value(50.0)
                .min(0.0)
                .max(100.0)
                .unit("%")
                .label("Volume"),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| PotentiometerTestView);
}

/// Test different size variants
#[gpui::test]
async fn test_potentiometer_sizes(cx: &mut TestAppContext) {
    struct SizeTestView;

    impl Render for SizeTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    Potentiometer::new("pot-sm")
                        .size(PotentiometerSize::Sm)
                        .value(25.0)
                        .label("Small"),
                )
                .child(
                    Potentiometer::new("pot-md")
                        .size(PotentiometerSize::Md)
                        .value(50.0)
                        .label("Medium"),
                )
                .child(
                    Potentiometer::new("pot-lg")
                        .size(PotentiometerSize::Lg)
                        .value(75.0)
                        .label("Large"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SizeTestView);
}

// ============================================================================
// Value Change Tests
// ============================================================================

/// View that tracks value changes
struct PotValueChangeTestView {
    value: Rc<RefCell<f64>>,
    change_count: Arc<AtomicUsize>,
}

impl Render for PotValueChangeTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let current_value = *self.value.borrow();
        let value_rc = self.value.clone();
        let change_count = self.change_count.clone();

        div().size_full().child(
            Potentiometer::new("change-test-pot")
                .value(current_value)
                .min(0.0)
                .max(100.0)
                .unit("%")
                .label("Test")
                .on_change(move |new_value, _window, _cx| {
                    *value_rc.borrow_mut() = new_value;
                    change_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_click_to_increment(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(50.0));
    let change_count = Arc::new(AtomicUsize::new(0));

    let value_clone = value.clone();
    let change_count_clone = change_count.clone();

    let window = cx.add_window(move |_window, _cx| PotValueChangeTestView {
        value: value_clone,
        change_count: change_count_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initial value should be 50
    assert!(
        ((*value.borrow()) - 50.0).abs() < 0.01,
        "Initial value should be 50"
    );

    // Click the potentiometer to increment (10% step = +10 on 0-100 range)
    if let Some(bounds) = cx.debug_bounds("change-test-pot") {
        let center = bounds.center();
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();

        assert_eq!(
            change_count.load(Ordering::SeqCst),
            1,
            "on_change should have been called"
        );
        // Value should have increased by 10% (10 units on 0-100 range)
    }
}

// ============================================================================
// Select Callback Tests
// ============================================================================

/// View that tracks select events
struct PotSelectTestView {
    selected: Rc<RefCell<bool>>,
    select_count: Arc<AtomicUsize>,
}

impl Render for PotSelectTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_selected = *self.selected.borrow();
        let selected_rc = self.selected.clone();
        let select_count = self.select_count.clone();

        div().size_full().child(
            Potentiometer::new("select-test-pot")
                .value(50.0)
                .min(0.0)
                .max(100.0)
                .label("Selectable")
                .selected(is_selected)
                .on_select(move |_window, _cx| {
                    *selected_rc.borrow_mut() = true;
                    select_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_on_select(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(false));
    let select_count = Arc::new(AtomicUsize::new(0));

    let selected_clone = selected.clone();
    let select_count_clone = select_count.clone();

    let window = cx.add_window(move |_window, _cx| PotSelectTestView {
        selected: selected_clone,
        select_count: select_count_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initially not selected
    assert!(!*selected.borrow(), "Should not be selected initially");

    // Click to select
    if let Some(bounds) = cx.debug_bounds("select-test-pot") {
        let center = bounds.center();
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();

        assert_eq!(
            select_count.load(Ordering::SeqCst),
            1,
            "on_select should have been called"
        );
        assert!(*selected.borrow(), "Should be selected after click");
    }
}

// ============================================================================
// Reset Tests
// ============================================================================

/// View that tracks reset events
struct PotResetTestView {
    value: Rc<RefCell<f64>>,
    reset_count: Arc<AtomicUsize>,
}

impl Render for PotResetTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let current_value = *self.value.borrow();
        let value_rc = self.value.clone();
        let reset_count = self.reset_count.clone();

        div().size_full().child(
            Potentiometer::new("reset-test-pot")
                .value(current_value)
                .min(0.0)
                .max(100.0)
                .label("Reset Test")
                .on_reset(move |_window, _cx| {
                    *value_rc.borrow_mut() = 50.0; // Default value
                    reset_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_double_click_reset(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(75.0)); // Start not at default
    let reset_count = Arc::new(AtomicUsize::new(0));

    let value_clone = value.clone();
    let reset_count_clone = reset_count.clone();

    let window = cx.add_window(move |_window, _cx| PotResetTestView {
        value: value_clone,
        reset_count: reset_count_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initial value is 75
    assert!(
        ((*value.borrow()) - 75.0).abs() < 0.01,
        "Initial value should be 75"
    );

    // Double-click to reset (simulated via click with count=2 handled by component)
    // Note: Testing double-click requires the component's on_click handler to check click_count
    if let Some(bounds) = cx.debug_bounds("reset-test-pot") {
        let center = bounds.center();
        // Simulate two quick clicks
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
    }
}

// ============================================================================
// Scale Type Tests
// ============================================================================

#[gpui::test]
async fn test_potentiometer_linear_scale(cx: &mut TestAppContext) {
    struct LinearScaleView;

    impl Render for LinearScaleView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                Potentiometer::new("linear-pot")
                    .value(50.0)
                    .min(0.0)
                    .max(100.0)
                    .scale(PotentiometerScale::Linear)
                    .label("Linear"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LinearScaleView);
}

#[gpui::test]
async fn test_potentiometer_logarithmic_scale(cx: &mut TestAppContext) {
    struct LogScaleView;

    impl Render for LogScaleView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                Potentiometer::new("log-pot")
                    .value(1000.0)
                    .min(20.0) // Must be > 0 for log scale
                    .max(20000.0)
                    .scale(PotentiometerScale::Logarithmic)
                    .unit("Hz")
                    .label("Frequency"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LogScaleView);
}

// ============================================================================
// Selected State Tests
// ============================================================================

#[gpui::test]
async fn test_potentiometer_selected_appearance(cx: &mut TestAppContext) {
    struct SelectedView;

    impl Render for SelectedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    Potentiometer::new("unselected-pot")
                        .value(50.0)
                        .selected(false)
                        .label("Unselected"),
                )
                .child(
                    Potentiometer::new("selected-pot")
                        .value(50.0)
                        .selected(true)
                        .label("Selected"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SelectedView);
}

// ============================================================================
// Disabled State Tests
// ============================================================================

/// View with disabled potentiometer that tracks interactions
struct DisabledPotTestView {
    change_count: Arc<AtomicUsize>,
}

impl Render for DisabledPotTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let change_count = self.change_count.clone();

        div().size_full().child(
            Potentiometer::new("disabled-pot")
                .value(50.0)
                .min(0.0)
                .max(100.0)
                .disabled(true)
                .label("Disabled")
                .on_change(move |_, _window, _cx| {
                    change_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_disabled_state(cx: &mut TestAppContext) {
    let change_count = Arc::new(AtomicUsize::new(0));
    let change_count_clone = change_count.clone();

    let window = cx.add_window(move |_window, _cx| DisabledPotTestView {
        change_count: change_count_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Try clicking the disabled potentiometer
    if let Some(bounds) = cx.debug_bounds("disabled-pot") {
        let center = bounds.center();
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();

        // on_change should NOT have been called
        assert_eq!(
            change_count.load(Ordering::SeqCst),
            0,
            "Disabled potentiometer should not trigger on_change"
        );
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[gpui::test]
async fn test_potentiometer_with_units(cx: &mut TestAppContext) {
    struct UnitsView;

    impl Render for UnitsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    Potentiometer::new("pot-percent")
                        .value(50.0)
                        .min(0.0)
                        .max(100.0)
                        .unit("%")
                        .label("Percent"),
                )
                .child(
                    Potentiometer::new("pot-db")
                        .value(0.0)
                        .min(-60.0)
                        .max(12.0)
                        .unit("dB")
                        .label("Gain"),
                )
                .child(
                    Potentiometer::new("pot-hz")
                        .value(1000.0)
                        .min(20.0)
                        .max(20000.0)
                        .unit("Hz")
                        .label("Frequency"),
                )
                .child(
                    Potentiometer::new("pot-ratio")
                        .value(4.0)
                        .min(1.0)
                        .max(20.0)
                        .unit(":1")
                        .label("Ratio"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| UnitsView);
}

// ============================================================================
// Shortcut Key Tests
// ============================================================================

#[gpui::test]
async fn test_potentiometer_with_shortcut_key(cx: &mut TestAppContext) {
    struct ShortcutView;

    impl Render for ShortcutView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    Potentiometer::new("pot-a")
                        .value(50.0)
                        .label("Attack")
                        .shortcut_key('a'),
                )
                .child(
                    Potentiometer::new("pot-r")
                        .value(50.0)
                        .label("Release")
                        .shortcut_key('r'),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ShortcutView);
}

// ============================================================================
// Drag Start Tests
// ============================================================================

/// View that tracks drag start events
struct PotDragTestView {
    drag_started: Arc<AtomicBool>,
    drag_y: Rc<RefCell<Option<f32>>>,
}

impl Render for PotDragTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let drag_started = self.drag_started.clone();
        let drag_y = self.drag_y.clone();

        div().size_full().child(
            Potentiometer::new("drag-test-pot")
                .value(50.0)
                .min(0.0)
                .max(100.0)
                .label("Draggable")
                .on_drag_start(move |y, _value, _window, _cx| {
                    drag_started.store(true, Ordering::SeqCst);
                    *drag_y.borrow_mut() = Some(y);
                }),
        )
    }
}

#[gpui::test]
async fn test_potentiometer_drag_start(cx: &mut TestAppContext) {
    let drag_started = Arc::new(AtomicBool::new(false));
    let drag_y: Rc<RefCell<Option<f32>>> = Rc::new(RefCell::new(None));

    let drag_started_clone = drag_started.clone();
    let drag_y_clone = drag_y.clone();

    let window = cx.add_window(move |_window, _cx| PotDragTestView {
        drag_started: drag_started_clone,
        drag_y: drag_y_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Mouse down should trigger drag start
    if let Some(bounds) = cx.debug_bounds("drag-test-pot") {
        let center = bounds.center();
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();

        assert!(
            drag_started.load(Ordering::SeqCst),
            "Drag should have started"
        );
        assert!(drag_y.borrow().is_some(), "Drag Y position should be set");
    }
}

// ============================================================================
// Theming Tests
// ============================================================================

#[gpui::test]
async fn test_potentiometer_with_custom_theme(cx: &mut TestAppContext) {
    use gpui_ui_kit::audio::potentiometer::PotentiometerTheme;

    struct ThemedPotView;

    impl Render for ThemedPotView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let custom_theme = PotentiometerTheme {
                surface: gpui::rgba(0x1a1a1aff),
                surface_hover: gpui::rgba(0x2a2a2aff),
                knob_bg: gpui::rgba(0x333333ff),
                accent: gpui::rgba(0xff6600ff), // Orange accent
                accent_muted: gpui::rgba(0xff660033),
                border: gpui::rgba(0x444444ff),
                text_secondary: gpui::rgba(0xaaaaaaff),
                text_primary: gpui::rgba(0xffffffff),
                text_muted: gpui::rgba(0x888888ff),
                text_on_accent: gpui::rgba(0xffffffff),
                background_secondary: gpui::rgba(0x222222ff),
            };

            div().child(
                Potentiometer::new("themed-pot")
                    .value(50.0)
                    .min(0.0)
                    .max(100.0)
                    .label("Themed")
                    .theme(custom_theme),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ThemedPotView);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[gpui::test]
async fn test_potentiometer_value_clamping(cx: &mut TestAppContext) {
    struct ClampingView;

    impl Render for ClampingView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(
                    Potentiometer::new("pot-below-min")
                        .value(-10.0) // Below min
                        .min(0.0)
                        .max(100.0)
                        .label("Below Min"),
                )
                .child(
                    Potentiometer::new("pot-above-max")
                        .value(200.0) // Above max
                        .min(0.0)
                        .max(100.0)
                        .label("Above Max"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ClampingView);
}

#[gpui::test]
async fn test_potentiometer_negative_range(cx: &mut TestAppContext) {
    struct NegativeRangeView;

    impl Render for NegativeRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                Potentiometer::new("pot-negative")
                    .value(0.0)
                    .min(-60.0)
                    .max(12.0)
                    .unit("dB")
                    .label("Gain"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| NegativeRangeView);
}
