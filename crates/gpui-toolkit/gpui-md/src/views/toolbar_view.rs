use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::MdAppState;

/// Formatting toolbar with bold, italic, code, heading buttons.
pub struct ToolbarView {
    state: Entity<MdAppState>,
}

impl ToolbarView {
    pub fn new(state: Entity<MdAppState>) -> Self {
        Self { state }
    }
}

impl Render for ToolbarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.clone();
        let surface = theme.surface;
        let border = theme.border;
        let text_secondary = theme.text_secondary;
        let surface_hover = theme.surface_hover;

        div()
            .id("toolbar")
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(toolbar_button("B", state.clone(), surface_hover, text_secondary, |s| s.toggle_format("**", "**")))
            .child(toolbar_button("I", state.clone(), surface_hover, text_secondary, |s| s.toggle_format("*", "*")))
            .child(toolbar_button("`", state.clone(), surface_hover, text_secondary, |s| s.toggle_format("`", "`")))
            .child(toolbar_button("S\u{0336}", state.clone(), surface_hover, text_secondary, |s| s.toggle_format("~~", "~~")))
            .child(toolbar_separator(border))
            .child(toolbar_button("H1", state.clone(), surface_hover, text_secondary, |s| s.insert_text("# ")))
            .child(toolbar_button("H2", state.clone(), surface_hover, text_secondary, |s| s.insert_text("## ")))
            .child(toolbar_button("H3", state.clone(), surface_hover, text_secondary, |s| s.insert_text("### ")))
            .child(toolbar_separator(border))
            .child(toolbar_button("Link", state.clone(), surface_hover, text_secondary, |s| s.toggle_format("[", "](url)")))
            .child(toolbar_button("Code", state.clone(), surface_hover, text_secondary, |s| s.insert_text("```\n\n```\n")))
            .child(toolbar_button("---", state.clone(), surface_hover, text_secondary, |s| s.insert_text("\n---\n")))
            .child(toolbar_button("Task", state, surface_hover, text_secondary, |s| s.insert_text("- [ ] ")))
    }
}

fn toolbar_button(
    label: &str,
    state: Entity<MdAppState>,
    hover_bg: Rgba,
    text_color: Rgba,
    action: impl Fn(&mut MdAppState) + 'static,
) -> AnyElement {
    let label = label.to_string();
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .text_size(px(13.0))
        .text_color(text_color)
        .font_weight(FontWeight::MEDIUM)
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
            state.update(cx, |s, _cx| action(s));
        })
        .child(label)
        .into_any_element()
}

fn toolbar_separator(color: Rgba) -> AnyElement {
    div()
        .w(px(1.0))
        .h(px(16.0))
        .mx_1()
        .bg(color)
        .into_any_element()
}
