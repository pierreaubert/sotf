//! Recording Capture Step (Step 2)
//!
//! Run test signals per channel, display state, and plot results.

use crate::app::types::{ChannelRecordingState, RecordingResult, RecordingSignalType};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Heading, NumberInput, NumberInputSize,
    Progress, ProgressSize, ProgressVariant, Select, SelectOption, StackAlign, StackJustify,
    StackSpacing, Text, TextSize, VStack,
};

impl PlayerView {
    /// Render the capture step UI
    pub(crate) fn render_recording_capture_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Heading::h4("Signal Recording"))
                    .child(Text::new(translations.recording_capture_desc).size(TextSize::Xs)),
            )
            .child(self.render_signal_config_section(cx))
            .child(self.render_channel_config_section(cx))
            .child(self.render_channel_status_section(cx))
            .child(self.render_channel_metrics_section(cx))
            .child(self.render_capture_redo_actions(cx))
    }

    /// Render signal configuration section
    fn render_signal_config_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let _ = state;

        Card::new().content(
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(Text::label(translations.recording_signal_type_label))
                        .child(self.render_signal_type_dropdown(cx)),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(Text::label(translations.recording_duration_label))
                        .child(self.render_duration_dropdown(cx)),
                ),
        )
    }

    /// Render signal type dropdown
    fn render_signal_type_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = RecordingSignalType::all()
            .iter()
            .map(|t| SelectOption::new(t.as_str(), t.as_str()))
            .collect();

        Select::new("signal_type")
            .options(options)
            .selected(recording_state.signal_type.as_str())
            .is_open(recording_state.signal_type_dropdown_open)
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .signal_type_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.measurement_state.recording_state.signal_type =
                                match value.as_ref() {
                                    "Sweep" => RecordingSignalType::Sweep,
                                    "White Noise" => RecordingSignalType::WhiteNoise,
                                    "Pink Noise" => RecordingSignalType::PinkNoise,
                                    "MLS" => RecordingSignalType::Mls,
                                    "Dirac" => RecordingSignalType::Dirac,
                                    _ => RecordingSignalType::Sweep,
                                };
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .signal_type_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render duration dropdown
    fn render_duration_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let options = vec![
            SelectOption::new("5", "5 seconds"),
            SelectOption::new("10", "10 seconds"),
            SelectOption::new("15", "15 seconds"),
            SelectOption::new("20", "20 seconds"),
        ];

        let current_duration = recording_state.signal_duration_secs as i32;

        Select::new("signal_duration")
            .options(options)
            .selected(current_duration.to_string())
            .is_open(recording_state.duration_dropdown_open)
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .duration_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            if let Ok(duration) = value.parse::<f32>() {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .signal_duration_secs = duration;
                            }
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .duration_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render per-channel sweep frequency configuration
    fn render_channel_config_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let is_sweep = recording_state.signal_type == RecordingSignalType::Sweep;

        if !is_sweep || recording_state.channel_recordings.is_empty() {
            return Card::new()
                .content(Text::caption(
                    "Frequency range configuration is only available for sweep signals",
                ))
                .into_any_element();
        }

        // Group by speaker (channel_index), show 1 row per speaker.
        // With multi-mic setups, multiple ChannelRecording entries share the same
        // channel_index — freq range is a speaker property, not per-mic.
        let mut seen_speakers = std::collections::HashSet::new();
        let channel_data: Vec<(usize, String, f32, f32)> = recording_state
            .channel_recordings
            .iter()
            .enumerate()
            .filter_map(|(vec_idx, r)| {
                if seen_speakers.insert(r.channel_index) {
                    // Use the speaker name (strip " (Mic N)" suffix if present)
                    let speaker_name = r
                        .channel_name
                        .find(" (Mic ")
                        .map(|pos| r.channel_name[..pos].to_string())
                        .unwrap_or_else(|| r.channel_name.clone());
                    Some((vec_idx, speaker_name, r.sweep_start_freq, r.sweep_end_freq))
                } else {
                    None
                }
            })
            .collect();
        let _ = state;
        let view = cx.entity().clone();

        Card::new()
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::eyebrow(translations.recording_channel_frequency_range)
                            .color(theme.accent),
                    )
                    .children(
                        channel_data
                            .iter()
                            .map(|(idx, name, start_freq, end_freq)| {
                                let idx = *idx;
                                let view_start = view.clone();
                                let view_end = view.clone();
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .child(div().w(px(100.0)).child(
                                        // intentional: channel-label column
                                        Text::label(format!("{}:", name)).color(theme.text_primary),
                                    ))
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new(translations.recording_start_label)
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            )
                                            .child(
                                                NumberInput::new(SharedString::from(format!(
                                                    "ch_start_freq_{idx}"
                                                )))
                                                .value(*start_freq as f64)
                                                .min(1.0)
                                                .max(20000.0)
                                                .step(1.0)
                                                .decimals(0)
                                                .unit("Hz")
                                                .size(NumberInputSize::Xs)
                                                .width(90.0)
                                                .on_change(move |val, _window, cx| {
                                                    view_start.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            // Update all mic entries for this speaker
                                                            let speaker_idx = state
                                                                .app
                                                                .measurement_state
                                                                .recording_state
                                                                .channel_recordings
                                                                .get(idx)
                                                                .map(|r| r.channel_index);
                                                            if let Some(si) = speaker_idx {
                                                                for rec in &mut state
                                                                    .app
                                                                    .measurement_state
                                                                    .recording_state
                                                                    .channel_recordings
                                                                {
                                                                    if rec.channel_index == si {
                                                                        rec.sweep_start_freq =
                                                                            val as f32;
                                                                    }
                                                                }
                                                            }
                                                        });
                                                        cx.notify();
                                                    });
                                                }),
                                            ),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new(translations.recording_end_label)
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            )
                                            .child(
                                                NumberInput::new(SharedString::from(format!(
                                                    "ch_end_freq_{idx}"
                                                )))
                                                .value(*end_freq as f64)
                                                .min(100.0)
                                                .max(48000.0)
                                                .step(100.0)
                                                .decimals(0)
                                                .unit("Hz")
                                                .size(NumberInputSize::Xs)
                                                .width(90.0)
                                                .on_change(move |val, _window, cx| {
                                                    view_end.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            // Update all mic entries for this speaker
                                                            let speaker_idx = state
                                                                .app
                                                                .measurement_state
                                                                .recording_state
                                                                .channel_recordings
                                                                .get(idx)
                                                                .map(|r| r.channel_index);
                                                            if let Some(si) = speaker_idx {
                                                                for rec in &mut state
                                                                    .app
                                                                    .measurement_state
                                                                    .recording_state
                                                                    .channel_recordings
                                                                {
                                                                    if rec.channel_index == si {
                                                                        rec.sweep_end_freq =
                                                                            val as f32;
                                                                    }
                                                                }
                                                            }
                                                        });
                                                        cx.notify();
                                                    });
                                                }),
                                            ),
                                    )
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    /// Render per-channel recording metrics (avg SPL and noise floor)
    fn render_channel_metrics_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;

        // Collect metrics for channels that have results
        let metrics: Vec<(String, usize, f32, f32)> = recording_state
            .channel_recordings
            .iter()
            .filter_map(|rec| {
                let result = rec.result.as_ref()?;
                if result.frequencies.is_empty() {
                    return None;
                }

                // Compute avg SPL, excluding noise-floor sentinels (-200 dB)
                // from narrow-band sweeps (e.g., LFE 20-500 Hz)
                let mut sum = 0.0_f32;
                let mut count = 0usize;
                for (&freq, &mag) in result.frequencies.iter().zip(result.magnitude_db.iter()) {
                    if (20.0..=20000.0).contains(&freq) && mag > -150.0 {
                        sum += mag;
                        count += 1;
                    }
                }
                let avg_spl = if count > 0 { sum / count as f32 } else { 0.0 };

                // Estimate noise floor from the lowest 5% of valid magnitude values
                // (exclude -200 dB sentinels from out-of-band frequencies)
                let mut sorted_mags: Vec<f32> = result
                    .magnitude_db
                    .iter()
                    .copied()
                    .filter(|&m| m > -150.0)
                    .collect();
                sorted_mags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let bottom_count = (sorted_mags.len() / 20).max(1);
                let noise_floor: f32 = if sorted_mags.is_empty() {
                    -100.0
                } else {
                    sorted_mags[..bottom_count].iter().sum::<f32>() / bottom_count as f32
                };

                // Mic input channel used for this recording
                let mic_ch = recording_state
                    .recording_config
                    .channel_mappings
                    .get(rec.mic_index)
                    .copied()
                    .unwrap_or(0);

                Some((rec.channel_name.clone(), mic_ch, avg_spl, noise_floor))
            })
            .collect();

        if metrics.is_empty() {
            return div().into_any_element();
        }

        Card::new()
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::eyebrow(translations.recording_channel_metrics).color(theme.accent),
                    )
                    // Header row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                div().w(px(100.0)).child(
                                    // intentional: metrics-table column width
                                    Text::label(translations.recording_channel_column)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(60.0)).child(
                                    // intentional: metrics-table column width
                                    Text::label(translations.recording_mic_in)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    // intentional: metrics-table column width
                                    Text::label(translations.recording_avg_spl)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    // intentional: metrics-table column width
                                    Text::label(translations.recording_noise_floor)
                                        .color(theme.text_secondary),
                                ),
                            ),
                    )
                    // Data rows
                    .children(metrics.iter().map(|(name, mic_ch, avg_spl, noise_floor)| {
                        let spl_color = if *avg_spl < -50.0 {
                            theme.warning
                        } else {
                            theme.text_primary
                        };
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                div().w(px(100.0)).child(
                                    // intentional: metrics-table column width
                                    Text::new(format!("{}:", name))
                                        .size(TextSize::Xs)
                                        .color(theme.text_primary),
                                ),
                            )
                            .child(
                                div().w(px(60.0)).child(
                                    // intentional: metrics-table column width
                                    Text::new(format!("Ch {}", mic_ch + 1))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    // intentional: metrics-table column width
                                    Text::new(format!("{:.1} dB", avg_spl))
                                        .size(TextSize::Xs)
                                        .color(spl_color),
                                ),
                            )
                            .child(div().w(px(80.0)).child(
                                // intentional: metrics-table column width
                                Text::caption(format!("{:.1} dB", noise_floor)),
                            ))
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    /// Render channel status section with recording controls
    fn render_channel_status_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let is_recording = state.app.measurement_state.recording_state.is_recording();
        let status_message = state
            .app
            .measurement_state
            .recording_state
            .status_message
            .clone();
        let recording_progress = state
            .app
            .measurement_state
            .recording_state
            .recording_progress;
        let noise_floor_warning = state
            .app
            .measurement_state
            .recording_state
            .noise_floor_warning
            .clone();
        let _ = state;

        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .justify(StackJustify::SpaceBetween)
                        .align(StackAlign::Center)
                        .child(
                            Text::eyebrow(translations.recording_channel_status)
                                .color(theme.accent),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .when(!is_recording, |stack| {
                                    let view = view.clone();
                                    let theme = theme.clone();
                                    let label = translations.recording_record_all_channels;
                                    stack.child(
                                        Button::new("record_all", label)
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click({
                                                let view = view.clone();
                                                move |_, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.start_recording_all_channels(cx);
                                                    });
                                                }
                                            }),
                                    )
                                })
                                .when(is_recording, |stack| {
                                    let view = view.clone();
                                    let theme = theme.clone();
                                    let label = translations.recording_stop_recording;
                                    stack.child(
                                        Button::new("stop_recording", label)
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click({
                                                let view = view.clone();
                                                move |_, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.stop_recording(cx);
                                                    });
                                                }
                                            }),
                                    )
                                }),
                        ),
                )
                .child(self.render_channel_list(cx))
                .when(!status_message.is_empty(), |stack| {
                    stack.child(Text::new(status_message.clone()).size(TextSize::Xs))
                })
                .when(noise_floor_warning.is_some(), |stack| {
                    let warning_msg = noise_floor_warning.clone().unwrap_or_default();
                    let warning_bg = theme.warning_background;
                    stack.child(
                        div()
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .bg(warning_bg)
                            .border_1()
                            .border_color(theme.warning)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .align(StackAlign::Center)
                                    .child(Text::new("⚠").size(TextSize::Sm).color(theme.warning))
                                    .child(
                                        Text::new(warning_msg)
                                            .size(TextSize::Xs)
                                            .color(theme.warning),
                                    ),
                            ),
                    )
                })
                .when(is_recording, |stack| {
                    stack.child(
                        Progress::new(recording_progress)
                            .size(ProgressSize::Sm)
                            .variant(ProgressVariant::Default),
                    )
                }),
        )
    }

    /// Render the list of channels with their recording status.
    ///
    /// Groups by speaker (channel_index). With multi-mic, shows mic sub-statuses
    /// within each speaker row.
    fn render_channel_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = crate::components::design::Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();
        let is_recording = recording_state.is_recording();
        let num_mics = recording_state
            .recording_config
            .channel_mappings
            .len()
            .max(1);

        if recording_state.channel_recordings.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    div()
                        .p(d.card)
                        .rounded(d.r_md)
                        .bg(theme.surface)
                        .child(Text::caption(
                            "No channels configured. Please go back and configure your devices.",
                        )),
                )
                .into_any_element();
        }

        // Build grouped data: Vec<(speaker_name, speaker_idx, first_vec_idx, Vec<(mic_idx, state)>)>
        type SpeakerGroup = (String, usize, usize, Vec<(usize, ChannelRecordingState)>);
        let mut speaker_groups: Vec<SpeakerGroup> = Vec::new();
        for (vec_idx, rec) in recording_state.channel_recordings.iter().enumerate() {
            if let Some(group) = speaker_groups
                .iter_mut()
                .find(|(_, si, _, _)| *si == rec.channel_index)
            {
                group.3.push((rec.mic_index, rec.state));
            } else {
                let speaker_name = rec
                    .channel_name
                    .find(" (Mic ")
                    .map(|pos| rec.channel_name[..pos].to_string())
                    .unwrap_or_else(|| rec.channel_name.clone());
                speaker_groups.push((
                    speaker_name,
                    rec.channel_index,
                    vec_idx,
                    vec![(rec.mic_index, rec.state)],
                ));
            }
        }

        VStack::new()
            .spacing(StackSpacing::Xs)
            .children(speaker_groups.into_iter().map(
                move |(speaker_name, _speaker_idx, first_vec_idx, mic_states)| {
                    let theme = theme.clone();
                    let view = view.clone();

                    // Aggregate state across all mics for this speaker
                    let all_done = mic_states
                        .iter()
                        .all(|(_, s)| *s == ChannelRecordingState::Done);
                    let any_recording = mic_states
                        .iter()
                        .any(|(_, s)| *s == ChannelRecordingState::Recording);
                    let any_error = mic_states
                        .iter()
                        .any(|(_, s)| *s == ChannelRecordingState::Error);

                    let (state_text, state_color) = if any_recording {
                        (translations.recording_state_recording, theme.warning)
                    } else if all_done {
                        (translations.recording_state_complete, theme.success)
                    } else if any_error {
                        (translations.recording_state_error, theme.error)
                    } else {
                        (translations.recording_state_not_recorded, theme.text_muted)
                    };
                    let state_icon: AnyElement = if any_recording {
                        // intentional: 8px dot status indicator, not a scaling icon
                        div()
                            .size(px(8.0))
                            .rounded_full()
                            .bg(theme.warning)
                            .into_any_element()
                    } else if all_done {
                        Icon::new(IconName::Check)
                            .size(IconSize::Xs)
                            .color(theme.success)
                            .into_any_element()
                    } else if any_error {
                        Icon::new(IconName::X)
                            .size(IconSize::Xs)
                            .color(theme.error)
                            .into_any_element()
                    } else {
                        // intentional: 8px dot status indicator, not a scaling icon
                        div()
                            .size(px(8.0))
                            .rounded_full()
                            .border_1()
                            .border_color(theme.text_muted)
                            .into_any_element()
                    };

                    let button_label = if all_done {
                        translations.recording_re_record
                    } else {
                        translations.recording_record
                    };

                    let mut row = div()
                        .flex()
                        .flex_col()
                        .gap(d.grid)
                        .p(d.pad_y)
                        .rounded(d.r_md)
                        .bg(theme.surface)
                        .child(
                            // Main speaker row
                            div()
                                .flex()
                                .items_center()
                                .gap(d.gap_md)
                                .child(
                                    div().w(px(100.0)).child(
                                        // intentional: speaker-label column
                                        Text::label(format!("{}:", speaker_name))
                                            .color(theme.text_primary),
                                    ),
                                )
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .align(StackAlign::Center)
                                        .child(state_icon)
                                        .child(
                                            Text::new(state_text)
                                                .size(TextSize::Xs)
                                                .color(state_color),
                                        ),
                                )
                                .child(div().flex_1())
                                .child(
                                    Button::new(
                                        SharedString::from(format!(
                                            "record_speaker_{}",
                                            first_vec_idx
                                        )),
                                        button_label,
                                    )
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Xs)
                                    .disabled(is_recording || any_recording)
                                    .theme(theme.to_button_theme())
                                    .on_click({
                                        let view = view.clone();
                                        move |_, cx| {
                                            view.update(cx, |this, cx| {
                                                this.start_recording_channel(first_vec_idx, cx);
                                            });
                                        }
                                    }),
                                ),
                        );

                    // Show per-mic sub-statuses when multi-mic and at least one is done
                    if num_mics > 1 {
                        let has_any_result = mic_states
                            .iter()
                            .any(|(_, s)| *s != ChannelRecordingState::Empty);
                        if has_any_result {
                            row = row.child(div().pl_6().child(
                                HStack::new().spacing(StackSpacing::Sm).children(
                                    mic_states.iter().map(|(mic_idx, mic_state)| {
                                        let icon_el: AnyElement = match mic_state {
                                            ChannelRecordingState::Empty => {
                                                // intentional: 8px dot status indicator
                                                div()
                                                    .size(px(8.0))
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(theme.text_muted)
                                                    .into_any_element()
                                            }
                                            ChannelRecordingState::Recording => {
                                                // intentional: 8px dot status indicator
                                                div()
                                                    .size(px(8.0))
                                                    .rounded_full()
                                                    .bg(theme.warning)
                                                    .into_any_element()
                                            }
                                            ChannelRecordingState::Done => {
                                                Icon::new(IconName::Check)
                                                    .size(IconSize::Xs)
                                                    .color(theme.success)
                                                    .into_any_element()
                                            }
                                            ChannelRecordingState::Error => Icon::new(IconName::X)
                                                .size(IconSize::Xs)
                                                .color(theme.error)
                                                .into_any_element(),
                                        };
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new(format!("Mic {}:", mic_idx + 1))
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            )
                                            .child(icon_el)
                                            .into_any_element()
                                    }),
                                ),
                            ));
                        }
                    }

                    row.into_any_element()
                },
            ))
            .into_any_element()
    }

    /// Render capture action buttons (redo and load from file)
    fn render_capture_redo_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let is_recording = recording_state.is_recording();
        let view = cx.entity().clone();

        let has_recordings = recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == ChannelRecordingState::Done);

        HStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Button::new("redo_recordings", translations.recording_redo_all)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .disabled(!has_recordings || is_recording)
                    .theme(theme.to_button_theme())
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.reset_all_recordings(cx);
                            });
                        }
                    }),
            )
            .child(
                Button::new("load_from_file", translations.recording_load_from_file)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .disabled(is_recording)
                    .theme(theme.to_button_theme())
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.load_recordings_from_file(cx);
                            });
                        }
                    }),
            )
    }

    // ==========================================================================
    // Recording control methods
    // ==========================================================================

    /// Start recording all channels sequentially
    pub fn start_recording_all_channels(&mut self, cx: &mut Context<Self>) {
        // Enable auto-record mode and clear all previous results upfront
        // so the evaluating graphs don't show stale data from a prior session.
        self.state.update(cx, |state, _| {
            state
                .app
                .measurement_state
                .recording_state
                .auto_record_remaining = true;
            for rec in &mut state
                .app
                .measurement_state
                .recording_state
                .channel_recordings
            {
                rec.state = ChannelRecordingState::Empty;
                rec.result = None;
            }
        });

        // Start with the first channel
        self.start_recording_channel(0, cx);

        log::info!("Starting auto-record mode - all channels will be recorded sequentially");
    }

    /// Start recording a speaker (all mics captured simultaneously).
    ///
    /// `channel_idx` is a vec index into `channel_recordings` — any entry belonging
    /// to the target speaker works (typically the first one for that speaker).
    /// With a single mic this behaves exactly as before; with N mics it captures
    /// all N input channels in one pass and populates every mic entry for the speaker.
    #[allow(clippy::type_complexity)]
    pub fn start_recording_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
        use sotf_audio_player::signal_recorder::{
            DEFAULT_MLS_ORDER, SignalParams, SignalType, generate_signal, write_temp_wav,
        };

        // --- Gather parameters from state ---
        #[derive(Clone)]
        struct MicInfo {
            vec_idx: Option<usize>,
            #[allow(dead_code)]
            mic_index: usize,
            is_loopback: bool,
            speaker_index: usize,
            mic_position_index: usize,
            input_channel: u16,
            calibration: Option<String>,
            safe_name: String,
        }

        struct RecParams {
            signal_type: SignalType,
            duration_secs: f32,
            level_db: f32,
            sweep_start_freq: f32,
            sweep_end_freq: f32,
            output_device: String,
            input_device: String,
            output_channel: u16,
            sample_rate: u32,
            speaker_name: String,
            recording_directory: Option<String>,
            mics: Vec<MicInfo>,
            ctc_strategy: crate::app::types::CtcMatrixExportStrategy,
        }

        let params = {
            let state = self.state.read(cx);
            let rec_state = &state.app.measurement_state.recording_state;

            let channel = match rec_state.channel_recordings.get(channel_idx) {
                Some(c) => c,
                None => {
                    log::error!("Invalid channel index: {}", channel_idx);
                    return;
                }
            };

            let speaker_idx = channel.channel_index;
            let position_idx = channel.mic_position_index;
            let sweep_start = channel.sweep_start_freq;
            let sweep_end = channel.sweep_end_freq;

            // Collect all ear-mic entries for this speaker at the current
            // position. Other positions are separate physical mic placements.
            let mut mics: Vec<MicInfo> = rec_state
                .channel_recordings
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    r.channel_index == speaker_idx && r.mic_position_index == position_idx
                })
                .map(|(vi, r)| {
                    let input_ch = rec_state
                        .recording_config
                        .channel_mappings
                        .get(r.mic_index)
                        .or_else(|| rec_state.recording_config.channel_mappings.first())
                        .copied()
                        .unwrap_or(0) as u16;
                    let calibration = rec_state
                        .mic_calibration_paths
                        .get(r.mic_index)
                        .and_then(|p| p.clone())
                        .or_else(|| rec_state.mic_calibration_path.clone());
                    let safe_name: String = r
                        .channel_name
                        .chars()
                        .map(|c| {
                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();
                    MicInfo {
                        vec_idx: Some(vi),
                        mic_index: r.mic_index,
                        is_loopback: false,
                        speaker_index: speaker_idx,
                        mic_position_index: position_idx,
                        input_channel: input_ch,
                        calibration,
                        safe_name,
                    }
                })
                .collect();

            if rec_state.recording_config.ctc_matrix_strategy
                == crate::app::types::CtcMatrixExportStrategy::RawSweep
                && let Some(loopback_input) = rec_state.recording_config.ctc_loopback_input_channel
            {
                let safe_speaker: String = channel
                    .channel_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '_' || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                mics.push(MicInfo {
                    vec_idx: None,
                    mic_index: usize::MAX,
                    is_loopback: true,
                    speaker_index: speaker_idx,
                    mic_position_index: position_idx,
                    input_channel: loopback_input as u16,
                    calibration: None,
                    safe_name: format!("{}_Pos_{}_Loopback", safe_speaker, position_idx + 1),
                });
            }

            let signal_type = match rec_state.signal_type {
                RecordingSignalType::Sweep => SignalType::Sweep,
                RecordingSignalType::WhiteNoise => SignalType::WhiteNoise,
                RecordingSignalType::PinkNoise => SignalType::PinkNoise,
                RecordingSignalType::Mls => SignalType::Mls,
                RecordingSignalType::Dirac => SignalType::Dirac,
                RecordingSignalType::DelayProbe => {
                    log::warn!(
                        "DelayProbe selected in per-channel mode; use probe_channel_delays() instead. Falling back to Sweep."
                    );
                    SignalType::Sweep
                }
            };

            let output_ch = rec_state
                .playback_config
                .channel_mappings
                .get(speaker_idx)
                .map(|m| m.interface_channel())
                .unwrap_or(0) as u16;

            let speaker_name = channel
                .channel_name
                .find(" (Mic ")
                .map(|pos| channel.channel_name[..pos].to_string())
                .unwrap_or_else(|| channel.channel_name.clone());

            RecParams {
                signal_type,
                duration_secs: rec_state.signal_duration_secs,
                level_db: rec_state.signal_level_db,
                sweep_start_freq: sweep_start,
                sweep_end_freq: sweep_end,
                output_device: rec_state.playback_config.device_name.clone(),
                input_device: rec_state.recording_config.device_name.clone(),
                output_channel: output_ch,
                sample_rate: rec_state.playback_config.sample_rate,
                speaker_name,
                recording_directory: rec_state.recording_directory.clone(),
                mics,
                ctc_strategy: rec_state.recording_config.ctc_matrix_strategy,
            }
        };

        let recording_dir = match params.recording_directory {
            Some(ref dir) => std::path::PathBuf::from(dir),
            None => {
                log::error!("No recording directory selected");
                self.state.update(cx, |state, _| {
                    state.app.measurement_state.recording_state.status_message =
                        "Please select a recording directory in the Configuration step".to_string();
                });
                cx.notify();
                return;
            }
        };

        // Ensure the recording directory exists before writing WAV/CSV files
        if let Err(e) = std::fs::create_dir_all(&recording_dir) {
            log::error!(
                "Failed to create recording directory {:?}: {}",
                recording_dir,
                e
            );
            self.state.update(cx, |state, _| {
                state.app.measurement_state.recording_state.status_message =
                    format!("Cannot create recording directory: {}", e);
            });
            cx.notify();
            return;
        }

        // Mark all mic entries for this speaker as Recording and clear old results
        // so the evaluating graphs reset immediately (not showing stale data).
        let mic_vec_indices: Vec<usize> = params.mics.iter().filter_map(|m| m.vec_idx).collect();
        self.state.update(cx, |state, _| {
            for &vi in &mic_vec_indices {
                if let Some(recording) = state
                    .app
                    .measurement_state
                    .recording_state
                    .channel_recordings
                    .get_mut(vi)
                {
                    recording.state = ChannelRecordingState::Recording;
                    recording.result = None; // Clear old result so graphs reset
                }
            }
            state
                .app
                .measurement_state
                .recording_state
                .current_recording_channel = Some(channel_idx);
            state.app.measurement_state.recording_state.status_message =
                format!("Recording {}...", params.speaker_name);
            state
                .app
                .measurement_state
                .recording_state
                .recording_progress = 0.0;
        });
        cx.notify();

        // Log the exact parameters for diagnostics
        log::warn!(
            "Recording params: speaker={}, speaker_idx={}, output_ch={}, sweep={:.0}-{:.0}Hz, sr={}, level={:.1}dB, mics={}, mic_vec_indices={:?}",
            params.speaker_name,
            // Re-read speaker_idx from the first mic entry
            params
                .mics
                .first()
                .map(|m| { m.speaker_index })
                .unwrap_or(999),
            params.output_channel,
            params.sweep_start_freq,
            params.sweep_end_freq,
            params.sample_rate,
            params.level_db,
            params.mics.len(),
            mic_vec_indices,
        );

        // Generate signal
        let amplitude = 10.0_f32.powf(params.level_db / 20.0);
        let sig_params = match params.signal_type {
            SignalType::Sweep => SignalParams::Sweep {
                start_freq: params.sweep_start_freq,
                end_freq: params.sweep_end_freq,
                amp: amplitude,
            },
            SignalType::WhiteNoise | SignalType::PinkNoise => {
                SignalParams::Noise { amp: amplitude }
            }
            SignalType::Mls => SignalParams::Mls {
                order: DEFAULT_MLS_ORDER,
                amp: amplitude,
            },
            SignalType::Dirac => SignalParams::Dirac { amp: amplitude },
            _ => SignalParams::Sweep {
                start_freq: params.sweep_start_freq,
                end_freq: params.sweep_end_freq,
                amp: amplitude,
            },
        };

        let signal = match generate_signal(
            params.signal_type,
            &sig_params,
            params.duration_secs,
            params.sample_rate,
        ) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to generate signal: {}", e);
                self.state.update(cx, |state, _| {
                    for &vi in &mic_vec_indices {
                        if let Some(rec) = state
                            .app
                            .measurement_state
                            .recording_state
                            .channel_recordings
                            .get_mut(vi)
                        {
                            rec.state = ChannelRecordingState::Error;
                        }
                    }
                    state
                        .app
                        .measurement_state
                        .recording_state
                        .current_recording_channel = None;
                    state.app.measurement_state.recording_state.status_message =
                        format!("Error: {}", e);
                });
                cx.notify();
                return;
            }
        };

        let temp_wav = match write_temp_wav(&signal, params.sample_rate, 1) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to write temp WAV: {}", e);
                self.state.update(cx, |state, _| {
                    for &vi in &mic_vec_indices {
                        if let Some(rec) = state
                            .app
                            .measurement_state
                            .recording_state
                            .channel_recordings
                            .get_mut(vi)
                        {
                            rec.state = ChannelRecordingState::Error;
                        }
                    }
                    state
                        .app
                        .measurement_state
                        .recording_state
                        .current_recording_channel = None;
                    state.app.measurement_state.recording_state.status_message =
                        format!("Error: {}", e);
                });
                cx.notify();
                return;
            }
        };

        if params.ctc_strategy == crate::app::types::CtcMatrixExportStrategy::RawSweep {
            let reference_path = recording_dir.join("ctc_reference_sweep.wav");
            if let Err(e) = sotf_audio_player::signal_recorder::write_wav_file(
                &reference_path,
                &signal,
                params.sample_rate,
                1,
            ) {
                log::warn!("Failed to persist CTC reference sweep: {}", e);
            } else {
                self.state.update(cx, |state, _| {
                    state
                        .app
                        .measurement_state
                        .recording_state
                        .ctc_reference_sweep_path =
                        Some(reference_path.to_string_lossy().to_string());
                });
            }
        }

        // Build per-mic paths
        let wav_paths: Vec<std::path::PathBuf> = params
            .mics
            .iter()
            .map(|m| recording_dir.join(format!("{}.wav", m.safe_name)))
            .collect();
        let csv_paths: Vec<std::path::PathBuf> = params
            .mics
            .iter()
            .map(|m| recording_dir.join(format!("{}.csv", m.safe_name)))
            .collect();
        let input_channels: Vec<u16> = params.mics.iter().map(|m| m.input_channel).collect();
        let mic_calibrations: Vec<Option<String>> =
            params.mics.iter().map(|m| m.calibration.clone()).collect();

        let weak_state = self.state.downgrade();
        let weak_view = cx.weak_entity();
        let reference_signal = signal.clone();
        let temp_wav_path = temp_wav.path().to_path_buf();
        let mics = params.mics.clone();
        let speaker_name = params.speaker_name.clone();
        let signal_type = params.signal_type;
        let sweep_start_freq = params.sweep_start_freq;
        let sweep_end_freq = params.sweep_end_freq;
        let output_channel = params.output_channel;
        let output_device = params.output_device.clone();
        let input_device = params.input_device.clone();
        let sample_rate = params.sample_rate;

        cx.spawn(async move |_, cx| {
            use sotf_audio_player::signal_recorder::SignalType;

            let sweep_range = if signal_type == SignalType::Sweep {
                Some((sweep_start_freq, sweep_end_freq))
            } else {
                None
            };

            let out_dev = if output_device.is_empty() {
                None
            } else {
                Some(output_device.as_str())
            };
            let in_dev = if input_device.is_empty() {
                None
            } else {
                Some(input_device.as_str())
            };

            let num_mics = mics.len();
            log::info!(
                "Starting recording: speaker={}, output_ch={}, {} mics, input_chs={:?}",
                speaker_name,
                output_channel,
                num_mics,
                input_channels,
            );

            // Use multi-channel recording when multiple mics, single-channel otherwise
            #[cfg(not(target_os = "ios"))]
            let results: Result<Vec<_>, String> = if num_mics <= 1 {
                use sotf_audio_player::signal_recorder::record_and_analyze;
                record_and_analyze(
                    &temp_wav_path,
                    &wav_paths[0],
                    &reference_signal,
                    sample_rate,
                    &csv_paths[0],
                    output_channel,
                    input_channels[0],
                    out_dev,
                    in_dev,
                    mic_calibrations[0].as_deref(),
                    sweep_range,
                )
                .map(|r| vec![r])
            } else {
                use sotf_audio_player::signal_recorder::record_and_analyze_multi;
                record_and_analyze_multi(
                    &temp_wav_path,
                    &wav_paths,
                    &reference_signal,
                    sample_rate,
                    &csv_paths,
                    output_channel,
                    &input_channels,
                    out_dev,
                    in_dev,
                    &mic_calibrations,
                    sweep_range,
                )
            };

            #[cfg(target_os = "ios")]
            let results: Result<Vec<sotf_audio::AnalysisResult>, String> =
                Err("Recording not available on iOS".to_string());

            // Update state with results
            let Some(state_entity) = weak_state.upgrade() else { return; };
            let (should_auto_continue, next_channel_idx, need_position_modal, cur_pos) =
                state_entity.update(&mut cx.clone(), |state, _| {
                    let should_continue = match results {
                        Ok(analysis_results) => {
                            const NOISE_FLOOR_THRESHOLD_DB: f32 = -50.0;
                            let mut any_low_signal = false;

                            for (mic_i, analysis_result) in
                                analysis_results.into_iter().enumerate()
                            {
                                let mic = &mics[mic_i];
                                let mic_name = &mics[mic_i].safe_name;

                                // Compute avg SPL within the actual sweep range
                                // (not hardcoded 100-10000 Hz — LFE uses 20-500 Hz)
                                let avg_spl = {
                                    let avg_min = sweep_start_freq.max(20.0);
                                    let avg_max = sweep_end_freq.min(20000.0);
                                    let mut sum = 0.0_f32;
                                    let mut count = 0;
                                    for (&freq, &mag) in analysis_result
                                        .frequencies
                                        .iter()
                                        .zip(analysis_result.spl_db.iter())
                                    {
                                        if freq >= avg_min && freq <= avg_max && mag > -150.0 {
                                            sum += mag;
                                            count += 1;
                                        }
                                    }
                                    if count > 0 {
                                        sum / count as f32
                                    } else {
                                        0.0
                                    }
                                };

                                if avg_spl < NOISE_FLOOR_THRESHOLD_DB {
                                    any_low_signal = true;
                                    log::warn!(
                                        "Noise floor warning: {} avg SPL = {:.1} dB",
                                        mic_name,
                                        avg_spl,
                                    );
                                }

                                let rec_result = RecordingResult {
                                    channel: mic.vec_idx.unwrap_or(usize::MAX),
                                    wav_path: Some(wav_paths[mic_i].to_string_lossy().to_string()),
                                    csv_path: Some(csv_paths[mic_i].to_string_lossy().to_string()),
                                    frequencies: analysis_result.frequencies,
                                    magnitude_db: analysis_result.spl_db,
                                    phase_deg: analysis_result.phase_deg,
                                    impulse_response: Some(analysis_result.impulse_response),
                                    impulse_time_ms: Some(analysis_result.impulse_time_ms),
                                    excess_group_delay_ms: Some(
                                        analysis_result.excess_group_delay_ms,
                                    ),
                                    thd_percent: Some(analysis_result.thd_percent),
                                    harmonic_distortion_db: Some(
                                        analysis_result.harmonic_distortion_db,
                                    ),
                                    rt60_ms: Some(analysis_result.rt60_ms),
                                    clarity_c50_db: Some(analysis_result.clarity_c50_db),
                                    clarity_c80_db: Some(analysis_result.clarity_c80_db),
                                    spectrogram_db: Some(analysis_result.spectrogram_db),
                                };

                                if mic.is_loopback {
                                    state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .transfer_matrix_loopbacks
                                        .retain(|r| {
                                            r.speaker_index != mic.speaker_index
                                                || r.mic_position_index != mic.mic_position_index
                                        });
                                    state
                                        .app
                                        .measurement_state
                                        .recording_state
                                        .transfer_matrix_loopbacks
                                        .push(crate::app::types::TransferMatrixLoopbackRecording {
                                            speaker_index: mic.speaker_index,
                                            mic_position_index: mic.mic_position_index,
                                            wav_path: wav_paths[mic_i]
                                                .to_string_lossy()
                                                .to_string(),
                                        });
                                    continue;
                                }

                                let Some(vi) = mic.vec_idx else {
                                    continue;
                                };
                                if let Some(recording) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_recordings
                                    .get_mut(vi)
                                {
                                    recording.state = ChannelRecordingState::Done;
                                    recording.result = Some(rec_result);
                                }
                            }

                            if any_low_signal {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .noise_floor_warning = Some(format!(
                                    "Speaker '{}' has mic(s) with very low signal. Check connections or increase level.",
                                    speaker_name,
                                ));
                            } else {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .noise_floor_warning = None;
                            }

                            state.app.measurement_state.recording_state.status_message =
                                format!("{} recording complete", speaker_name);

                            state
                                .app
                                .measurement_state
                                .recording_state
                                .auto_record_remaining
                        }
                        Err(e) => {
                            log::error!("Recording failed: {}", e);
                            for mic in &mics {
                                let Some(vec_idx) = mic.vec_idx else {
                                    continue;
                                };
                                if let Some(recording) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_recordings
                                    .get_mut(vec_idx)
                                {
                                    recording.state = ChannelRecordingState::Error;
                                }
                            }
                            state.app.measurement_state.recording_state.status_message =
                                format!("Recording error: {}", e);
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .auto_record_remaining = false;
                            false
                        }
                    };

                    state
                        .app
                        .measurement_state
                        .recording_state
                        .current_recording_channel = None;
                    state
                        .app
                        .measurement_state
                        .recording_state
                        .recording_progress = 1.0;

                    // Find the next speaker to record AT THE CURRENT
                    // POSITION. When the current position is fully Done,
                    // we don't return an index — the caller pauses for
                    // the move-position modal (or finishes if no more
                    // positions remain).
                    let rec_state = &state.app.measurement_state.recording_state;
                    let num_positions = rec_state.recording_config.num_positions.max(1);
                    let cur_pos = rec_state.current_position();
                    let next_channel_idx = if should_continue && cur_pos < num_positions {
                        rec_state.next_channel_in_position(cur_pos)
                    } else {
                        None
                    };
                    let need_position_modal = should_continue
                        && next_channel_idx.is_none()
                        && cur_pos < num_positions;

                    (
                        should_continue,
                        next_channel_idx,
                        need_position_modal,
                        cur_pos,
                    )
                });

            if should_auto_continue {
                if let Some(next_idx) = next_channel_idx {
                    log::info!("Auto-recording: starting next speaker at idx {}", next_idx);
                    let _ = weak_view.update(cx, |view, cx| {
                        view.start_recording_channel(next_idx, cx);
                    });
                } else if need_position_modal {
                    // Pause auto-record and ask the user to move the
                    // microphones to the next seat. The modal's Continue
                    // button resumes by calling start_recording_channel
                    // for the first speaker at `cur_pos`.
                    log::info!(
                        "Auto-recording: position {} done; prompting user to move mics to position {}",
                        cur_pos,
                        cur_pos + 1
                    );
                    let _ = weak_view.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            let rec_state =
                                &mut state.app.measurement_state.recording_state;
                            rec_state.move_position_modal_open = true;
                            rec_state.pending_next_position = Some(cur_pos);
                            rec_state.status_message = format!(
                                "Move microphones to position {} and click Continue",
                                cur_pos + 1
                            );
                        });
                        cx.notify();
                    });
                } else {
                    log::info!("Auto-recording complete - all channels recorded, saving JSON");
                    let _ = weak_view.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .auto_record_remaining = false;
                            state.app.measurement_state.recording_state.status_message =
                                "All channels recorded, saving...".to_string();
                        });
                        // Auto-save recordings.json with all channel data
                        view.save_recordings(cx);
                        cx.notify();
                    });
                }
            }

            drop(temp_wav);
        })
        .detach();

        log::info!("Recording started for speaker {}", params.speaker_name);
    }

    /// Stop all recording
    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state
                .app
                .measurement_state
                .recording_state
                .current_recording_channel = None;
            state
                .app
                .measurement_state
                .recording_state
                .recording_progress = 0.0;
            state.app.measurement_state.recording_state.status_message =
                "Recording stopped".to_string();
            state
                .app
                .measurement_state
                .recording_state
                .auto_record_remaining = false; // Disable auto-record mode

            // Reset any channels that were recording back to empty
            for recording in &mut state
                .app
                .measurement_state
                .recording_state
                .channel_recordings
            {
                if recording.state == ChannelRecordingState::Recording {
                    recording.state = ChannelRecordingState::Empty;
                }
            }
        });
        cx.notify();

        log::info!("Recording stopped - auto-record mode disabled");
    }

    /// Reset all recordings to empty state
    fn reset_all_recordings(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            for recording in &mut state
                .app
                .measurement_state
                .recording_state
                .channel_recordings
            {
                recording.state = ChannelRecordingState::Empty;
                recording.result = None;
            }
            state
                .app
                .measurement_state
                .recording_state
                .current_recording_channel = None;
            state
                .app
                .measurement_state
                .recording_state
                .recording_progress = 0.0;
            state.app.measurement_state.recording_state.status_message = String::new();
            state
                .app
                .measurement_state
                .recording_state
                .auto_record_remaining = false;
        });
        cx.notify();

        log::info!("All recordings reset");
    }

    fn rewrite_optional_path_to_dir(path: &mut Option<String>, dir: &std::path::Path) {
        let Some(existing) = path else {
            return;
        };
        let Some(file_name) = std::path::Path::new(existing).file_name() else {
            return;
        };
        *existing = dir.join(file_name).to_string_lossy().to_string();
    }

    fn rewrite_path_to_dir(path: &mut String, dir: &std::path::Path) {
        let Some(file_name) = std::path::Path::new(path).file_name() else {
            return;
        };
        *path = dir.join(file_name).to_string_lossy().to_string();
    }

    /// Make the on-disk recording directory match the Save-step name.
    ///
    /// Capture starts in a timestamped folder so intermediate WAV/CSV files
    /// have somewhere stable to land. At save time the user-facing name wins:
    /// we rename that folder to `<base>/<safe_save_name>` and rewrite cached
    /// absolute paths so CTC/raw-sweep export reads from the moved files.
    fn ensure_named_recording_directory(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let (current_dir, target_dir) = {
            let state = self.state.read(cx);
            let rec = &state.app.measurement_state.recording_state;
            (
                rec.recording_directory.clone(),
                rec.named_recording_directory(),
            )
        };

        let Some(target_dir) = target_dir else {
            return current_dir.ok_or_else(|| "No recording directory set".to_string());
        };

        let target_dir_string = target_dir.to_string_lossy().to_string();
        let Some(current_dir) = current_dir else {
            std::fs::create_dir_all(&target_dir)
                .map_err(|e| format!("failed to create '{}': {}", target_dir.display(), e))?;
            self.state.update(cx, |state, _| {
                state
                    .app
                    .measurement_state
                    .recording_state
                    .recording_directory = Some(target_dir_string.clone());
            });
            return Ok(target_dir_string);
        };

        let current_path = std::path::PathBuf::from(&current_dir);
        if current_path == target_dir {
            return Ok(target_dir_string);
        }

        if let Some(parent) = target_dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create '{}': {}", parent.display(), e))?;
        }

        if current_path.exists() {
            if target_dir.exists() {
                return Err(format!(
                    "target directory already exists: {}",
                    target_dir.display()
                ));
            }
            std::fs::rename(&current_path, &target_dir).map_err(|e| {
                format!(
                    "failed to rename '{}' to '{}': {}",
                    current_path.display(),
                    target_dir.display(),
                    e
                )
            })?;
        } else {
            std::fs::create_dir_all(&target_dir)
                .map_err(|e| format!("failed to create '{}': {}", target_dir.display(), e))?;
        }

        self.state.update(cx, |state, _| {
            let rec = &mut state.app.measurement_state.recording_state;
            rec.recording_directory = Some(target_dir_string.clone());
            for channel in &mut rec.channel_recordings {
                if let Some(result) = &mut channel.result {
                    Self::rewrite_optional_path_to_dir(&mut result.wav_path, &target_dir);
                    Self::rewrite_optional_path_to_dir(&mut result.csv_path, &target_dir);
                }
            }
            Self::rewrite_optional_path_to_dir(&mut rec.probe_capture.wav_path, &target_dir);
            Self::rewrite_optional_path_to_dir(&mut rec.bass_anchor_capture.wav_path, &target_dir);
            Self::rewrite_optional_path_to_dir(&mut rec.ctc_reference_sweep_path, &target_dir);
            for loopback in &mut rec.transfer_matrix_loopbacks {
                Self::rewrite_path_to_dir(&mut loopback.wav_path, &target_dir);
            }
        });

        Ok(target_dir_string)
    }

    /// Save recordings to a JSON file in the recording directory
    /// Outputs autoeq::RoomConfig format compatible with roomeq CLI
    pub(crate) fn save_recordings(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::RoomEqMeasurementsFile;
        use autoeq::{OptimizerConfig, RecordingConfiguration, RoomConfig};
        use sotf_audio_player::room_eq_types::{
            DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY, build_speakers_from_recordings,
            ctc_system_config_for_speaker_names, default_bass_management_crossovers,
            room_eq_channel_is_bass_output,
        };

        let recording_dir = match self.ensure_named_recording_directory(cx) {
            Ok(dir) => dir,
            Err(e) => {
                log::error!("Failed to prepare recording directory: {}", e);
                self.state.update(cx, |state, _| {
                    state.app.measurement_state.recording_state.status_message =
                        format!("Failed to prepare output directory: {}", e);
                });
                cx.notify();
                return;
            }
        };

        // Get recordings, recording directory, configuration, and convert to RoomConfig format
        let (room_config, recording_dir, ctc_raw_fallback) = {
            let state = self.state.read(cx);
            let rec_state = &state.app.measurement_state.recording_state;
            let recordings = &rec_state.channel_recordings;

            let channel_names: Vec<String> = rec_state
                .playback_config
                .channel_mappings
                .iter()
                .map(|m| m.group_name.clone())
                .collect();
            let mic_names = vec!["left_ear".to_string(), "right_ear".to_string()];
            let ctc_strategy = rec_state.recording_config.ctc_matrix_strategy;
            let mut ctc_reference_sweep = None;
            let mut ctc_raw_sweep_range = None;
            let mut ctc_raw_fallback = false;
            let mut ctc_measurements = if ctc_strategy
                == crate::app::types::CtcMatrixExportStrategy::RawSweep
            {
                ctc_reference_sweep = rec_state.ctc_reference_sweep_path.as_ref().map(|path| {
                    std::path::Path::new(path)
                        .strip_prefix(&recording_dir)
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|_| std::path::PathBuf::from(path))
                });
                match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings_with_strategy(
                    recordings,
                    &channel_names,
                    &mic_names,
                    rec_state.recording_config.sample_rate,
                    std::path::Path::new(&recording_dir),
                    ctc_strategy,
                    rec_state.recording_config.ctc_loopback_input_channel,
                    &rec_state.transfer_matrix_loopbacks,
                ) {
                    Ok(Some(measurements)) => match sotf_audio_player::room_eq_types::ctc_uniform_sweep_range_for_measurements(
                        recordings,
                        &channel_names,
                        &measurements,
                    ) {
                        Some(range) => {
                            ctc_raw_sweep_range = Some(range);
                            Some(measurements)
                        }
                        None => {
                            ctc_raw_fallback = true;
                            log::warn!(
                                "Raw-sweep CTC matrix mixes sweep ranges or references missing recordings; falling back to measured impulse-response export"
                            );
                            None
                        }
                    },
                    Ok(None) => {
                        ctc_raw_fallback = true;
                        log::warn!(
                            "Raw-sweep CTC matrix is incomplete; falling back to measured impulse-response export"
                        );
                        None
                    }
                    Err(e) => {
                        ctc_raw_fallback = true;
                        log::warn!("Could not export CTC transfer matrix: {}", e);
                        None
                    }
                }
            } else {
                None
            };
            if ctc_measurements.is_none() {
                ctc_measurements =
                    match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
                        recordings,
                        &channel_names,
                        &mic_names,
                        rec_state.recording_config.sample_rate,
                        std::path::Path::new(&recording_dir),
                    ) {
                        Ok(measurements) => measurements,
                        Err(e) => {
                            log::warn!("Could not export CTC transfer matrix: {}", e);
                            None
                        }
                    };
                ctc_reference_sweep = None;
            }

            // Build recording configuration from current state
            let recording_config = RecordingConfiguration {
                playback_device_name: Some(rec_state.playback_config.device_name.clone()),
                playback_device_id: Some(rec_state.playback_config.device_id.clone()),
                playback_sample_rate: Some(rec_state.playback_config.sample_rate),
                playback_channels: Some(rec_state.playback_config.num_channels),
                speaker_configuration: Some(
                    rec_state
                        .playback_config
                        .speaker_configuration
                        .as_str()
                        .to_string(),
                ),
                channel_names: Some(channel_names.clone()),
                recording_device_name: Some(rec_state.recording_config.device_name.clone()),
                recording_device_id: Some(rec_state.recording_config.device_id.clone()),
                recording_sample_rate: Some(rec_state.recording_config.sample_rate),
                recording_channels: Some(rec_state.recording_config.num_channels),
                mic_calibration_path: rec_state.mic_calibration_path.clone(),
                mic_calibration_paths: {
                    let paths = &rec_state.mic_calibration_paths;
                    if paths.is_empty() {
                        None
                    } else {
                        Some(paths.clone())
                    }
                },
                recording_directory: Some(recording_dir.clone()),
                signal_type: Some(rec_state.signal_type.as_str().to_string()),
                signal_duration_secs: Some(rec_state.signal_duration_secs),
                signal_level_db: Some(rec_state.signal_level_db),
                // Sweep parameters for recomputing metrics from WAV
                sweep_start_freq: Some(rec_state.sweep_start_freq),
                sweep_end_freq: Some(rec_state.sweep_end_freq),
                // Room info collected on the save step. `room_dimensions`
                // is converted to canonical metric; `setup_description`
                // and `channel_speakers` round-trip as-is. `None` when
                // the user left the field blank.
                room_dimensions: rec_state.room_dimensions_for_save(),
                setup_description: {
                    let s = rec_state.setup_description.trim();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                },
                channel_speakers: rec_state.channel_speakers_map_for_save(),
                // Tone-burst delay probe captured during the Probe
                // step. Translate the engine `ProbeDelayResults` into
                // the autoeq-local `ProbeResultsLegacy` mirror so the
                // RoomConfig JSON only depends on autoeq types.
                probe_results: rec_state.probe_capture.results.as_ref().map(|r| {
                    autoeq::roomeq::ProbeResultsLegacy {
                        channels: r
                            .channels
                            .iter()
                            .map(|c| autoeq::roomeq::ProbeChannelResultLegacy {
                                channel_name: c.channel_name.clone(),
                                channel_index: c.channel_index,
                                arrival_ms: c.arrival_ms,
                                gain_db: c.gain_db,
                                snr_db: c.snr_db,
                            })
                            .collect(),
                        sample_rate: r.sample_rate,
                        alignment_delays_ms: r.alignment_delays_ms.clone(),
                    }
                }),
                probe_wav_relative: rec_state
                    .probe_capture
                    .wav_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string()),
                // GD-Opt v2 Phase GD-1e — bass anchor results, when captured.
                bass_anchor_results: rec_state.bass_anchor_capture.results.as_ref().map(|r| {
                    autoeq::roomeq::BassAnchorResultsLegacy {
                        channels: r
                            .channels
                            .iter()
                            .map(|c| autoeq::roomeq::BassAnchorChannelResultLegacy {
                                channel_name: c.channel_name.clone(),
                                channel_index: c.channel_index,
                                bass_anchor_phase_deg: c.bass_anchor_phase_deg,
                                bass_anchor_magnitude: c.bass_anchor_magnitude,
                                bass_anchor_stability_deg: c.bass_anchor_stability_deg,
                                bass_anchor_loopback_phase_deg: c.bass_anchor_loopback_phase_deg,
                                bass_anchor_coherence: c.bass_anchor_coherence,
                            })
                            .collect(),
                        sample_rate: r.sample_rate,
                        bass_freq_hz: r.bass_freq_hz,
                        bass_duration_s: r.bass_duration_s,
                    }
                }),
                bass_anchor_wav_relative: rec_state
                    .bass_anchor_capture
                    .wav_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string()),
                // GD-Opt v2 Phase GD-1b fields — wired from RecordingState UI knobs.
                // bass_octave_duration_s of 3.0 is the default; the UI allows 1×/2×/4×
                // presets (1.5/3.0/5.0). pre_silence_s defaults to 2.0.
                // post_silence_s is None here (derived from RT60 at record time).
                bass_octave_duration_s: Some(rec_state.bass_octave_duration_s),
                pre_silence_s: Some(rec_state.pre_silence_s),
                post_silence_s: rec_state.post_silence_s,
                // Remaining GD-Opt v2 fields (later phases): leave as None for now.
                sweep_level_db_spl: None,
                num_sweeps: None,
                coherence_threshold: None,
                bass_probe_freq_hz: Some(rec_state.bass_anchor_capture.bass_freq_hz),
                bass_probe_duration_s: Some(rec_state.bass_anchor_capture.bass_duration_s),
                mic_phase_calibration_path: None,
                mic_phase_calibration_paths: None,
                // GD-Opt v2 Phase GD-1e.5 — SPL calibration, when captured
                // and the user has entered their meter reading.
                spl_calibration: rec_state.spl_calibration_capture.to_spl_calibration(),
                recording_seed: None,
                num_positions: {
                    let n = rec_state.recording_config.num_positions.max(1);
                    if n > 1 { Some(n) } else { None }
                },
            };

            // Group every completed (channel × mic × position) take by
            // channel_index so each output channel produces exactly one
            // SpeakerConfig — multi-mic / multi-position takes become a
            // MeasurementSource::Multiple that roomeq averages into one
            // EQ chain per real channel.
            let speakers = build_speakers_from_recordings(
                recordings,
                &channel_names,
                rec_state.channel_speakers_map_for_save().as_ref(),
            );

            if speakers.is_empty() {
                log::warn!("No completed recordings to save");
                return;
            }

            let ctc = ctc_measurements.map(|measurements| {
                let raw = ctc_reference_sweep.is_some();
                autoeq::roomeq::CtcConfig {
                    // Off by default — binaural CTC is opt-in, the
                    // CTC stanza is written so the user can flip it
                    // on later without re-recording, but roomeq must
                    // not run the CTC solver until they do.
                    enabled: false,
                    matrix_source: if raw { "raw_sweep" } else { "measured" }.to_string(),
                    measurements: Some(measurements),
                    reference_sweep: ctc_reference_sweep,
                    sweep_duration_s: if raw {
                        Some(rec_state.signal_duration_secs as f64)
                    } else {
                        None
                    },
                    sweep_start_hz: if raw {
                        ctc_raw_sweep_range.map(|(start, _)| start as f64)
                    } else {
                        None
                    },
                    sweep_end_hz: if raw {
                        ctc_raw_sweep_range.map(|(_, end)| end as f64)
                    } else {
                        None
                    },
                    ..Default::default()
                }
            });
            // Always emit the system (logical role) map from the
            // recorded speakers, independent of CTC enable state.
            // roomeq uses it to interpret the layout (LFE/sub
            // detection, bass-management routing). Flipping CTC on
            // later does not require re-recording.
            let has_bass_output = speakers
                .keys()
                .any(|name| room_eq_channel_is_bass_output(name));
            let bass_management_crossover =
                has_bass_output.then(|| DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY.to_string());
            let system = ctc_system_config_for_speaker_names(
                speakers.keys().map(String::as_str),
                bass_management_crossover,
            );
            let crossovers = has_bass_output.then(default_bass_management_crossovers);

            // Build RoomConfig
            let room_config = RoomConfig {
                version: "1.1.0".to_string(),
                system,
                speakers,
                crossovers,
                target_curve: None,
                optimizer: OptimizerConfig::default(),
                recording_config: Some(recording_config),
                ctc,
                cea2034_cache: None,
            };

            (room_config, recording_dir, ctc_raw_fallback)
        };

        // Save to recording directory (no dialog needed)
        let json_path = std::path::Path::new(&recording_dir).join("recordings.json");

        match serde_json::to_string_pretty(&room_config) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&json_path, json) {
                    log::error!("Failed to write recordings file: {}", e);
                    self.state.update(cx, |state, _| {
                        state.app.measurement_state.recording_state.status_message =
                            format!("Failed to save: {}", e);
                    });
                } else {
                    log::info!("Recordings saved to {:?}", json_path);
                    self.state.update(cx, |state, _| {
                        let suffix = if ctc_raw_fallback {
                            " (raw-sweep CTC incomplete; saved measured CTC)"
                        } else {
                            ""
                        };
                        state.app.measurement_state.recording_state.status_message =
                            format!("Saved to {}{}", json_path.display(), suffix);
                    });
                }
            }
            Err(e) => {
                log::error!("Failed to serialize recordings: {}", e);
                self.state.update(cx, |state, _| {
                    state.app.measurement_state.recording_state.status_message =
                        format!("Failed to serialize: {}", e);
                });
            }
        }
        cx.notify();

        log::info!("Save recordings completed");
    }

    /// Load recordings from a JSON file (autoeq RoomConfig format)
    pub(crate) fn load_recordings_from_file(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let weak_state = self.state.downgrade();

            cx.spawn(async move |_, cx| {
                // Open file dialog
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let file_path = file.path().to_path_buf();
                    let file_dir = file_path.parent().map(|p| p.to_path_buf());

                    // Get file size
                    let file_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);

                    let Some(state_entity) = weak_state.upgrade() else {
                        return;
                    };

                    let _ = file_size;
                    // Read file content — only the autoeq RoomConfig
                    // format is supported; older RoomEqMeasurementsFile
                    // files surface as a load error.
                    match std::fs::read_to_string(&file_path) {
                        Ok(json) => {
                            Self::load_recordings_internal(
                                state_entity,
                                &mut cx.clone(),
                                &json,
                                &file_path,
                                file_dir,
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to read recordings file: {}", e);
                            state_entity.update(&mut cx.clone(), |state, _| {
                                state.app.measurement_state.recording_state.status_message =
                                    format!("Failed to read: {}", e);
                            });
                        }
                    }
                }
            })
            .detach();

            log::info!("Load recordings from file initiated");
        }
    }

    /// Internal function to load recordings from parsed JSON
    /// Supports both new RoomConfig format and legacy RoomEqMeasurementsFile format
    fn load_recordings_internal(
        state_entity: Entity<crate::app::state::AppState>,
        cx: &mut gpui::AsyncApp,
        json: &str,
        file_path: &std::path::Path,
        file_dir: Option<std::path::PathBuf>,
    ) {
        use crate::app::types::{ChannelRecording, ChannelRecordingState, RecordingResult};

        // Try to parse as new RoomConfig format first
        if let Ok(room_config) = serde_json::from_str::<autoeq::RoomConfig>(json) {
            log::info!(
                "Loaded {} speakers from {:?} (RoomConfig format)",
                room_config.speakers.len(),
                file_path
            );

            let file_path_display = file_path.display().to_string();
            state_entity.update(cx, |state, _| {
                // Convert speakers to ChannelRecordings
                let recordings: Vec<ChannelRecording> = room_config
                    .speakers
                    .into_iter()
                    .enumerate()
                    .filter_map(|(idx, (channel_name, speaker_config))| {
                        // Extract inline measurement from speaker config
                        let inline = match speaker_config {
                            autoeq::SpeakerConfig::Single(source) => match source {
                                autoeq::MeasurementSource::Single(s) => {
                                    s.measurement.inline_data().cloned()
                                }
                                autoeq::MeasurementSource::Multiple(m) => m
                                    .measurements
                                    .first()
                                    .and_then(|r| r.inline_data())
                                    .cloned(),
                                autoeq::MeasurementSource::InMemory(_)
                                | autoeq::MeasurementSource::InMemoryMultiple(_) => None,
                            },
                            _ => None, // Groups not yet supported in this conversion
                        };

                        inline.map(|inline_data| {
                            // Convert absolute paths from relative
                            let wav_path = inline_data.wav_path.as_ref().and_then(|wav| {
                                file_dir.as_ref().map(|dir| {
                                    let abs_path = dir.join(wav);
                                    if abs_path.exists() {
                                        abs_path.to_string_lossy().to_string()
                                    } else {
                                        wav.clone()
                                    }
                                })
                            });
                            let csv_path = inline_data.csv_path.as_ref().and_then(|csv| {
                                file_dir.as_ref().map(|dir| {
                                    let abs_path = dir.join(csv);
                                    if abs_path.exists() {
                                        abs_path.to_string_lossy().to_string()
                                    } else {
                                        csv.clone()
                                    }
                                })
                            });

                            // Check if inline data is empty - if so, load from CSV
                            let (frequencies, magnitude_db, phase_deg) = if inline_data
                                .frequencies
                                .is_empty()
                            {
                                // Try to load from CSV file using autoeq's reader
                                if let Some(ref csv) = csv_path {
                                    let csv_full_path = std::path::PathBuf::from(csv);
                                    if let Ok(curve) =
                                        autoeq::read::read_curve_from_csv(&csv_full_path)
                                    {
                                        log::info!(
                                            "Loaded {} frequency points from CSV for channel '{}'",
                                            curve.freq.len(),
                                            channel_name
                                        );
                                        (
                                            curve.freq.iter().map(|&f| f as f32).collect(),
                                            curve.spl.iter().map(|&s| s as f32).collect(),
                                            curve
                                                .phase
                                                .map(|p| p.iter().map(|&v| v as f32).collect())
                                                .unwrap_or_default(),
                                        )
                                    } else {
                                        log::warn!(
                                            "Failed to load CSV for channel '{}': {:?}",
                                            channel_name,
                                            csv_full_path
                                        );
                                        (Vec::new(), Vec::new(), Vec::new())
                                    }
                                } else {
                                    log::warn!(
                                        "No CSV path and empty inline data for channel '{}'",
                                        channel_name
                                    );
                                    (Vec::new(), Vec::new(), Vec::new())
                                }
                            } else {
                                // Use inline data
                                (
                                    inline_data.frequencies.iter().map(|&f| f as f32).collect(),
                                    inline_data.magnitude_db.iter().map(|&m| m as f32).collect(),
                                    inline_data
                                        .phase_deg
                                        .clone()
                                        .unwrap_or_default()
                                        .iter()
                                        .map(|&p| p as f32)
                                        .collect(),
                                )
                            };

                            // Try to load extended metrics from CSV file
                            let extended_metrics =
                                crate::components::migration::load_extended_metrics(
                                    csv_path.as_deref(),
                                    file_dir.as_deref(),
                                );

                            let (
                                thd_percent,
                                rt60_ms,
                                clarity_c50_db,
                                clarity_c80_db,
                                excess_group_delay_ms,
                            ) = if let Some(metrics) = extended_metrics {
                                log::info!(
                                    "Loaded extended metrics for channel '{}' from CSV",
                                    channel_name
                                );
                                (
                                    metrics.thd_percent,
                                    metrics.rt60_ms,
                                    metrics.clarity_c50_db,
                                    metrics.clarity_c80_db,
                                    metrics.excess_group_delay_ms,
                                )
                            } else {
                                (None, None, None, None, None)
                            };

                            let result = RecordingResult {
                                channel: idx,
                                wav_path,
                                csv_path,
                                frequencies,
                                magnitude_db,
                                phase_deg,
                                impulse_response: None,
                                impulse_time_ms: None,
                                excess_group_delay_ms,
                                thd_percent,
                                harmonic_distortion_db: None,
                                rt60_ms,
                                clarity_c50_db,
                                clarity_c80_db,
                                spectrogram_db: None,
                            };

                            let mut rec = ChannelRecording::new(idx, channel_name);
                            rec.state = ChannelRecordingState::Done;
                            rec.result = Some(result);
                            rec
                        })
                    })
                    .collect();

                // Filter out channels with empty frequency data (can happen with
                // older RoomConfig versions where CSV paths are unresolvable)
                let recordings: Vec<ChannelRecording> = recordings
                    .into_iter()
                    .filter(|r| {
                        r.result
                            .as_ref()
                            .is_some_and(|res| !res.frequencies.is_empty())
                    })
                    .collect();

                let rec_state = &mut state.app.measurement_state.recording_state;
                rec_state.channel_recordings = recordings.clone();

                // Also set the recording directory to the file's directory
                if let Some(dir) = &file_dir {
                    rec_state.recording_directory = Some(dir.to_string_lossy().to_string());
                }

                rec_state.status_message = format!(
                    "Loaded {} channels from {}",
                    recordings.len(),
                    file_path_display
                );
            });
            return;
        }

        // Not a RoomConfig — there is no legacy fallback any more.
        log::error!(
            "{} is not in the autoeq RoomConfig format (no \"speakers\" map)",
            file_path.display()
        );
        state_entity.update(cx, |state, _| {
            state.app.measurement_state.recording_state.status_message = format!(
                "{} is not in the current RoomConfig format — re-run the Recording wizard to regenerate it.",
                file_path.display()
            );
        });
    }

    /// Render the migration confirmation modal
    /// Using a simple manual modal instead of Dialog component to debug click issues
    pub(crate) fn render_migration_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let rec_state = &state.app.measurement_state.recording_state;

        let file_path = rec_state.migration_file_path.clone().unwrap_or_default();
        let file_size_mb = rec_state.migration_file_size.unwrap_or(0) as f64 / 1_000_000.0;
        let channel_count = rec_state.migration_channel_count;

        // Extract just the filename for display
        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.clone());

        let view = cx.entity().clone();
        let view2 = view.clone();

        // Simple modal implementation (not using Dialog component)
        // Use a backdrop that only closes on direct clicks (not on child elements)
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                // Backdrop layer - clickable to close
                div()
                    .id("migration-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(theme.overlay_bg)
                    .on_click({
                        let view = view.clone();
                        move |_event, _window, cx| {
                            log::info!("Backdrop clicked - closing modal");
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let rec_state =
                                        &mut state.app.measurement_state.recording_state;
                                    rec_state.migration_modal_open = false;
                                    rec_state.migration_pending_json = None;
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                // Modal container - positioned above backdrop
                div()
                    .id("migration-modal-container")
                    .relative()
                    .w(px(480.0)) // intentional: fixed modal dialog width
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.accent)
                    .rounded(d.r_lg)
                    .shadow_lg()
                    .overflow_hidden()
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(d.card)
                            .py(d.pad_x)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(d.text_lg)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child(translations.recording_convert_format_title),
                            ),
                    )
                    // Content
                    .child(
                        div().px(d.card).py(d.card).child(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new(translations.recording_older_format)
                                        .size(TextSize::Sm)
                                        .color(theme.text_primary),
                                )
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            HStack::new()
                                                .spacing(StackSpacing::Sm)
                                                .child(
                                                    Text::new(translations.recording_file_label)
                                                        .size(TextSize::Xs)
                                                        .color(theme.text_secondary),
                                                )
                                                .child(
                                                    Text::new(file_name)
                                                        .size(TextSize::Xs)
                                                        .color(theme.text_primary),
                                                ),
                                        )
                                        .child(
                                            HStack::new()
                                                .spacing(StackSpacing::Sm)
                                                .child(
                                                    Text::new(translations.recording_size_label)
                                                        .size(TextSize::Xs)
                                                        .color(theme.text_secondary),
                                                )
                                                .child(
                                                    Text::new(format!("{:.2} MB", file_size_mb))
                                                        .size(TextSize::Xs)
                                                        .color(theme.warning),
                                                ),
                                        )
                                        .child(
                                            HStack::new()
                                                .spacing(StackSpacing::Sm)
                                                .child(
                                                    Text::new(
                                                        translations.recording_channels_label,
                                                    )
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                                )
                                                .child(
                                                    Text::new(format!("{}", channel_count))
                                                        .size(TextSize::Xs)
                                                        .color(theme.text_primary),
                                                ),
                                        ),
                                ),
                        ),
                    )
                    // Footer with buttons
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(d.gap_md)
                            .px(d.card)
                            .py(d.pad_x)
                            .border_t_1()
                            .border_color(theme.border)
                            // Cancel button - simple div
                            .child(
                                div()
                                    .id("migration-cancel-btn")
                                    .px(d.pad_x)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.surface_hover)
                                    .text_color(theme.text_secondary)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .child(translations.general_cancel)
                                    .on_click({
                                        let view = view.clone();
                                        move |_event, _window, cx| {
                                            log::info!("Cancel button clicked!");
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    let rec_state = &mut state
                                                        .app
                                                        .measurement_state
                                                        .recording_state;
                                                    rec_state.migration_modal_open = false;
                                                    rec_state.migration_pending_json = None;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            // Convert button - simple div
                            .child(
                                div()
                                    .id("migration-convert-btn")
                                    .px(d.pad_x)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.accent)
                                    .text_color(theme.text_on_accent)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child(translations.recording_convert_button)
                                    .on_click({
                                        move |_event, _window, cx| {
                                            log::info!("Convert button clicked!");
                                            view2.update(cx, |this, cx| {
                                                this.perform_migration(cx);
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
    }

    /// Perform the migration from legacy format to new format
    ///
    /// This will:
    /// 1. Back up the original JSON file with a `.bak` extension
    /// 2. Write updated CSV files with full data from the inline JSON
    /// 3. Write a new lightweight JSON file to the original location
    fn perform_migration(&mut self, cx: &mut Context<Self>) {
        // Legacy `RoomEqMeasurementsFile` migration is no longer
        // supported — the only on-disk format is `autoeq::RoomConfig`.
        // The modal trigger has been disabled, but in case the modal
        // surfaces from stale state, dismiss it cleanly with a message
        // instead of touching the legacy parser.
        self.state.update(cx, |state, _| {
            let rec_state = &mut state.app.measurement_state.recording_state;
            rec_state.migration_modal_open = false;
            rec_state.migration_pending_json = None;
            rec_state.status_message =
                "Legacy file migration is no longer supported. Re-record to regenerate the file."
                    .to_string();
        });
        cx.notify();
    }

    /// Render the "move microphones to next position" modal. Same
    /// manual-modal pattern as `render_migration_modal` (the comment
    /// there explains why we don't use the `Dialog` component here).
    pub(crate) fn render_move_position_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let rec_state = &state.app.measurement_state.recording_state;

        // `pending_next_position` is the index of the position the user
        // just *finished*; the next pass is `pending + 1` (one-based).
        let just_finished = rec_state.pending_next_position.unwrap_or(0);
        let next_pos_one_based = just_finished + 2;
        let total_positions = rec_state.recording_config.num_positions.max(1);

        let view = cx.entity().clone();
        let view_cancel = view.clone();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("move-position-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(theme.overlay_bg),
            )
            .child(
                div()
                    .id("move-position-modal-container")
                    .relative()
                    .w(px(480.0)) // intentional: fixed modal dialog width
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.accent)
                    .rounded(d.r_lg)
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(d.card)
                            .py(d.pad_x)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(d.text_lg)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child(format!(
                                        "Move microphones to position {} of {}",
                                        next_pos_one_based, total_positions
                                    )),
                            ),
                    )
                    .child(
                        div().px(d.card).py(d.card).child(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new(format!(
                                        "Reposition every configured microphone to seat {}, then click Continue. Click Cancel to stop the session and save what you have so far.",
                                        next_pos_one_based
                                    ))
                                    .size(TextSize::Sm)
                                    .color(theme.text_primary),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(d.gap_md)
                            .px(d.card)
                            .py(d.pad_x)
                            .border_t_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .id("move-position-cancel-btn")
                                    .px(d.pad_x)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.surface_hover)
                                    .text_color(theme.text_secondary)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .child("Cancel session")
                                    .on_click(move |_event, _window, cx| {
                                        view_cancel.update(cx, |this, cx| {
                                            this.cancel_position_modal(cx);
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .id("move-position-continue-btn")
                                    .px(d.pad_x)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.accent)
                                    .text_color(theme.text_on_accent)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child("Continue")
                                    .on_click(move |_event, _window, cx| {
                                        view.update(cx, |this, cx| {
                                            this.continue_position_modal(cx);
                                        });
                                    }),
                            ),
                    ),
            )
    }

    /// Continue button handler for the move-position modal. Closes the
    /// modal and resumes auto-record at the next position.
    fn continue_position_modal(&mut self, cx: &mut Context<Self>) {
        let next_idx_opt = self.state.update(cx, |state, _| {
            let rec_state = &mut state.app.measurement_state.recording_state;
            rec_state.move_position_modal_open = false;
            let just_finished = rec_state.pending_next_position.take().unwrap_or(0);
            let next_pos = just_finished + 1;
            let num_positions = rec_state.recording_config.num_positions.max(1);
            if next_pos >= num_positions {
                // Defensive: shouldn't happen — modal only opens when
                // there's another position. Treat as completion.
                rec_state.auto_record_remaining = false;
                None
            } else {
                rec_state.status_message = format!("Recording position {}...", next_pos + 1);
                rec_state.next_channel_in_position(next_pos)
            }
        });
        if let Some(next_idx) = next_idx_opt {
            self.start_recording_channel(next_idx, cx);
        } else {
            // No more entries to record — auto-save and finish.
            self.save_recordings(cx);
        }
        cx.notify();
    }

    /// Cancel button handler for the move-position modal. Stops the
    /// auto-record session in place; the user can save partial results
    /// from the Save step.
    fn cancel_position_modal(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let rec_state = &mut state.app.measurement_state.recording_state;
            rec_state.move_position_modal_open = false;
            rec_state.pending_next_position = None;
            rec_state.auto_record_remaining = false;
            rec_state.status_message = "Recording session cancelled".to_string();
        });
        cx.notify();
    }
}
