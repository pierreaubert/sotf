//! Integration tests for VolumeKnob component
//!
//! Tests the volume knob component including:
//! - Basic rendering
//! - Value changes
//! - Mute state
//! - Size configuration
//! - Label display
//! - Theme customization
//! - Color overrides
//! - Double-click mute toggle

use gpui::{Context, TestAppContext, VisualTestContext, Window, div, px, prelude::*};
use gpui_ui_kit::volume_knob::{VolumeKnob, VolumeKnobTheme};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// Basic Rendering Tests
// ============================================================================

struct VolumeKnobTestView;

impl Render for VolumeKnobTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            VolumeKnob::new()
                .id("test-volume-knob")
                .value(0.7)
                .label("VOL")
        )
    }
}

#[gpui::test]
async fn test_volume_knob_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| VolumeKnobTestView);
}

// ============================================================================
// Value Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_value_range(cx: &mut TestAppContext) {
    struct ValueRangeView;

    impl Render for ValueRangeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VolumeKnob::new()
                        .id("vol-0")
                        .value(0.0)
                        .label("0%")
                )
                .child(
                    VolumeKnob::new()
                        .id("vol-25")
                        .value(0.25)
                        .label("25%")
                )
                .child(
                    VolumeKnob::new()
                        .id("vol-50")
                        .value(0.5)
                        .label("50%")
                )
                .child(
                    VolumeKnob::new()
                        .id("vol-75")
                        .value(0.75)
                        .label("75%")
                )
                .child(
                    VolumeKnob::new()
                        .id("vol-100")
                        .value(1.0)
                        .label("100%")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ValueRangeView);
}

/// View that tracks value changes
struct VolumeChangeTestView {
    value: Rc<RefCell<f32>>,
    change_count: Arc<AtomicUsize>,
}

impl Render for VolumeChangeTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let current_value = *self.value.borrow();
        let value_rc = self.value.clone();
        let change_count = self.change_count.clone();

        div().size_full().child(
            VolumeKnob::new()
                .id("change-test-knob")
                .value(current_value)
                .label("VOL")
                .on_change(move |new_val, _window, _cx| {
                    *value_rc.borrow_mut() = new_val;
                    change_count.fetch_add(1, Ordering::SeqCst);
                })
        )
    }
}

#[gpui::test]
async fn test_volume_knob_on_change(cx: &mut TestAppContext) {
    let value: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.5));
    let change_count = Arc::new(AtomicUsize::new(0));

    let value_clone = value.clone();
    let change_count_clone = change_count.clone();

    let window = cx.add_window(move |_window, _cx| VolumeChangeTestView {
        value: value_clone,
        change_count: change_count_clone,
    });

    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Initial value should be 0.5
    assert_eq!(*value.borrow(), 0.5);
}

// ============================================================================
// Mute State Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_muted(cx: &mut TestAppContext) {
    struct MutedView;

    impl Render for MutedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VolumeKnob::new()
                        .id("not-muted")
                        .value(0.7)
                        .muted(false)
                        .label("ON")
                )
                .child(
                    VolumeKnob::new()
                        .id("muted")
                        .value(0.7)
                        .muted(true)
                        .label("MUTE")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| MutedView);
}

/// View that tracks mute toggle
struct MuteToggleTestView {
    muted: Rc<RefCell<bool>>,
    toggle_count: Arc<AtomicUsize>,
}

impl Render for MuteToggleTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_muted = *self.muted.borrow();
        let muted_rc = self.muted.clone();
        let toggle_count = self.toggle_count.clone();

        div().size_full().child(
            VolumeKnob::new()
                .id("mute-toggle-knob")
                .value(0.7)
                .muted(is_muted)
                .label(if is_muted { "MUTE" } else { "VOL" })
                .on_mute_toggle(move |new_muted, _window, _cx| {
                    *muted_rc.borrow_mut() = new_muted;
                    toggle_count.fetch_add(1, Ordering::SeqCst);
                })
        )
    }
}

#[gpui::test]
async fn test_volume_knob_mute_toggle(cx: &mut TestAppContext) {
    let muted: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let toggle_count = Arc::new(AtomicUsize::new(0));

    let muted_clone = muted.clone();
    let toggle_count_clone = toggle_count.clone();

    let _window = cx.add_window(move |_window, _cx| MuteToggleTestView {
        muted: muted_clone,
        toggle_count: toggle_count_clone,
    });

    // Initially not muted
    assert!(!*muted.borrow());
    assert_eq!(toggle_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Size Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_sizes(cx: &mut TestAppContext) {
    struct SizesView;

    impl Render for SizesView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .items_end()
                .gap_4()
                .child(
                    VolumeKnob::new()
                        .id("size-20")
                        .size(px(20.0))
                        .value(0.7)
                        .label("XS")
                )
                .child(
                    VolumeKnob::new()
                        .id("size-32")
                        .size(px(32.0))
                        .value(0.7)
                        .label("S")
                )
                .child(
                    VolumeKnob::new()
                        .id("size-40")
                        .size(px(40.0))
                        .value(0.7)
                        .label("M")
                )
                .child(
                    VolumeKnob::new()
                        .id("size-56")
                        .size(px(56.0))
                        .value(0.7)
                        .label("L")
                )
                .child(
                    VolumeKnob::new()
                        .id("size-80")
                        .size(px(80.0))
                        .value(0.7)
                        .label("XL")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| SizesView);
}

// ============================================================================
// Label Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_labels(cx: &mut TestAppContext) {
    struct LabelsView;

    impl Render for LabelsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VolumeKnob::new()
                        .id("label-vol")
                        .value(0.7)
                        .label("VOL")
                )
                .child(
                    VolumeKnob::new()
                        .id("label-master")
                        .value(0.8)
                        .size(px(50.0))
                        .label("M")
                )
                .child(
                    VolumeKnob::new()
                        .id("label-empty")
                        .value(0.5)
                        .label("")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| LabelsView);
}

// ============================================================================
// Theme Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_with_custom_theme(cx: &mut TestAppContext) {
    struct ThemedView;

    impl Render for ThemedView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let custom_theme = VolumeKnobTheme {
                accent: gpui::hsla(0.1, 1.0, 0.5, 1.0),  // Orange
                muted: gpui::hsla(0.0, 0.0, 0.3, 1.0),
                background: gpui::hsla(0.0, 0.0, 0.15, 1.0),
                text: gpui::hsla(0.0, 0.0, 0.95, 1.0),
            };

            div().child(
                VolumeKnob::new()
                    .id("themed-knob")
                    .theme(custom_theme)
                    .value(0.7)
                    .label("THM")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ThemedView);
}

// ============================================================================
// Color Override Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_color_overrides(cx: &mut TestAppContext) {
    struct ColorOverridesView;

    impl Render for ColorOverridesView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap_4()
                .child(
                    VolumeKnob::new()
                        .id("accent-red")
                        .value(0.7)
                        .accent_color(gpui::hsla(0.0, 1.0, 0.5, 1.0))  // Red
                        .label("R")
                )
                .child(
                    VolumeKnob::new()
                        .id("accent-green")
                        .value(0.7)
                        .accent_color(gpui::hsla(0.33, 1.0, 0.4, 1.0))  // Green
                        .label("G")
                )
                .child(
                    VolumeKnob::new()
                        .id("accent-blue")
                        .value(0.7)
                        .accent_color(gpui::hsla(0.6, 1.0, 0.5, 1.0))  // Blue
                        .label("B")
                )
        }
    }

    let _window = cx.add_window(|_window, _cx| ColorOverridesView);
}

#[gpui::test]
async fn test_volume_knob_muted_color_override(cx: &mut TestAppContext) {
    struct MutedColorView;

    impl Render for MutedColorView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("muted-color-knob")
                    .value(0.7)
                    .muted(true)
                    .muted_color(gpui::hsla(0.0, 1.0, 0.3, 1.0))  // Dark red when muted
                    .label("MUTE")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| MutedColorView);
}

#[gpui::test]
async fn test_volume_knob_bg_color_override(cx: &mut TestAppContext) {
    struct BgColorView;

    impl Render for BgColorView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("bg-color-knob")
                    .value(0.7)
                    .bg_color(gpui::hsla(0.6, 0.3, 0.2, 1.0))  // Dark blue
                    .label("BG")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| BgColorView);
}

#[gpui::test]
async fn test_volume_knob_text_color_override(cx: &mut TestAppContext) {
    struct TextColorView;

    impl Render for TextColorView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("text-color-knob")
                    .value(0.7)
                    .text_color(gpui::hsla(0.1, 1.0, 0.5, 1.0))  // Orange text
                    .label("TXT")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| TextColorView);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[gpui::test]
async fn test_volume_knob_zero_value(cx: &mut TestAppContext) {
    struct ZeroValueView;

    impl Render for ZeroValueView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("zero-value")
                    .value(0.0)
                    .label("0")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| ZeroValueView);
}

#[gpui::test]
async fn test_volume_knob_max_value(cx: &mut TestAppContext) {
    struct MaxValueView;

    impl Render for MaxValueView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("max-value")
                    .value(1.0)
                    .label("MAX")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| MaxValueView);
}

#[gpui::test]
async fn test_volume_knob_over_max_clamped(cx: &mut TestAppContext) {
    struct OverMaxView;

    impl Render for OverMaxView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("over-max")
                    .value(1.5)  // Over 1.0, should be clamped
                    .label("CLAMP")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| OverMaxView);
}

#[gpui::test]
async fn test_volume_knob_negative_clamped(cx: &mut TestAppContext) {
    struct NegativeView;

    impl Render for NegativeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("negative")
                    .value(-0.5)  // Negative, should be clamped to 0
                    .label("NEG")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| NegativeView);
}

#[gpui::test]
async fn test_volume_knob_default(cx: &mut TestAppContext) {
    struct DefaultView;

    impl Render for DefaultView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(VolumeKnob::default())
        }
    }

    let _window = cx.add_window(|_window, _cx| DefaultView);
}

#[gpui::test]
async fn test_volume_knob_all_color_overrides(cx: &mut TestAppContext) {
    struct AllOverridesView;

    impl Render for AllOverridesView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                VolumeKnob::new()
                    .id("all-overrides")
                    .value(0.7)
                    .accent_color(gpui::hsla(0.8, 1.0, 0.5, 1.0))  // Purple
                    .muted_color(gpui::hsla(0.0, 0.5, 0.3, 1.0))
                    .bg_color(gpui::hsla(0.0, 0.0, 0.05, 1.0))
                    .text_color(gpui::hsla(0.8, 1.0, 0.8, 1.0))
                    .label("ALL")
            )
        }
    }

    let _window = cx.add_window(|_window, _cx| AllOverridesView);
}
