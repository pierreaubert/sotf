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
            // Library Overview Stats
            .child(
                div()
                    .flex()
                    .gap_6()
                    .p_4()
                    .bg(theme.background_secondary)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                         div().flex().flex_col()
                            .child(div().text_xs().text_color(theme.text_secondary).child("TOTAL ALBUMS"))
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child(format!("{}", album_count)))
                    )
                    .child(
                         div().flex().flex_col()
                            .child(div().text_xs().text_color(theme.text_secondary).child("TOTAL TRACKS"))
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child(format!("{}", track_count)))
                    )
                     .child(
                         div().flex().flex_col()
                            .child(div().text_xs().text_color(theme.text_secondary).child("DIRECTORIES"))
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child(format!("{}", directories.len())))
                    )
            )
            // Directories List Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Managed Directories"))
                    .child(
                         Button::new("add-directory-btn", "Add Directory")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Sm)
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    cx.spawn(async move |view: WeakEntity<PlayerView>, mut cx| {
                                        if let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await {
                                            let path = handle.path().to_path_buf();
                                            let _ = view.update(cx, |view, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.add_directory(path);
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }).detach();
                                }),
                            ),
                    )
            )
            // Directories Table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .overflow_hidden()
                    .children(
                        directories.iter().enumerate().map(|(idx, dir)| {
                            let theme = theme.clone();
                            let bg = if idx % 2 == 0 { theme.background } else { theme.background_secondary };
                            
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .p_3()
                                .bg(bg)
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(div().text_sm().child(dir.path.display().to_string()))
                                        .child(
                                            div()
                                            .flex()
                                            .gap_4()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child(format!("{} albums", dir.album_count))
                                            .child(format!("{} tracks", dir.file_count))
                                        )
                                )
                                .child(
                                     Button::new(("remove-btn", idx), "Remove")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        // TODO: Implement actual remove specific directory command
                                        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                // Temporarily use selection index for remove, hacky but matches current API
                                                state.app.selected_directory_index = idx;
                                                state.app.remove_selected_directory();
                                            });
                                            cx.notify();
                                        }))
                                )
                        })
                    )
            )
            // Scan Progress Popup (Overlay-like or inline)
            .child(
                if scan_in_progress {
                    div()
                        .mt_4()
                        .p_4()
                        .rounded_md()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_sm().font_weight(FontWeight::BOLD).child("Library Scan in Progress..."))
                        .child(
                             div().flex().gap_6()
                                .child(div().text_sm().child(format!("Tracks Found: {}", scan_progress_tracks)))
                                .child(div().text_sm().child(format!("Albums Found: {}", scan_progress_albums)))
                        )
                        .child(
                             Button::new("cancel-scan-btn", "Cancel Scan")
                                .variant(ButtonVariant::Destructive)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.cancel_library_scan();
                                    });
                                }))
                        )
                } else {
                    div().flex().child(" ") // Empty placeholder if not scanning? Or just nothing
                }
            )
            // Actions Section
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Library Actions"))
                    .child(
                        HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Button::new("scan-btn", if scan_in_progress { "Scanning..." } else { "Scan Library" })
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .disabled(scan_in_progress)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.start_library_scan(cx);
                                }))
                        )
                        .child(
                             Button::new("rescan-btn", "Rescan All")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .disabled(scan_in_progress)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        let _ = state.app.rescan_library();
                                    });
                                }))
                        )
                        .child(
                             Button::new("replaygain-btn", "Scan ReplayGain")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .disabled(scan_in_progress)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.scan_replay_gain();
                                    });
                                }))
                        )
                    )
            )
             // Progress Indicator
            .when(scan_in_progress, |el| {
                el.child(
                    div()
                        .mt_4()
                        .p_3()
                        .bg(theme.background_secondary)
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().text_sm().child("Scanning in progress..."))
                        .child(
                            div().text_xs().text_color(theme.text_secondary)
                            .child(format!("{} tracks, {} albums found so far", scan_progress_tracks, scan_progress_albums))
                        )
                )
            })
    }
}
