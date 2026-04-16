//! Mini-buffer view — renders the active `MiniBufferState` as a bottom bar,
//! and shows echo-area hints (C-x prefix, universal arg) when idle.
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

        let surface = theme.surface;
        let border = theme.border;
        let text_color = theme.text_primary;
        let accent = theme.accent;
        let text_on_accent = theme.text_on_accent;
        let text_muted = theme.text_muted;

        // ---- Active minibuffer prompt (M-x, find-file, switch-buffer, etc.) ----
        if state.minibuffer.active {
            let label = state
                .minibuffer
                .prompt
                .as_ref()
                .map(|p| p.label())
                .unwrap_or_default();

            let input_text = state.minibuffer.input.clone();
            let cursor_pos = state.minibuffer.cursor_pos;
            let before: String = input_text.chars().take(cursor_pos).collect();
            let cursor_char: char = input_text.chars().nth(cursor_pos).unwrap_or(' ');
            let after: String = input_text.chars().skip(cursor_pos + 1).collect();

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

            return div()
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
                        .py(px(4.0))
                        .min_h(px(24.0))
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
                .children(rendered_items);
        }

        // ---- Echo area: C-x prefix hints ----
        if state.c_x_r_pending {
            let hints = vec![
                ("k", "kill-rectangle"),
                ("y", "yank-rectangle"),
                ("d", "delete-rectangle"),
            ];
            return self.render_prefix_hints("C-x r ", &hints, surface, border, text_color, text_muted);
        }
        if state.c_x_pending {
            let hints = vec![
                ("C-f", "find-file"),
                ("C-s", "save-buffer"),
                ("C-x", "exchange-point-and-mark"),
                ("C-c", "quit"),
                ("b", "switch-buffer"),
                ("k", "kill-buffer"),
                ("u", "undo"),
                ("h", "select-all"),
                ("(", "start-macro"),
                (")", "stop-macro"),
                ("e", "execute-macro"),
                ("r", "rectangle..."),
            ];
            return self.render_prefix_hints("C-x ", &hints, surface, border, text_color, text_muted);
        }

        // ---- Echo area: empty (single line, like Emacs) ----
        div()
            .w_full()
            .px_3()
            .py(px(4.0))
            .min_h(px(24.0))
            .bg(surface)
            .text_size(px(13.0))
            .font_family("monospace")
            .text_color(text_muted)
    }
}

impl MiniBufferView {
    fn render_prefix_hints(
        &self,
        prefix: &str,
        hints: &[(&str, &str)],
        surface: Rgba,
        border: Rgba,
        text_color: Rgba,
        text_muted: Rgba,
    ) -> Div {
        let mut container = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(surface)
            .border_t_1()
            .border_color(border);

        // Prompt line
        container = container.child(
            div()
                .flex()
                .flex_row()
                .px_3()
                .py_1()
                .text_size(px(13.0))
                .font_family("monospace")
                .text_color(text_color)
                .child(format!("{}-", prefix)),
        );

        // Hint items in a compact grid
        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_x(px(16.0))
            .px_3()
            .py(px(2.0))
            .text_size(px(12.0))
            .font_family("monospace")
            .text_color(text_muted);
        for (key, desc) in hints {
            row = row.child(format!("{} → {}", key, desc));
        }
        container = container.child(row);

        container
    }
}
