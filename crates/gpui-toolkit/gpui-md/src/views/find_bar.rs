use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::MdAppState;

/// Find (and optionally Replace) bar shown at the top of the editor.
pub struct FindBar {
    state: Entity<MdAppState>,
}

impl FindBar {
    pub fn new(state: Entity<MdAppState>) -> Self {
        Self { state }
    }
}

impl Render for FindBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let find_visible = state.find_bar_visible;
        let replace_visible = state.replace_bar_visible;
        let query = state.find_query.clone();
        let replace = state.replace_text.clone();
        let match_count = if query.is_empty() {
            0
        } else {
            state.document.text().matches(&query).count()
        };

        if !find_visible {
            return div().id("find-bar-hidden");
        }

        let state_for_close = self.state.clone();
        let state_for_next = self.state.clone();
        let state_for_replace = self.state.clone();
        let state_for_replace_all = self.state.clone();

        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let text_primary = theme.text_primary;
        let surface_hover = theme.surface_hover;

        let mut bar = div()
            .id("find-bar")
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .bg(surface)
            .border_b_1()
            .border_color(border);

        // Find row
        bar = bar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text_muted)
                        .child("Find:"),
                )
                .child(
                    div()
                        .flex_grow()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(border)
                        .text_size(px(13.0))
                        .text_color(text_primary)
                        .font_family("monospace")
                        .child(if query.is_empty() {
                            div().text_color(text_muted).child("Type to search...")
                        } else {
                            div().child(query)
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(text_muted)
                        .child(format!("{} matches", match_count)),
                )
                .child(find_bar_button("Next", surface_hover, text_primary, move |_window, cx| {
                    state_for_next.update(cx, |s, _cx| s.find_next());
                }))
                .child(find_bar_button("Close", surface_hover, text_primary, move |_window, cx| {
                    state_for_close.update(cx, |s, _cx| s.toggle_find());
                })),
        );

        if replace_visible {
            bar = bar.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(text_muted)
                            .w(px(35.0))
                            .child("Replace:"),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(border)
                            .text_size(px(13.0))
                            .text_color(text_primary)
                            .font_family("monospace")
                            .child(if replace.is_empty() {
                                div().text_color(text_muted).child("Replace with...")
                            } else {
                                div().child(replace)
                            }),
                    )
                    .child(find_bar_button("Replace", surface_hover, text_primary, move |_window, cx| {
                        state_for_replace.update(cx, |s, _cx| s.replace_next());
                    }))
                    .child(find_bar_button("All", surface_hover, text_primary, move |_window, cx| {
                        state_for_replace_all.update(cx, |s, _cx| s.replace_all());
                    })),
            );
        }

        bar
    }
}

fn find_bar_button(
    label: &str,
    hover_bg: Rgba,
    text_color: Rgba,
    handler: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let label = label.to_string();
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .text_size(px(11.0))
        .text_color(text_color)
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_up(MouseButton::Left, move |_ev, window, cx| {
            handler(window, cx);
        })
        .child(label)
        .into_any_element()
}
