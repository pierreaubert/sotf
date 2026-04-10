//! Mini-buffer view — renders the active `MiniBufferState` as a bottom bar.
//!
//! Key handling is done in `editor_pane.rs` — this view is pure render.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::MdAppState;

pub struct MiniBufferView {
    state: Entity<MdAppState>,
}

impl MiniBufferView {
    pub fn new(state: Entity<MdAppState>) -> Self {
        Self { state }
    }
}

impl Render for MiniBufferView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = cx.theme();

        if !state.minibuffer.active {
            return div();
        }

        let surface = theme.surface;
        let border = theme.border;
        let text_color = theme.text_primary;
        let accent = theme.accent;
        let text_on_accent = theme.text_on_accent;
        let text_muted = theme.text_muted;

        let label = state
            .minibuffer
            .prompt
            .as_ref()
            .map(|p| p.label())
            .unwrap_or_default();

        // Render input with a simple cursor marker.
        let input_text = state.minibuffer.input.clone();
        let cursor_pos = state.minibuffer.cursor_pos;
        let before: String = input_text.chars().take(cursor_pos).collect();
        let cursor_char: char = input_text.chars().nth(cursor_pos).unwrap_or(' ');
        let after: String = input_text.chars().skip(cursor_pos + 1).collect();

        // Collect up to 10 filtered candidates for display.
        let mut rendered_items: Vec<AnyElement> = Vec::new();
        for (display_idx, &cand_idx) in state
            .minibuffer
            .filtered
            .iter()
            .enumerate()
            .take(10)
        {
            let Some(cand) = state.minibuffer.candidates.get(cand_idx) else {
                continue;
            };
            let selected = display_idx == state.minibuffer.selected;
            let mut item = div()
                .flex()
                .flex_row()
                .px_3()
                .py(px(2.0))
                .text_size(px(12.0))
                .font_family("monospace");
            item = if selected {
                item.bg(accent).text_color(text_on_accent)
            } else {
                item.text_color(text_muted)
            };
            rendered_items.push(item.child(format!("  {}", cand)).into_any_element());
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(surface)
            .border_t_1()
            .border_color(border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .px_3()
                    .py_1()
                    .text_size(px(13.0))
                    .font_family("monospace")
                    .text_color(text_color)
                    .child(label)
                    .child(before)
                    .child(
                        div()
                            .bg(accent)
                            .text_color(text_on_accent)
                            .child(String::from(cursor_char)),
                    )
                    .child(after),
            )
            .children(rendered_items)
    }
}
