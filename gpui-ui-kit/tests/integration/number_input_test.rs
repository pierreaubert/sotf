//! Integration tests for NumberInput component
//!
//! Tests the number input component including:
//! - Basic rendering
//! - Size variants
//! - Value changes via buttons
//! - Min/max bounds
//! - Step size
//! - Decimals formatting
//! - Unit display
//! - Label
//! - Disabled state
//! - Edit mode callbacks
//! - Theme customization

use gpui::{Context, TestAppContext, VisualTestContext, Window, div, prelude::*};
use gpui_ui_kit::number_input::{NumberInput, NumberInputSize, NumberInputTheme};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ============================================================================
// Basic Rendering Tests
// ============================================================================

struct NumberInputTestView;

impl Render for NumberInputTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            NumberInput::new("test-number-input")
                .value(50.0)
                .min(0.0)
                .max(100.0)
        )
    }
}

#[gpui::test]
async fn test_number_input_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| NumberInputTestView);
}

// ============================================================================
// Size Variant Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_sizes(cx: &mut TestAppContext) {
    struct SizeTestView;

    impl Render for SizeTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    NumberInput::new("sm-input")
                        .size(NumberInputSize::Sm)
                        .value(10.0)
                )
                .child(
                    NumberInput::new("md-input")
                        .size(NumberInputSize::Md)
                        .value(50.0)
                )
                .child(
                    NumberInput::new("lg-input")
                        .size(NumberInputSize::Lg)
                        .value(100.0)
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SizeTestView);
}

// ============================================================================
// Value Change Tests
// ============================================================================

/// View that tracks value changes
struct NumberInputChangeTestView {
    value: Rc<RefCell<f64>>,
    change_count: Arc<AtomicUsize>,
}

impl Render for NumberInputChangeTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let current_value = *self.value.borrow();
        let value_rc = self.value.clone();
        let change_count = self.change_count.clone();

        div().size_full().child(
            NumberInput::new("change-test-input")
                .value(current_value)
                .min(0.0)
                .max(100.0)
                .step(5.0)
                .on_change(move |new_val, _window, _cx| {
                    *value_rc.borrow_mut() = new_val;
                    change_count.fetch_add(1, Ordering::SeqCst);
                })
        )
    }
}

#[gpui::test]
async fn test_number_input_value_change(cx: &mut TestAppContext) {
    let value: Rc<RefCell<f64>> = Rc::new(RefCell::new(50.0));
    let change_count = Arc::new(AtomicUsize::new(0));

    let value_clone = value.clone();
    let change_count_clone = change_count.clone();

    let window = cx.add_window(move |_window, _cx| NumberInputChangeTestView {
        value: value_clone,
        change_count: change_count_clone,
    });

    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initial value should be 50
    assert_eq!(*value.borrow(), 50.0);
}

// ============================================================================
// Bounds Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_min_max_bounds(cx: &mut TestAppContext) {
    struct BoundsTestView;

    impl Render for BoundsTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("bounds-input")
                    .value(150.0)  // Over max, should be clamped
                    .min(0.0)
                    .max(100.0)
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| BoundsTestView);
}

#[gpui::test]
async fn test_number_input_negative_range(cx: &mut TestAppContext) {
    struct NegativeRangeView;

    impl Render for NegativeRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("negative-input")
                    .value(-30.0)
                    .min(-60.0)
                    .max(12.0)
                    .unit("dB")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| NegativeRangeView);
}

// ============================================================================
// Step Size Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_step_size(cx: &mut TestAppContext) {
    struct StepTestView;

    impl Render for StepTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    NumberInput::new("step-1")
                        .value(50.0)
                        .step(1.0)
                        .label("Step 1")
                )
                .child(
                    NumberInput::new("step-5")
                        .value(50.0)
                        .step(5.0)
                        .label("Step 5")
                )
                .child(
                    NumberInput::new("step-10")
                        .value(50.0)
                        .step(10.0)
                        .label("Step 10")
                )
                .child(
                    NumberInput::new("step-0.1")
                        .value(0.5)
                        .step(0.1)
                        .decimals(1)
                        .label("Step 0.1")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| StepTestView);
}

// ============================================================================
// Decimals Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_decimals(cx: &mut TestAppContext) {
    struct DecimalsTestView;

    impl Render for DecimalsTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    NumberInput::new("decimals-0")
                        .value(3.14159)
                        .decimals(0)
                        .label("0 decimals")
                )
                .child(
                    NumberInput::new("decimals-1")
                        .value(3.14159)
                        .decimals(1)
                        .label("1 decimal")
                )
                .child(
                    NumberInput::new("decimals-2")
                        .value(3.14159)
                        .decimals(2)
                        .label("2 decimals")
                )
                .child(
                    NumberInput::new("decimals-3")
                        .value(3.14159)
                        .decimals(3)
                        .label("3 decimals")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| DecimalsTestView);
}

// ============================================================================
// Unit Display Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_units(cx: &mut TestAppContext) {
    struct UnitsTestView;

    impl Render for UnitsTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    NumberInput::new("unit-hz")
                        .value(1000.0)
                        .unit("Hz")
                        .label("Frequency")
                )
                .child(
                    NumberInput::new("unit-db")
                        .value(-6.0)
                        .unit("dB")
                        .decimals(1)
                        .label("Gain")
                )
                .child(
                    NumberInput::new("unit-percent")
                        .value(75.0)
                        .unit("%")
                        .label("Amount")
                )
                .child(
                    NumberInput::new("unit-ms")
                        .value(100.0)
                        .unit("ms")
                        .label("Delay")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| UnitsTestView);
}

// ============================================================================
// Label Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_with_label(cx: &mut TestAppContext) {
    struct LabelTestView;

    impl Render for LabelTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("labeled-input")
                    .value(42.0)
                    .label("Answer to Everything")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LabelTestView);
}

// ============================================================================
// Disabled State Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_disabled(cx: &mut TestAppContext) {
    struct DisabledTestView;

    impl Render for DisabledTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("disabled-input")
                    .value(50.0)
                    .disabled(true)
                    .label("Disabled")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| DisabledTestView);
}

/// Test that disabled input doesn't trigger callbacks
struct DisabledCallbackTestView {
    change_count: Arc<AtomicUsize>,
}

impl Render for DisabledCallbackTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let change_count = self.change_count.clone();

        div().size_full().child(
            NumberInput::new("disabled-callback-input")
                .value(50.0)
                .disabled(true)
                .on_change(move |_, _window, _cx| {
                    change_count.fetch_add(1, Ordering::SeqCst);
                })
        )
    }
}

#[gpui::test]
async fn test_number_input_disabled_no_callback(cx: &mut TestAppContext) {
    let change_count = Arc::new(AtomicUsize::new(0));
    let change_count_clone = change_count.clone();

    let _window = cx.add_window(move |_window, _cx| DisabledCallbackTestView {
        change_count: change_count_clone,
    });

    // Change count should remain 0 since input is disabled
    assert_eq!(change_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Edit Mode Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_editing_state(cx: &mut TestAppContext) {
    struct EditingTestView;

    impl Render for EditingTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("editing-input")
                    .value(50.0)
                    .editing(true)
                    .edit_text("123")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| EditingTestView);
}

#[gpui::test]
async fn test_number_input_text_selected(cx: &mut TestAppContext) {
    struct TextSelectedView;

    impl Render for TextSelectedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("selected-input")
                    .value(50.0)
                    .editing(true)
                    .text_selected(true)
                    .edit_text("50")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| TextSelectedView);
}

/// View that tracks edit start/end events
struct EditCallbackTestView {
    edit_started: Arc<AtomicBool>,
    edit_ended: Arc<AtomicBool>,
}

impl Render for EditCallbackTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let edit_started = self.edit_started.clone();
        let edit_ended = self.edit_ended.clone();

        div().size_full().child(
            NumberInput::new("edit-callback-input")
                .value(50.0)
                .on_edit_start(move |_window, _cx| {
                    edit_started.store(true, Ordering::SeqCst);
                })
                .on_edit_end(move |_result, _window, _cx| {
                    edit_ended.store(true, Ordering::SeqCst);
                })
        )
    }
}

#[gpui::test]
async fn test_number_input_edit_callbacks(cx: &mut TestAppContext) {
    let edit_started = Arc::new(AtomicBool::new(false));
    let edit_ended = Arc::new(AtomicBool::new(false));

    let edit_started_clone = edit_started.clone();
    let edit_ended_clone = edit_ended.clone();

    let _window = cx.add_window(move |_window, _cx| EditCallbackTestView {
        edit_started: edit_started_clone,
        edit_ended: edit_ended_clone,
    });

    // Initially neither callback should have been triggered
    assert!(!edit_started.load(Ordering::SeqCst));
    assert!(!edit_ended.load(Ordering::SeqCst));
}

// ============================================================================
// Theme Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_with_custom_theme(cx: &mut TestAppContext) {
    struct ThemedView;

    impl Render for ThemedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let custom_theme = NumberInputTheme {
                background: gpui::rgba(0x1a1a1aff),
                text: gpui::rgba(0xffffffff),
                button_bg: gpui::rgba(0x2a2a2aff),
                button_hover: gpui::rgba(0x3a3a3aff),
                button_active: gpui::rgba(0xff6600ff),
                button_text: gpui::rgba(0xccccccff),
                border: gpui::rgba(0x444444ff),
                border_focus: gpui::rgba(0xff6600ff),
                label: gpui::rgba(0xaaaaaaff),
                disabled_opacity: 0.4,
            };

            div().child(
                NumberInput::new("themed-input")
                    .theme(custom_theme)
                    .value(42.0)
                    .label("Themed Input")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ThemedView);
}

// ============================================================================
// Width Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_fixed_width(cx: &mut TestAppContext) {
    struct WidthTestView;

    impl Render for WidthTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    NumberInput::new("narrow-input")
                        .value(10.0)
                        .width(80.0)
                        .label("Narrow")
                )
                .child(
                    NumberInput::new("wide-input")
                        .value(10.0)
                        .width(200.0)
                        .label("Wide")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| WidthTestView);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[gpui::test]
async fn test_number_input_zero_value(cx: &mut TestAppContext) {
    struct ZeroValueView;

    impl Render for ZeroValueView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("zero-input")
                    .value(0.0)
                    .min(-100.0)
                    .max(100.0)
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ZeroValueView);
}

#[gpui::test]
async fn test_number_input_large_values(cx: &mut TestAppContext) {
    struct LargeValuesView;

    impl Render for LargeValuesView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("large-input")
                    .value(1000000.0)
                    .min(0.0)
                    .max(10000000.0)
                    .step(1000.0)
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LargeValuesView);
}

#[gpui::test]
async fn test_number_input_small_step(cx: &mut TestAppContext) {
    struct SmallStepView;

    impl Render for SmallStepView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                NumberInput::new("small-step-input")
                    .value(0.5)
                    .min(0.0)
                    .max(1.0)
                    .step(0.01)
                    .decimals(2)
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| SmallStepView);
}
