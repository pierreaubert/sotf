//! Library settings content

use crate::app::types::ReplayGainMode;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Divider, HStack, NumberInput, NumberInputSize, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_library_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let scan_in_progress = state.app.library_state.scan_in_progress;
        let scan_progress_tracks = state.app.library_state.scan_progress_tracks;
        let scan_progress_albums = state.app.library_state.scan_progress_albums;
        let directories = state.app.library_state.library.directories.clone();
        let album_count = state.app.library_state.library.albums.len();
        let track_count: usize = state
            .app
            .library_state
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
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_total_albums),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{}", album_count)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_total_tracks),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{}", track_count)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_directories),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{}", directories.len())),
                            ),
                    ),
            )
            // Directories List Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_managed_directories),
                    )
                    .child(
                        Button::new("add-directory-btn", translations.directories_add)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|_view, _: &MouseUpEvent, _window, cx| {
                                    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
                                    {
                                        cx.spawn(async move |view: WeakEntity<PlayerView>, cx| {
                                            if let Some(handle) =
                                                rfd::AsyncFileDialog::new().pick_folder().await
                                            {
                                                let path = handle.path().to_path_buf();
                                                let _ = view.update(cx, |view, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.add_directory(path);
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .detach();
                                    }
                                }),
                            ),
                    ),
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
                    .children(directories.iter().enumerate().map(|(idx, dir)| {
                        let theme = theme.clone();
                        let bg = if idx % 2 == 0 {
                            theme.background
                        } else {
                            theme.background_secondary
                        };

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
                                            .child(format!(
                                                "{} {}",
                                                dir.album_count,
                                                translations.library_albums.to_lowercase()
                                            ))
                                            .child(format!(
                                                "{} {}",
                                                dir.file_count,
                                                translations.library_tracks.to_lowercase()
                                            )),
                                    ),
                            )
                            .child(
                                Button::new(("remove-btn", idx), translations.settings_remove)
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Xs)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_click(cx.listener(
                                        move |view, _: &ClickEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                // Set selection to this index and remove
                                                state.app.selected_directory_index = idx;
                                                state.app.remove_selected_directory();
                                            });
                                            cx.notify();
                                        },
                                    )),
                            )
                    })),
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
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_library_actions),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Button::new(
                                    "scan-btn",
                                    if scan_in_progress {
                                        translations.library_scanning
                                    } else {
                                        translations.library_scan
                                    },
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .disabled(scan_in_progress)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(
                                    |view, _: &ClickEvent, _window, cx| {
                                        view.start_library_scan(cx);
                                    },
                                )),
                            )
                            .child(
                                Button::new("rescan-btn", translations.settings_rescan_all)
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .disabled(scan_in_progress)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            if state.app.rescan_library().is_ok()
                                                && state.app.library_state.scan_in_progress
                                            {
                                                // Show progress modal
                                                state.app.scan_progress_modal = Some(
                                                    crate::app::types::ScanProgressModal::new(
                                                        crate::app::types::ScanType::Library,
                                                    ),
                                                );
                                            }
                                        });
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // ReplayGain Section
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_replaygain),
                    )
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
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_enable_replaygain),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(translations.settings_replaygain_desc),
                                            ),
                                    )
                                    .child(
                                        // Toggle switch (simulated with button for now or use Checkbox if available)
                                        Button::new(
                                            "replay-gain-toggle",
                                            if state.app.playback.replay_gain_enabled {
                                                translations.settings_on
                                            } else {
                                                translations.settings_off
                                            },
                                        )
                                        .variant(if state.app.playback.replay_gain_enabled {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .size(ButtonSize::Xs)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.playback.replay_gain_enabled =
                                                        !state.app.playback.replay_gain_enabled;
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(Divider::new().color(theme.border))
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_mode),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(translations.settings_mode_desc),
                                            ),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                Button::new(
                                                    "rg-mode-track",
                                                    translations.settings_track,
                                                )
                                                .variant(
                                                    if state.app.playback.replay_gain_mode
                                                        == ReplayGainMode::Track
                                                    {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    },
                                                )
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(
                                                    |view, _: &ClickEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.playback.replay_gain_mode =
                                                                ReplayGainMode::Track;
                                                        });
                                                        cx.notify();
                                                    },
                                                )),
                                            )
                                            .child(
                                                Button::new(
                                                    "rg-mode-album",
                                                    translations.settings_album,
                                                )
                                                .variant(
                                                    if state.app.playback.replay_gain_mode
                                                        == ReplayGainMode::Album
                                                    {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    },
                                                )
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(
                                                    |view, _: &ClickEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.playback.replay_gain_mode =
                                                                ReplayGainMode::Album;
                                                        });
                                                        cx.notify();
                                                    },
                                                )),
                                            ),
                                    ),
                            )
                            .child(Divider::new().color(theme.border))
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(
                                                        translations.settings_compute_replaygain,
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(
                                                        translations
                                                            .settings_compute_replaygain_desc,
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            "replaygain-scan-btn",
                                            translations.settings_compute,
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .disabled(scan_in_progress) // Also disable if library scan is running
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.scan_replay_gain();
                                                    // Show progress modal if scan started
                                                    if state
                                                        .app
                                                        .scan_ctrl
                                                        .replay_gain_manager
                                                        .in_progress
                                                    {
                                                        state.app.scan_progress_modal = Some(
                                                        crate::app::types::ScanProgressModal::new(
                                                            crate::app::types::ScanType::ReplayGain,
                                                        ),
                                                    );
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            // Audio Analysis Section (Bliss)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_audio_analysis),
                    )
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
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_compute_bliss),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(
                                                        translations.settings_compute_bliss_desc,
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            "bliss-scan-btn",
                                            translations.settings_compute,
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .disabled(scan_in_progress)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.scan_bliss();
                                                    // Show progress modal if scan started
                                                    if state.app.scan_ctrl.bliss_manager.in_progress
                                                    {
                                                        state.app.scan_progress_modal = Some(
                                                        crate::app::types::ScanProgressModal::new(
                                                            crate::app::types::ScanType::Bliss,
                                                        ),
                                                    );
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(Divider::new().color(theme.border))
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_compute_waveform),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(
                                                        translations.settings_compute_waveform_desc,
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            "waveform-scan-btn",
                                            translations.settings_compute,
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .disabled(scan_in_progress)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.compute_waveform();
                                                    // Show progress modal if scan started
                                                    if state
                                                        .app
                                                        .scan_ctrl
                                                        .waveform_manager
                                                        .in_progress
                                                    {
                                                        state.app.scan_progress_modal = Some(
                                                        crate::app::types::ScanProgressModal::new(
                                                            crate::app::types::ScanType::Waveform,
                                                        ),
                                                    );
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            // Scanner Threads Section
            .child(self.render_scanner_threads_section(cx))
            // Database Maintenance Section
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_database_maintenance),
                    )
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
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_clean_database),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
                                                    .child(
                                                        translations.settings_clean_database_desc,
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new("clean-db-btn", translations.settings_clean)
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .disabled(scan_in_progress)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.clean_database();
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            ),
                    ),
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
                        .child(
                            div()
                                .text_sm()
                                .child(translations.settings_scanning_in_progress),
                        )
                        .child(
                            div().text_xs().text_color(theme.text_secondary).child(
                                translations
                                    .settings_scan_progress
                                    .replace("{}", &scan_progress_tracks.to_string())
                                    .replacen("{}", &scan_progress_albums.to_string(), 1),
                            ),
                        ),
                )
            })
    }

    /// Render the scanner threads section for library settings
    fn render_scanner_threads_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let current_threads = state.app.ui_state.scanner_threads;
        let max_cores = state.app.ui_state.max_cpu_cores;

        let max_allowed = max_cores.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() as u8)
                .unwrap_or(4)
        });
        let current_value = current_threads.unwrap_or(max_allowed.min(4)) as f64;

        let state_entity = self.state.clone();

        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Scanner Threads"),
            )
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
                                VStack::new()
                                    .spacing(gpui_ui_kit::StackSpacing::Xs)
                                    .child(
                                        Text::new("Thread Count")
                                            .size(TextSize::Sm)
                                            .weight(TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new("Number of threads for background scanning (waveform, bliss, replaygain). Lower values reduce memory usage.")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child(
                                NumberInput::new("scanner-threads-input")
                                    .value(current_value)
                                    .range(1.0, max_allowed as f64)
                                    .step(1.0)
                                    .decimals(0)
                                    .size(NumberInputSize::Sm)
                                    .width(100.0)
                                    .on_change(move |val, _window, cx| {
                                        let threads = (val as u8).clamp(1, max_allowed);
                                        state_entity.update(cx, |state, _cx| {
                                            state.app.ui_state.scanner_threads = Some(threads);
                                            state
                                                .app
                                                .scan_ctrl
                                                .set_num_threads(Some(threads as usize));
                                        });
                                    }),
                            ),
                    ),
            )
    }
}
