//! Directory manager screen rendering functions

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_directory_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_add_mode = state.app.input_mode == crate::app::InputMode::AddDirectory;
        let tree_items = state.app.get_directory_tree_items();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Directory Manager"),
            )
            .when(is_add_mode, |parent| {
                parent.child(
                    div()
                        .p_3()
                        .mb_4()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .border_1()
                        .border_color(rgb(0x007acc))
                        .child(div().text_sm().child("Add Directory"))
                        .child(
                            div()
                                .text_sm()
                                .mt_2()
                                .child(format!(
                                    "Path: {}{}",
                                    state.app.directory_input,
                                    if is_add_mode { "█" } else { "" }
                                ))
                        )
                        .child(
                            div()
                                .text_xs()
                                .mt_2()
                                .text_color(rgb(0x999999))
                                .child("Tab: autocomplete, Enter: add, Esc: cancel")
                        )
                )
            })
            .child(
                div()
                    .id("directory-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                                        .children(tree_items.iter().enumerate().map(|(i, (path, level, expanded))| {
                        let is_selected = i == state.app.selected_directory_index;
                        let indent = "  ".repeat(*level);
                        let prefix = if *level == 0 {
                            if *expanded { "▼ " } else { "▶ " }
                        } else {
                            "  "
                        };

                        div()
                            .p_2()
                            .rounded_md()
                            .when(is_selected, |div| div.bg(rgb(0x264f78)))
                            .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}{}{}", indent, prefix, path.display()))
                            )
                    }))
            )
            .when(state.app.scan_in_progress, |parent| {
                parent.child(
                    div()
                        .p_3()
                        .mt_4()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .border_1()
                        .border_color(rgb(0x4ec9b0))
                        .child(div().text_sm().child("Scanning library..."))
                        .child(
                            div()
                                .text_xs()
                                .mt_2()
                                .child(format!(
                                    "{} tracks, {} albums found",
                                    state.app.scan_progress_tracks,
                                    state.app.scan_progress_albums
                                ))
                        )
                )
            })
            .child(
                div()
                    .p_3()
                    .mt_4()
                    .rounded_md()
                    .bg(rgb(0x1e1e1e))
                    .text_xs()
                    .text_color(rgb(0x999999))
                    .child("Shift-A: Add Directory | Shift-S: Scan Library | D: Remove | Enter/L: Expand | Esc: Cancel")
            )
    }
}
