// intentional-file: fixed pixel values here are graph and plugin control geometry.
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::RecordingTranslations;
use crate::app::types::{
    CalibrationData, ChannelMapping, CtcMatrixExportStrategy, RecordingState, SpeakerConfiguration,
};
use crate::components::design::Ds;
use crate::components::graphs::common::theme_to_chart_theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{SharedString, div, px, rems};
use gpui_px::{ScaleType, line};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Badge, BadgeVariant, Button, ButtonVariant, Column,
    ComponentSize, HStack, Heading, Input, NumberInput, Select, SelectOption, StackAlign,
    StackSpacing, Table, Text, TextSize, VStack,
};
use sotf_audio_player::recording_helpers::clamp_num_sweeps;

// Match Browse/Clear and keep every config-step control on the same size tier.
const CONFIG_CONTROL_SIZE: ComponentSize = ComponentSize::Sm;

/// Standard channel group definitions
const CHANNEL_GROUPS: &[(&str, &str)] = &[
    ("L", "Left (L)"),
    ("R", "Right (R)"),
    ("C", "Center (C)"),
    ("LFE", "Subwoofer (LFE)"),
    ("Sub1", "Subwoofer 1 (Sub1)"),
    ("Sub2", "Subwoofer 2 (Sub2)"),
    ("SL", "Surround Left (SL)"),
    ("SR", "Surround Right (SR)"),
    ("BL", "Back Left (BL)"),
    ("BR", "Back Right (BR)"),
    ("WL", "Wide Left (WL)"),
    ("WR", "Wide Right (WR)"),
    ("TFL", "Top Front Left (TFL)"),
    ("TFR", "Top Front Right (TFR)"),
    ("TML", "Top Middle Left (TML)"),
    ("TMR", "Top Middle Right (TMR)"),
    ("TBL", "Top Back Left (TBL)"),
    ("TBR", "Top Back Right (TBR)"),
];

impl PlayerView {
    /// Render the config step UI
    pub(crate) fn render_recording_config_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let workflow_text =
            crate::app::i18n::WorkflowTranslations::for_language(state.app.ui_state.language);
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let expanded_sections = state
            .app
            .measurement_state
            .recording_state
            .config_accordion_expanded
            .clone();
        let view = cx.weak_entity();

        // Build accordion content for each section (convert to AnyElement to release borrows)
        let playback_content = self.render_playback_device_content(cx).into_any_element();
        let recording_content = self.render_recording_device_content(cx).into_any_element();
        let calibration_content = self.render_mic_calibration_content(cx).into_any_element();
        let output_dir_content = self.render_output_directory_content(cx).into_any_element();
        let advanced_sweep_content = self
            .render_advanced_sweep_quality_content(cx)
            .into_any_element();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Heading::h4(workflow_text.audio_device_configuration))
                    .child(Text::new(translations.recording_config_desc).size(TextSize::Xs)),
            )
            .child(
                // Accordion with five sections
                Accordion::new()
                    .aria_label(workflow_text.audio_device_configuration)
                    .mode(AccordionMode::Multiple)
                    .expanded(expanded_sections)
                    .item(
                        AccordionItem::new("playback", recording_text.playback_device)
                            .content(playback_content),
                    )
                    .item(
                        AccordionItem::new("recording", recording_text.recording_device)
                            .content(recording_content),
                    )
                    .item(
                        AccordionItem::new("calibration", recording_text.microphone_calibration)
                            .content(calibration_content),
                    )
                    .item(
                        AccordionItem::new("output_dir", recording_text.output_directory)
                            .content(output_dir_content),
                    )
                    .item(
                        AccordionItem::new(
                            "advanced_sweep",
                            recording_text.advanced_measurement_quality,
                        )
                        .content(advanced_sweep_content),
                    )
                    .on_change({
                        let view = view.clone();
                        move |item_id, is_expanded, _window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let expanded = &mut state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .config_accordion_expanded;
                                    if is_expanded {
                                        if !expanded.contains(item_id) {
                                            expanded.push(item_id.clone());
                                        }
                                    } else {
                                        expanded.retain(|id| id != item_id);
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
    }

    /// Render playback device content for accordion
    pub(super) fn render_playback_device_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        let d = Ds::from_cx(cx);
        let device_dropdown = self.render_playback_device_dropdown(cx).into_any_element();
        let sample_rate = self
            .render_playback_sample_rate_dropdown(cx)
            .into_any_element();
        let speaker_config = self.render_speaker_config_dropdown(cx).into_any_element();
        let channel_mapping = self.render_playback_channel_mapping(cx).into_any_element();

        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(Text::label(translations.recording_output_device))
                    .child(div().flex_1().min_w_0().child(device_dropdown)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(d.gap_md)
                    .child(sample_rate)
                    .child(speaker_config),
            )
            .child(channel_mapping)
    }

    /// Render recording device content for accordion
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    #[expect(
        clippy::cast_precision_loss,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Numeric input controls deliberately round their bounded UI parameter values"
    )]
    pub(super) fn render_recording_device_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let text = RecordingTranslations::for_language(state.app.ui_state.language);
        let language = state.app.ui_state.language;
        let (
            theme,
            num_channels,
            sample_rate,
            max_input_channels,
            num_positions,
            ctc_strategy,
            ctc_loopback,
        ) = {
            let rec_config = &state.app.measurement_state.recording_state.recording_config;
            let input_devices = &state.app.audio_device_state.input_devices;
            // Find selected device, or fall back to first available device
            let selected_device = input_devices
                .iter()
                .find(|d| d.name == rec_config.device_name)
                .or_else(|| input_devices.first());
            let max_ch = selected_device
                .and_then(|d| d.default_config.as_ref())
                .map_or(128, |c| c.channels as usize);
            (
                state.app.ui_state.theme.clone(),
                rec_config.num_channels,
                rec_config.sample_rate,
                max_ch,
                rec_config.num_positions.max(1),
                rec_config.ctc_matrix_strategy,
                rec_config.ctc_loopback_input_channel,
            )
        };
        let view = cx.weak_entity();

        let device_label = VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::label(translations.recording_input_device).color(theme.text_secondary));

        // Sample rate dropdown row
        let sample_rate_row = self
            .render_recording_sample_rate_dropdown(cx)
            .into_any_element();

        // Info badges
        let badges = HStack::new()
            .spacing(StackSpacing::Xs)
            .child(Badge::new(format!("{num_channels} ch")).variant(BadgeVariant::Info))
            .child(Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info));

        let channel_count_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(translations.recording_num_channels)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("recording_channel_count")
                    .label(translations.recording_num_channels)
                    .value(num_channels.min(max_input_channels) as f64)
                    .min(1.0)
                    .max(max_input_channels as f64)
                    .step(1.0)
                    .size(CONFIG_CONTROL_SIZE.into())
                    .on_change({
                        let view = view.clone();
                        move |value, _window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .recording_config
                                        .num_channels = value as usize;
                                    update_recording_channel_mappings(
                                        &mut state.app.measurement_state.recording_state,
                                    );
                                });
                                cx.notify();
                            });
                        }
                    })
            });

        let ctc_strategy_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(text.ctc_matrix)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                Button::new("ctc_matrix_strategy", ctc_strategy.as_str())
                    .size(CONFIG_CONTROL_SIZE.into())
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |_, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                let cfg = &mut state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .recording_config;
                                cfg.ctc_matrix_strategy = match cfg.ctc_matrix_strategy {
                                    CtcMatrixExportStrategy::ImpulseResponse => {
                                        CtcMatrixExportStrategy::RawSweep
                                    }
                                    CtcMatrixExportStrategy::RawSweep => {
                                        CtcMatrixExportStrategy::ImpulseResponse
                                    }
                                };
                            });
                            cx.notify();
                        });
                    })
            });

        let ctc_loopback_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(text.loopback_input)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("ctc_loopback_input")
                    .label(text.loopback_input)
                    .value(ctc_loopback.map_or(1, |ch| ch + 1) as f64)
                    .min(1.0)
                    .max(max_input_channels as f64)
                    .step(1.0)
                    .size(CONFIG_CONTROL_SIZE.into())
                    .on_change(move |value, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .recording_config
                                    .ctc_loopback_input_channel =
                                    Some((value as usize).saturating_sub(1));
                            });
                            cx.notify();
                        });
                    })
            });

        // Number of measurement positions. Each position runs a full
        // speaker × mic sweep; between positions a modal asks the user
        // to move the microphones to the next seat.
        let positions_row = VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        Text::new(text.positions)
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child({
                        let view = view.clone();
                        NumberInput::new("recording_num_positions")
                            .label(text.positions)
                            .value(num_positions as f64)
                            .min(1.0)
                            .max(8.0)
                            .step(1.0)
                            .size(CONFIG_CONTROL_SIZE.into())
                            .on_change({
                                let view = view.clone();
                                move |value, _window, cx| {
                                    let n = (value as usize).max(1);
                                    let _ = view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            let rs =
                                                &mut state.app.measurement_state.recording_state;
                                            rs.recording_config.num_positions = n;
                                            rs.init_channel_recordings();
                                            rs.sync_channel_speakers_length();
                                        });
                                        cx.notify();
                                    });
                                }
                            })
                    }),
            )
            .when(num_positions > 1, |stack| {
                // Anchoring guidance: delays/levels are computed relative to
                // position 1, so it must be the main listening position.
                stack.child(
                    Text::caption(
                        crate::app::i18n::RecordingWorkflowTranslations::for_language(language)
                            .positions_mlp_hint,
                    )
                    .color(theme.text_muted),
                )
            });

        // Render device dropdown first, converting to AnyElement to release borrow
        let device_dropdown = self.render_recording_device_dropdown(cx).into_any_element();
        // Render channel mapping second (after first borrow is released)
        let channel_mapping = self.render_recording_channel_mapping(cx).into_any_element();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(device_label.child(device_dropdown))
            .child(sample_rate_row)
            .child(badges)
            .child(channel_count_row)
            .child(positions_row)
            .child(ctc_strategy_row)
            .when(ctc_strategy == CtcMatrixExportStrategy::RawSweep, |this| {
                this.child(ctc_loopback_row)
            })
            .child(channel_mapping)
    }

    /// Render microphone calibration content for accordion
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_mic_calibration_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let text = RecordingTranslations::for_language(state.app.ui_state.language);
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();
        let num_channels = recording_state.recording_config.num_channels;

        // Collect per-channel paths and data
        let channel_paths: Vec<Option<String>> = (0..num_channels)
            .map(|i| {
                recording_state
                    .mic_calibration_paths
                    .get(i)
                    .cloned()
                    .flatten()
            })
            .collect();
        let channel_data: Vec<Option<CalibrationData>> = (0..num_channels)
            .map(|i| {
                recording_state
                    .mic_calibration_data_per_channel
                    .get(i)
                    .cloned()
                    .flatten()
            })
            .collect();

        let mut container = VStack::new().spacing(StackSpacing::Sm);

        // Render one row per channel (ch used as index into multiple vecs and for element IDs)
        #[allow(
            clippy::needless_range_loop,
            reason = "Layout coordinates address the paired indexed arrays"
        )]
        for ch in 0..num_channels {
            let path = channel_paths[ch].clone();
            let ch_view = view.clone();
            let ch_theme = theme.clone();

            let input_id = format!("calibration_file_{ch}");
            let browse_id = format!("browse_calibration_{ch}");
            let clear_id = format!("clear_calibration_{ch}");

            let mut row = HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center);

            // Show channel label only when multiple channels
            if num_channels > 1 {
                row = row.child(
                    Text::new(format!("Channel {}:", ch + 1))
                        .size(TextSize::Xs)
                        .color(ch_theme.text_secondary),
                );
            }

            row = row
                .child(
                    Input::new(gpui::SharedString::from(input_id))
                        .placeholder(translations.recording_no_calibration_placeholder)
                        .value(path.clone().unwrap_or_default())
                        .size(CONFIG_CONTROL_SIZE.into())
                        .disabled(true),
                )
                .child(
                    Button::new(
                        gpui::ElementId::from(gpui::SharedString::from(browse_id)),
                        translations.recording_browse,
                    )
                    .variant(ButtonVariant::Secondary)
                    .size(CONFIG_CONTROL_SIZE.into())
                    .theme(ch_theme.to_button_theme())
                    .on_click({
                        let view = ch_view.clone();
                        move |_, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.browse_calibration_file_for_channel(ch, cx);
                            });
                        }
                    }),
                );

            if path.is_some() {
                row = row.child(
                    Button::new(
                        gpui::ElementId::from(gpui::SharedString::from(clear_id)),
                        translations.recording_clear,
                    )
                    .variant(ButtonVariant::Secondary)
                    .size(CONFIG_CONTROL_SIZE.into())
                    .theme(ch_theme.to_button_theme())
                    .on_click({
                        let view = ch_view.clone();
                        move |_, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let rs = &mut state.app.measurement_state.recording_state;
                                    if let Some(slot) = rs.mic_calibration_paths.get_mut(ch) {
                                        *slot = None;
                                    }
                                    if let Some(slot) =
                                        rs.mic_calibration_data_per_channel.get_mut(ch)
                                    {
                                        *slot = None;
                                    }
                                    // Sync legacy fields from channel 0
                                    if ch == 0 {
                                        rs.mic_calibration_path = None;
                                        rs.mic_calibration_data = None;
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
                );
            }

            container = container.child(row);
        }

        container = container.child(Text::caption(translations.recording_calibration_help));

        // Add calibration graph when any channel has data
        let cal_entries: Vec<(usize, CalibrationData)> = channel_data
            .into_iter()
            .enumerate()
            .filter_map(|(i, d)| d.map(|data| (i, data)))
            .collect();
        if !cal_entries.is_empty() {
            container = container.child(Self::render_calibration_graph_multi(
                &cal_entries,
                text,
                &theme,
                &d,
            ));
        }

        container
    }

    /// Render output directory content for accordion
    pub(super) fn render_output_directory_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        let base_dir = recording_state.recording_base_directory.clone();
        let recording_dir = recording_state.recording_directory.clone();
        let has_directory = recording_dir.is_some();

        let display_path = recording_dir
            .clone()
            .unwrap_or_else(|| translations.recording_no_directory.to_string());

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        Text::new(translations.recording_save_location)
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(Text::label(display_path).color(if has_directory {
                        theme.text_primary
                    } else {
                        theme.warning
                    })),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Button::new("browse_output_dir", translations.recording_browse)
                            .variant(ButtonVariant::Secondary)
                            .size(CONFIG_CONTROL_SIZE.into())
                            .theme(theme.to_button_theme())
                            .on_click({
                                let view = view.clone();
                                move |_, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.browse_recording_directory(cx);
                                    });
                                }
                            }),
                    )
                    .when(base_dir.is_some(), |stack| {
                        let view = view.clone();
                        let theme = theme.clone();
                        stack.child(
                            Button::new("clear_output_dir", translations.recording_clear)
                                .variant(ButtonVariant::Secondary)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .theme(theme.to_button_theme())
                                .on_click({
                                    move |_, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .recording_base_directory = None;
                                                state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .recording_directory = None;
                                            });
                                            cx.notify();
                                        });
                                    }
                                }),
                        )
                    }),
            )
            .child(Text::caption(recording_text.timestamped_subdirectory))
            .when(!has_directory, |stack| {
                stack.child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(Text::new("⚠").size(TextSize::Xs).color(theme.warning))
                        .child(
                            Text::new(translations.recording_no_output_dir)
                                .size(TextSize::Xs)
                                .color(theme.warning),
                        ),
                )
            })
    }

    /// Render the "Advanced: Measurement Quality" accordion section (GD-Opt v2 §2.7).
    ///
    /// Three knobs:
    /// - Bass precision: 1×/2×/4× segmented control → `bass_octave_duration_s` ∈ {1.5, 3.0, 5.0}
    /// - Pre-silence: numeric input in seconds (default 2.0)
    /// - Post-silence: numeric input in seconds (blank = derived from RT60 estimate)
    ///
    /// The estimated capture time for the whole session is also shown.
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Numeric input controls deliberately round their bounded UI parameter values"
    )]
    #[expect(
        clippy::cast_precision_loss,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    pub(super) fn render_advanced_sweep_quality_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let rec_state = &state.app.measurement_state.recording_state;
        let bass_dur = rec_state.bass_octave_duration_s;
        let pre_s = rec_state.pre_silence_s;
        let post_s_opt = rec_state.post_silence_s;
        let num_sweeps = rec_state.num_sweeps;
        let view = cx.weak_entity();

        // Bass precision presets: (label, value in s/octave)
        // 1× = 1.5 s/oct, 2× (default) = 3.0 s/oct, 4× = 5.0 s/oct
        let presets: &[(&str, f32)] = &[("1×", 1.5), ("2×", 3.0), ("4×", 5.0)];

        let mut bass_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(translations.recording_bass_precision)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            );

        for &(label, value) in presets {
            let is_active = (bass_dur - value).abs() < 0.1;
            let view = view.clone();
            let btn_theme = theme.clone();
            bass_row = bass_row.child(
                Button::new(
                    gpui::SharedString::from(format!("bass_preset_{label}")),
                    label,
                )
                .variant(if is_active {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .size(CONFIG_CONTROL_SIZE.into())
                .theme(btn_theme.to_button_theme())
                .on_click(move |_, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .bass_octave_duration_s = value;
                        });
                        cx.notify();
                    });
                }),
            );
        }

        // Pre-silence numeric input.
        let pre_row = {
            let view = view.clone();
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(
                    Text::new(translations.recording_pre_silence)
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .child(
                    NumberInput::new("pre_silence_s")
                        .label(translations.recording_pre_silence)
                        .value(f64::from(pre_s))
                        .min(0.0)
                        .max(10.0)
                        .step(0.5)
                        .size(CONFIG_CONTROL_SIZE.into())
                        .on_change(move |value, _window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state.app.measurement_state.recording_state.pre_silence_s =
                                        value as f32;
                                });
                                cx.notify();
                            });
                        }),
                )
        };

        // Post-silence numeric input (blank = auto-derive from RT60).
        let post_row = {
            let view = view.clone();
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(
                    Text::new(translations.recording_post_silence)
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .child(
                    NumberInput::new("post_silence_s")
                        .label(translations.recording_post_silence)
                        .value(f64::from(post_s_opt.unwrap_or(0.0)))
                        .min(0.0)
                        .max(30.0)
                        .step(0.5)
                        .size(CONFIG_CONTROL_SIZE.into())
                        .on_change(move |value, _window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state.app.measurement_state.recording_state.post_silence_s =
                                        if value < 0.001 {
                                            None // 0 = auto-derive from RT60
                                        } else {
                                            Some(value as f32)
                                        };
                                });
                                cx.notify();
                            });
                        }),
                )
                .child(Text::caption(if post_s_opt.is_none() {
                    "(auto: RT60 + 1 s)"
                } else {
                    ""
                }))
        };

        // Estimate the session from the same effective signal and padding
        // configuration used by capture, then scale by the selected outputs
        // and configured sweeps. This stays accurate for non-sweep signals,
        // custom frequency bounds, and non-default sample rates.
        let channels = rec_state.playback_config.channel_mappings.len();
        let estimated_secs = if channels == 0 {
            None
        } else {
            use sotf_audio_player::recording_helpers::{
                capture_signal_params, effective_acquisition_config, measurement_amplitude,
                signal_type_for,
            };

            let signal_type = signal_type_for(rec_state.signal_type);
            let signal_params = capture_signal_params(
                signal_type,
                rec_state.sweep_start_freq,
                rec_state.sweep_end_freq,
                measurement_amplitude(f64::from(rec_state.signal_level_db)),
                Some(bass_dur),
                Some(pre_s),
                post_s_opt,
            );
            effective_acquisition_config(
                signal_type,
                &signal_params,
                rec_state.signal_duration_secs,
                rec_state.playback_config.sample_rate,
            )
            .ok()
            .map(|config| {
                f64::from(config.playback_duration_secs) * channels as f64 * f64::from(num_sweeps)
            })
        };
        let estimated_time = match estimated_secs {
            Some(seconds) if seconds < 60.0 => format!("~{} s", seconds.round() as u32),
            Some(seconds) => format!("~{} min", (seconds / 60.0).round() as u32),
            None => "unavailable for current signal settings".to_string(),
        };

        // Sweeps-per-channel numeric input. 2 is never offered: outlier
        // rejection needs at least 3 takes, so the setter routes through
        // `clamp_num_sweeps` (1 or 3..=8).
        let sweeps_row = {
            let view = view.clone();
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(recording_text.repeat.sweeps_per_channel)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            NumberInput::new("num_sweeps")
                                .label(recording_text.repeat.sweeps_per_channel)
                                .value(f64::from(num_sweeps))
                                .min(1.0)
                                .max(8.0)
                                .step(1.0)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .on_change(move |value, _window, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .num_sweeps =
                                                clamp_num_sweeps(value.max(0.0) as u16);
                                        });
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    Text::caption(recording_text.repeat.sweeps_per_channel_hint)
                        .color(theme.text_muted),
                )
        };

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(Text::caption(recording_text.bass_precision_description))
            .child(bass_row)
            .child(sweeps_row)
            .child(pre_row)
            .child(post_row)
            .child(Text::caption(format!(
                "Estimated capture time ({channels} channels × {num_sweeps} sweeps): {estimated_time}"
            )))
    }

    /// Open directory dialog to select recording output directory
    pub(crate) fn browse_recording_directory(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let weak_state = self.state.downgrade();

            cx.spawn(async move |_, cx| {
                // Open directory dialog
                let folder = rfd::AsyncFileDialog::new()
                    .set_title("Select Recording Output Directory")
                    .pick_folder()
                    .await;

                if let Some(folder) = folder {
                    let base_path = folder.path().to_string_lossy().to_string();
                    log::info!("Selected recording directory: {base_path}");

                    // Create timestamped subdirectory name
                    let now = chrono::Local::now();
                    let timestamp_dir = format!("recording-{}", now.format("%Y%m%d-%H%M%S"));
                    let full_path = std::path::Path::new(&base_path)
                        .join(&timestamp_dir)
                        .to_string_lossy()
                        .to_string();

                    let (base_path, full_path, status_message) =
                        match std::fs::create_dir_all(&full_path) {
                            Ok(()) => {
                                log::info!("Created recording directory: {full_path}");
                                (base_path, full_path, None)
                            }
                            Err(e) => {
                                log::error!("Failed to create recording directory: {e}");
                                let (fallback_base, fallback_dir) =
                                    crate::app::config::default_recording_paths();
                                match (fallback_base, fallback_dir) {
                                    (Some(base), Some(dir)) => (
                                        base,
                                        dir,
                                        Some(format!(
                                            "Cannot write to selected folder; using app recordings folder: {e}"
                                        )),
                                    ),
                                    _ => return,
                                }
                            }
                        };

                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    state_entity.update(&mut cx.clone(), |state, _| {
                        let rec = &mut state.app.measurement_state.recording_state;
                        rec.recording_base_directory = Some(base_path);
                        rec.recording_directory = Some(full_path);
                        if let Some(message) = status_message {
                            rec.status_message = message;
                            rec.status_severity =
                                crate::app::types::recording::RecordingStatusSeverity::Warning;
                        }
                    });
                }
            })
            .detach();
        }
    }

    /// Render playback device dropdown
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_playback_device_dropdown(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        let options: Vec<SelectOption> = state
            .app
            .audio_device_state
            .output_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.playback_config.device_name.is_empty() {
            state
                .app
                .audio_device_state
                .output_devices
                .first()
                .map_or_default(|d| d.name.clone())
        } else {
            recording_state.playback_config.device_name.clone()
        };

        Select::new("playback_device")
            .size(CONFIG_CONTROL_SIZE.into())
            .options(options)
            .selected(selected_value)
            .placeholder(translations.recording_select_playback_placeholder)
            .is_open(recording_state.playback_device_dropdown_open)
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .playback_device_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .playback_config
                                .device_name = value.to_string();
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .playback_config
                                .device_id = value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .audio_device_state
                                .output_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                // Set default sample rate (prefer 48k, then 44.1k, then default)
                                let rates = &device.available_sample_rates;
                                let default_rate = if rates.contains(&48000) {
                                    48000
                                } else if rates.contains(&44100) {
                                    44100
                                } else {
                                    device
                                        .default_config
                                        .as_ref()
                                        .map_or(48000, |c| c.sample_rate)
                                };

                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .sample_rate = default_rate;
                                // Update available sample rates from device
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .available_sample_rates = device.available_sample_rates.clone();

                                // Clamp playback channels to device max
                                if let Some(max_ch) =
                                    device.default_config.as_ref().map(|c| c.channels as usize)
                                {
                                    let pb = &mut state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .playback_config;
                                    if pb.num_channels > max_ch {
                                        // Truncate channel mappings to fit device
                                        while pb.total_interface_channels() > max_ch
                                            && !pb.channel_mappings.is_empty()
                                        {
                                            pb.channel_mappings.pop();
                                        }
                                        pb.sync_channel_count();
                                    }
                                }
                            }
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .playback_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render playback sample rate dropdown
    pub(super) fn render_playback_sample_rate_dropdown(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        let available_rates = &recording_state.playback_config.available_sample_rates;
        let options: Vec<SelectOption> = available_rates
            .iter()
            .map(|&rate| {
                let label = if rate >= 1000 {
                    format!("{} kHz", rate / 1000)
                } else {
                    format!("{rate} Hz")
                };
                SelectOption::new(rate.to_string(), label)
            })
            .collect();

        let selected_value = recording_state.playback_config.sample_rate.to_string();

        HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(translations.recording_sample_rate)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(100.0)).child(
                    // intentional: sample-rate dropdown width
                    Select::new("playback_sample_rate")
                        .width(100.0)
                        .size(CONFIG_CONTROL_SIZE.into())
                        .options(options)
                        .selected(selected_value)
                        .placeholder(translations.recording_select_rate_placeholder)
                        .is_open(recording_state.playback_sample_rate_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_sample_rate_dropdown_open = is_open;
                                    });
                                    cx.notify();
                                });
                            }
                        })
                        .on_change({
                            let view = view.clone();
                            move |value, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Ok(rate) = value.parse::<u32>() {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .sample_rate = rate;
                                        }
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_sample_rate_dropdown_open = false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
    }

    /// Render speaker configuration dropdown with channel count and sample rate badges
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_speaker_config_dropdown(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let num_channels = recording_state.playback_config.num_channels;
        let sample_rate = recording_state.playback_config.sample_rate;
        let view = cx.weak_entity();

        // Determine max output channels from the selected device
        let max_output_channels = state
            .app
            .audio_device_state
            .output_devices
            .iter()
            .find(|d| d.name == recording_state.playback_config.device_name)
            .and_then(|d| d.default_config.as_ref())
            .map_or(128, |c| c.channels as usize);

        // Only show speaker configs that fit within the device's channel count
        let options: Vec<SelectOption> = SpeakerConfiguration::all()
            .iter()
            .filter(|config| {
                **config == SpeakerConfiguration::Custom
                    || config.channel_count() <= max_output_channels
            })
            .map(|config| SelectOption::new(config.as_str(), config.as_str()))
            .collect();

        let selected_value = recording_state
            .playback_config
            .speaker_configuration
            .as_str();

        HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(translations.recording_speaker_config)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(100.0)).child(
                    // intentional: speaker-config dropdown width
                    Select::new("speaker_config")
                        .width(100.0)
                        .size(CONFIG_CONTROL_SIZE.into())
                        .options(options)
                        .selected(selected_value)
                        .placeholder(translations.recording_select_config_placeholder)
                        .is_open(recording_state.speaker_config_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .speaker_config_dropdown_open = is_open;
                                    });
                                    cx.notify();
                                });
                            }
                        })
                        .on_change({
                            let view = view.clone();
                            move |value, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        // Find the matching configuration
                                        let new_config = SpeakerConfiguration::all()
                                            .iter()
                                            .find(|c| c.as_str() == value.as_ref())
                                            .copied()
                                            .unwrap_or(SpeakerConfiguration::Custom);

                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_config
                                            .speaker_configuration = new_config;

                                        // Update channel count and mappings based on configuration
                                        if new_config == SpeakerConfiguration::Custom {
                                            // For custom config, keep current speaker count but use generic names
                                            let num_speakers = state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .channel_mappings
                                                .len();
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .channel_mappings = (0..num_speakers)
                                                .map(|i| {
                                                    ChannelMapping::single(
                                                        i,
                                                        format!("Ch{}", i + 1),
                                                    )
                                                })
                                                .collect();
                                        } else {
                                            // Set default channel names for the configuration
                                            let channel_names = new_config.default_channel_names();
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .channel_mappings = channel_names
                                                .iter()
                                                .enumerate()
                                                .map(|(i, name)| ChannelMapping::single(i, *name))
                                                .collect();
                                        }
                                        // Sync total channel count
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_config
                                            .sync_channel_count();

                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .speaker_config_dropdown_open = false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                Text::new(translations.recording_channels_label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(Badge::new(format!("{num_channels}")).variant(BadgeVariant::Info))
            .child(Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info))
    }

    /// Render playback channel mapping table (speaker-centric view)
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_playback_channel_mapping(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            self.state.read(cx).app.ui_state.language,
        );
        // Extract all needed data upfront, then release the borrow
        let (theme, speaker_data, is_custom) = {
            let state = self.state.read(cx);
            let mappings: Vec<_> = state
                .app
                .measurement_state
                .recording_state
                .playback_config
                .channel_mappings
                .iter()
                .map(|m| (m.interface_channels.clone(), m.group_name.clone()))
                .collect();
            let is_custom = state
                .app
                .measurement_state
                .recording_state
                .playback_config
                .speaker_configuration
                == SpeakerConfiguration::Custom;
            (state.app.ui_state.theme.clone(), mappings, is_custom)
        };
        let view = cx.weak_entity();

        let scale = self.state.read(cx).app.ui_state.font_scale;
        let headers = recording_text.speaker_columns();
        let speaker_count = speaker_data.len();
        let name_view = view.clone();
        let type_view = view.clone();
        let channel_view = view.clone();
        let table = Table::new("playback-speaker-routing", speaker_data)
            .aria_label(translations.recording_speaker_config)
            .column(
                Column::new("speaker", headers[0])
                    .width(px(80.0 * scale))
                    .sortable(false)
                    .resizable(false)
                    .cell_render(|_: &(Vec<usize>, String), idx, _, _| {
                        Text::new(format!("#{}", idx + 1)).size(TextSize::Sm)
                    }),
            )
            .column(
                Column::new("name", headers[1])
                    .width(px(192.0 * scale))
                    .sortable(false)
                    .resizable(false)
                    .cell_render(move |row: &(Vec<usize>, String), idx, _, cx| {
                        name_view
                            .update(cx, |this, cx| {
                                if is_custom {
                                    this.render_channel_name_input_raw(cx, idx, &row.1)
                                        .into_any_element()
                                } else {
                                    this.render_channel_group_dropdown(cx, idx, &row.1)
                                        .into_any_element()
                                }
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
            .column(
                Column::new("type", headers[2])
                    .width(px(128.0 * scale))
                    .sortable(false)
                    .resizable(false)
                    .cell_render(move |row: &(Vec<usize>, String), idx, _, cx| {
                        type_view
                            .update(cx, |this, cx| {
                                this.render_speaker_mode_toggle(cx, idx, row.0.len() > 1)
                                    .into_any_element()
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
            .column(
                Column::new("channel", headers[3])
                    .width(px(192.0 * scale))
                    .sortable(false)
                    .resizable(false)
                    .cell_render(move |row: &(Vec<usize>, String), idx, _, cx| {
                        channel_view
                            .update(cx, |this, cx| {
                                if row.0.len() > 1 {
                                    this.render_multi_channel_list(
                                        cx,
                                        idx,
                                        &row.0,
                                        &this.state.read(cx).app.ui_state.theme.clone(),
                                    )
                                    .into_any_element()
                                } else {
                                    this.render_playback_interface_channel(
                                        cx,
                                        idx,
                                        row.0.first().copied().unwrap_or(0),
                                    )
                                    .into_any_element()
                                }
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            );
        let table = div().w_full().child(table);
        #[cfg(feature = "dev-api")]
        let table = table.dev_track("recording.playback.routing");
        let stack = VStack::new().spacing(StackSpacing::Xs).child(table);

        if is_custom {
            let theme_clone = theme.clone();
            let view_add = view.clone();
            let view_remove = view.clone();
            stack
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(div().w(rems(5.0)))
                        .child(
                            Button::new("custom_add_speaker", recording_text.add_speaker)
                                .variant(ButtonVariant::Ghost)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .theme(theme_clone.to_button_theme())
                                .on_click(move |_, cx| {
                                    let _ = view_add.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            let cfg = &mut state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config;
                                            let next_ch = cfg.total_interface_channels();
                                            let idx = cfg.channel_mappings.len() + 1;
                                            cfg.channel_mappings.push(ChannelMapping::single(
                                                next_ch,
                                                format!("Ch{idx}"),
                                            ));
                                            cfg.sync_channel_count();
                                        });
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("custom_remove_speaker", recording_text.remove_speaker)
                                .variant(ButtonVariant::Ghost)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .theme(theme.clone().to_button_theme())
                                .disabled(speaker_count <= 1)
                                .on_click(move |_, cx| {
                                    let _ = view_remove.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            let cfg = &mut state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config;
                                            if cfg.channel_mappings.len() > 1 {
                                                cfg.channel_mappings.pop();
                                                cfg.sync_channel_count();
                                            }
                                        });
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .into_any_element()
        } else {
            stack.into_any_element()
        }
    }

    /// Render an interface channel selector without a repeated visual label.
    #[expect(
        clippy::cast_precision_loss,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Numeric input controls deliberately round their bounded UI parameter values"
    )]
    fn render_playback_interface_channel(
        &self,
        cx: &mut Context<Self>,
        speaker_idx: usize,
        interface_ch: usize,
    ) -> impl IntoElement {
        let label = {
            let state = self.state.read(cx);
            state.app.ui_state.translations.recording_channel_label
        };
        let view = cx.weak_entity();
        let input = NumberInput::new(SharedString::from(format!("speaker_{speaker_idx}_ch_0")))
            .aria_label(format!("{label} {}", speaker_idx + 1))
            .value((interface_ch + 1) as f64)
            .min(1.0)
            .max(128.0)
            .step(1.0)
            .size(CONFIG_CONTROL_SIZE.into())
            .on_change(move |value, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.state.update(cx, |state, _| {
                        let config =
                            &mut state.app.measurement_state.recording_state.playback_config;
                        if let Some(mapping) = config.channel_mappings.get_mut(speaker_idx)
                            && let Some(channel) = mapping.interface_channels.first_mut()
                        {
                            *channel = (value as usize).saturating_sub(1);
                        }
                        config.sync_channel_count();
                    });
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let input = input.dev_track(format!("recording.playback.channel.{speaker_idx}"));
        input
    }

    /// Render the single/multi mode toggle for a speaker
    pub(super) fn render_speaker_mode_toggle(
        &self,
        cx: &mut Context<Self>,
        speaker_idx: usize,
        is_multi: bool,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            state.app.ui_state.language,
        );
        let is_open = state
            .app
            .measurement_state
            .recording_state
            .speaker_mode_dropdown_open
            == Some(speaker_idx);
        let view = cx.weak_entity();

        let options = vec![
            SelectOption::new("single", recording_text.single),
            SelectOption::new("multi", recording_text.multi),
        ];

        let selected = if is_multi { "multi" } else { "single" };

        div().w_full().child(
            // The table column owns the control width.
            Select::new(SharedString::from(format!("speaker_mode_{speaker_idx}")))
                .aria_label(format!(
                    "{} / {}",
                    recording_text.single, recording_text.multi
                ))
                .width(100.0)
                .size(CONFIG_CONTROL_SIZE.into())
                .options(options)
                .selected(selected)
                .is_open(is_open)
                .theme(theme.to_select_theme())
                .on_toggle({
                    let view = view.clone();
                    move |open, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .speaker_mode_dropdown_open = open.then_some(speaker_idx);
                            });
                            cx.notify();
                        });
                    }
                })
                .on_change({
                    let view = view.clone();
                    move |value, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                if let Some(mapping) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .channel_mappings
                                    .get_mut(speaker_idx)
                                {
                                    if value == "multi" && mapping.interface_channels.len() == 1 {
                                        // Switch to multi: add a second channel
                                        let next_ch = mapping.interface_channels[0] + 1;
                                        mapping.interface_channels.push(next_ch);
                                    } else if value == "single"
                                        && mapping.interface_channels.len() > 1
                                    {
                                        // Switch to single: keep only first channel
                                        mapping.interface_channels.truncate(1);
                                    }
                                }
                                // Renumber all channels sequentially
                                renumber_interface_channels(
                                    &mut state.app.measurement_state.recording_state,
                                );
                                // Close dropdown and sync channel count
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .speaker_mode_dropdown_open = None;
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .sync_channel_count();
                            });
                            cx.notify();
                        });
                    }
                }),
        )
    }

    /// Render the channel list for a multi-channel speaker
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    #[expect(
        clippy::cast_precision_loss,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Numeric input controls deliberately round their bounded UI parameter values"
    )]
    pub(super) fn render_multi_channel_list(
        &self,
        cx: &mut Context<Self>,
        speaker_idx: usize,
        interface_channels: &[usize],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let view = cx.weak_entity();
        let theme = theme.clone();
        let recording_text = crate::app::i18n::RecordingWorkflowTranslations::for_language(
            self.state.read(cx).app.ui_state.language,
        );

        VStack::new()
            .spacing(StackSpacing::Xs)
            .children(
                interface_channels
                    .iter()
                    .enumerate()
                    .map(|(ch_idx, &interface_ch)| {
                        let view = view.clone();
                        let theme = theme.clone();

                        HStack::new()
                            .spacing(StackSpacing::Xs)
                            .align(StackAlign::Center)
                            .child(div().w(px(70.0)).child({
                                // intentional: fixed ch-number input column
                                let view = view.clone();
                                NumberInput::new(SharedString::from(format!(
                                    "speaker_{speaker_idx}_ch_{ch_idx}"
                                )))
                                .aria_label(format!("{} {}", recording_text.multi, ch_idx + 1))
                                .value((interface_ch + 1) as f64)
                                .min(1.0)
                                .max(128.0)
                                .step(1.0)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .on_change({
                                    let view = view.clone();
                                    move |value, _window, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                if let Some(mapping) = state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .playback_config
                                                    .channel_mappings
                                                    .get_mut(speaker_idx)
                                                    && let Some(ch) =
                                                        mapping.interface_channels.get_mut(ch_idx)
                                                {
                                                    *ch = (value as usize).saturating_sub(1);
                                                }
                                            });
                                            cx.notify();
                                        });
                                    }
                                })
                            }))
                            .child({
                                // Remove button
                                let view = view.clone();
                                let can_remove = interface_channels.len() > 2;
                                Button::new(
                                    SharedString::from(format!("remove_ch_{speaker_idx}_{ch_idx}")),
                                    "\u{2715}",
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(CONFIG_CONTROL_SIZE.into())
                                .theme(theme.to_button_theme())
                                .disabled(!can_remove)
                                .on_click({
                                    move |_, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                if let Some(mapping) = state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .playback_config
                                                    .channel_mappings
                                                    .get_mut(speaker_idx)
                                                {
                                                    mapping.remove_channel(ch_idx);
                                                }
                                                // Renumber all channels sequentially
                                                renumber_interface_channels(
                                                    &mut state
                                                        .app
                                                        .measurement_state
                                                        .recording_state,
                                                );
                                                state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .playback_config
                                                    .sync_channel_count();
                                            });
                                            cx.notify();
                                        });
                                    }
                                })
                            })
                            .into_any_element()
                    }),
            )
            .child({
                // Add channel button - indent to align with channel rows
                let view = view.clone();
                let theme = theme.clone();
                HStack::new().spacing(StackSpacing::Xs).child(
                    Button::new(
                        SharedString::from(format!("add_ch_{speaker_idx}")),
                        recording_text.add,
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(CONFIG_CONTROL_SIZE.into())
                    .theme(theme.to_button_theme())
                    .on_click({
                        move |_, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    if let Some(mapping) = state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .playback_config
                                        .channel_mappings
                                        .get_mut(speaker_idx)
                                    {
                                        // Add a placeholder channel (will be renumbered)
                                        mapping.add_channel(0);
                                    }
                                    // Renumber all channels sequentially
                                    renumber_interface_channels(
                                        &mut state.app.measurement_state.recording_state,
                                    );
                                    state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .playback_config
                                        .sync_channel_count();
                                });
                                cx.notify();
                            });
                        }
                    }),
                )
            })
    }

    /// Render custom text input for channel name (raw, without wrapper div)
    pub(super) fn render_channel_name_input_raw(
        &self,
        cx: &mut Context<Self>,
        channel_idx: usize,
        current_name: &str,
    ) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        let view = cx.weak_entity();
        let current_name = current_name.to_string();

        div().size_full().child(
            Input::new(SharedString::from(format!("channel_name_{channel_idx}")))
                .placeholder(translations.recording_name_placeholder)
                .value(current_name)
                .size(CONFIG_CONTROL_SIZE.into())
                .on_change({
                    let view = view.clone();
                    move |value, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                if let Some(mapping) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .channel_mappings
                                    .get_mut(channel_idx)
                                {
                                    mapping.group_name = value.to_string();
                                }
                            });
                            cx.notify();
                        });
                    }
                }),
        )
    }

    /// Render channel group dropdown for a specific channel (playback only)
    pub(super) fn render_channel_group_dropdown(
        &self,
        cx: &mut Context<Self>,
        channel_idx: usize,
        current_group: &str,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();
        let current_group = current_group.to_string();

        // Build options from CHANNEL_GROUPS
        let options: Vec<SelectOption> = CHANNEL_GROUPS
            .iter()
            .map(|(id, name)| SelectOption::new(*id, *name))
            .collect();

        // Check if this channel's dropdown is open
        let is_open = recording_state.channel_name_dropdown_open == Some(channel_idx);

        // Find the display label for current selection
        let selected_label: SharedString = CHANNEL_GROUPS
            .iter()
            .find(|(id, _)| *id == current_group.as_str())
            .map_or_else(
                || SharedString::from(current_group.clone()),
                |(_, name)| SharedString::from(*name),
            );

        div().w_full().child(
            // The table column owns the control width.
            Select::new(SharedString::from(format!("channel_name_{channel_idx}")))
                .size(CONFIG_CONTROL_SIZE.into())
                .options(options)
                .selected(current_group.clone())
                .placeholder(selected_label)
                .is_open(is_open)
                .theme(theme.to_select_theme())
                .on_toggle({
                    let view = view.clone();
                    move |is_open, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_name_dropdown_open = is_open.then_some(channel_idx);
                            });
                            cx.notify();
                        });
                    }
                })
                .on_change({
                    let view = view.clone();
                    move |value, _window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                if let Some(mapping) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .channel_mappings
                                    .get_mut(channel_idx)
                                {
                                    mapping.group_name = value.to_string();
                                }
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_name_dropdown_open = None;
                            });
                            cx.notify();
                        });
                    }
                }),
        )
    }

    /// Render recording device dropdown
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(super) fn render_recording_device_dropdown(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        let options: Vec<SelectOption> = state
            .app
            .audio_device_state
            .input_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.recording_config.device_name.is_empty() {
            state
                .app
                .audio_device_state
                .input_devices
                .first()
                .map_or_default(|d| d.name.clone())
        } else {
            recording_state.recording_config.device_name.clone()
        };

        Select::new("recording_device")
            .size(CONFIG_CONTROL_SIZE.into())
            .options(options)
            .selected(selected_value)
            .placeholder(translations.recording_select_recording_placeholder)
            .is_open(recording_state.recording_device_dropdown_open)
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .recording_device_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .recording_config
                                .device_name = value.to_string();
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .recording_config
                                .device_id = value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .audio_device_state
                                .input_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                // Set default sample rate (prefer 48k, then 44.1k, then default)
                                let rates = &device.available_sample_rates;
                                let default_rate = if rates.contains(&48000) {
                                    48000
                                } else if rates.contains(&44100) {
                                    44100
                                } else {
                                    device
                                        .default_config
                                        .as_ref()
                                        .map_or(48000, |c| c.sample_rate)
                                };

                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .recording_config
                                    .sample_rate = default_rate;
                                // Update available sample rates from device
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .recording_config
                                    .available_sample_rates = device.available_sample_rates.clone();

                                // Clamp channel count to device max
                                if let Some(max_ch) =
                                    device.default_config.as_ref().map(|c| c.channels as usize)
                                {
                                    let rec = &mut state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .recording_config;
                                    if rec.num_channels > max_ch {
                                        rec.num_channels = max_ch;
                                        update_recording_channel_mappings(
                                            &mut state.app.measurement_state.recording_state,
                                        );
                                    }
                                }
                            }
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .recording_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render recording sample rate dropdown
    pub(super) fn render_recording_sample_rate_dropdown(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        let available_rates = &recording_state.recording_config.available_sample_rates;
        let options: Vec<SelectOption> = available_rates
            .iter()
            .map(|&rate| {
                let label = if rate >= 1000 {
                    format!("{} kHz", rate / 1000)
                } else {
                    format!("{rate} Hz")
                };
                SelectOption::new(rate.to_string(), label)
            })
            .collect();

        let selected_value = recording_state.recording_config.sample_rate.to_string();

        HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new(translations.recording_sample_rate)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(100.0)).child(
                    // intentional: sample-rate dropdown width
                    Select::new("recording_sample_rate")
                        .size(CONFIG_CONTROL_SIZE.into())
                        .options(options)
                        .selected(selected_value)
                        .placeholder(translations.recording_select_rate_placeholder)
                        .is_open(recording_state.recording_sample_rate_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .recording_sample_rate_dropdown_open = is_open;
                                    });
                                    cx.notify();
                                });
                            }
                        })
                        .on_change({
                            let view = view.clone();
                            move |value, _window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Ok(rate) = value.parse::<u32>() {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .recording_config
                                                .sample_rate = rate;
                                        }
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .recording_sample_rate_dropdown_open = false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
    }

    /// Render recording channel mapping table
    #[expect(
        clippy::cast_precision_loss,
        reason = "UI controls and graph coordinates use approximate floating-point parameter values"
    )]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Numeric input controls deliberately round their bounded UI parameter values"
    )]
    pub(super) fn render_recording_channel_mapping(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.weak_entity();

        VStack::new()
            .spacing(StackSpacing::Xs)
            .children(
                recording_state
                    .recording_config
                    .channel_mappings
                    .iter()
                    .enumerate()
                    .map(|(idx, &interface_ch)| {
                        let view = view.clone();
                        let theme = theme.clone();

                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                Text::new(format!("Channel {}:", idx + 1))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .align(StackAlign::Center)
                                    .child(Text::caption(translations.recording_interface))
                                    .child({
                                        let view = view.clone();
                                        NumberInput::new(SharedString::from(format!(
                                            "recording_interface_{idx}"
                                        )))
                                        .value((interface_ch + 1) as f64)
                                        .min(1.0)
                                        .max(128.0)
                                        .step(1.0)
                                        .size(CONFIG_CONTROL_SIZE.into())
                                        .on_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        if let Some(m) = state
                                                            .app
                                                            .measurement_state
                                                            .recording_state
                                                            .recording_config
                                                            .channel_mappings
                                                            .get_mut(idx)
                                                        {
                                                            *m = (value as usize).saturating_sub(1);
                                                        }
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        })
                                    }),
                            )
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// Render calibration data as a frequency response graph with multiple channel curves
    pub(super) fn render_calibration_graph_multi(
        entries: &[(usize, CalibrationData)],
        text: RecordingTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> impl IntoElement {
        use crate::components::graphs::response_graphs::channel_color;

        let chart_width: f32 = 500.0;
        let chart_height: f32 = 200.0;

        // Find y-axis range across all channels
        let (min_db, max_db) = entries.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(min, max), (_, data)| {
                data.spl_db
                    .iter()
                    .fold((min, max), |(min, max), &v| (min.min(v), max.max(v)))
            },
        );

        let y_min = if min_db.is_finite() {
            min_db - 5.0
        } else {
            -10.0
        };
        let y_max = if max_db.is_finite() {
            max_db + 5.0
        } else {
            10.0
        };

        let chart_theme = theme_to_chart_theme(theme);

        // Build chart from first channel as primary series
        let (first_ch, first_data) = &entries[0];
        let first_color = channel_color(theme, *first_ch);
        let first_label = if entries.len() == 1 {
            "Calibration".to_string()
        } else {
            format!("Ch {}", first_ch + 1)
        };

        let mut builder = line(&first_data.frequencies, &first_data.spl_db)
            .x_scale(ScaleType::Log)
            .y_scale(ScaleType::Linear)
            .x_label("Frequency (Hz)")
            .y_label("SPL (dB)")
            .x_range(20.0, 20000.0)
            .y_range(y_min, y_max)
            .size(chart_width, chart_height)
            .color(first_color)
            .stroke_width(2.0)
            .label(first_label)
            .theme(chart_theme);

        // Add additional channels as extra series
        for &(ch_idx, ref data) in &entries[1..] {
            let color = channel_color(theme, ch_idx);
            builder = builder.add_series_with_x(
                &data.frequencies,
                &data.spl_db,
                Some(format!("Ch {}", ch_idx + 1)),
                color,
                2.0,
                1.0,
            );
        }

        let chart_element = builder.build();

        div()
            .mt(d.section)
            .p(d.pad_y)
            .bg(theme.surface)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.border)
            .child(match chart_element {
                Ok(chart) => chart.into_any_element(),
                Err(_) => div()
                    .w(px(chart_width)) // intentional: match chart canvas dimensions
                    .h(px(chart_height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Text::new(text.calibration_graph_unavailable).color(theme.text_secondary),
                    )
                    .into_any_element(),
            })
            .into_any_element()
    }

    /// Open file dialog to browse for calibration file for a specific channel
    pub(super) fn browse_calibration_file_for_channel(
        &mut self,
        channel_idx: usize,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let weak_state = self.state.downgrade();

            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("CSV", &["csv", "txt"])
                    .add_filter("All files", &["*"])
                    .set_title("Select Microphone Calibration File")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_string_lossy().to_string();
                    log::info!("Selected calibration file for channel {channel_idx}: {path}");

                    let calibration_data = match std::fs::read_to_string(&path) {
                        Ok(content) => CalibrationData::parse(&content),
                        Err(e) => {
                            log::error!("Failed to read calibration file: {e}");
                            None
                        }
                    };

                    if let Some(ref data) = calibration_data {
                        log::info!(
                            "Parsed calibration data: {} points, freq range {:.0}-{:.0} Hz",
                            data.frequencies.len(),
                            data.frequencies.first().unwrap_or(&0.0),
                            data.frequencies.last().unwrap_or(&0.0)
                        );
                    }

                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };
                    state_entity.update(&mut cx.clone(), |state, _| {
                        let rs = &mut state.app.measurement_state.recording_state;

                        // Grow vecs if needed
                        while rs.mic_calibration_paths.len() <= channel_idx {
                            rs.mic_calibration_paths.push(None);
                        }
                        while rs.mic_calibration_data_per_channel.len() <= channel_idx {
                            rs.mic_calibration_data_per_channel.push(None);
                        }

                        rs.mic_calibration_paths[channel_idx] = Some(path.clone());
                        rs.mic_calibration_data_per_channel[channel_idx]
                            .clone_from(&calibration_data);

                        // Sync legacy fields from channel 0
                        if channel_idx == 0 {
                            rs.mic_calibration_path = Some(path);
                            rs.mic_calibration_data = calibration_data;
                        }
                    });
                }
            })
            .detach();
        }
    }
}

/// Update recording channel mappings and calibration vecs when channel count changes
fn update_recording_channel_mappings(state: &mut RecordingState) {
    let target_count = state.recording_config.num_channels;
    let current_count = state.recording_config.channel_mappings.len();

    if target_count > current_count {
        // Add new mappings (1-indexed for display)
        for i in current_count..target_count {
            state.recording_config.channel_mappings.push(i);
        }
    } else if target_count < current_count {
        // Remove extra mappings
        state
            .recording_config
            .channel_mappings
            .truncate(target_count);
    }

    // Sync calibration vecs to match channel count
    let cal_paths = &mut state.mic_calibration_paths;
    while cal_paths.len() < target_count {
        cal_paths.push(None);
    }
    cal_paths.truncate(target_count);

    let cal_data = &mut state.mic_calibration_data_per_channel;
    while cal_data.len() < target_count {
        cal_data.push(None);
    }
    cal_data.truncate(target_count);
}

/// Renumber all interface channels sequentially across all speakers.
/// This ensures that when a speaker switches to multi mode, subsequent
/// speakers have their channel numbers updated accordingly.
/// Example: Stereo L=0, R=1. If L becomes multi with channels 0,1,
/// then R should become channel 2.
fn renumber_interface_channels(state: &mut RecordingState) {
    let mut next_channel = 0usize;
    for mapping in &mut state.playback_config.channel_mappings {
        // Renumber each channel in this speaker sequentially
        for ch in &mut mapping.interface_channels {
            *ch = next_channel;
            next_channel += 1;
        }
    }
}
