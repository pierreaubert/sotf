//! Regression tests for NumberInput component issues.

use gpui::{
    Context, TestAppContext, VisualTestContext, Window, div, prelude::*,
};
use gpui_ui_kit::number_input::NumberInput;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct GlobalShortcutView {
    shortcut_triggered: Arc<AtomicBool>,
}

impl Render for GlobalShortcutView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let triggered = self.shortcut_triggered.clone();
        
        div()
            .id("root")
            .size_full()
            // Simulate a global shortcut handler at a parent level (e.g. Play/Pause toggle)
            .on_key_down(move |event, _window, _cx| {
                if event.keystroke.key == "space" {
                    triggered.store(true, Ordering::SeqCst);
                }
            })
            .child(
                div()
                    .p_4()
                    .child(
                        NumberInput::new("test-input")
                            .value(50.0)
                    )
            )
    }
}

#[gpui::test]
async fn test_number_input_blocks_parent_shortcuts(cx: &mut TestAppContext) {
    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    let window = cx.add_window(move |_window, _cx| GlobalShortcutView {
        shortcut_triggered: triggered_clone,
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // 1. Click to enter edit mode
    // Try to find the root first to ensure rendering is working
    let _root_bounds = cx.debug_bounds("root").expect("Root bounds not found");
    
    let bounds = cx.debug_bounds("test-input").expect("Input bounds not found");
    cx.simulate_mouse_down(bounds.center(), gpui::MouseButton::Left, gpui::Modifiers::default());
    cx.simulate_mouse_up(bounds.center(), gpui::MouseButton::Left, gpui::Modifiers::default());
    cx.run_until_parked();

    // 2. Type a space
    cx.simulate_keystrokes("space");
    cx.run_until_parked();

    // ON THE CURRENT BROKEN VERSION, this should trigger the shortcut because stop_propagation is missing
    assert!(!triggered.load(Ordering::SeqCst), "Parent shortcut should NOT have been triggered while editing number");
}
