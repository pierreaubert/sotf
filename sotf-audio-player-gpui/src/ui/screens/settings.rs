//! Settings screen rendering functions

use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::ui::PlayerView;
use gpui_ui_kit::{Button, ButtonVariant, ButtonSize, HStack, StackSpacing};
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let expanded = state.app.expanded_settings_sections.clone();

        let library_expanded = expanded.contains(&"library".to_string());
        let appearance_expanded = expanded.contains(&"appearance".to_string());
        let audio_device_expanded = expanded.contains(&"audio-device".to_string());
        let plugins_expanded = expanded.contains(&"plugins".to_string());
        let room_eq_expanded = expanded.contains(&"room-eq".to_string());
        let headphone_expanded = expanded.contains(&"headphone".to_string());

        // Pre-render all content sections (convert to AnyElement to release borrow)
        let library_content = self.render_library_settings_content(cx).into_any_element();
        let appearance_content = self.render_appearance_settings_content(cx).into_any_element();
        let audio_device_content = self.render_audio_device_settings_content(cx).into_any_element();
        let plugins_content = self.render_plugins_screen(cx).into_any_element();
        let room_eq_content = self.render_roomeq_settings_content(cx).into_any_element();
        let headphone_content = self.render_headphone_settings_content(cx).into_any_element();

        // Pre-render all headers (convert to AnyElement to release borrow)
        let library_header = self.render_accordion_header("library", "Library", library_expanded, true, cx).into_any_element();
        let appearance_header = self.render_accordion_header("appearance", "Appearance", appearance_expanded, false, cx).into_any_element();
        let audio_device_header = self.render_accordion_header("audio-device", "Audio Device", audio_device_expanded, false, cx).into_any_element();
        let plugins_header = self.render_accordion_header("plugins", "Plugins", plugins_expanded, false, cx).into_any_element();
        let room_eq_header = self.render_accordion_header("room-eq", "Room EQ", room_eq_expanded, false, cx).into_any_element();
        let headphone_header = self.render_accordion_header("headphone", "Headphone", headphone_expanded, false, cx).into_any_element();

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .overflow_hidden()
                    // Library section
                    .child(library_header)
                    .when(library_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(library_content)
                        )
                    })
                    // Appearance section
                    .child(appearance_header)
                    .when(appearance_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(appearance_content)
                        )
                    })
                    // Audio Device section
                    .child(audio_device_header)
                    .when(audio_device_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(audio_device_content)
                        )
                    })
                    // Plugins section
                    .child(plugins_header)
                    .when(plugins_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(plugins_content)
                        )
                    })
                    // Room EQ section
                    .child(room_eq_header)
                    .when(room_eq_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(room_eq_content)
                        )
                    })
                    // Headphone section
                    .child(headphone_header)
                    .when(headphone_expanded, |el| {
                        el.child(
                            div()
                                .px_4()
                                .py_3()
                                .bg(theme.background)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(headphone_content)
                        )
                    })
            )
    }

    fn render_accordion_header(
        &self,
        id: &'static str,
        title: &'static str,
        is_expanded: bool,
        is_first: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();
        let id_string = id.to_string();

        let mut header = div()
            .id(SharedString::from(format!("accordion-header-{}", id)))
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .bg(theme.surface)
            .cursor_pointer()
            .hover(|s| s.bg(theme.surface_hover))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    let id = id_string.clone();
                    view.state.update(cx, |state, _cx| {
                        if state.app.expanded_settings_sections.contains(&id) {
                            state.app.expanded_settings_sections.retain(|s| s != &id);
                        } else {
                            state.app.expanded_settings_sections.push(id);
                        }
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_primary)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(if is_expanded { "▼" } else { "▶" }),
            );

        if !is_first {
            header = header.border_t_1().border_color(theme.border);
        }

        header
    }

    fn render_library_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                                        state.app.input_mode =
                                            crate::app::InputMode::AddDirectory;
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
                            if scan_in_progress { "Scanning..." } else { "Scan Library" }
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

    fn render_appearance_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme_id = state.app.theme_id;
        let language = state.app.language;
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap_6()
            // Theme selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Theme"),
                    )
                    .child({
                        let button_theme = theme.to_button_theme();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(ThemeId::all().iter().map(|id| {
                                let is_selected = theme_id == *id;
                                let id = *id;
                                let btn_theme = button_theme.clone();
                                Button::new(
                                    SharedString::from(format!("theme-{}", id.name())),
                                    id.name()
                                )
                                    .variant(if is_selected { ButtonVariant::Primary } else { ButtonVariant::Secondary })
                                    .size(ButtonSize::Sm)
                                    .selected(is_selected)
                                    .theme(btn_theme)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.set_theme(id);
                                            });
                                            cx.notify();
                                        }),
                                    )
                            }))
                    }),
            )
            // Language selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Language"),
                    )
                    .child({
                        let button_theme = theme.to_button_theme();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(Language::all().iter().map(|lang| {
                                let is_selected = language == *lang;
                                let lang = *lang;
                                let btn_theme = button_theme.clone();
                                Button::new(
                                    SharedString::from(format!("language-{}", lang.name())),
                                    lang.name()
                                )
                                    .variant(if is_selected { ButtonVariant::Primary } else { ButtonVariant::Secondary })
                                    .size(ButtonSize::Sm)
                                    .selected(is_selected)
                                    .theme(btn_theme)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.set_language(lang);
                                            });
                                            cx.notify();
                                        }),
                                    )
                            }))
                    }),
            )
    }

    fn render_audio_device_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
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
            )
    }

    fn render_roomeq_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .text_sm()
            .text_color(theme.text_secondary)
            .child("Room equalization and speaker correction settings will be added here.")
    }

    fn render_headphone_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .gap_4()
            // Headphone EQ intro
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
            // EQ Design Parameters (reusable component)
            .child(self.render_eq_design_params(&headphone_params, "headphone", &theme, cx))
            // Optimization Fine Tuning Parameters (reusable component)
            .child(self.render_optimization_tuning_params(&headphone_params, "headphone", &theme, cx))
            // Generate EQ button
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
                    .mt_4()
                    .pt_4()
                    .border_t_1()
                    .border_color(theme.border)
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
