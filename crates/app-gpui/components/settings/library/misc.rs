#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::LibraryFolderTranslations;
#[cfg(feature = "dev-api")]
use crate::app::state::QaLibraryPickerResult;
use crate::app::types::ReplayGainMode;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::accessibility::{
    AccessibilityExt, AccessibilityNode, AriaProps, AriaRole, AriaState, apply_native_accessibility,
};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Divider, HStack, NumberInput, NumberInputSize, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

fn register_library_button(
    cx: &mut App,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> ElementId {
    let id = ElementId::from(id.into());
    cx.register_accessible(AccessibilityNode {
        element_id: id.clone(),
        label: label.into(),
        props: AriaProps::with_role(AriaRole::Button),
    });
    id
}

fn apply_library_button_accessibility(
    button: Stateful<Div>,
    label: impl Into<SharedString>,
    disabled: bool,
) -> Stateful<Div> {
    apply_native_accessibility(
        button,
        label,
        &AriaProps::with_role(AriaRole::Button).maybe_state(disabled, AriaState::Disabled),
    )
}

impl PlayerView {
    fn commit_library_settings_directory(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            if state.app.add_directory(path) {
                let layout = state.layout.read(cx);
                if let Err(error) = state.app.save_config(layout) {
                    log::warn!("Failed to persist library directory: {error}");
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                        format!("Could not save the library folder: {error}"),
                    ));
                }
            }
        });
        cx.notify();
    }

    fn choose_library_settings_directory(&mut self, cx: &mut Context<Self>) {
        #[cfg(feature = "dev-api")]
        {
            let qa_result = self.state.update(cx, |state, _cx| {
                state.app.settings.library.qa_picker_result.take()
            });
            if let Some(result) = qa_result {
                match result {
                    QaLibraryPickerResult::Selected(path) => {
                        self.commit_library_settings_directory(path, cx);
                    }
                    QaLibraryPickerResult::PermissionDenied(path) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.settings.library.directory_error = Some(
                                sotf_audio_player::LibraryDirectoryAccessError::PermissionDenied(
                                    path,
                                ),
                            );
                        });
                        cx.notify();
                    }
                }
                return;
            }
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        cx.spawn(async move |view: WeakEntity<PlayerView>, cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await {
                let path = handle.path().to_path_buf();
                let _ = view.update(cx, |view, cx| {
                    view.commit_library_settings_directory(path, cx);
                });
            }
        })
        .detach();
    }

    fn confirm_remove_library_settings_directory(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.settings.library.pending_remove_index = None;
            if state.app.remove_library_directory(index) {
                let layout = state.layout.read(cx);
                if let Err(error) = state.app.save_config(layout) {
                    log::warn!("Failed to persist library directory removal: {error}");
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                        format!("Could not save the library folder removal: {error}"),
                    ));
                }
            }
        });
        cx.notify();
    }

    pub(crate) fn render_library_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (
            theme,
            translations,
            folder_text,
            scan_in_progress,
            directories,
            directory_error,
            scan_error,
            pending_remove_index,
            replay_gain_enabled,
            replay_gain_mode,
            album_count,
            track_count,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
                LibraryFolderTranslations::for_language(state.app.ui_state.language),
                state.app.library_state.scan_in_progress,
                state.app.library_state.library.directories.clone(),
                state.app.settings.library.directory_error.clone(),
                state.app.settings.library.scan_error.clone(),
                state.app.settings.library.pending_remove_index,
                state.app.playback.replay_gain_enabled,
                state.app.playback.replay_gain_mode,
                state.app.library_state.library.albums.len(),
                state
                    .app
                    .library_state
                    .library
                    .albums
                    .iter()
                    .map(|album| album.tracks.len())
                    .sum::<usize>(),
            )
        };

        div()
            .flex()
            .flex_col()
            .gap(d.section)
            // Library Overview Stats
            .child(
                div()
                    .flex()
                    .gap(d.section_lg)
                    .p(d.card)
                    .bg(theme.background_secondary)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_total_albums),
                            )
                            .child(
                                div()
                                    .text_size(d.text_lg)
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
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_total_tracks),
                            )
                            .child(
                                div()
                                    .text_size(d.text_lg)
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
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child(translations.settings_directories),
                            )
                            .child(
                                div()
                                    .text_size(d.text_lg)
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
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_managed_directories),
                    )
                    .child({
                        let id = register_library_button(
                            cx,
                            "settings-library-add-button",
                            translations.directories_add,
                        );
                        let button = Button::new(id, translations.directories_add)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                view.choose_library_settings_directory(cx);
                            }));
                        let button = apply_library_button_accessibility(
                            button,
                            translations.directories_add,
                            false,
                        );
                        #[cfg(feature = "dev-api")]
                        let button = button.dev_track("settings.library.add");
                        button
                    }),
            )
            .when_some(directory_error, |content, error| {
                let id = register_library_button(
                    cx,
                    "settings-library-folder-retry-button",
                    folder_text.retry,
                );
                let retry = Button::new(id, folder_text.retry)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .build()
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.choose_library_settings_directory(cx);
                    }));
                let retry = apply_library_button_accessibility(retry, folder_text.retry, false);
                #[cfg(feature = "dev-api")]
                let retry = retry.dev_track("settings.library.retry-folder");

                let alert = div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.error)
                    .bg(theme.background_secondary)
                    .child(Text::section_header(folder_text.access_problem).color(theme.error))
                    .child(Text::body(folder_text.access_error_message(&error)))
                    .child(div().child(retry));
                #[cfg(feature = "dev-api")]
                let alert = alert.dev_track("settings.library.folder-error");
                content.child(alert)
            })
            // Directories Table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(theme.border)
                    .rounded(d.r_md)
                    .overflow_hidden()
                    .children(directories.iter().enumerate().map(|(idx, dir)| {
                        let theme = theme.clone();
                        let bg = if idx % 2 == 0 {
                            theme.background
                        } else {
                            theme.background_secondary
                        };
                        let action = if pending_remove_index == Some(idx) {
                            let cancel_id = register_library_button(
                                cx,
                                format!("settings-library-remove-{idx}-cancel"),
                                folder_text.cancel,
                            );
                            let cancel = Button::new(cancel_id, folder_text.cancel)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.settings.library.pending_remove_index = None;
                                        cx.notify();
                                    });
                                }));
                            let cancel = apply_library_button_accessibility(
                                cancel,
                                folder_text.cancel,
                                false,
                            );
                            #[cfg(feature = "dev-api")]
                            let cancel =
                                cancel.dev_track(format!("settings.library.remove.{idx}.cancel"));

                            let confirm_id = register_library_button(
                                cx,
                                format!("settings-library-remove-{idx}-confirm"),
                                folder_text.confirm_remove,
                            );
                            let confirm = Button::new(confirm_id, folder_text.confirm_remove)
                                .variant(ButtonVariant::Destructive)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    view.confirm_remove_library_settings_directory(idx, cx);
                                }));
                            let confirm = apply_library_button_accessibility(
                                confirm,
                                folder_text.confirm_remove,
                                false,
                            );
                            #[cfg(feature = "dev-api")]
                            let confirm =
                                confirm.dev_track(format!("settings.library.remove.{idx}.confirm"));

                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap(d.grid)
                                .child(
                                    Text::label(folder_text.remove_question).color(theme.warning),
                                )
                                .child(Text::caption(folder_text.remove_warning))
                                .child(div().flex().gap(d.gap_md).child(cancel).child(confirm))
                                .into_any_element()
                        } else {
                            let remove_id = register_library_button(
                                cx,
                                format!("settings-library-remove-{idx}"),
                                translations.settings_remove,
                            );
                            let remove = Button::new(remove_id, translations.settings_remove)
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.settings.library.pending_remove_index = Some(idx);
                                        cx.notify();
                                    });
                                }));
                            let remove = apply_library_button_accessibility(
                                remove,
                                translations.settings_remove,
                                false,
                            );
                            #[cfg(feature = "dev-api")]
                            let remove = remove.dev_track(format!("settings.library.remove.{idx}"));
                            remove.into_any_element()
                        };

                        let row = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .when(pending_remove_index == Some(idx), |row| {
                                row.flex_col().items_start()
                            })
                            .gap(d.gap_md)
                            .p(d.pad_x)
                            .bg(bg)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .overflow_hidden()
                                            .child(dir.path.display().to_string()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap(d.section)
                                            .text_size(d.text_xs)
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
                            .child(action);
                        #[cfg(feature = "dev-api")]
                        let row = row.dev_track(format!("settings.library.directory.{idx}"));
                        row
                    })),
            )
            // Scan Progress Popup (removed as per user request)
            .child(div().h_0())
            // Actions Section
            .child(
                div()
                    .mt(d.section)
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_library_actions),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child({
                                let label = if scan_in_progress {
                                    translations.library_scanning
                                } else {
                                    translations.library_scan
                                };
                                let id = register_library_button(
                                    cx,
                                    "settings-library-scan-button",
                                    label,
                                );
                                let button = Button::new(id, label)
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .disabled(scan_in_progress)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                        view.start_library_scan(cx);
                                    }));
                                let button = apply_library_button_accessibility(
                                    button,
                                    label,
                                    scan_in_progress,
                                );
                                #[cfg(feature = "dev-api")]
                                let button = button.dev_track("settings.library.scan");
                                button
                            })
                            .child({
                                let id = register_library_button(
                                    cx,
                                    "settings-library-rescan-button",
                                    translations.settings_rescan_all,
                                );
                                let button = Button::new(id, translations.settings_rescan_all)
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .disabled(scan_in_progress)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            let _ = state.app.rescan_library();
                                        });
                                        cx.notify();
                                    }));
                                let button = apply_library_button_accessibility(
                                    button,
                                    translations.settings_rescan_all,
                                    scan_in_progress,
                                );
                                #[cfg(feature = "dev-api")]
                                let button = button.dev_track("settings.library.rescan");
                                button
                            }),
                    ),
            )
            .when_some(scan_error, |content, error| {
                let id = register_library_button(
                    cx,
                    "settings-library-scan-retry-button",
                    folder_text.retry_scan,
                );
                let retry = Button::new(id, folder_text.retry_scan)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .build()
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.start_library_scan(cx);
                    }));
                let retry =
                    apply_library_button_accessibility(retry, folder_text.retry_scan, false);
                #[cfg(feature = "dev-api")]
                let retry = retry.dev_track("settings.library.retry-scan");
                let alert = div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.error)
                    .bg(theme.background_secondary)
                    .child(Text::section_header(folder_text.scan_failed).color(theme.error))
                    .child(Text::body(error))
                    .child(div().child(retry));
                #[cfg(feature = "dev-api")]
                let alert = alert.dev_track("settings.library.scan-error");
                content.child(alert)
            })
            // ReplayGain Section
            .child(
                div()
                    .mt(d.section)
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_replaygain),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.section)
                            .p(d.card)
                            .bg(theme.background_secondary)
                            .rounded(d.r_md)
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_enable_replaygain),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
                                                    .text_color(theme.text_secondary)
                                                    .child(translations.settings_replaygain_desc),
                                            ),
                                    )
                                    .child(
                                        // Toggle switch (simulated with button for now or use Checkbox if available)
                                        Button::new(
                                            "replay-gain-toggle",
                                            if replay_gain_enabled {
                                                translations.settings_on
                                            } else {
                                                translations.settings_off
                                            },
                                        )
                                        .variant(if replay_gain_enabled {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .size(ButtonSize::Xs)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_mode),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
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
                                                    if replay_gain_mode == ReplayGainMode::Track {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    },
                                                )
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(
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
                                                    if replay_gain_mode == ReplayGainMode::Album {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    },
                                                )
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(
                                                        translations.settings_compute_replaygain,
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
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
                                        .on_click_event(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.scan_replay_gain();
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
                    .mt(d.section)
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_audio_analysis),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.section)
                            .p(d.card)
                            .bg(theme.background_secondary)
                            .rounded(d.r_md)
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_compute_bliss),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
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
                                        .on_click_event(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.scan_bliss();
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_compute_waveform),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
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
                                        .on_click_event(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.compute_waveform();
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
                    .mt(d.section)
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .child(translations.settings_database_maintenance),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.section)
                            .p(d.card)
                            .bg(theme.background_secondary)
                            .rounded(d.r_md)
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
                                                    .text_size(d.text_sm)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(translations.settings_clean_database),
                                            )
                                            .child(
                                                div()
                                                    .text_size(d.text_xs)
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
                                            .on_click_event(cx.listener(
                                                |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.clear_local_library();
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
    }

    /// Render the scanner threads section for library settings
    pub(super) fn render_scanner_threads_section(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = SettingsSurfaceTranslations::for_language(state.app.ui_state.language);
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
            .mt(d.section)
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::BOLD)
                    .child(text.scanner_threads),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    .p(d.card)
                    .bg(theme.background_secondary)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                VStack::new()
                                    .spacing(gpui_ui_kit::StackSpacing::Xs)
                                    .child(
                                        Text::new(text.thread_count)
                                            .size(TextSize::Sm)
                                            .weight(TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new(text.thread_count_description)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child(
                                NumberInput::new("scanner-threads-input")
                                    .label(text.thread_count)
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
                                                .scan
                                                .ctrl
                                                .set_num_threads(Some(threads as usize));
                                        });
                                    }),
                            ),
                    ),
            )
    }
}
use crate::app::i18n::SettingsSurfaceTranslations;
