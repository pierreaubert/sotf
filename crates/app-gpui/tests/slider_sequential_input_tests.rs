//! Slider gestures accumulate while the application has not redrawn yet.

use gpui::{
    Context, Modifiers, MouseButton, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, point, px,
};
use gpui_ui_kit::Slider;
use std::{cell::Cell, rc::Rc};

struct SliderView {
    value: Rc<Cell<f32>>,
}

impl Render for SliderView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        let value = self.value.clone();
        div().size_full().child(
            Slider::new("sequential-gain")
                .value(self.value.get())
                .min(-60.0)
                .max(24.0)
                .step(0.5)
                .on_change(move |next, _, _| {
                    // Deliberately defer repaint, as real application updates can.
                    value.set(next);
                }),
        )
    }
}

#[gpui::test]
fn slider_keyboard_uses_latest_gesture_before_redraw(cx: &mut TestAppContext) {
    let value = Rc::new(Cell::new(-7.5));
    let window = cx.add_window({
        let value = value.clone();
        move |_, _| SliderView { value }
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    let position = point(px(100.0), px(10.0));
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    cx.simulate_keystrokes("home");
    assert_eq!(value.get(), -60.0);
    cx.simulate_keystrokes("right");
    assert_eq!(value.get(), -56.0);
    cx.simulate_keystrokes("right");
    assert_eq!(value.get(), -52.0);

    // A later application render supplies the authoritative value again.
    value.set(13.0);
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.simulate_keystrokes("right");
    assert_eq!(value.get(), 17.0);
}
