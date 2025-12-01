//! Settings screen rendering functions

mod appearance;
mod audio_device;
pub mod directory;
mod headphone;
mod library;
mod room_eq;

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let expanded = state.app.expanded_settings_sections.clone();

        let library_expanded = expanded.contains(&"library".to_string());
        let appearance_expanded = expanded.contains(&"appearance".to_string());
        let audio_device_expanded = expanded.contains(&"audio-device".to_string());
        let plugins_expanded = expanded.contains(&"plugins".to_string());
        let room_eq_expanded = expanded.contains(&"room-eq".to_string());
        let headphone_expanded = expanded.contains(&"headphone".to_string());

        // Pre-render all content sections (convert to AnyElement to release borrow)
        let library_content = self.render_library_settings_content(cx).into_any_element();
        let appearance_content = self
            .render_appearance_settings_content(cx)
            .into_any_element();
        let audio_device_content = self
            .render_audio_device_settings_content(cx)
            .into_any_element();
        let plugins_content = self.render_plugins_screen(cx).into_any_element();
        let room_eq_content = self.render_roomeq_settings_content(cx).into_any_element();
        let headphone_content = self
            .render_headphone_settings_content(cx)
            .into_any_element();

        // Pre-render all headers (convert to AnyElement to release borrow)
        let library_header = self
            .render_accordion_header("library", "Library", library_expanded, true, cx)
            .into_any_element();
        let appearance_header = self
            .render_accordion_header("appearance", "Appearance", appearance_expanded, false, cx)
            .into_any_element();
        let audio_device_header = self
            .render_accordion_header(
                "audio-device",
                "Audio Device",
                audio_device_expanded,
                false,
                cx,
            )
            .into_any_element();
        let plugins_header = self
            .render_accordion_header("plugins", "Plugins", plugins_expanded, false, cx)
            .into_any_element();
        let room_eq_header = self
            .render_accordion_header("room-eq", "Room EQ", room_eq_expanded, false, cx)
            .into_any_element();
        let headphone_header = self
            .render_accordion_header("headphone", "Headphone", headphone_expanded, false, cx)
            .into_any_element();

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .overflow_hidden()
                    // Library section
                    .child(library_header)
                    .when(library_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(library_content),
                        )
                    })
                    // Appearance section
                    .child(appearance_header)
                    .when(appearance_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(appearance_content),
                        )
                    })
                    // Audio Device section
                    .child(audio_device_header)
                    .when(audio_device_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(audio_device_content),
                        )
                    })
                    // Plugins section
                    .child(plugins_header)
                    .when(plugins_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(plugins_content),
                        )
                    })
                    // Room EQ section
                    .child(room_eq_header)
                    .when(room_eq_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(room_eq_content),
                        )
                    })
                    // Headphone section
                    .child(headphone_header)
                    .when(headphone_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(headphone_content),
                        )
                    }),
            )
    }

    fn render_accordion_header(
        &self,
        id: &'static str,
        title: &'static str,
        is_expanded: bool,
        is_first: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();
        let id_string = id.to_string();

        let mut header = div()
            .id(SharedString::from(format!("accordion-header-{}", id)))
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .bg(theme.surface)
            .cursor_pointer()
            .hover(|s| s.bg(theme.surface_hover))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    let id = id_string.clone();
                    view.state.update(cx, |state, _cx| {
                        if state.app.expanded_settings_sections.contains(&id) {
                            state.app.expanded_settings_sections.retain(|s| s != &id);
                        } else {
                            state.app.expanded_settings_sections.push(id);
                        }
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_primary)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(if is_expanded { "▼" } else { "▶" }),
            );

        if !is_first {
            header = header.border_t_1().border_color(theme.border);
        }

        header
    }
}
