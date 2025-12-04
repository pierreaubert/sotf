//! Library settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, HStack, StackSpacing};

impl PlayerView {
    pub(crate) fn render_library_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let scan_in_progress = state.app.scan_in_progress;
        let scan_progress_tracks = state.app.scan_progress_tracks;
        let scan_progress_albums = state.app.scan_progress_albums;
        let directories = state.app.library.directories.clone();
        let album_count = state.app.library.albums.len();
        let track_count: usize = state
            .app
            .library
            .albums
            .iter()
            .map(|a| a.tracks.len())
            .sum();

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Library stats
            .child(
                div()
                    .flex()
                    .gap_4()
                    .text_sm()
                    .text_color(theme.text_secondary)
                    .child(format!("{} albums", album_count))
                    .child(format!("{} tracks", track_count))
                    .child(format!("{} directories", directories.len())),
            )
            // Directories list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(directories.iter().map(|dir| {
                        let theme = theme.clone();
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(dir.path.display().to_string())
                    })),
            )
            // Add directory button
            .child({
                let button_theme = theme.to_button_theme();
                let button_theme2 = button_theme.clone();
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Button::new("add-directory-btn", "Add Directory")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(button_theme)
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.input_mode = crate::app::InputMode::AddDirectory;
                                        state.app.directory_input.clear();
                                    });
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        Button::new("manage-directories-btn", "Manage Directories")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(button_theme2)
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.current_screen =
                                            crate::app::Screen::DirectoryManager;
                                    });
                                    cx.notify();
                                }),
                            ),
                    )
                    .build()
            })
            // Scan section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_4()
                    .pt_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Library Scan"),
                    )
                    // Progress bar (if scanning)
                    .when(scan_in_progress, |el| {
                        el.child(
                            gpui::div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    gpui::div()
                                        .text_xs()
                                        .text_color(theme.text_secondary)
                                        .child(format!(
                                            "Scanning... {} tracks, {} albums found",
                                            scan_progress_tracks, scan_progress_albums
                                        )),
                                )
                                .child(
                                    // Progress bar container
                                    gpui::div()
                                        .h(px(8.0))
                                        .bg(theme.background_secondary)
                                        .rounded_full()
                                        .overflow_hidden()
                                        .child(
                                            // Animated progress indicator
                                            gpui::div()
                                                .h_full()
                                                .w(px(100.0))
                                                .bg(theme.accent)
                                                .rounded_full(),
                                        ),
                                ),
                        )
                    })
                    // Scan button
                    .child({
                        let button_theme = theme.to_button_theme();
                        let btn = Button::new(
                            "scan-library-btn",
                            if scan_in_progress {
                                "Scanning..."
                            } else {
                                "Scan Library"
                            },
                        )
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Md)
                        .disabled(scan_in_progress)
                        .theme(button_theme)
                        .build();
                        if scan_in_progress {
                            btn
                        } else {
                            btn.on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.start_library_scan(cx);
                                }),
                            )
                        }
                    }),
            )
    }
}
