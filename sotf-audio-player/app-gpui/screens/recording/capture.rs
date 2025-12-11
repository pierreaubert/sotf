//! Recording Capture Step (Step 2)
//!
//! Run test signals per channel, display state, and plot results.

use crate::app::types::{ChannelRecordingState, RecordingSignalType};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, HStack, Progress, ProgressSize,
    ProgressVariant, Select, SelectOption, StackAlign, StackJustify, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    /// Render the capture step UI
    pub(crate) fn render_recording_capture_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Signal Recording")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new("Test each channel individually. Signals will play sequentially with a 1-second pause between channels.")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    ),
            )
            .child(self.render_signal_config_section(cx))
            .child(self.render_channel_status_section(cx))
            .child(self.render_frequency_response_plot(cx))
            .child(self.render_capture_actions(cx))
    }

    /// Render signal configuration section
    fn render_signal_config_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let signal_level_db = state.app.recording_state.signal_level_db;
        drop(state);

        Card::new().content(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Center)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Signal Type:")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold)
                                .color(theme.text_secondary),
                        )
                        .child(self.render_signal_type_dropdown(cx)),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Duration:")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold)
                                .color(theme.text_secondary),
                        )
                        .child(self.render_duration_dropdown(cx)),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Level:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Badge::new(format!("{:.0} dB", signal_level_db))
                                .variant(BadgeVariant::Info),
                        ),
                ),
        )
    }

    /// Render signal type dropdown
    fn render_signal_type_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = RecordingSignalType::all()
            .iter()
            .map(|t| SelectOption::new(t.as_str(), t.as_str()))
            .collect();

        Select::new("signal_type")
            .options(options)
            .selected(recording_state.signal_type.as_str())
            .is_open(recording_state.signal_type_dropdown_open)
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.signal_type_dropdown_open = is_open;
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
                            state.app.recording_state.signal_type = match value.as_ref() {
                                "Sweep" => RecordingSignalType::Sweep,
                                "White Noise" => RecordingSignalType::WhiteNoise,
                                "Pink Noise" => RecordingSignalType::PinkNoise,
                                _ => RecordingSignalType::Sweep,
                            };
                            state.app.recording_state.signal_type_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render duration dropdown
    fn render_duration_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
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
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.duration_dropdown_open = is_open;
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
                                state.app.recording_state.signal_duration_secs = duration;
                            }
                            state.app.recording_state.duration_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render channel status section with recording controls
    fn render_channel_status_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let is_recording = state.app.recording_state.is_recording();
        let status_message = state.app.recording_state.status_message.clone();
        let recording_progress = state.app.recording_state.recording_progress;
        drop(state);

        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .justify(StackJustify::SpaceBetween)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("CHANNEL STATUS")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Bold)
                                .color(theme.accent),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .when(!is_recording, |stack| {
                                    let view = view.clone();
                                    stack.child(
                                        Button::new("record_all", "Record All Channels")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Md)
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
                                    stack.child(
                                        Button::new("stop_recording", "Stop Recording")
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Md)
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
                    let theme = theme.clone();
                    stack.child(
                        Text::new(status_message.clone())
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                })
                .when(is_recording, |stack| {
                    stack.child(
                        Progress::new(recording_progress)
                            .size(ProgressSize::Md)
                            .variant(ProgressVariant::Default),
                    )
                }),
        )
    }

    /// Render the list of channels with their recording status
    fn render_channel_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();
        let is_recording = recording_state.is_recording();

        if recording_state.channel_recordings.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    div().p_4().rounded_md().bg(theme.surface).child(
                        Text::new(
                            "No channels configured. Please go back and configure your devices.",
                        )
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                    ),
                )
                .into_any_element();
        }

        VStack::new()
            .spacing(StackSpacing::Sm)
            .children(recording_state.channel_recordings.iter().enumerate().map(
                |(idx, recording)| {
                    let theme = theme.clone();
                    let view = view.clone();
                    let channel_name = recording.channel_name.clone();
                    let channel_state = recording.state;

                    let (state_icon, state_text, state_color) = match channel_state {
                        ChannelRecordingState::Empty => ("○", "Not recorded", theme.text_muted),
                        ChannelRecordingState::Recording => ("●", "Recording...", theme.warning),
                        ChannelRecordingState::Done => ("✓", "Complete", theme.success),
                        ChannelRecordingState::Error => ("✗", "Error", theme.error),
                    };

                    let button_label = if channel_state == ChannelRecordingState::Done {
                        "Re-record"
                    } else {
                        "Record"
                    };

                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_2()
                        .rounded_md()
                        .bg(theme.surface)
                        .child(
                            div().w(px(100.0)).child(
                                Text::new(format!("{}:", channel_name))
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_primary),
                            ),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(Text::new(state_icon).size(TextSize::Sm).color(state_color))
                                .child(Text::new(state_text).size(TextSize::Sm).color(state_color)),
                        )
                        .child(div().flex_1()) // Spacer
                        .child(
                            Button::new(SharedString::from(format!("record_ch_{}", idx)), button_label)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .disabled(
                                    is_recording
                                        || channel_state == ChannelRecordingState::Recording,
                                )
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.start_recording_channel(idx, cx);
                                        });
                                    }
                                }),
                        )
                        .into_any_element()
                },
            ))
            .into_any_element()
    }

    /// Render frequency response plot placeholder
    fn render_frequency_response_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;

        let has_results = recording_state
            .channel_recordings
            .iter()
            .any(|r| r.result.is_some());

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("FREQUENCY RESPONSE (20 Hz - 20 kHz)")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    // TODO: Integrate actual plotting (d3rs or similar)
                    div()
                        .h(px(300.0))
                        .w_full()
                        .rounded_md()
                        .bg(theme.surface)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Text::new("Frequency response plot will be rendered here")
                                .size(TextSize::Sm)
                                .color(theme.text_muted),
                        )
                        .into_any_element()
                } else {
                    div()
                        .h(px(300.0))
                        .w_full()
                        .rounded_md()
                        .bg(theme.surface)
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            Text::new("Frequency Response Plot")
                                .size(TextSize::Md)
                                .weight(TextWeight::Semibold)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(
                                "Start recording to see frequency and phase response for all channels",
                            )
                            .size(TextSize::Sm)
                            .color(theme.text_muted),
                        )
                        .into_any_element()
                }),
        )
    }

    /// Render capture action buttons (Load/Save/Redo)
    fn render_capture_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        let has_recordings = recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == ChannelRecordingState::Done);

        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Button::new("load_recordings", "Load")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .on_click({
                        move |_, _cx| {
                            // TODO: Implement load recordings
                            log::info!("Load recordings clicked");
                        }
                    }),
            )
            .child(
                Button::new("save_recordings", "Save")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(!has_recordings)
                    .on_click({
                        move |_, _cx| {
                            // TODO: Implement save recordings
                            log::info!("Save recordings clicked");
                        }
                    }),
            )
            .child(
                Button::new("redo_recordings", "Redo All")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(!has_recordings)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.reset_all_recordings(cx);
                            });
                        }
                    }),
            )
    }

    // ==========================================================================
    // Recording control methods
    // ==========================================================================

    /// Start recording all channels sequentially
    fn start_recording_all_channels(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.app.recording_state.status_message = "Starting recording...".to_string();
            // Mark first channel as recording
            if !state.app.recording_state.channel_recordings.is_empty() {
                state.app.recording_state.current_recording_channel = Some(0);
                state.app.recording_state.channel_recordings[0].state =
                    ChannelRecordingState::Recording;
            }
        });
        cx.notify();

        // TODO: Actually start the recording via audio engine
        // For now just simulate
        log::info!("Starting recording for all channels");
    }

    /// Start recording a single channel
    fn start_recording_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            if let Some(recording) = state
                .app
                .recording_state
                .channel_recordings
                .get_mut(channel_idx)
            {
                recording.state = ChannelRecordingState::Recording;
                state.app.recording_state.current_recording_channel = Some(channel_idx);
                state.app.recording_state.status_message =
                    format!("Recording channel {}...", recording.channel_name);
            }
        });
        cx.notify();

        // TODO: Actually start the recording via audio engine
        log::info!("Starting recording for channel {}", channel_idx);
    }

    /// Stop all recording
    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.app.recording_state.current_recording_channel = None;
            state.app.recording_state.recording_progress = 0.0;
            state.app.recording_state.status_message = "Recording stopped".to_string();

            // Reset any channels that were recording back to empty
            for recording in &mut state.app.recording_state.channel_recordings {
                if recording.state == ChannelRecordingState::Recording {
                    recording.state = ChannelRecordingState::Empty;
                }
            }
        });
        cx.notify();

        log::info!("Recording stopped");
    }

    /// Reset all recordings to empty state
    fn reset_all_recordings(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            for recording in &mut state.app.recording_state.channel_recordings {
                recording.state = ChannelRecordingState::Empty;
                recording.result = None;
            }
            state.app.recording_state.current_recording_channel = None;
            state.app.recording_state.recording_progress = 0.0;
            state.app.recording_state.status_message = String::new();
        });
        cx.notify();

        log::info!("All recordings reset");
    }
}
