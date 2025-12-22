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
        let signal_level_db = state.app.recording_state.signal_level_db;
        let _ = state;

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
                                            state.app.recording_state.signal_level_db = val as f32;
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
        let _theme = state.app.theme.clone();
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
        let _theme = state.app.theme.clone();
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
                    stack.child(Text::new(status_message.clone()).size(TextSize::Sm))
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
                            Button::new(
                                SharedString::from(format!("record_ch_{}", idx)),
                                button_label,
                            )
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .disabled(
                                is_recording || channel_state == ChannelRecordingState::Recording,
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

    /// Render capture action buttons (redo and load from file)
    fn render_capture_redo_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let recording_state = &state.app.recording_state;
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
    fn start_recording_all_channels(&mut self, cx: &mut Context<Self>) {
        // Enable auto-record mode
        self.state.update(cx, |state, _| {
            state.app.recording_state.auto_record_remaining = true;
        });

        // Start with the first channel
        self.start_recording_channel(0, cx);

        log::info!("Starting auto-record mode - all channels will be recorded sequentially");
    }

    /// Start recording a single channel
    fn start_recording_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
        use sotf_audio_player::signal_recorder::{
            SignalParams, SignalType, generate_signal, write_temp_wav,
        };

        // Get recording parameters from state
        let (
            signal_type,
            duration_secs,
            level_db,
            output_device,
            input_device,
            output_channel,
            input_channel,
            sample_rate,
            mic_calibration,
            channel_name,
            recording_directory,
        ) = {
            let state = self.state.read(cx);
            let rec_state = &state.app.recording_state;

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

            // Get output channel from playback config (1-indexed in UI, convert to 0-indexed for cpal)
            let output_ch = rec_state
                .playback_config
                .channel_mappings
                .get(channel_idx)
                .map(|m| m.interface_channel.saturating_sub(1))
                .unwrap_or(0);

            // Get input channel from recording config (1-indexed in UI, convert to 0-indexed for cpal)
            let input_ch = rec_state
                .recording_config
                .channel_mappings
                .first()
                .map(|ch| ch.saturating_sub(1))
                .unwrap_or(0);

            (
                signal_type,
                rec_state.signal_duration_secs,
                rec_state.signal_level_db,
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
                    state.app.recording_state.status_message =
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
                .recording_state
                .channel_recordings
                .get_mut(channel_idx)
            {
                recording.state = ChannelRecordingState::Recording;
            }
            state.app.recording_state.current_recording_channel = Some(channel_idx);
            state.app.recording_state.status_message =
                format!("Recording channel {}...", channel_name);
            state.app.recording_state.recording_progress = 0.0;
        });
        cx.notify();

        // Convert dB level to linear amplitude
        let amplitude = 10.0_f32.powf(level_db / 20.0);

        // Generate signal parameters
        let params = match signal_type {
            SignalType::Sweep => SignalParams::Sweep {
                start_freq: 20.0,
                end_freq: 20000.0,
                amp: amplitude,
            },
            SignalType::WhiteNoise | SignalType::PinkNoise => {
                SignalParams::Noise { amp: amplitude }
            }
            _ => SignalParams::Sweep {
                start_freq: 20.0,
                end_freq: 20000.0,
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
                        .recording_state
                        .channel_recordings
                        .get_mut(channel_idx)
                    {
                        recording.state = ChannelRecordingState::Error;
                    }
                    state.app.recording_state.current_recording_channel = None;
                    state.app.recording_state.status_message = format!("Error: {}", e);
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
                        .recording_state
                        .channel_recordings
                        .get_mut(channel_idx)
                    {
                        recording.state = ChannelRecordingState::Error;
                    }
                    state.app.recording_state.current_recording_channel = None;
                    state.app.recording_state.status_message = format!("Error: {}", e);
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
            use sotf_audio_player::signal_recorder::record_and_analyze;

            log::info!(
                "Starting recording: output_ch={}, input_ch={}, device={}",
                output_channel,
                input_channel,
                output_device
            );

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
            );

            // Parse results and update state
            let (should_auto_continue, next_channel_idx) = state_entity
                .update(&mut cx.clone(), |state, _| {
                    let should_continue = match result {
                        Ok(()) => {
                            // Analyze the recorded WAV file using the new library function
                            use sotf_audio::signal_analysis::{
                                WavAnalysisConfig, analyze_wav_file, write_wav_analysis_csv,
                            };

                            // Use log sweep config since we're recording sweep measurements
                            let config = WavAnalysisConfig::for_log_sweep();

                            match analyze_wav_file(&recorded_wav_path, &config) {
                                Ok(analysis_result) => {
                                    // Write the CSV file
                                    if let Err(e) =
                                        write_wav_analysis_csv(&analysis_result, &csv_path)
                                    {
                                        log::error!("Failed to write CSV: {}", e);
                                    }

                                    if let Some(recording) = state
                                        .app
                                        .recording_state
                                        .channel_recordings
                                        .get_mut(channel_idx)
                                    {
                                        recording.state = ChannelRecordingState::Done;
                                        recording.result = Some(RecordingResult {
                                            channel: channel_idx,
                                            wav_path: Some(
                                                recorded_wav_path.to_string_lossy().to_string(),
                                            ),
                                            csv_path: Some(csv_path.to_string_lossy().to_string()),
                                            frequencies: analysis_result.frequencies,
                                            magnitude_db: analysis_result.magnitude_db,
                                            phase_deg: analysis_result.phase_deg,
                                        });
                                    }
                                    state.app.recording_state.status_message =
                                        format!("Channel {} recording complete", channel_name);

                                    // Check if we should auto-record the next channel
                                    state.app.recording_state.auto_record_remaining
                                }
                                Err(e) => {
                                    log::error!("Failed to analyze recording: {}", e);
                                    if let Some(recording) = state
                                        .app
                                        .recording_state
                                        .channel_recordings
                                        .get_mut(channel_idx)
                                    {
                                        recording.state = ChannelRecordingState::Error;
                                    }
                                    state.app.recording_state.status_message =
                                        format!("Analysis error: {}", e);
                                    false
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Recording failed: {}", e);
                            if let Some(recording) = state
                                .app
                                .recording_state
                                .channel_recordings
                                .get_mut(channel_idx)
                            {
                                recording.state = ChannelRecordingState::Error;
                            }
                            state.app.recording_state.status_message =
                                format!("Recording error: {}", e);

                            // Stop auto-recording on error
                            state.app.recording_state.auto_record_remaining = false;
                            false
                        }
                    };

                    state.app.recording_state.current_recording_channel = None;
                    state.app.recording_state.recording_progress = 1.0;

                    // Find next channel to record if in auto-record mode
                    let next_channel_idx = if should_continue {
                        state
                            .app
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
                })
                .ok()
                .unwrap_or((false, None));

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
                            state.app.recording_state.auto_record_remaining = false;
                            state.app.recording_state.status_message =
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
            state.app.recording_state.current_recording_channel = None;
            state.app.recording_state.recording_progress = 0.0;
            state.app.recording_state.status_message = "Recording stopped".to_string();
            state.app.recording_state.auto_record_remaining = false; // Disable auto-record mode

            // Reset any channels that were recording back to empty
            for recording in &mut state.app.recording_state.channel_recordings {
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
            for recording in &mut state.app.recording_state.channel_recordings {
                recording.state = ChannelRecordingState::Empty;
                recording.result = None;
            }
            state.app.recording_state.current_recording_channel = None;
            state.app.recording_state.recording_progress = 0.0;
            state.app.recording_state.status_message = String::new();
            state.app.recording_state.auto_record_remaining = false;
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
            let rec_state = &state.app.recording_state;
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
                        state.app.recording_state.status_message = format!("Failed to save: {}", e);
                    });
                } else {
                    log::info!("Recordings saved to {:?}", json_path);
                    self.state.update(cx, |state, _| {
                        state.app.recording_state.status_message =
                            format!("Saved to {}", json_path.display());
                    });
                }
            }
            Err(e) => {
                log::error!("Failed to serialize recordings: {}", e);
                self.state.update(cx, |state, _| {
                    state.app.recording_state.status_message =
                        format!("Failed to serialize: {}", e);
                });
            }
        }
        cx.notify();

        log::info!("Save recordings completed");
    }

    /// Load recordings from a JSON file (RoomEqMeasurementsFile format)
    pub(crate) fn load_recordings_from_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{ChannelRecording, RoomEqMeasurementsFile};

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

                // Read file content
                match std::fs::read_to_string(&file_path) {
                    Ok(json) => {
                        // Deserialize as RoomEqMeasurementsFile
                        match serde_json::from_str::<RoomEqMeasurementsFile>(&json) {
                            Ok(measurements_file) => {
                                log::info!(
                                    "Loaded {} channel measurements from {:?}",
                                    measurements_file.channels.len(),
                                    file_path
                                );

                                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                    // Convert ChannelMeasurement to ChannelRecording
                                    let recordings: Vec<ChannelRecording> = measurements_file
                                        .channels
                                        .into_iter()
                                        .enumerate()
                                        .map(|(idx, cm)| {
                                            // Convert relative paths in result to absolute paths
                                            let mut result = cm.measurement;
                                            if let (Some(dir), Some(wav)) =
                                                (&file_dir, &result.wav_path)
                                            {
                                                let abs_path = dir.join(wav);
                                                if abs_path.exists() {
                                                    result.wav_path = Some(
                                                        abs_path.to_string_lossy().to_string(),
                                                    );
                                                }
                                            }
                                            if let (Some(dir), Some(csv)) =
                                                (&file_dir, &result.csv_path)
                                            {
                                                let abs_path = dir.join(csv);
                                                if abs_path.exists() {
                                                    result.csv_path = Some(
                                                        abs_path.to_string_lossy().to_string(),
                                                    );
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

                                    state.app.recording_state.channel_recordings = recordings;

                                    // Also set the recording directory to the file's directory
                                    if let Some(dir) = file_dir {
                                        state.app.recording_state.recording_directory =
                                            Some(dir.to_string_lossy().to_string());
                                    }

                                    state.app.recording_state.status_message = format!(
                                        "Loaded {} channels from {}",
                                        state.app.recording_state.channel_recordings.len(),
                                        file_path.display()
                                    );
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse recordings JSON: {}", e);
                                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                    state.app.recording_state.status_message =
                                        format!("Failed to parse: {}", e);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read recordings file: {}", e);
                        let _ = state_entity.update(&mut cx.clone(), |state, _| {
                            state.app.recording_state.status_message =
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
