//! Settings screen rendering functions

use crate::app::types::SettingsTab;
use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_tab = state.app.selected_settings_tab;
        let theme = state.app.theme.clone();

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .child(self.render_settings_tabs(selected_tab, theme.clone(), cx))
            .child(self.render_settings_content(selected_tab, cx))
    }

    fn render_settings_tabs(
        &self,
        selected_tab: SettingsTab,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .justify_center()
            .gap_px()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(self.render_settings_tab("Library", SettingsTab::Library, selected_tab, theme.clone(), cx))
            .child(self.render_settings_tab("Appearance", SettingsTab::Appearance, selected_tab, theme.clone(), cx))
            .child(self.render_settings_tab("Audio Device", SettingsTab::AudioDevice, selected_tab, theme.clone(), cx))
            .child(self.render_settings_tab("Plugins", SettingsTab::Plugins, selected_tab, theme.clone(), cx))
            .child(self.render_settings_tab("Room EQ", SettingsTab::RoomEQ, selected_tab, theme.clone(), cx))
            .child(self.render_settings_tab("Headphone", SettingsTab::Headphone, selected_tab, theme, cx))
    }

    fn render_settings_tab(
        &self,
        label: &'static str,
        tab: SettingsTab,
        selected_tab: SettingsTab,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = tab == selected_tab;

        div()
            .id(SharedString::from(format!("settings-tab-{:?}", tab)))
            .px_6()
            .py_3()
            .cursor_pointer()
            .when(is_selected, |d| {
                d.bg(theme.surface)
                    .border_t_2()
                    .border_color(theme.accent)
                    .text_color(theme.accent)
            })
            .when(!is_selected, |d| {
                d.bg(theme.background_secondary)
                    .text_color(theme.text_secondary)
                    .hover(|style| style.bg(theme.surface))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.selected_settings_tab = tab;
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            )
    }

    fn render_settings_content(
        &self,
        selected_tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_4()
            .child(match selected_tab {
                SettingsTab::Library => self.render_library_settings(cx).into_any_element(),
                SettingsTab::Appearance => self.render_appearance_settings(cx).into_any_element(),
                SettingsTab::AudioDevice => self.render_audio_device_settings(cx).into_any_element(),
                SettingsTab::Plugins => self.render_plugins_screen(cx).into_any_element(),
                SettingsTab::RoomEQ => self.render_roomeq_settings(cx).into_any_element(),
                SettingsTab::Headphone => self.render_headphone_settings(cx).into_any_element(),
            })
    }

    fn render_library_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let scan_in_progress = state.app.scan_in_progress;
        let scan_progress_tracks = state.app.scan_progress_tracks;
        let scan_progress_albums = state.app.scan_progress_albums;
        let directories = state.app.library.directories.clone();
        let album_count = state.app.library.albums.len();
        let track_count: usize = state.app.library.albums.iter().map(|a| a.tracks.len()).sum();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_6()
            // Library section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Library"),
                    )
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
                            .mt_2()
                            .children(directories.iter().map(|dir| {
                                let theme = theme.clone();
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(dir.path.display().to_string())
                            })),
                    )
                    // Add directory button
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .mt_2()
                            .child(
                                div()
                                    .id("add-directory-btn")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .text_sm()
                                    .bg(theme.surface_hover)
                                    .hover(|style| style.bg(theme.background_tertiary))
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.input_mode =
                                                    crate::app::InputMode::AddDirectory;
                                                state.app.directory_input.clear();
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("Add Directory"),
                            )
                            .child(
                                div()
                                    .id("manage-directories-btn")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .text_sm()
                                    .bg(theme.surface_hover)
                                    .hover(|style| style.bg(theme.background_tertiary))
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.current_screen =
                                                    crate::app::Screen::DirectoryManager;
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("Manage Directories"),
                            ),
                    )
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
                            .child(
                                div()
                                    .id("scan-library-btn")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .when(scan_in_progress, |div| {
                                        div.bg(theme.surface)
                                            .text_color(theme.text_muted)
                                            .cursor_not_allowed()
                                    })
                                    .when(!scan_in_progress, |div| {
                                        div.bg(theme.accent)
                                            .text_color(theme.text_primary)
                                            .cursor_pointer()
                                            .hover(|style| style.opacity(0.9))
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, _: &MouseUpEvent, _window, cx| {
                                                        view.start_library_scan(cx);
                                                    },
                                                ),
                                            )
                                    })
                                    .child(if scan_in_progress {
                                        "Scanning..."
                                    } else {
                                        "Scan Library"
                                    }),
                            ),
                    ),
            )
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme_id = state.app.theme_id;
        let language = state.app.language;
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_6()
            // Theme selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Theme"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(ThemeId::all().iter().map(|id| {
                                let is_selected = theme_id == *id;
                                let id = *id;
                                let theme = theme.clone();
                                div()
                                    .id(SharedString::from(format!("theme-{}", id.name())))
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .when(is_selected, |d| {
                                        d.bg(theme.accent).text_color(theme.text_primary)
                                    })
                                    .when(!is_selected, |d| {
                                        d.bg(theme.surface_hover)
                                            .text_color(theme.text_secondary)
                                            .hover(|style| style.bg(theme.background_tertiary))
                                    })
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.set_theme(id);
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(div().text_sm().child(id.name()))
                            })),
                    ),
            )
            // Language selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Language"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(Language::all().iter().map(|lang| {
                                let is_selected = language == *lang;
                                let lang = *lang;
                                let theme = theme.clone();
                                div()
                                    .id(SharedString::from(format!("language-{}", lang.name())))
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .when(is_selected, |d| {
                                        d.bg(theme.accent).text_color(theme.text_primary)
                                    })
                                    .when(!is_selected, |d| {
                                        d.bg(theme.surface_hover)
                                            .text_color(theme.text_secondary)
                                            .hover(|style| style.bg(theme.background_tertiary))
                                    })
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.set_language(lang);
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(div().text_sm().child(lang.name()))
                            })),
                    ),
            )
    }

    fn render_audio_device_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Audio Output Devices"),
                    )
                    .child(
                        // Grid layout with 2 columns
                        div().grid().grid_cols(2).gap_3().children(
                            state
                                .app
                                .output_devices
                                .iter()
                                .enumerate()
                                .map(|(idx, device)| {
                                    let is_selected = state.app.selected_output_device_index == idx;
                                    let sample_rate = device
                                        .default_config
                                        .as_ref()
                                        .map(|c| c.sample_rate)
                                        .unwrap_or(0);
                                    let channels = device
                                        .default_config
                                        .as_ref()
                                        .map(|c| c.channels)
                                        .unwrap_or(0);
                                    let theme = theme.clone();

                                    div()
                                        .p_3()
                                        .rounded_md()
                                        .when(is_selected, |div| {
                                            div.bg(theme.accent)
                                                .border_2()
                                                .border_color(theme.accent)
                                        })
                                        .when(!is_selected, |div| {
                                            div.bg(theme.surface_hover)
                                                .hover(|style| style.bg(theme.background_tertiary))
                                        })
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.selected_output_device_index = idx;
                                                    if let Some(device) = state.app.output_devices.get(idx)
                                                    {
                                                        state.app.current_output_device_name =
                                                            Some(device.name.clone());

                                                        // If playing, restart track with new device
                                                        if state.app.is_playing {
                                                            if let Some(queue_idx) =
                                                                state.app.current_queue_index
                                                            {
                                                                if let Some(item) =
                                                                    state.app.queue.get(queue_idx)
                                                                {
                                                                    if let Some(track) =
                                                                        item.current_track()
                                                                    {
                                                                        let path = track.path.clone();
                                                                        Self::play_track(state, path);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(device.name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .gap_3()
                                                        .text_xs()
                                                        .text_color(theme.text_secondary)
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .gap_1()
                                                                .child("📊")
                                                                .child(format!("{} ch", channels)),
                                                        )
                                                        .child(div().flex().gap_1().child("🎵").child(
                                                            if sample_rate >= 1000 {
                                                                format!("{} kHz", sample_rate / 1000)
                                                            } else {
                                                                format!("{} Hz", sample_rate)
                                                            },
                                                        )),
                                                )
                                                .when(device.is_default, |this| {
                                                    this.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.success)
                                                            .child("✓ Default"),
                                                    )
                                                }),
                                        )
                                }),
                        ),
                    ),
            )
    }

    fn render_roomeq_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Room EQ Settings"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_secondary)
                            .child("Room equalization and speaker correction settings will be added here."),
                    ),
            )
    }

    fn render_headphone_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, headphone_curve_path, headphone_target, headphone_params, optimization_running, optimization_progress) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.headphone_curve_path.clone(),
                state.app.headphone_target.clone(),
                state.app.headphone_params.clone(),
                state.app.headphone_optimization_running,
                state.app.headphone_optimization_progress.clone(),
            )
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_6()
            // Headphone EQ Optimization section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Headphone EQ Optimization"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_secondary)
                            .child("Generate EQ curves for headphones using industry-standard target curves"),
                    )
                    // Headphone curve file selection
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Headphone Measurement File"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(theme.background_secondary)
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(if headphone_curve_path.is_empty() {
                                                String::from("No file selected")
                                            } else {
                                                headphone_curve_path.clone()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("browse-headphone-curve")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_xs()
                                            .bg(theme.surface_hover)
                                            .hover(|style| style.bg(theme.background_tertiary))
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                    view.browse_headphone_curve(cx);
                                                }),
                                            )
                                            .child("📁 Browse"),
                                    ),
                            ),
                    )
                    // Target curve selection
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Target Curve"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .children(
                                        [
                                            ("harman-over-ear-2018", "Harman Over-Ear 2018"),
                                            ("harman-over-ear-2015", "Harman Over-Ear 2015"),
                                            ("harman-over-ear-2013", "Harman Over-Ear 2013"),
                                            ("harman-in-ear-2019", "Harman In-Ear 2019"),
                                        ]
                                        .iter()
                                        .map(|(value, label)| {
                                            let is_selected = headphone_target == *value;
                                            let value = value.to_string();
                                            let theme = theme.clone();
                                            div()
                                                .id(SharedString::from(format!(
                                                    "headphone-target-{}",
                                                    value
                                                )))
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .text_xs()
                                                .cursor_pointer()
                                                .when(is_selected, |d| {
                                                    d.bg(theme.accent).text_color(theme.text_primary)
                                                })
                                                .when(!is_selected, |d| {
                                                    d.bg(theme.surface_hover)
                                                        .text_color(theme.text_secondary)
                                                        .hover(|style| {
                                                            style.bg(theme.background_tertiary)
                                                        })
                                                })
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _: &MouseUpEvent, _window, cx| {
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.headphone_target =
                                                                    value.clone();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .child(div().text_xs().child(*label))
                                        }),
                                    ),
                            ),
                    )
                    // Loss function selection
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Optimization Goal"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .children(
                                        crate::optimization_params::HEADPHONE_LOSS_OPTIONS
                                            .iter()
                                            .map(|(value, label)| {
                                                let is_selected = headphone_params.loss == *value;
                                                let value = value.to_string();
                                                let theme = theme.clone();
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "headphone-loss-{}",
                                                        value
                                                    )))
                                                    .px_3()
                                                    .py_2()
                                                    .rounded_md()
                                                    .text_xs()
                                                    .cursor_pointer()
                                                    .when(is_selected, |d| {
                                                        d.bg(theme.accent).text_color(theme.text_primary)
                                                    })
                                                    .when(!is_selected, |d| {
                                                        d.bg(theme.surface_hover)
                                                            .text_color(theme.text_secondary)
                                                            .hover(|style| {
                                                                style.bg(theme.background_tertiary)
                                                            })
                                                    })
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |view, _: &MouseUpEvent, _window, cx| {
                                                                view.state.update(cx, |state, _cx| {
                                                                    state.app.headphone_params.loss =
                                                                        value.clone();
                                                                });
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .child(div().text_xs().child(*label))
                                            }),
                                    ),
                            ),
                    )
            )
            // EQ Design Parameters (reusable component)
            .child(self.render_eq_design_params(&headphone_params, "headphone", &theme, cx))
            // Optimization Fine Tuning Parameters (reusable component)
            .child(self.render_optimization_tuning_params(&headphone_params, "headphone", &theme, cx))
            // Generate EQ button
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .id("generate-headphone-eq")
                            .w_full()
                            .px_4()
                            .py_3()
                            .rounded_md()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .bg(theme.accent)
                            .text_color(theme.text_primary)
                            .cursor_pointer()
                            .hover(|style| style.opacity(0.9))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.run_headphone_optimization(cx);
                                }),
                            )
                            .child("🎧 Generate Headphone EQ"),
                    ),
            )
            // Optimization progress (if running)
            .when(optimization_running, |div| {
                let progress = optimization_progress.clone();
                div.child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .bg(theme.surface)
                        .rounded_lg()
                        .child(
                            gpui::div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Optimization Progress"),
                        )
                        .child(
                            gpui::div()
                                .text_xs()
                                .text_color(theme.text_secondary)
                                .child(if progress.is_empty() {
                                    "Starting optimization...".to_string()
                                } else {
                                    format!(
                                        "Iteration: {} | Loss: {:.4}",
                                        progress.last().map(|(i, _)| *i).unwrap_or(0),
                                        progress.last().map(|(_, f)| *f).unwrap_or(0.0)
                                    )
                                }),
                        )
                        .child(
                            // Simple progress indicator
                            gpui::div()
                                .h(px(4.0))
                                .bg(theme.background_secondary)
                                .rounded_full()
                                .overflow_hidden()
                                .child(
                                    gpui::div()
                                        .h_full()
                                        .w(px(100.0))
                                        .bg(theme.accent)
                                        .rounded_full(),
                                ),
                        ),
                )
            })
            // Saved EQ files section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Saved EQ Curves"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("EQ files are stored in ~/Library/Application Support/org.spinorama.sotf/EQ"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .mt_2()
                            .children({
                                let eq_files = self.list_saved_eq_files();
                                if eq_files.is_empty() {
                                    vec![div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("No saved EQ files")]
                                } else {
                                    eq_files
                                        .into_iter()
                                        .map(|path| {
                                            let filename = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("Unknown")
                                                .to_string();
                                            let path_clone = path.clone();
                                            let path_clone2 = path.clone();
                                            let theme = theme.clone();

                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(theme.surface_hover)
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_xs()
                                                        .text_color(theme.text_primary)
                                                        .child(filename),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_sm()
                                                        .text_xs()
                                                        .bg(theme.accent)
                                                        .cursor_pointer()
                                                        .hover(|style| style.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.load_headphone_eq(
                                                                        path_clone.clone(),
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                        .child("Load"),
                                                )
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_sm()
                                                        .text_xs()
                                                        .bg(theme.error)
                                                        .cursor_pointer()
                                                        .hover(|style| style.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.delete_headphone_eq(
                                                                        path_clone2.clone(),
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                        .child("Delete")
                                                )
                                        })
                                        .collect()
                                }
                            }),
                    ),
            )
    }
}
