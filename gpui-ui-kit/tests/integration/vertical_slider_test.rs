//! Integration tests for VerticalSlider component
//!
//! Tests the vertical slider component including:
//! - Basic rendering
//! - Size variants
//! - Value changes
//! - Linear and logarithmic scales
//! - Ticks display
//! - Selected state
//! - Disabled state
//! - Drag callbacks
//! - Reset callback
//! - Theme customization

use gpui::{Context, TestAppContext, VisualTestContext, Window, div, prelude::*};
use gpui_ui_kit::audio::vertical_slider::{
    VerticalSlider, VerticalSliderSize, VerticalSliderTheme,
};
use gpui_ui_kit::scale::Scale;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ============================================================================
// Basic Rendering Tests
// ============================================================================

struct VerticalSliderTestView;

impl Render for VerticalSliderTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            VerticalSlider::new("test-vslider")
                .value(50.0)
                .min(0.0)
                .max(100.0)
                .label("Volume"),
        )
    }
}

#[gpui::test]
async fn test_vertical_slider_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| VerticalSliderTestView);
}

// ============================================================================
// Size Variant Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_sizes(cx: &mut TestAppContext) {
    struct SizeTestView;

    impl Render for SizeTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("sm-slider")
                        .size(VerticalSliderSize::Sm)
                        .value(50.0)
                        .label("Small"),
                )
                .child(
                    VerticalSlider::new("md-slider")
                        .size(VerticalSliderSize::Md)
                        .value(50.0)
                        .label("Medium"),
                )
                .child(
                    VerticalSlider::new("lg-slider")
                        .size(VerticalSliderSize::Lg)
                        .value(50.0)
                        .label("Large"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SizeTestView);
}

#[gpui::test]
async fn test_vertical_slider_custom_height(cx: &mut TestAppContext) {
    struct CustomHeightView;

    impl Render for CustomHeightView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("custom-height")
                    .height(200.0)
                    .value(50.0)
                    .label("Custom Height"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| CustomHeightView);
}

// ============================================================================
// Value Change Tests
// ============================================================================

/// View that tracks value changes
struct SliderChangeTestView {
    value: Rc<RefCell<f64>>,
    change_count: Arc<AtomicUsize>,
}

impl Render for SliderChangeTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let current_value = *self.value.borrow();
        let value_rc = self.value.clone();
        let change_count = self.change_count.clone();

        div().size_full().child(
            VerticalSlider::new("change-test-slider")
                .value(current_value)
                .min(0.0)
                .max(100.0)
                .on_change(move |new_val, _window, _cx| {
                    *value_rc.borrow_mut() = new_val;
                    change_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_vertical_slider_value_change(cx: &mut TestAppContext) {
    let value: Rc<RefCell<f64>> = Rc::new(RefCell::new(50.0));
    let change_count = Arc::new(AtomicUsize::new(0));

    let value_clone = value.clone();
    let change_count_clone = change_count.clone();

    let window = cx.add_window(move |_window, _cx| SliderChangeTestView {
        value: value_clone,
        change_count: change_count_clone,
    });

    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initial value should be 50
    assert_eq!(*value.borrow(), 50.0);
}

// ============================================================================
// Scale Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_linear_scale(cx: &mut TestAppContext) {
    struct LinearScaleView;

    impl Render for LinearScaleView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("linear-slider")
                    .scale(Scale::Linear)
                    .value(50.0)
                    .min(0.0)
                    .max(100.0)
                    .label("Linear"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LinearScaleView);
}

#[gpui::test]
async fn test_vertical_slider_logarithmic_scale(cx: &mut TestAppContext) {
    struct LogScaleView;

    impl Render for LogScaleView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("log-slider")
                    .scale(Scale::Logarithmic)
                    .value(1000.0)
                    .min(20.0)
                    .max(20000.0)
                    .unit("Hz")
                    .label("Frequency"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LogScaleView);
}

// ============================================================================
// Ticks Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_with_ticks(cx: &mut TestAppContext) {
    struct TicksView;

    impl Render for TicksView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("ticks-linear")
                        .scale(Scale::Linear)
                        .with_ticks()
                        .value(50.0)
                        .min(0.0)
                        .max(100.0)
                        .label("Linear Ticks"),
                )
                .child(
                    VerticalSlider::new("ticks-log")
                        .scale(Scale::Logarithmic)
                        .with_ticks()
                        .value(1000.0)
                        .min(20.0)
                        .max(20000.0)
                        .label("Log Ticks"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| TicksView);
}

// ============================================================================
// Selected State Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_selected(cx: &mut TestAppContext) {
    struct SelectedView;

    impl Render for SelectedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("not-selected")
                        .value(50.0)
                        .selected(false)
                        .label("Not Selected"),
                )
                .child(
                    VerticalSlider::new("selected")
                        .value(50.0)
                        .selected(true)
                        .label("Selected"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SelectedView);
}

/// Test on_select callback
struct SelectCallbackTestView {
    select_count: Arc<AtomicUsize>,
}

impl Render for SelectCallbackTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let select_count = self.select_count.clone();

        div().size_full().child(
            VerticalSlider::new("select-callback-slider")
                .value(50.0)
                .on_select(move |_window, _cx| {
                    select_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_vertical_slider_on_select(cx: &mut TestAppContext) {
    let select_count = Arc::new(AtomicUsize::new(0));
    let select_count_clone = select_count.clone();

    let _window = cx.add_window(move |_window, _cx| SelectCallbackTestView {
        select_count: select_count_clone,
    });

    // Initial state - no selections yet
    assert_eq!(select_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Disabled State Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_disabled(cx: &mut TestAppContext) {
    struct DisabledView;

    impl Render for DisabledView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("disabled-slider")
                    .value(50.0)
                    .disabled(true)
                    .label("Disabled"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| DisabledView);
}

// ============================================================================
// Unit Display Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_units(cx: &mut TestAppContext) {
    struct UnitsView;

    impl Render for UnitsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("unit-db")
                        .value(-6.0)
                        .min(-60.0)
                        .max(12.0)
                        .unit("dB")
                        .label("Gain"),
                )
                .child(
                    VerticalSlider::new("unit-hz")
                        .value(1000.0)
                        .min(20.0)
                        .max(20000.0)
                        .unit("Hz")
                        .scale(Scale::Logarithmic)
                        .label("Freq"),
                )
                .child(
                    VerticalSlider::new("unit-percent")
                        .value(0.5)
                        .min(0.0)
                        .max(1.0)
                        .unit("%")
                        .label("Mix"),
                )
                .child(
                    VerticalSlider::new("unit-ratio")
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
async fn test_vertical_slider_shortcut_key(cx: &mut TestAppContext) {
    struct ShortcutView;

    impl Render for ShortcutView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("shortcut-g")
                        .value(0.0)
                        .min(-12.0)
                        .max(12.0)
                        .shortcut_key('g')
                        .label("Gain"),
                )
                .child(
                    VerticalSlider::new("shortcut-f")
                        .value(1000.0)
                        .min(20.0)
                        .max(20000.0)
                        .shortcut_key('f')
                        .label("Frequency"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ShortcutView);
}

// ============================================================================
// Drag Callback Tests
// ============================================================================

/// Test on_drag_start callback
struct DragCallbackTestView {
    drag_started: Arc<AtomicBool>,
}

impl Render for DragCallbackTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let drag_started = self.drag_started.clone();

        div().size_full().child(
            VerticalSlider::new("drag-callback-slider")
                .value(50.0)
                .on_drag_start(move |_y, _value, _window, _cx| {
                    drag_started.store(true, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_vertical_slider_on_drag_start(cx: &mut TestAppContext) {
    let drag_started = Arc::new(AtomicBool::new(false));
    let drag_started_clone = drag_started.clone();

    let _window = cx.add_window(move |_window, _cx| DragCallbackTestView {
        drag_started: drag_started_clone,
    });

    // Initially drag hasn't started
    assert!(!drag_started.load(Ordering::SeqCst));
}

// ============================================================================
// Reset Callback Tests
// ============================================================================

/// Test on_reset callback (double-click)
struct ResetCallbackTestView {
    reset_count: Arc<AtomicUsize>,
}

impl Render for ResetCallbackTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let reset_count = self.reset_count.clone();

        div().size_full().child(
            VerticalSlider::new("reset-callback-slider")
                .value(75.0)
                .on_reset(move |_window, _cx| {
                    reset_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
    }
}

#[gpui::test]
async fn test_vertical_slider_on_reset(cx: &mut TestAppContext) {
    let reset_count = Arc::new(AtomicUsize::new(0));
    let reset_count_clone = reset_count.clone();

    let _window = cx.add_window(move |_window, _cx| ResetCallbackTestView {
        reset_count: reset_count_clone,
    });

    // Initially no resets
    assert_eq!(reset_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Theme Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_with_custom_theme(cx: &mut TestAppContext) {
    struct ThemedView;

    impl Render for ThemedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let custom_theme = VerticalSliderTheme {
                surface: gpui::rgba(0x2a2a2aff),
                surface_hover: gpui::rgba(0x3a3a3aff),
                track_bg: gpui::rgba(0x1a1a1aff),
                accent: gpui::rgba(0xff6600ff),
                accent_muted: gpui::rgba(0xff660033),
                border: gpui::rgba(0x444444ff),
                text_secondary: gpui::rgba(0xaaaaaaff),
                text_primary: gpui::rgba(0xffffffff),
                text_muted: gpui::rgba(0x888888ff),
                text_on_accent: gpui::rgba(0xffffffff),
                background_secondary: gpui::rgba(0x2a2a2aff),
            };

            div().child(
                VerticalSlider::new("themed-slider")
                    .theme(custom_theme)
                    .value(50.0)
                    .label("Themed"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ThemedView);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[gpui::test]
async fn test_vertical_slider_value_clamping(cx: &mut TestAppContext) {
    struct ClampingView;

    impl Render for ClampingView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VerticalSlider::new("over-max")
                        .value(150.0) // Over max
                        .min(0.0)
                        .max(100.0)
                        .label("Over Max"),
                )
                .child(
                    VerticalSlider::new("under-min")
                        .value(-50.0) // Under min
                        .min(0.0)
                        .max(100.0)
                        .label("Under Min"),
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ClampingView);
}

#[gpui::test]
async fn test_vertical_slider_zero_range(cx: &mut TestAppContext) {
    struct ZeroRangeView;

    impl Render for ZeroRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("zero-range")
                    .value(50.0)
                    .min(50.0)
                    .max(50.0) // Same as min
                    .label("Zero Range"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ZeroRangeView);
}

#[gpui::test]
async fn test_vertical_slider_negative_range(cx: &mut TestAppContext) {
    struct NegativeRangeView;

    impl Render for NegativeRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("negative-range")
                    .value(-30.0)
                    .min(-60.0)
                    .max(12.0)
                    .unit("dB")
                    .label("Gain"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| NegativeRangeView);
}

#[gpui::test]
async fn test_vertical_slider_large_range(cx: &mut TestAppContext) {
    struct LargeRangeView;

    impl Render for LargeRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("large-range")
                    .value(5000.0)
                    .min(20.0)
                    .max(20000.0)
                    .scale(Scale::Logarithmic)
                    .with_ticks()
                    .unit("Hz")
                    .label("Wide Range"),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| LargeRangeView);
}

#[gpui::test]
async fn test_vertical_slider_all_features(cx: &mut TestAppContext) {
    struct AllFeaturesView;

    impl Render for AllFeaturesView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VerticalSlider::new("all-features")
                    .value(1000.0)
                    .min(20.0)
                    .max(20000.0)
                    .scale(Scale::Logarithmic)
                    .with_ticks()
                    .unit("Hz")
                    .label("Frequency")
                    .shortcut_key('f')
                    .size(VerticalSliderSize::Lg)
                    .selected(true),
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| AllFeaturesView);
}
