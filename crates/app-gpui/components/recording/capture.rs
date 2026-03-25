//! Recording Capture Step (Step 2)
//!
//! Run test signals per channel, display state, and plot results.

use crate::app::types::{ChannelRecordingState, RecordingResult, RecordingSignalType};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, NumberInput, NumberInputSize, Progress,
    ProgressSize, ProgressVariant, Select, SelectOption, StackAlign, StackJustify, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Render the capture step UI
    pub(crate) fn render_recording_capture_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Signal Recording")
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold),
                    )
                    .child(
                        Text::new("Test each channel individually. Signals will play sequentially with a 1-second pause between channels.")
                            .size(TextSize::Xs),
                    ),
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
        let signal_level_db = state.app.measurement_state.recording_state.signal_level_db;
        let _ = state;

        Card::new().content(
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Signal Type:")
                                .size(TextSize::Xs)
                                .weight(TextWeight::Semibold),
                        )
                        .child(self.render_signal_type_dropdown(cx)),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Duration:")
                                .size(TextSize::Xs)
                                .weight(TextWeight::Semibold),
                        )
                        .child(self.render_duration_dropdown(cx)),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(Text::new("Level:").size(TextSize::Xs))
                        .child({
                            let view = cx.entity().clone();
                            NumberInput::new("signal_level")
                                .value(signal_level_db as f64)
                                .min(-60.0)
                                .max(6.0)
                                .step(1.0)
                                .decimals(0)
                                .unit("dB")
                                .size(NumberInputSize::Xs)
                                .width(100.0)
                                .on_change(move |val, _window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .signal_level_db = val as f32;
                                        });
                                        cx.notify();
                                    });
                                })
                        }),
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
        let recording_state = &state.app.measurement_state.recording_state;
        let is_sweep = recording_state.signal_type == RecordingSignalType::Sweep;

        if !is_sweep || recording_state.channel_recordings.is_empty() {
            return Card::new()
                .content(
                    Text::new("Frequency range configuration is only available for sweep signals")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
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
                        Text::new("CHANNEL FREQUENCY RANGE")
                            .size(TextSize::Xs)
                            .weight(TextWeight::Bold)
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
                                    .child(
                                        div().w(px(100.0)).child(
                                            Text::new(format!("{}:", name))
                                                .size(TextSize::Xs)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.text_primary),
                                        ),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new("Start:")
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
                                                Text::new("End:")
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
                        Text::new("CHANNEL METRICS")
                            .size(TextSize::Xs)
                            .weight(TextWeight::Bold)
                            .color(theme.accent),
                    )
                    // Header row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                div().w(px(100.0)).child(
                                    Text::new("Channel")
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(60.0)).child(
                                    Text::new("Mic In")
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    Text::new("Avg SPL")
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    Text::new("Noise Floor")
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
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
                                    Text::new(format!("{}:", name))
                                        .size(TextSize::Xs)
                                        .color(theme.text_primary),
                                ),
                            )
                            .child(
                                div().w(px(60.0)).child(
                                    Text::new(format!("Ch {}", mic_ch + 1))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    Text::new(format!("{:.1} dB", avg_spl))
                                        .size(TextSize::Xs)
                                        .color(spl_color),
                                ),
                            )
                            .child(
                                div().w(px(80.0)).child(
                                    Text::new(format!("{:.1} dB", noise_floor))
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                ),
                            )
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    /// Render channel status section with recording controls
    fn render_channel_status_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
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
                            Text::new("CHANNEL STATUS")
                                .size(TextSize::Xs)
                                .weight(TextWeight::Bold)
                                .color(theme.accent),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .when(!is_recording, |stack| {
                                    let view = view.clone();
                                    let theme = theme.clone();
                                    stack.child(
                                        Button::new("record_all", "Record All Channels")
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
                                    stack.child(
                                        Button::new("stop_recording", "Stop Recording")
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
                    // Use a semi-transparent amber/warning background (HSL: ~45deg hue for amber)
                    let warning_bg = gpui::hsla(0.125, 0.8, 0.5, 0.15);
                    stack.child(
                        div()
                            .p_3()
                            .rounded_md()
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
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
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
                    div().p_4().rounded_md().bg(theme.surface).child(
                        Text::new(
                            "No channels configured. Please go back and configure your devices.",
                        )
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                    ),
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

                    let (state_icon, state_text, state_color) = if any_recording {
                        ("●", "Recording...", theme.warning)
                    } else if all_done {
                        ("✓", "Complete", theme.success)
                    } else if any_error {
                        ("✗", "Error", theme.error)
                    } else {
                        ("○", "Not recorded", theme.text_muted)
                    };

                    let button_label = if all_done { "Re-record" } else { "Record" };

                    let mut row = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .rounded_md()
                        .bg(theme.surface)
                        .child(
                            // Main speaker row
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div().w(px(100.0)).child(
                                        Text::new(format!("{}:", speaker_name))
                                            .size(TextSize::Xs)
                                            .weight(TextWeight::Semibold)
                                            .color(theme.text_primary),
                                    ),
                                )
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new(state_icon)
                                                .size(TextSize::Xs)
                                                .color(state_color),
                                        )
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
                                        let (icon, color) = match mic_state {
                                            ChannelRecordingState::Empty => ("○", theme.text_muted),
                                            ChannelRecordingState::Recording => {
                                                ("●", theme.warning)
                                            }
                                            ChannelRecordingState::Done => ("✓", theme.success),
                                            ChannelRecordingState::Error => ("✗", theme.error),
                                        };
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new(format!("Mic {}:", mic_idx + 1))
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            )
                                            .child(Text::new(icon).size(TextSize::Xs).color(color))
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
                Button::new("redo_recordings", "Redo All")
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
                Button::new("load_from_file", "Load from File")
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
            SignalParams, SignalType, generate_signal, write_temp_wav,
        };

        // --- Gather parameters from state ---
        #[derive(Clone)]
        struct MicInfo {
            vec_idx: usize,
            #[allow(dead_code)]
            mic_index: usize,
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
            let sweep_start = channel.sweep_start_freq;
            let sweep_end = channel.sweep_end_freq;

            // Collect all mic entries for this speaker
            let mics: Vec<MicInfo> = rec_state
                .channel_recordings
                .iter()
                .enumerate()
                .filter(|(_, r)| r.channel_index == speaker_idx)
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
                        vec_idx: vi,
                        mic_index: r.mic_index,
                        input_channel: input_ch,
                        calibration,
                        safe_name,
                    }
                })
                .collect();

            let signal_type = match rec_state.signal_type {
                RecordingSignalType::Sweep => SignalType::Sweep,
                RecordingSignalType::WhiteNoise => SignalType::WhiteNoise,
                RecordingSignalType::PinkNoise => SignalType::PinkNoise,
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
            log::error!("Failed to create recording directory {:?}: {}", recording_dir, e);
            self.state.update(cx, |state, _| {
                state.app.measurement_state.recording_state.status_message =
                    format!("Cannot create recording directory: {}", e);
            });
            cx.notify();
            return;
        }

        // Mark all mic entries for this speaker as Recording and clear old results
        // so the evaluating graphs reset immediately (not showing stale data).
        let mic_vec_indices: Vec<usize> = params.mics.iter().map(|m| m.vec_idx).collect();
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
                .map(|m| {
                    self.state
                        .read(cx)
                        .app
                        .measurement_state
                        .recording_state
                        .channel_recordings
                        .get(m.vec_idx)
                        .map(|r| r.channel_index)
                        .unwrap_or(999)
                })
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

        let state_entity = self.state.clone();
        let view_entity = cx.entity().clone();
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
            let (should_auto_continue, next_channel_idx) =
                state_entity.update(&mut cx.clone(), |state, _| {
                    let should_continue = match results {
                        Ok(analysis_results) => {
                            const NOISE_FLOOR_THRESHOLD_DB: f32 = -50.0;
                            let mut any_low_signal = false;

                            for (mic_i, analysis_result) in
                                analysis_results.into_iter().enumerate()
                            {
                                let vi = mics[mic_i].vec_idx;
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

                                if let Some(recording) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_recordings
                                    .get_mut(vi)
                                {
                                    recording.state = ChannelRecordingState::Done;
                                    recording.result = Some(RecordingResult {
                                        channel: vi,
                                        wav_path: Some(
                                            wav_paths[mic_i].to_string_lossy().to_string(),
                                        ),
                                        csv_path: Some(
                                            csv_paths[mic_i].to_string_lossy().to_string(),
                                        ),
                                        frequencies: analysis_result.frequencies,
                                        magnitude_db: analysis_result.spl_db,
                                        phase_deg: analysis_result.phase_deg,
                                        impulse_response: Some(
                                            analysis_result.impulse_response,
                                        ),
                                        impulse_time_ms: Some(
                                            analysis_result.impulse_time_ms,
                                        ),
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
                                        spectrogram_db: Some(
                                            analysis_result.spectrogram_db,
                                        ),
                                    });
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
                                if let Some(recording) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .channel_recordings
                                    .get_mut(mic.vec_idx)
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

                    // Find next speaker to record: first entry with Empty state
                    // that belongs to a speaker not yet recorded.
                    // We find the first Empty entry and use it as the start of the
                    // next speaker group.
                    let next_channel_idx = if should_continue {
                        let mut seen = std::collections::HashSet::new();
                        state
                            .app
                            .measurement_state
                            .recording_state
                            .channel_recordings
                            .iter()
                            .enumerate()
                            .find(|(_, r)| {
                                r.state == ChannelRecordingState::Empty
                                    && seen.insert(r.channel_index)
                            })
                            .map(|(idx, _)| idx)
                    } else {
                        None
                    };

                    (should_continue, next_channel_idx)
                });

            if should_auto_continue {
                if let Some(next_idx) = next_channel_idx {
                    log::info!("Auto-recording: starting next speaker at idx {}", next_idx);
                    view_entity.update(cx, |view, cx| {
                        view.start_recording_channel(next_idx, cx);
                    });
                } else {
                    log::info!("Auto-recording complete - all channels recorded, saving JSON");
                    view_entity.update(cx, |view, cx| {
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

    /// Save recordings to a JSON file in the recording directory
    /// Outputs autoeq::RoomConfig format compatible with roomeq CLI
    pub(crate) fn save_recordings(&mut self, cx: &mut Context<Self>) {
        use autoeq::{
            InlineMeasurement, MeasurementRef, MeasurementSource, OptimizerConfig,
            RecordingConfiguration, RoomConfig, SpeakerConfig,
        };
        use std::collections::HashMap;

        // Get recordings, recording directory, configuration, and convert to RoomConfig format
        let (room_config, recording_dir) = {
            let state = self.state.read(cx);
            let rec_state = &state.app.measurement_state.recording_state;
            let recordings = &rec_state.channel_recordings;
            let recording_dir = rec_state.recording_directory.clone();

            // Check if recording directory is set
            let recording_dir = match recording_dir {
                Some(dir) => dir,
                None => {
                    log::error!("No recording directory set");
                    return;
                }
            };

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
                channel_names: Some(
                    rec_state
                        .playback_config
                        .channel_mappings
                        .iter()
                        .map(|m| m.group_name.clone())
                        .collect(),
                ),
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
            };

            // Convert ChannelRecording to speakers HashMap with inline measurements
            let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();

            for rec in recordings.iter() {
                if let Some(result) = &rec.result {
                    // Convert absolute paths to relative (just filename)
                    let relative_wav = result
                        .wav_path
                        .as_ref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .map(|f| f.to_string_lossy().to_string());
                    let relative_csv = result
                        .csv_path
                        .as_ref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .map(|f| f.to_string_lossy().to_string());

                    // Store only file references, not inline data
                    // Data will be loaded from CSV on demand
                    let inline_measurement = InlineMeasurement {
                        frequencies: Vec::new(),
                        magnitude_db: Vec::new(),
                        phase_deg: None,
                        name: Some(rec.channel_name.clone()),
                        wav_path: relative_wav,
                        csv_path: relative_csv,
                    };

                    let measurement_ref = MeasurementRef::Inline(inline_measurement);
                    let measurement_source =
                        MeasurementSource::Single(autoeq::read::MeasurementSingle {
                            measurement: measurement_ref,
                            speaker_name: None,
                        });
                    let speaker_config = SpeakerConfig::Single(measurement_source);

                    speakers.insert(rec.channel_name.clone(), speaker_config);
                }
            }

            if speakers.is_empty() {
                log::warn!("No completed recordings to save");
                return;
            }

            // Build RoomConfig
            let room_config = RoomConfig {
                version: "1.1.0".to_string(),
                system: None,
                speakers,
                crossovers: None,
                target_curve: None,
                optimizer: OptimizerConfig::default(),
                recording_config: Some(recording_config),
                cea2034_cache: None,
            };

            (room_config, recording_dir)
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
                        state.app.measurement_state.recording_state.status_message =
                            format!("Saved to {}", json_path.display());
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

    /// Load recordings from a JSON file (RoomEqMeasurementsFile format)
    ///
    /// Detects legacy format (large inline data) and prompts for migration.
    pub(crate) fn load_recordings_from_file(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            use crate::app::types::RoomEqMeasurementsFile;

            let state_entity = self.state.clone();

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
                    let file_size =
                        std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);

                    // Read file content
                    match std::fs::read_to_string(&file_path) {
                        Ok(json) => {
                            // Check if this is a legacy format (large file with inline data)
                            let needs_migration =
                                Self::check_needs_migration(&json, file_size);

                            if needs_migration {
                                // Show migration modal instead of loading directly
                                log::info!(
                                    "Detected legacy format ({:.2} MB), showing migration modal",
                                    file_size as f64 / 1_000_000.0
                                );

                                // Count channels for display
                                let channel_count =
                                    RoomEqMeasurementsFile::from_json_str(&json)
                                        .map(|m| m.channels.len())
                                        .unwrap_or(0);

                                state_entity.update(&mut cx.clone(), |state, _| {
                                    let rec_state =
                                        &mut state.app.measurement_state.recording_state;
                                    rec_state.migration_modal_open = true;
                                    rec_state.migration_file_path =
                                        Some(file_path.to_string_lossy().to_string());
                                    rec_state.migration_file_dir =
                                        file_dir.map(|d| d.to_string_lossy().to_string());
                                    rec_state.migration_file_size = Some(file_size);
                                    rec_state.migration_channel_count = channel_count;
                                    rec_state.migration_pending_json = Some(json);
                                });
                            } else {
                                // Load normally for new format or small files
                                Self::load_recordings_internal(
                                    state_entity,
                                    &mut cx.clone(),
                                    &json,
                                    &file_path,
                                    file_dir,
                                );
                            }
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

    /// Check if a JSON file needs migration (legacy format with large inline data)
    fn check_needs_migration(json: &str, file_size: u64) -> bool {
        crate::components::migration::check_needs_migration(json, file_size)
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

        // Fall back to legacy RoomEqMeasurementsFile format
        use crate::app::types::RoomEqMeasurementsFile;
        match RoomEqMeasurementsFile::from_json_str(json) {
            Ok(measurements_file) => {
                log::info!(
                    "Loaded {} channel measurements from {:?} (legacy format)",
                    measurements_file.channels.len(),
                    file_path
                );

                let file_path_display = file_path.display().to_string();
                state_entity.update(cx, |state, _| {
                    // Convert ChannelMeasurement to ChannelRecording
                    let recordings: Vec<ChannelRecording> = measurements_file
                        .channels
                        .into_iter()
                        .enumerate()
                        .map(|(idx, cm)| {
                            // Convert relative paths in result to absolute paths
                            let mut result = cm.measurement;
                            if let (Some(dir), Some(wav)) = (&file_dir, &result.wav_path) {
                                let abs_path = dir.join(wav);
                                if abs_path.exists() {
                                    result.wav_path = Some(abs_path.to_string_lossy().to_string());
                                }
                            }
                            if let (Some(dir), Some(csv)) = (&file_dir, &result.csv_path) {
                                let abs_path = dir.join(csv);
                                if abs_path.exists() {
                                    result.csv_path = Some(abs_path.to_string_lossy().to_string());
                                }
                            }

                            let mut rec = ChannelRecording::new(idx, cm.channel_name);
                            rec.state = ChannelRecordingState::Done;
                            rec.result = Some(result);
                            rec
                        })
                        .collect();

                    let rec_state = &mut state.app.measurement_state.recording_state;
                    rec_state.channel_recordings = recordings.clone();

                    // Also set the recording directory to the file's directory
                    if let Some(dir) = file_dir {
                        rec_state.recording_directory = Some(dir.to_string_lossy().to_string());
                    }

                    rec_state.status_message = format!(
                        "Loaded {} channels from {}",
                        recordings.len(),
                        file_path_display
                    );
                });
            }
            Err(e) => {
                log::error!("Failed to parse recordings JSON: {}", e);
                state_entity.update(cx, |state, _| {
                    state.app.measurement_state.recording_state.status_message =
                        format!("Failed to parse: {}", e);
                });
            }
        }
    }

    /// Render the migration confirmation modal
    /// Using a simple manual modal instead of Dialog component to debug click issues
    pub(crate) fn render_migration_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
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
                    .w(px(480.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.accent)
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child("Convert Recording Format"),
                            ),
                    )
                    // Content
                    .child(
                        div().px_4().py_4().child(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new("This recording file uses an older format.")
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
                                                    Text::new("File:")
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
                                                    Text::new("Size:")
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
                                                    Text::new("Channels:")
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
                            .gap_3()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(theme.border)
                            // Cancel button - simple div
                            .child(
                                div()
                                    .id("migration-cancel-btn")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .text_color(theme.text_secondary)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .child("Cancel")
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
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(theme.text_on_accent)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child("Convert")
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
        use crate::app::types::{ChannelRecording, ChannelRecordingState};
        use crate::components::migration;

        log::info!("perform_migration: STARTED");

        let state = self.state.read(cx);
        let rec_state = &state.app.measurement_state.recording_state;

        log::info!(
            "perform_migration: modal_open={}, has_pending_json={}, file_path={:?}",
            rec_state.migration_modal_open,
            rec_state.migration_pending_json.is_some(),
            rec_state.migration_file_path
        );

        let json = match &rec_state.migration_pending_json {
            Some(j) => j.clone(),
            None => {
                log::error!("No pending JSON for migration");
                return;
            }
        };

        let file_path = rec_state.migration_file_path.clone().unwrap_or_default();
        let file_dir = rec_state
            .migration_file_dir
            .clone()
            .map(std::path::PathBuf::from);

        // Release the borrow on state before proceeding with migration
        let _ = state;

        let original_path = std::path::PathBuf::from(&file_path);
        let session_dir = file_dir.clone().unwrap_or_else(|| {
            original_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        });

        // Use shared migration module for file operations
        match migration::perform_migration(&json, &original_path, &session_dir) {
            Ok(result) => {
                log::info!(
                    "Migration complete: {} channels, backup at {:?}",
                    result.channel_count,
                    result.backup_path
                );

                // Re-parse the measurements to build ChannelRecording objects
                use crate::app::types::RoomEqMeasurementsFile;
                match RoomEqMeasurementsFile::from_json_str(&json) {
                    Ok(measurements_file) => {
                        // Create recordings directly from the already-parsed measurements_file
                        let recordings: Vec<ChannelRecording> = measurements_file
                            .channels
                            .iter()
                            .enumerate()
                            .map(|(idx, ch)| {
                                let safe_channel_name =
                                    migration::sanitize_filename(&ch.channel_name);

                                // Convert relative paths to absolute paths
                                let mut result = ch.measurement.clone();
                                result.csv_path = Some(
                                    session_dir
                                        .join(format!("{}.csv", safe_channel_name))
                                        .to_string_lossy()
                                        .to_string(),
                                );
                                if let Some(wav) = &result.wav_path {
                                    let abs_path = session_dir.join(wav);
                                    if abs_path.exists() {
                                        result.wav_path =
                                            Some(abs_path.to_string_lossy().to_string());
                                    }
                                }

                                let mut rec = ChannelRecording::new(idx, ch.channel_name.clone());
                                rec.state = ChannelRecordingState::Done;
                                rec.result = Some(result);
                                rec
                            })
                            .collect();

                        let num_channels = recordings.len();

                        // Update state with the recordings and close modal
                        self.state.update(cx, |state, _| {
                            let rec_state = &mut state.app.measurement_state.recording_state;
                            rec_state.channel_recordings = recordings;
                            rec_state.migration_modal_open = false;
                            rec_state.migration_pending_json = None;
                            rec_state.status_message = format!(
                                "Converted {} channels. Original backed up to .bak",
                                num_channels
                            );
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to re-parse JSON after migration: {}", e);
                        self.state.update(cx, |state, _| {
                            let rec_state = &mut state.app.measurement_state.recording_state;
                            rec_state.migration_modal_open = false;
                            rec_state.migration_pending_json = None;
                            rec_state.status_message =
                                format!("Migration files written but failed to load: {}", e);
                        });
                    }
                }
            }
            Err(e) => {
                log::error!("Migration failed: {}", e);
                self.state.update(cx, |state, _| {
                    let rec_state = &mut state.app.measurement_state.recording_state;
                    rec_state.migration_modal_open = false;
                    rec_state.migration_pending_json = None;
                    rec_state.status_message = format!("Migration failed: {}", e);
                });
            }
        }

        cx.notify();
    }
}
