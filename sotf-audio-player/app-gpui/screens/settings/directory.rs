//! Directory manager screen rendering functions

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, HStack, Progress, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_directory_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let is_add_mode = state.app.input_mode == crate::app::InputMode::AddDirectory;
        let tree_items = state.app.get_directory_tree_items();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                Text::new("Directory Manager")
                    .size(TextSize::Lg)
                    .weight(TextWeight::Semibold),
            )
            .when(is_add_mode, |parent| {
                let theme = theme.clone();
                parent.child(
                    div()
                        .p_3()
                        .my_4()
                        .rounded_md()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.accent)
                        .child(
                            Text::new("Add Directory")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold),
                        )
                        .child(
                            div()
                                .mt_2()
                                .p_2()
                                .rounded_md()
                                .bg(theme.background)
                                .border_1()
                                .border_color(theme.border)
                                .child(
                                    Text::new(format!(
                                        "{}█",
                                        state.app.directory_input
                                    ))
                                    .size(TextSize::Sm),
                                ),
                        )
                        .child(
                            Text::new("Tab: autocomplete, Enter: add, Esc: cancel")
                                .size(TextSize::Xs)
                                .muted(true),
                        ),
                )
            })
            .child(
                div()
                    .id("directory-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .mt_4()
                    .children(tree_items.iter().enumerate().map(|(i, (path, level, expanded))| {
                        let is_selected = i == state.app.selected_directory_index;
                        let indent = "  ".repeat(*level);
                        let prefix = if *level == 0 {
                            if *expanded { "▼ " } else { "▶ " }
                        } else {
                            "  "
                        };
                        let theme = theme.clone();

                        div()
                            .p_2()
                            .rounded_md()
                            .cursor_pointer()
                            .when(is_selected, |div| div.bg(theme.accent_muted))
                            .when(!is_selected, |div| {
                                div.bg(theme.surface)
                                    .hover(|s| s.bg(theme.surface_hover))
                            })
                            .child(
                                Text::new(format!("{}{}{}", indent, prefix, path.display()))
                                    .size(TextSize::Sm)
                                    .color(if is_selected {
                                        theme.text_primary
                                    } else {
                                        theme.text_secondary
                                    }),
                            )
                    })),
            )
            .when(state.app.scan_in_progress, |parent| {
                let theme = theme.clone();
                parent.child(
                    div()
                        .p_3()
                        .mt_4()
                        .rounded_md()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.success)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Md)
                                        .child(
                                            Text::new("Scanning library...")
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold),
                                        )
                                        .child(
                                            Badge::new(format!(
                                                "{} tracks",
                                                state.app.scan_progress_tracks
                                            ))
                                            .variant(BadgeVariant::Primary),
                                        )
                                        .child(
                                            Badge::new(format!(
                                                "{} albums",
                                                state.app.scan_progress_albums
                                            ))
                                            .variant(BadgeVariant::Info),
                                        ),
                                )
                                // Indeterminate progress - use animated style
                                .child(Progress::new(50.0).striped(true)),
                        ),
                )
            })
            .child(
                div()
                    .p_3()
                    .mt_4()
                    .rounded_md()
                    .bg(theme.background_secondary)
                    .child(
                        Text::new(
                            "Shift-A: Add Directory | Shift-S: Scan Library | D: Remove | Enter/L: Expand | Esc: Cancel",
                        )
                        .size(TextSize::Xs)
                        .muted(true),
                    ),
            )
    }
}
