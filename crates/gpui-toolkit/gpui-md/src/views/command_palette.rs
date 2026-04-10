//! M-x command palette view -- rendered inline at the bottom of the editor.
//! Key interception is handled by editor_pane.rs on_key_down, not by this view.
//! This view only handles rendering the palette UI.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::{MdAppState, PaletteMode};

pub struct CommandPalette {
    state: Entity<MdAppState>,
}

impl CommandPalette {
    pub fn new(state: Entity<MdAppState>) -> Self {
        Self { state }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = cx.theme();
        let surface_color = theme.surface;
        let border_color = theme.border;
        let text_color = theme.text_primary;
        let accent = theme.accent;

        if !state.command_palette.visible {
            return div();
        }

        let prompt = match state.command_palette.mode {
            PaletteMode::Command => "M-x: ",
            PaletteMode::GotoLine => "Goto line: ",
            PaletteMode::SwitchBuffer => "Switch to buffer: ",
            PaletteMode::KillBuffer => "Kill buffer: ",
        };
        let query = state.command_palette.query.clone();

        let mut items: Vec<(String, String, bool)> = Vec::new();
        if matches!(state.command_palette.mode, PaletteMode::Command) {
            for (display_idx, &cmd_idx) in state.command_palette.filtered_indices.iter().enumerate()
            {
                if display_idx >= 10 {
                    break;
                }
                if let Some(cmd) = state.command_palette.commands.get(cmd_idx) {
                    let selected = display_idx == state.command_palette.selected;
                    items.push((cmd.name.clone(), cmd.description.clone(), selected));
                }
            }
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(surface_color)
            .border_t_1()
            .border_color(border_color)
            .child(
                div()
                    .flex()
                    .px_3()
                    .py_1()
                    .text_size(px(13.0))
                    .font_family("monospace")
                    .text_color(text_color)
                    .child(format!("{}{}", prompt, query)),
            )
            .children(items.into_iter().map(move |(name, desc, selected)| {
                div()
                    .flex()
                    .px_3()
                    .py(px(2.0))
                    .text_size(px(12.0))
                    .font_family("monospace")
                    .text_color(if selected { accent } else { text_color })
                    .when(selected, |el| el.bg(border_color))
                    .child(format!("  {} -- {}", name, desc))
            }))
    }
}
