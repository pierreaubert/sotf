//! Header component rendering

use crate::app::Screen;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(rgb(0x2d2d2d))
            .border_b_1()
            .border_color(rgb(0x3e3e3e))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child("SOTF Audio Player"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_tab_button("Library", Screen::Library, cx))
                    .child(self.render_tab_button("Queue", Screen::Queue, cx))
                    .child(self.render_tab_button("Plugins", Screen::Plugins, cx))
                    .child(self.render_tab_button("Devices", Screen::Devices, cx)),
            )
    }

    pub(crate) fn render_tab_button(
        &self,
        label: &str,
        screen: Screen,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_active = state.app.current_screen == screen;

        let button = div()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .child(label.to_string());

        if is_active {
            button.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
        } else {
            button
                .bg(rgb(0x3e3e3e))
                .hover(|style| style.bg(rgb(0x505050)))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                        view.switch_screen(screen, cx);
                    }),
                )
        }
    }
}
