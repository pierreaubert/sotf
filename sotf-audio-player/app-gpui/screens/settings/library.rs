//! Library settings content

use crate::app::types::ReplayGainMode;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Divider, HStack, StackSpacing};

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
            // Scan Progress Popup (removed as per user request)
            .child(div().h_0())
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
                        )
                    )
            // ReplayGain Section
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("ReplayGain"))
                    .child(
                        div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .bg(theme.background_secondary)
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .child(
                             HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .child(div().text_sm().font_weight(FontWeight::BOLD).child("Enable ReplayGain"))
                                        .child(div().text_xs().text_color(theme.text_secondary).child("Automatically adjust volume to a standard level"))
                                )
                                .child(
                                    // Toggle switch (simulated with button for now or use Checkbox if available)
                                    Button::new("replay-gain-toggle", if state.app.replay_gain_enabled { "On" } else { "Off" })
                                        .variant(if state.app.replay_gain_enabled { ButtonVariant::Primary } else { ButtonVariant::Secondary })
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.replay_gain_enabled = !state.app.replay_gain_enabled;
                                            });
                                            cx.notify();
                                        }))
                                )
                        )
                        .child(Divider::new().color(theme.border))
                        .child(
                             HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .child(div().text_sm().font_weight(FontWeight::BOLD).child("Mode"))
                                        .child(div().text_xs().text_color(theme.text_secondary).child("Track (per-song) or Album (per-work) normalization"))
                                )
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Button::new("rg-mode-track", "Track")
                                                .variant(if state.app.replay_gain_mode == ReplayGainMode::Track { ButtonVariant::Primary } else { ButtonVariant::Ghost })
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.replay_gain_mode = ReplayGainMode::Track;
                                                    });
                                                    cx.notify();
                                                }))
                                        )
                                        .child(
                                            Button::new("rg-mode-album", "Album")
                                                .variant(if state.app.replay_gain_mode == ReplayGainMode::Album { ButtonVariant::Primary } else { ButtonVariant::Ghost })
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.replay_gain_mode = ReplayGainMode::Album;
                                                    });
                                                    cx.notify();
                                                }))
                                        )
                                )
                        )
                        .child(Divider::new().color(theme.border))
                        .child(
                             HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .flex_col()
                                        .child(div().text_sm().font_weight(FontWeight::BOLD).child("Compute ReplayGain"))
                                        .child(div().text_xs().text_color(theme.text_secondary).child("Analyze and tag tracks lacking ReplayGain data"))
                                )
                                .child(
                                     Button::new("replaygain-scan-btn", "Compute")
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .disabled(scan_in_progress) // Also disable if library scan is running
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
