//! Recording Capture Step (Step 2)
//!
//! Run test signals per channel, display state, and plot results.

use crate::app::types::{ChannelRecordingState, RecordingResult, RecordingSignalType};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, NumberInput, NumberInputSize, Progress,
    ProgressSize, ProgressVariant, Select, SelectOption, StackAlign, StackJustify, StackSize,
    StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Render the capture step UI
    pub(crate) fn render_recording_capture_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Signal Recording")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold),
                    )
                    .child(
                        Text::new("Test each channel individually. Signals will play sequentially with a 1-second pause between channels.")
                            .size(TextSize::Sm),
                    ),
            )
            .child(self.render_signal_config_section(cx))
            .child(self.render_channel_status_section(cx))
            .child(self.render_capture_redo_actions(cx))
    }

    /// Render signal configuration section
    fn render_signal_config_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let signal_level_db = state.app.measurement_state.recording_state.signal_level_db;
        let signal_type = state.app.measurement_state.recording_state.signal_type;
        let sweep_start_freq = state.app.measurement_state.recording_state.sweep_start_freq;
        let sweep_end_freq = state.app.measurement_state.recording_state.sweep_end_freq;
        let _ = state;

        let is_sweep = signal_type == RecordingSignalType::Sweep;

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
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
                                        .weight(TextWeight::Semibold),
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
                                        .weight(TextWeight::Semibold),
                                )
                                .child(self.render_duration_dropdown(cx)),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .align(StackAlign::Center)
                                .child(Text::new("Level:").size(TextSize::Sm))
                                .child({
                                    let view = cx.entity().clone();
                                    NumberInput::new("signal_level")
                                        .value(signal_level_db as f64)
                                        .min(-60.0)
                                        .max(6.0)
                                        .step(1.0)
                                        .decimals(0)
                                        .unit("dB")
                                        .size(NumberInputSize::Sm)
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
                .when(is_sweep, |stack| {
                    let view = cx.entity().clone();
                    let view2 = cx.entity().clone();
                    stack.child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .align(StackAlign::Center)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .align(StackAlign::Center)
                                    .child(Text::new("Start Freq:").size(TextSize::Sm))
                                    .child(
                                        NumberInput::new("sweep_start_freq")
                                            .value(sweep_start_freq as f64)
                                            .min(1.0)
                                            .max(20000.0)
                                            .step(1.0)
                                            .decimals(0)
                                            .unit("Hz")
                                            .size(NumberInputSize::Sm)
                                            .width(100.0)
                                            .on_change(move |val, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .recording_state
                                                            .sweep_start_freq = val as f32;
                                                    });
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .align(StackAlign::Center)
                                    .child(Text::new("End Freq:").size(TextSize::Sm))
                                    .child(
                                        NumberInput::new("sweep_end_freq")
                                            .value(sweep_end_freq as f64)
                                            .min(100.0)
                                            .max(48000.0)
                                            .step(100.0)
                                            .decimals(0)
                                            .unit("Hz")
                                            .size(NumberInputSize::Sm)
                                            .width(100.0)
                                            .on_change(move |val, _window, cx| {
                                                view2.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .recording_state
                                                            .sweep_end_freq = val as f32;
                                                    });
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            ),
                    )
                }),
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
                                    let theme = theme.clone();
                                    stack.child(
                                        Button::new("record_all", "Record All Channels")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Md)
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
                                            .size(ButtonSize::Md)
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
                    stack.child(Text::new(status_message.clone()).size(TextSize::Sm))
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
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .child(Text::new("⚠").size(TextSize::Md).color(theme.warning))
                                    .child(
                                        Text::new(warning_msg)
                                            .size(TextSize::Sm)
                                            .color(theme.warning),
                                    ),
                            ),
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
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
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
                            Button::new(
                                SharedString::from(format!("record_ch_{}", idx)),
                                button_label,
                            )
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .disabled(
                                is_recording || channel_state == ChannelRecordingState::Recording,
                            )
                            .theme(theme.to_button_theme())
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
            .spacing(StackSpacing::Md)
            .child(
                Button::new("redo_recordings", "Redo All")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
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
                    .size(ButtonSize::Md)
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
        // Enable auto-record mode
        self.state.update(cx, |state, _| {
            state
                .app
                .measurement_state
                .recording_state
                .auto_record_remaining = true;
        });

        // Start with the first channel
        self.start_recording_channel(0, cx);

        log::info!("Starting auto-record mode - all channels will be recorded sequentially");
    }

    /// Start recording a single channel
    pub fn start_recording_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
        use sotf_audio_player::signal_recorder::{
            SignalParams, SignalType, generate_signal, write_temp_wav,
        };

        // Get recording parameters from state
        let (
            signal_type,
            duration_secs,
            level_db,
            sweep_start_freq,
            sweep_end_freq,
            output_device,
            input_device,
            output_channel,
            input_channel,
            sample_rate,
            mic_calibration,
            channel_name,
            recording_directory,
        ): (
            _,              // signal_type
            f32,            // duration_secs
            f32,            // level_db
            f32,            // sweep_start_freq
            f32,            // sweep_end_freq
            String,         // output_device
            String,         // input_device
            u16,            // output_channel
            u16,            // input_channel
            u32,            // sample_rate
            Option<String>, // mic_calibration
            String,         // channel_name
            Option<String>, // recording_directory
        ) = {
            let state = self.state.read(cx);
            let rec_state = &state.app.measurement_state.recording_state;

            // Get the channel info
            let channel_info = rec_state.channel_recordings.get(channel_idx);
            if channel_info.is_none() {
                log::error!("Invalid channel index: {}", channel_idx);
                return;
            }
            let channel_name = channel_info.unwrap().channel_name.clone();
            let recording_directory = rec_state.recording_directory.clone();

            // Map signal type
            let signal_type = match rec_state.signal_type {
                RecordingSignalType::Sweep => SignalType::Sweep,
                RecordingSignalType::WhiteNoise => SignalType::WhiteNoise,
                RecordingSignalType::PinkNoise => SignalType::PinkNoise,
            };

            // Get output channel from playback config (stored as 0-based index)
            // For multi-channel speakers, use the first interface channel
            let output_ch = rec_state
                .playback_config
                .channel_mappings
                .get(channel_idx)
                .map(|m| m.interface_channel())
                .unwrap_or(0);

            // Get input channel from recording config (stored as 0-based index)
            let input_ch = rec_state
                .recording_config
                .channel_mappings
                .first()
                .copied()
                .unwrap_or(0);

            (
                signal_type,
                rec_state.signal_duration_secs,
                rec_state.signal_level_db,
                rec_state.sweep_start_freq,
                rec_state.sweep_end_freq,
                rec_state.playback_config.device_name.clone(),
                rec_state.recording_config.device_name.clone(),
                output_ch as u16,
                input_ch as u16,
                rec_state.playback_config.sample_rate,
                rec_state.mic_calibration_path.clone(),
                channel_name,
                recording_directory,
            )
        };

        // Check if recording directory is set
        let recording_dir = match recording_directory {
            Some(dir) => std::path::PathBuf::from(dir),
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

        // Update UI to show recording state
        self.state.update(cx, |state, _| {
            if let Some(recording) = state
                .app
                .measurement_state
                .recording_state
                .channel_recordings
                .get_mut(channel_idx)
            {
                recording.state = ChannelRecordingState::Recording;
            }
            state
                .app
                .measurement_state
                .recording_state
                .current_recording_channel = Some(channel_idx);
            state.app.measurement_state.recording_state.status_message =
                format!("Recording channel {}...", channel_name);
            state
                .app
                .measurement_state
                .recording_state
                .recording_progress = 0.0;
        });
        cx.notify();

        // Convert dB level to linear amplitude
        let amplitude = 10.0_f32.powf(level_db / 20.0);

        // Generate signal parameters
        let params = match signal_type {
            SignalType::Sweep => SignalParams::Sweep {
                start_freq: sweep_start_freq,
                end_freq: sweep_end_freq,
                amp: amplitude,
            },
            SignalType::WhiteNoise | SignalType::PinkNoise => {
                SignalParams::Noise { amp: amplitude }
            }
            _ => SignalParams::Sweep {
                start_freq: sweep_start_freq,
                end_freq: sweep_end_freq,
                amp: amplitude,
            },
        };

        // Generate the test signal
        let signal = match generate_signal(signal_type, &params, duration_secs, sample_rate) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to generate signal: {}", e);
                self.state.update(cx, |state, _| {
                    if let Some(recording) = state
                        .app
                        .measurement_state
                        .recording_state
                        .channel_recordings
                        .get_mut(channel_idx)
                    {
                        recording.state = ChannelRecordingState::Error;
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

        // Prepare signal with fades and padding
        let prepared_signal = signal.clone(); // prepare_signal(signal.clone(), sample_rate);

        // Write to temp file
        let temp_wav = match write_temp_wav(&prepared_signal, sample_rate, 1) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to write temp WAV: {}", e);
                self.state.update(cx, |state, _| {
                    if let Some(recording) = state
                        .app
                        .measurement_state
                        .recording_state
                        .channel_recordings
                        .get_mut(channel_idx)
                    {
                        recording.state = ChannelRecordingState::Error;
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

        // Create output paths in the recording directory
        // Use channel name for descriptive filenames (sanitize for filesystem)
        let safe_channel_name: String = channel_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let recorded_wav_path = recording_dir.join(format!("{}.wav", safe_channel_name));
        let csv_path = recording_dir.join(format!("{}.csv", safe_channel_name));

        // Spawn background task for recording
        let state_entity = self.state.clone();
        let view_entity = cx.entity().clone();
        let reference_signal = signal.clone();
        let temp_wav_path = temp_wav.path().to_path_buf();

        cx.spawn(async move |_, cx| {
            use sotf_audio_player::signal_recorder::{record_and_analyze, SignalType};

            log::info!(
                "Starting recording: output_ch={}, input_ch={}, device={}",
                output_channel,
                input_channel,
                output_device
            );

            // Determine sweep range for THD calculation
            let sweep_range = if signal_type == SignalType::Sweep {
                Some((sweep_start_freq, sweep_end_freq))
            } else {
                None
            };

            // Run the recording
            let result = record_and_analyze(
                &temp_wav_path,
                &recorded_wav_path,
                &reference_signal,
                sample_rate,
                &csv_path,
                output_channel,
                input_channel,
                if output_device.is_empty() {
                    None
                } else {
                    Some(output_device.as_str())
                },
                if input_device.is_empty() {
                    None
                } else {
                    Some(input_device.as_str())
                },
                mic_calibration.as_deref(),
                sweep_range,
            );

            // Parse results and update state
            let (should_auto_continue, next_channel_idx) =
                state_entity.update(&mut cx.clone(), |state, _| {
                    let should_continue = match result {
                        Ok(analysis_result) => {
                            // Check for noise floor warning (signal too weak)
                            // Compute average SPL in 100 Hz - 10 kHz range
                            const NOISE_FLOOR_THRESHOLD_DB: f32 = -50.0;
                            let avg_spl = {
                                let mut sum = 0.0_f32;
                                let mut count = 0;
                                for (&freq, &mag) in analysis_result
                                    .frequencies
                                    .iter()
                                    .zip(analysis_result.spl_db.iter())
                                {
                                    if freq >= 100.0 && freq <= 10000.0 {
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

                            // Set noise floor warning if signal is too weak
                            if avg_spl < NOISE_FLOOR_THRESHOLD_DB {
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .noise_floor_warning = Some(format!(
                                    "Channel '{}' has very low signal level ({:.1} dB). Check microphone connection or increase signal level.",
                                    channel_name, avg_spl
                                ));
                                log::warn!(
                                    "Noise floor warning: Channel '{}' avg SPL = {:.1} dB (threshold: {} dB)",
                                    channel_name,
                                    avg_spl,
                                    NOISE_FLOOR_THRESHOLD_DB
                                );
                            } else {
                                // Clear any previous warning
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .noise_floor_warning = None;
                            }

                            if let Some(recording) = state
                                .app
                                .measurement_state
                                .recording_state
                                .channel_recordings
                                .get_mut(channel_idx)
                            {
                                recording.state = ChannelRecordingState::Done;
                                recording.result = Some(RecordingResult {
                                    channel: channel_idx,
                                    wav_path: Some(recorded_wav_path.to_string_lossy().to_string()),
                                    csv_path: Some(csv_path.to_string_lossy().to_string()),
                                    frequencies: analysis_result.frequencies,
                                    magnitude_db: analysis_result.spl_db, // Use spl_db from AnalysisResult
                                    phase_deg: analysis_result.phase_deg,
                                    impulse_response: Some(analysis_result.impulse_response),
                                    impulse_time_ms: Some(analysis_result.impulse_time_ms),
                                    excess_group_delay_ms: Some(analysis_result.excess_group_delay_ms),
                                    thd_percent: Some(analysis_result.thd_percent),
                                    harmonic_distortion_db: Some(analysis_result.harmonic_distortion_db),
                                    rt60_ms: Some(analysis_result.rt60_ms),
                                    clarity_c50_db: Some(analysis_result.clarity_c50_db),
                                    clarity_c80_db: Some(analysis_result.clarity_c80_db),
                                    spectrogram_db: Some(analysis_result.spectrogram_db),
                                });
                            }
                            state.app.measurement_state.recording_state.status_message =
                                format!("Channel {} recording complete", channel_name);

                            // Check if we should auto-record the next channel
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .auto_record_remaining
                        }
                        Err(e) => {
                            log::error!("Recording failed: {}", e);
                            if let Some(recording) = state
                                .app
                                .measurement_state
                                .recording_state
                                .channel_recordings
                                .get_mut(channel_idx)
                            {
                                recording.state = ChannelRecordingState::Error;
                            }
                            state.app.measurement_state.recording_state.status_message =
                                format!("Recording error: {}", e);

                            // Stop auto-recording on error
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

                    // Find next channel to record if in auto-record mode
                    let next_channel_idx = if should_continue {
                        state
                            .app
                            .measurement_state
                            .recording_state
                            .channel_recordings
                            .iter()
                            .enumerate()
                            .find(|(_, r)| r.state == ChannelRecordingState::Empty)
                            .map(|(idx, _)| idx)
                    } else {
                        None
                    };

                    (should_continue, next_channel_idx)
                });

            // If we should continue auto-recording, start the next channel
            if should_auto_continue {
                if let Some(next_idx) = next_channel_idx {
                    log::info!("Auto-recording: starting next channel {}", next_idx);
                    let _ = view_entity.update(cx, |view, cx| {
                        view.start_recording_channel(next_idx, cx);
                    });
                } else {
                    // No more channels to record - auto-record complete
                    log::info!("Auto-recording complete - all channels recorded");
                    let _ = view_entity.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .recording_state
                                .auto_record_remaining = false;
                            state.app.measurement_state.recording_state.status_message =
                                "All channels recorded successfully!".to_string();
                        });
                        cx.notify();
                    });
                }
            }

            // Clean up temp file (it will be dropped automatically)
            drop(temp_wav);
        })
        .detach();

        log::info!("Recording started for channel {}", channel_idx);
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
    pub(crate) fn save_recordings(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{
            ChannelMeasurement, RecordingConfiguration, RecordingResult, RoomEqMeasurementsFile,
        };

        // Get recordings, recording directory, configuration, and convert to RoomEqMeasurementsFile format
        let (measurements_file, recording_dir) = {
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

            // Build configuration from current state
            let configuration = RecordingConfiguration {
                playback_device_name: rec_state.playback_config.device_name.clone(),
                playback_device_id: rec_state.playback_config.device_id.clone(),
                playback_sample_rate: rec_state.playback_config.sample_rate,
                playback_channels: rec_state.playback_config.num_channels,
                speaker_configuration: rec_state
                    .playback_config
                    .speaker_configuration
                    .as_str()
                    .to_string(),
                channel_names: rec_state
                    .playback_config
                    .channel_mappings
                    .iter()
                    .map(|m| m.group_name.clone())
                    .collect(),

                recording_device_name: rec_state.recording_config.device_name.clone(),
                recording_device_id: rec_state.recording_config.device_id.clone(),
                recording_sample_rate: rec_state.recording_config.sample_rate,
                recording_channels: rec_state.recording_config.num_channels,

                mic_calibration_path: rec_state.mic_calibration_path.clone(),
                recording_directory: Some(recording_dir.clone()),

                signal_type: rec_state.signal_type.as_str().to_string(),
                signal_duration_secs: rec_state.signal_duration_secs,
                signal_level_db: rec_state.signal_level_db,
            };

            // Convert ChannelRecording to ChannelMeasurement with relative paths
            let channels: Vec<ChannelMeasurement> = recordings
                .iter()
                .filter_map(|rec| {
                    rec.result.as_ref().map(|result| {
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

                        ChannelMeasurement {
                            channel_name: rec.channel_name.clone(),
                            measurement: RecordingResult {
                                channel: result.channel,
                                wav_path: relative_wav,
                                csv_path: relative_csv,
                                frequencies: result.frequencies.clone(),
                                magnitude_db: result.magnitude_db.clone(),
                                phase_deg: result.phase_deg.clone(),
                                impulse_response: result.impulse_response.clone(),
                                impulse_time_ms: result.impulse_time_ms.clone(),
                                excess_group_delay_ms: result.excess_group_delay_ms.clone(),
                                thd_percent: result.thd_percent.clone(),
                                harmonic_distortion_db: result.harmonic_distortion_db.clone(),
                                rt60_ms: result.rt60_ms.clone(),
                                clarity_c50_db: result.clarity_c50_db.clone(),
                                clarity_c80_db: result.clarity_c80_db.clone(),
                                spectrogram_db: result.spectrogram_db.clone(),
                            },
                            is_group: false,
                            group_drivers: Vec::new(),
                        }
                    })
                })
                .collect();

            if channels.is_empty() {
                log::warn!("No completed recordings to save");
                return;
            }

            (
                RoomEqMeasurementsFile::with_configuration(channels, configuration),
                recording_dir,
            )
        };

        // Save to recording directory (no dialog needed)
        let json_path = std::path::Path::new(&recording_dir).join("recordings.json");

        match serde_json::to_string_pretty(&measurements_file) {
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
                let file_size = std::fs::metadata(&file_path)
                    .map(|m| m.len())
                    .unwrap_or(0);

                // Read file content
                match std::fs::read_to_string(&file_path) {
                    Ok(json) => {
                        // Check if this is a legacy format (large file with inline data)
                        let needs_migration = Self::check_needs_migration(&json, file_size);

                        if needs_migration {
                            // Show migration modal instead of loading directly
                            log::info!(
                                "Detected legacy format ({:.2} MB), showing migration modal",
                                file_size as f64 / 1_000_000.0
                            );

                            // Count channels for display
                            let channel_count = RoomEqMeasurementsFile::from_json_str(&json)
                                .map(|m| m.channels.len())
                                .unwrap_or(0);

                            let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                let rec_state = &mut state.app.measurement_state.recording_state;
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
                        let _ = state_entity.update(&mut cx.clone(), |state, _| {
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

    /// Check if a JSON file needs migration (legacy format with large inline data)
    fn check_needs_migration(json: &str, file_size: u64) -> bool {
        // If file is small (<1MB), don't bother with migration
        if file_size < 1_000_000 {
            return false;
        }

        // Check if the JSON contains large inline frequency data
        // Look for "frequencies" or "magnitude_db" arrays with data
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            // Check channels array for inline data
            if let Some(channels) = value.get("channels").and_then(|c| c.as_array()) {
                for channel in channels {
                    // Check measurement.frequencies array
                    if let Some(measurement) = channel.get("measurement") {
                        if let Some(freqs) = measurement.get("frequencies").and_then(|f| f.as_array())
                        {
                            if freqs.len() > 100 {
                                return true;
                            }
                        }
                    }
                    // Check result.frequencies for older format
                    if let Some(result) = channel.get("result") {
                        if let Some(freqs) = result.get("frequencies").and_then(|f| f.as_array()) {
                            if freqs.len() > 100 {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Internal function to load recordings from parsed JSON
    fn load_recordings_internal(
        state_entity: Entity<crate::app::state::AppState>,
        cx: &mut gpui::AsyncApp,
        json: &str,
        file_path: &std::path::Path,
        file_dir: Option<std::path::PathBuf>,
    ) {
        use crate::app::types::{ChannelRecording, RoomEqMeasurementsFile};

        match RoomEqMeasurementsFile::from_json_str(json) {
            Ok(measurements_file) => {
                log::info!(
                    "Loaded {} channel measurements from {:?}",
                    measurements_file.channels.len(),
                    file_path
                );

                let file_path_display = file_path.display().to_string();
                let _ = state_entity.update(cx, |state, _| {
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
                                    result.wav_path =
                                        Some(abs_path.to_string_lossy().to_string());
                                }
                            }
                            if let (Some(dir), Some(csv)) = (&file_dir, &result.csv_path) {
                                let abs_path = dir.join(csv);
                                if abs_path.exists() {
                                    result.csv_path =
                                        Some(abs_path.to_string_lossy().to_string());
                                }
                            }

                            ChannelRecording {
                                channel_index: idx,
                                channel_name: cm.channel_name,
                                state: ChannelRecordingState::Done,
                                result: Some(result),
                            }
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
                let _ = state_entity.update(cx, |state, _| {
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

        let file_path = rec_state
            .migration_file_path
            .clone()
            .unwrap_or_default();
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
                                    let rec_state = &mut state.app.measurement_state.recording_state;
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
                        div()
                            .px_4()
                            .py_4()
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Lg)
                                    .child(
                                        Text::new("This recording file uses an older format.")
                                            .size(TextSize::Md)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        VStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Md)
                                                    .child(Text::new("File:").size(TextSize::Sm).color(theme.text_secondary))
                                                    .child(Text::new(file_name).size(TextSize::Sm).color(theme.text_primary)),
                                            )
                                            .child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Md)
                                                    .child(Text::new("Size:").size(TextSize::Sm).color(theme.text_secondary))
                                                    .child(Text::new(format!("{:.2} MB", file_size_mb)).size(TextSize::Sm).color(theme.warning)),
                                            )
                                            .child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Md)
                                                    .child(Text::new("Channels:").size(TextSize::Sm).color(theme.text_secondary))
                                                    .child(Text::new(format!("{}", channel_count)).size(TextSize::Sm).color(theme.text_primary)),
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
                                                    let rec_state = &mut state.app.measurement_state.recording_state;
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
        use crate::app::types::{
            ChannelRecording, ChannelRecordingState, RoomEqMeasurementsFile,
        };
        use sotf_audio::signal_analysis::AnalysisResult;

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

        // Step 1: Back up the original JSON file
        let backup_path = {
            let mut backup = original_path.clone();
            let extension = backup
                .extension()
                .map(|e| format!("{}.bak", e.to_string_lossy()))
                .unwrap_or_else(|| "bak".to_string());
            backup.set_extension(extension);
            backup
        };

        if let Err(e) = std::fs::copy(&original_path, &backup_path) {
            log::error!("Failed to back up original JSON file: {}", e);
            self.state.update(cx, |state, _| {
                let rec_state = &mut state.app.measurement_state.recording_state;
                rec_state.migration_modal_open = false;
                rec_state.migration_pending_json = None;
                rec_state.status_message = format!("Failed to back up original file: {}", e);
            });
            cx.notify();
            return;
        }
        log::info!("Backed up original JSON to {:?}", backup_path);

        // Parse and migrate
        match RoomEqMeasurementsFile::from_json_str(&json) {
            Ok(measurements_file) => {
                let session_dir = file_dir.clone().unwrap_or_else(|| {
                    original_path
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf()
                });

                log::info!(
                    "Migrating {} channels to new format in {:?}",
                    measurements_file.channels.len(),
                    session_dir
                );

                // Step 2: Extract data to CSV files for each channel
                // Use channel name for CSV filename (same as recording)
                for (idx, channel) in measurements_file.channels.iter().enumerate() {
                    // Sanitize channel name for filesystem
                    let safe_channel_name: String = channel
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
                    let csv_filename = format!("{}.csv", safe_channel_name);
                    let csv_path = session_dir.join(&csv_filename);

                    // Build AnalysisResult from the measurement data
                    let result = &channel.measurement;
                    let analysis = AnalysisResult {
                        frequencies: result.frequencies.clone(),
                        spl_db: result.magnitude_db.clone(),
                        phase_deg: result.phase_deg.clone(),
                        estimated_lag_samples: 0,
                        impulse_response: result.impulse_response.clone().unwrap_or_default(),
                        impulse_time_ms: result.impulse_time_ms.clone().unwrap_or_default(),
                        thd_percent: result.thd_percent.clone().unwrap_or_default(),
                        harmonic_distortion_db: result
                            .harmonic_distortion_db
                            .clone()
                            .unwrap_or_default(),
                        rt60_ms: result.rt60_ms.clone().unwrap_or_default(),
                        clarity_c50_db: result.clarity_c50_db.clone().unwrap_or_default(),
                        clarity_c80_db: result.clarity_c80_db.clone().unwrap_or_default(),
                        excess_group_delay_ms: result
                            .excess_group_delay_ms
                            .clone()
                            .unwrap_or_default(),
                        spectrogram_db: result.spectrogram_db.clone().unwrap_or_default(),
                    };

                    // Write CSV with extended format (overwrites existing CSV if present)
                    if let Err(e) = Self::write_migration_csv(&analysis, &csv_path) {
                        log::error!("Failed to write CSV for channel {}: {}", idx, e);
                    } else {
                        log::info!(
                            "Wrote migrated CSV ({} points): {:?}",
                            analysis.frequencies.len(),
                            csv_path
                        );
                    }
                }

                // Step 3: Write new lightweight JSON to the ORIGINAL file location
                if let Err(e) =
                    Self::write_lightweight_session(&measurements_file, &original_path)
                {
                    log::error!("Failed to write new JSON file: {}", e);
                    self.state.update(cx, |state, _| {
                        let rec_state = &mut state.app.measurement_state.recording_state;
                        rec_state.migration_modal_open = false;
                        rec_state.migration_pending_json = None;
                        rec_state.status_message = format!("Failed to write new JSON: {}", e);
                    });
                    cx.notify();
                    return;
                }
                log::info!("Wrote new lightweight JSON to {:?}", original_path);

                // Create recordings directly from the already-parsed measurements_file
                // (no need to re-read and re-parse the JSON we just wrote)
                let recordings: Vec<ChannelRecording> = measurements_file
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(idx, ch)| {
                        // Sanitize channel name for filesystem
                        let safe_channel_name: String = ch
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
                                result.wav_path = Some(abs_path.to_string_lossy().to_string());
                            }
                        }

                        ChannelRecording {
                            channel_index: idx,
                            channel_name: ch.channel_name.clone(),
                            state: ChannelRecordingState::Done,
                            result: Some(result),
                        }
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

                log::info!(
                    "Migration complete: {} channels loaded from migrated format",
                    num_channels
                );
            }
            Err(e) => {
                log::error!("Failed to parse JSON for migration: {}", e);
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

    /// Write analysis result to CSV with extended format
    fn write_migration_csv(
        analysis: &sotf_audio::signal_analysis::AnalysisResult,
        csv_path: &std::path::Path,
    ) -> Result<(), String> {
        use std::io::Write;

        let mut file = std::fs::File::create(csv_path)
            .map_err(|e| format!("Failed to create CSV: {}", e))?;

        // Header
        writeln!(
            file,
            "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms"
        )
        .map_err(|e| format!("Failed to write header: {}", e))?;

        // Data
        for i in 0..analysis.frequencies.len() {
            let freq = analysis.frequencies[i];
            let spl = analysis.spl_db[i];
            let phase = analysis.phase_deg[i];
            let thd = analysis.thd_percent.get(i).copied().unwrap_or(0.0);
            let rt60 = analysis.rt60_ms.get(i).copied().unwrap_or(0.0);
            let c50 = analysis.clarity_c50_db.get(i).copied().unwrap_or(0.0);
            let c80 = analysis.clarity_c80_db.get(i).copied().unwrap_or(0.0);
            let gd = analysis.excess_group_delay_ms.get(i).copied().unwrap_or(0.0);

            writeln!(
                file,
                "{:.6},{:.3},{:.6},{:.6},{:.3},{:.3},{:.3},{:.6}",
                freq, spl, phase, thd, rt60, c50, c80, gd
            )
            .map_err(|e| format!("Failed to write data: {}", e))?;
        }

        Ok(())
    }

    /// Write session.json file with full data
    /// This creates a proper RoomEqMeasurementsFile structure with csv_path set
    /// Note: We keep the full data in JSON since the app doesn't have CSV loading yet
    fn write_lightweight_session(
        measurements: &crate::app::types::RoomEqMeasurementsFile,
        path: &std::path::Path,
    ) -> Result<(), String> {
        use crate::app::types::{ChannelMeasurement, RoomEqMeasurementsFile};

        // Create channels with csv_path AND full data
        // (keeping data ensures app works; CSV is for backup/export)
        let channels: Vec<ChannelMeasurement> = measurements
            .channels
            .iter()
            .enumerate()
            .map(|(idx, ch)| {
                // Sanitize channel name for filesystem (same as in perform_migration)
                let safe_channel_name: String = ch
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

                // Clone the original measurement and add csv_path
                let mut measurement = ch.measurement.clone();
                measurement.channel = idx;
                measurement.csv_path = Some(format!("{}.csv", safe_channel_name));

                ChannelMeasurement {
                    channel_name: ch.channel_name.clone(),
                    measurement,
                    is_group: ch.is_group,
                    group_drivers: ch.group_drivers.clone(),
                }
            })
            .collect();

        let file_data = RoomEqMeasurementsFile {
            version: 2,
            channels,
            configuration: measurements.configuration.clone(),
        };

        let file = std::fs::File::create(path)
            .map_err(|e| format!("Failed to create session file: {}", e))?;
        serde_json::to_writer_pretty(file, &file_data)
            .map_err(|e| format!("Failed to serialize session: {}", e))?;

        Ok(())
    }
}
