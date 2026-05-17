//! Recording Configuration Step (Step 1)
//!
//! Device selection, channel routing, and microphone calibration.

use crate::app::types::{
    CalibrationData, ChannelMapping, CtcMatrixExportStrategy, RecordingState, SpeakerConfiguration,
};
use crate::components::design::Ds;
use crate::components::graphs::common::theme_to_chart_theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Badge, BadgeVariant, Button, ButtonSize,
    ButtonVariant, HStack, Heading, Input, InputSize, NumberInput, NumberInputSize, Select,
    SelectOption, StackAlign, StackSpacing, Text, TextSize, VStack,
};

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
                    .child(Heading::h4("Audio Device Configuration"))
                    .child(Text::new(translations.recording_config_desc).size(TextSize::Xs)),
            )
            .child(
                // Accordion with five sections
                Accordion::new()
                    .mode(AccordionMode::Multiple)
                    .expanded(expanded_sections)
                    .item(
                        AccordionItem::new("playback", "Playback Device").content(playback_content),
                    )
                    .item(
                        AccordionItem::new("recording", "Recording Device")
                            .content(recording_content),
                    )
                    .item(
                        AccordionItem::new("calibration", "Microphone Calibration")
                            .content(calibration_content),
                    )
                    .item(
                        AccordionItem::new("output_dir", "Output Directory")
                            .content(output_dir_content),
                    )
                    .item(
                        AccordionItem::new("advanced_sweep", "Advanced: Measurement Quality")
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
    fn render_playback_device_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        let device_label = VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::label(translations.recording_output_device));

        // Sample rate dropdown row
        let sample_rate_row = self
            .render_playback_sample_rate_dropdown(cx)
            .into_any_element();

        // Speaker configuration dropdown row
        let speaker_config_row = self.render_speaker_config_dropdown(cx).into_any_element();

        // Render device dropdown first, converting to AnyElement to release borrow
        let device_dropdown = self.render_playback_device_dropdown(cx).into_any_element();
        // Render channel mapping second (after first borrow is released)
        let channel_mapping = self.render_playback_channel_mapping(cx).into_any_element();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(device_label.child(device_dropdown))
            .child(sample_rate_row)
            .child(speaker_config_row)
            .child(channel_mapping)
    }

    /// Render recording device content for accordion
    fn render_recording_device_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
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
                .map(|c| c.channels as usize)
                .unwrap_or(128);
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
            .child(Badge::new(format!("{} ch", num_channels)).variant(BadgeVariant::Info))
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
                    .value(num_channels.min(max_input_channels) as f64)
                    .min(1.0)
                    .max(max_input_channels as f64)
                    .step(1.0)
                    .size(NumberInputSize::Xs)
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
                Text::new("CTC Matrix")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                Button::new("ctc_matrix_strategy", ctc_strategy.as_str())
                    .size(ButtonSize::Xs)
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
                Text::new("Loopback Input")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("ctc_loopback_input")
                    .value(ctc_loopback.map(|ch| ch + 1).unwrap_or(1) as f64)
                    .min(1.0)
                    .max(max_input_channels as f64)
                    .step(1.0)
                    .size(NumberInputSize::Xs)
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
        let positions_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new("Positions")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("recording_num_positions")
                    .value(num_positions as f64)
                    .min(1.0)
                    .max(8.0)
                    .step(1.0)
                    .size(NumberInputSize::Xs)
                    .on_change({
                        let view = view.clone();
                        move |value, _window, cx| {
                            let n = (value as usize).max(1);
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let rs = &mut state.app.measurement_state.recording_state;
                                    rs.recording_config.num_positions = n;
                                    rs.init_channel_recordings();
                                    rs.sync_channel_speakers_length();
                                });
                                cx.notify();
                            });
                        }
                    })
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
    fn render_mic_calibration_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
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
        #[allow(clippy::needless_range_loop)]
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
                        .size(InputSize::Sm)
                        .disabled(true),
                )
                .child(
                    Button::new(
                        gpui::ElementId::from(gpui::SharedString::from(browse_id)),
                        translations.recording_browse,
                    )
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
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
                    .size(ButtonSize::Sm)
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
                &theme,
                &d,
            ));
        }

        container
    }

    /// Render output directory content for accordion
    fn render_output_directory_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
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
                            .size(ButtonSize::Sm)
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
                                .size(ButtonSize::Xs)
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
            .child(Text::caption(
                "A timestamped subdirectory will be created for each recording session.",
            ))
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
    fn render_advanced_sweep_quality_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let rec_state = &state.app.measurement_state.recording_state;
        let bass_dur = rec_state.bass_octave_duration_s;
        let pre_s = rec_state.pre_silence_s;
        let post_s_opt = rec_state.post_silence_s;
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
                .size(ButtonSize::Xs)
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
                        .value(pre_s as f64)
                        .min(0.0)
                        .max(10.0)
                        .step(0.5)
                        .size(NumberInputSize::Xs)
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
                        .value(post_s_opt.unwrap_or(0.0) as f64)
                        .min(0.0)
                        .max(30.0)
                        .step(0.5)
                        .size(NumberInputSize::Xs)
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

        // Estimated capture-time hint (§2.7 default: ~14 min for 5 channels).
        // This is a rough estimate computed from bass_octave_duration_s only.
        // At the defaults (3 s/oct, 10 Hz → 20 kHz, 5 channels):
        //   bass: log2(100/10)*3.0 = ~9.97 s, mid: log2(10)*1.5 ≈ 4.98 s,
        //   high: log2(20)*1.15 ≈ 5.05 s → ~20 s/ch → 100 s per sweep pass.
        // Add pre+post: ~4 s → ~104 s per channel → 5 ch ≈ 520 s ≈ 9 min.
        // With 4 sweeps (§2.7 multi-sweep default) ≈ 36 min — but GD-1c adds
        // that; here we show single-sweep. §2.7 says ~14 min so that's the
        // expected rough value.
        let estimated_min = {
            let oct_bass = (100.0_f64 / 10.0_f64).log2();
            let oct_mid = (1000.0_f64 / 100.0_f64).log2();
            let oct_high = (20_000.0_f64 / 1000.0_f64).log2();
            let sweep_s = oct_bass * bass_dur as f64
                + oct_mid * (bass_dur as f64 * 0.5)
                + oct_high * (bass_dur as f64 * 0.25);
            let total_per_ch = sweep_s + pre_s as f64 + post_s_opt.unwrap_or(2.0) as f64;
            let channels = 5.0; // show for typical 5-channel system
            (total_per_ch * channels / 60.0).round() as u32
        };

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(Text::caption(
                "Increase bass precision to improve group delay accuracy below 100 Hz. \
                 Higher settings require longer recordings.",
            ))
            .child(bass_row)
            .child(pre_row)
            .child(post_row)
            .child(Text::caption(format!(
                "Estimated capture time (5 channels, 10 Hz–20 kHz): ~{estimated_min} min"
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
                    log::info!("Selected recording directory: {}", base_path);

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
                                log::info!("Created recording directory: {}", full_path);
                                (base_path, full_path, None)
                            }
                            Err(e) => {
                                log::error!("Failed to create recording directory: {}", e);
                                let (fallback_base, fallback_dir) =
                                    crate::app::config::default_recording_paths();
                                match (fallback_base, fallback_dir) {
                                    (Some(base), Some(dir)) => (
                                        base,
                                        dir,
                                        Some(format!(
                                            "Cannot write to selected folder; using app recordings folder: {}",
                                            e
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
                        }
                    });
                }
            })
            .detach();
        }
    }

    /// Render playback device dropdown
    fn render_playback_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .map(|d| d.name.clone())
                .unwrap_or_default()
        } else {
            recording_state.playback_config.device_name.clone()
        };

        Select::new("playback_device")
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
                                        .map(|c| c.sample_rate)
                                        .unwrap_or(48000)
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
    fn render_playback_sample_rate_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    format!("{} Hz", rate)
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
    fn render_speaker_config_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .map(|c| c.channels as usize)
            .unwrap_or(128);

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
            // Spacer to separate dropdown from badges
            .child(div().w(px(20.0))) // intentional: fixed spacer gap
            .child(
                Text::new(translations.recording_channels_label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(Badge::new(format!("{}", num_channels)).variant(BadgeVariant::Info))
            .child(Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info))
    }

    /// Render playback channel mapping table (speaker-centric view)
    fn render_playback_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
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

        // Fixed widths for consistent layout
        const LABEL_WIDTH: f32 = 80.0;
        const NAME_WIDTH: f32 = 140.0;

        let speaker_count = speaker_data.len();
        let stack =
            VStack::new()
                .spacing(StackSpacing::Xs)
                .children(speaker_data.iter().enumerate().map(
                    |(speaker_idx, (interface_channels, group_name))| {
                        let view = view.clone();
                        let theme = theme.clone();
                        let interface_channels = interface_channels.clone();
                        let group_name = group_name.clone();
                        let is_multi = interface_channels.len() > 1;

                        // For custom config, show text input; otherwise show dropdown
                        let name_widget = if is_custom {
                            div()
                                .w(px(NAME_WIDTH)) // intentional: fixed channel-name column width
                                .child(self.render_channel_name_input_raw(
                                    cx,
                                    speaker_idx,
                                    &group_name,
                                ))
                                .into_any_element()
                        } else {
                            self.render_channel_group_dropdown(cx, speaker_idx, &group_name)
                                .into_any_element()
                        };

                        // Build the speaker row content

                        if is_multi {
                            // Multi mode: show header row + channel list below
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    // Header row with speaker name, group, and mode toggle
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .align(StackAlign::Center)
                                        .child(
                                            div().w(px(LABEL_WIDTH)).child(
                                                // intentional: speaker-label column
                                                Text::new(format!("Speaker {}:", speaker_idx + 1))
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            ),
                                        )
                                        .child(name_widget)
                                        .child(
                                            self.render_speaker_mode_toggle(
                                                cx,
                                                speaker_idx,
                                                is_multi,
                                            )
                                            .into_any_element(),
                                        ),
                                )
                                .child(
                                    // Channel list for multi mode
                                    self.render_multi_channel_list(
                                        cx,
                                        speaker_idx,
                                        &interface_channels,
                                        &theme,
                                    )
                                    .into_any_element(),
                                )
                                .into_any_element()
                        } else {
                            // Single mode: inline interface channel input on same row
                            let interface_ch = interface_channels.first().copied().unwrap_or(0);
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .align(StackAlign::Center)
                                .child(
                                    div().w(px(LABEL_WIDTH)).child(
                                        // intentional: speaker-label column
                                        Text::new(format!("Speaker {}:", speaker_idx + 1))
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    ),
                                )
                                .child(name_widget)
                                .child(
                                    self.render_speaker_mode_toggle(cx, speaker_idx, is_multi)
                                        .into_any_element(),
                                )
                                .child(div().w(px(70.0))) // intentional: align with ch-number input column
                                .child(Text::caption(translations.recording_ch_short))
                                .child(div().w(px(30.0))) // intentional: spacer before ch number
                                .child({
                                    let view = view.clone();
                                    NumberInput::new(SharedString::from(format!(
                                        "speaker_{}_ch_0",
                                        speaker_idx
                                    )))
                                    .value((interface_ch + 1) as f64)
                                    .min(1.0)
                                    .max(128.0)
                                    .step(1.0)
                                    .size(NumberInputSize::Md)
                                    .on_change({
                                        let view = view.clone();
                                        move |value, _window, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    if let Some(m) = state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .playback_config
                                                        .channel_mappings
                                                        .get_mut(speaker_idx)
                                                        && let Some(ch) =
                                                            m.interface_channels.get_mut(0)
                                                    {
                                                        *ch = (value as usize).saturating_sub(1);
                                                    }
                                                    // Sync total channel count
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
                        }
                    },
                ));

        if is_custom {
            let theme_clone = theme.clone();
            let view_add = view.clone();
            let view_remove = view.clone();
            stack
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(div().w(px(LABEL_WIDTH))) // intentional: indent spacer matches header
                        .child(
                            Button::new("custom_add_speaker", "+ Add Speaker")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
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
                                                format!("Ch{}", idx),
                                            ));
                                            cfg.sync_channel_count();
                                        });
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("custom_remove_speaker", "- Remove Speaker")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
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

    /// Render the single/multi mode toggle for a speaker
    fn render_speaker_mode_toggle(
        &self,
        cx: &mut Context<Self>,
        speaker_idx: usize,
        is_multi: bool,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let is_open = state
            .app
            .measurement_state
            .recording_state
            .speaker_mode_dropdown_open
            == Some(speaker_idx);
        let view = cx.weak_entity();

        let options = vec![
            SelectOption::new("single", "Single"),
            SelectOption::new("multi", "Multi"),
        ];

        let selected = if is_multi { "multi" } else { "single" };

        div().w(px(80.0)).child(
            // intentional: speaker-mode dropdown width
            Select::new(SharedString::from(format!("speaker_mode_{}", speaker_idx)))
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
                                    .speaker_mode_dropdown_open =
                                    if open { Some(speaker_idx) } else { None };
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
    fn render_multi_channel_list(
        &self,
        cx: &mut Context<Self>,
        speaker_idx: usize,
        interface_channels: &[usize],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let view = cx.weak_entity();
        let theme = theme.clone();

        // Fixed widths matching the speaker row layout
        const LABEL_WIDTH: f32 = 80.0;
        const CH_LABEL_WIDTH: f32 = 60.0;

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
                            // Indent to align under the name column
                            .child(div().w(px(LABEL_WIDTH))) // intentional: indent spacer matches header
                            .child(div().w(px(CH_LABEL_WIDTH)).child(
                                // intentional: fixed ch-label column
                                Text::caption(format!("Ch {}:", ch_idx + 1)),
                            ))
                            .child(div().w(px(70.0)).child({
                                // intentional: fixed ch-number input column
                                let view = view.clone();
                                NumberInput::new(SharedString::from(format!(
                                    "speaker_{}_ch_{}",
                                    speaker_idx, ch_idx
                                )))
                                .value((interface_ch + 1) as f64)
                                .min(1.0)
                                .max(128.0)
                                .step(1.0)
                                .size(NumberInputSize::Xs)
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
                                    SharedString::from(format!(
                                        "remove_ch_{}_{}",
                                        speaker_idx, ch_idx
                                    )),
                                    "x",
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
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
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(div().w(px(LABEL_WIDTH))) // intentional: indent spacer matches header
                    .child(
                        Button::new(
                            SharedString::from(format!("add_ch_{}", speaker_idx)),
                            "+ Add",
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Xs)
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
    fn render_channel_name_input_raw(
        &self,
        cx: &mut Context<Self>,
        channel_idx: usize,
        current_name: &str,
    ) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        let view = cx.weak_entity();
        let current_name = current_name.to_string();

        div().size_full().child(
            Input::new(SharedString::from(format!("channel_name_{}", channel_idx)))
                .placeholder(translations.recording_name_placeholder)
                .value(current_name)
                .size(InputSize::Xs)
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
    fn render_channel_group_dropdown(
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
            .map(|(_, name)| SharedString::from(*name))
            .unwrap_or_else(|| SharedString::from(current_group.clone()));

        div().w(px(140.0)).child(
            // intentional: channel-name dropdown width
            Select::new(SharedString::from(format!("channel_name_{}", channel_idx)))
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
                                    .channel_name_dropdown_open =
                                    if is_open { Some(channel_idx) } else { None };
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
    fn render_recording_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .map(|d| d.name.clone())
                .unwrap_or_default()
        } else {
            recording_state.recording_config.device_name.clone()
        };

        Select::new("recording_device")
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
                                        .map(|c| c.sample_rate)
                                        .unwrap_or(48000)
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
    fn render_recording_sample_rate_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    format!("{} Hz", rate)
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
    fn render_recording_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                                            "recording_interface_{}",
                                            idx
                                        )))
                                        .value((interface_ch + 1) as f64)
                                        .min(1.0)
                                        .max(128.0)
                                        .step(1.0)
                                        .size(NumberInputSize::Xs)
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
    fn render_calibration_graph_multi(
        entries: &[(usize, CalibrationData)],
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
                        // intentional: render_calibration_graph_multi is a pure
                        // function that doesn't take a `translations` handle —
                        // restore the English literal to keep the static
                        // signature compatible. Future i18n pass should thread
                        // a `Translations` parameter through if needed.
                        Text::new("Unable to render calibration graph").color(theme.text_secondary),
                    )
                    .into_any_element(),
            })
            .into_any_element()
    }

    /// Open file dialog to browse for calibration file for a specific channel
    fn browse_calibration_file_for_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
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
                    log::info!(
                        "Selected calibration file for channel {}: {}",
                        channel_idx,
                        path
                    );

                    let calibration_data = match std::fs::read_to_string(&path) {
                        Ok(content) => CalibrationData::parse(&content),
                        Err(e) => {
                            log::error!("Failed to read calibration file: {}", e);
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
                        rs.mic_calibration_data_per_channel[channel_idx] = calibration_data.clone();

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
