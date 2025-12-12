//! Recording Capture Step (Step 2)
//!
//! Run test signals per channel, display state, and plot results.

use crate::app::types::{ChannelRecordingState, RecordingResult, RecordingSignalType};
use crate::ui::PlayerView;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::color::D3Color;
use d3rs::grid::{render_grid, GridConfig};
use d3rs::prelude::LogScale;
use d3rs::scale::LinearScale;
use d3rs::shape::{render_line, LineConfig, LinePoint};
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

    /// Render frequency response plot with d3rs
    fn render_frequency_response_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;

        // Collect results from all channels that have recordings
        let results: Vec<(String, RecordingResult)> = recording_state
            .channel_recordings
            .iter()
            .filter_map(|r| r.result.as_ref().map(|res| (r.channel_name.clone(), res.clone())))
            .collect();

        let has_results = !results.is_empty();

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
                    self.render_frequency_response_chart(&results, &theme)
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

    /// Render the actual frequency response chart using d3rs
    fn render_frequency_response_chart(
        &self,
        results: &[(String, RecordingResult)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 800.0;
        let chart_height: f32 = 300.0;
        let margin_left: f32 = 60.0;
        let margin_right: f32 = 20.0;
        let margin_top: f32 = 20.0;
        let margin_bottom: f32 = 40.0;

        let plot_width = chart_width - margin_left - margin_right;
        let plot_height = chart_height - margin_top - margin_bottom;

        let axis_theme = DefaultAxisTheme;

        // Create log scale for frequency (20Hz - 20kHz)
        let x_scale = LogScale::new()
            .domain(20.0, 20000.0)
            .range(0.0, plot_width as f64);

        // Find magnitude range from all results
        let (min_db, max_db) = results
            .iter()
            .flat_map(|(_, r)| r.magnitude_db.iter())
            .fold((0.0_f32, -100.0_f32), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        // Add some padding to the range
        let y_min = (min_db - 5.0).max(-60.0);
        let y_max = (max_db + 5.0).min(20.0);

        let y_scale = LinearScale::new()
            .domain(y_min as f64, y_max as f64)
            .range(plot_height as f64, 0.0);

        // Frequency tick values (log spaced)
        let freq_ticks: Vec<f64> = vec![20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];

        // Magnitude tick values
        let mag_range = (y_max - y_min) as i32;
        let mag_step = if mag_range > 40 { 10 } else if mag_range > 20 { 5 } else { 2 };
        let mag_ticks: Vec<f64> = ((y_min as i32 / mag_step * mag_step)..=(y_max as i32 / mag_step * mag_step + mag_step))
            .step_by(mag_step as usize)
            .map(|v| v as f64)
            .collect();

        // Colors for different channels
        let colors = [
            D3Color::from_hex(0x4285f4), // Blue
            D3Color::from_hex(0xea4335), // Red
            D3Color::from_hex(0x34a853), // Green
            D3Color::from_hex(0xfbbc04), // Yellow
            D3Color::from_hex(0x9c27b0), // Purple
            D3Color::from_hex(0x00bcd4), // Cyan
        ];

        // Build line elements for each channel
        let line_elements: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(idx, (_name, result))| {
                let color = colors[idx % colors.len()];

                // Convert frequency/magnitude data to LinePoint
                let points: Vec<LinePoint> = result
                    .frequencies
                    .iter()
                    .zip(result.magnitude_db.iter())
                    .filter(|&(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &m)| LinePoint { x: f as f64, y: m as f64 })
                    .collect();

                let line_config = LineConfig::new()
                    .stroke_color(color)
                    .stroke_width(2.0);

                render_line(&x_scale, &y_scale, &points, &line_config)
                    .into_any_element()
            })
            .collect();

        // Build legend
        let legend_items: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(idx, (name, _))| {
                let color = colors[idx % colors.len()];
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(12.0)).h(px(3.0)).bg(color.to_rgba()))
                    .child(
                        Text::new(name.clone())
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_2()
            // Legend
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children(legend_items),
            )
            // Chart container
            .child(
                div()
                    .w(px(chart_width as f32))
                    .h(px(chart_height as f32))
                    .bg(theme.surface)
                    .rounded_md()
                    .relative()
                    // Y-axis (magnitude in dB)
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(margin_top))
                            .w(px(margin_left))
                            .h(px(plot_height))
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left()
                                    .with_tick_values(mag_ticks.clone())
                                    .with_formatter(|v| format!("{:.0} dB", v)),
                                plot_height,
                                &axis_theme,
                            )),
                    )
                    // Plot area
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
                            // Grid
                            .child(render_grid(
                                &x_scale,
                                &y_scale,
                                &GridConfig::with_lines()
                                    .with_vertical_values(freq_ticks.clone())
                                    .with_horizontal_values(mag_ticks),
                                plot_width,
                                plot_height,
                                &axis_theme,
                            ))
                            // Lines
                            .children(line_elements),
                    )
                    // X-axis (frequency)
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top + plot_height))
                            .w(px(plot_width))
                            .h(px(margin_bottom))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom()
                                    .with_tick_values(freq_ticks)
                                    .with_formatter(|f| {
                                        if f >= 1000.0 {
                                            format!("{:.0}k", f / 1000.0)
                                        } else {
                                            format!("{:.0}", f)
                                        }
                                    }),
                                plot_width,
                                &axis_theme,
                            )),
                    ),
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
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.load_recordings(cx);
                            });
                        }
                    }),
            )
            .child(
                Button::new("save_recordings", "Save")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(!has_recordings)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.save_recordings(cx);
                            });
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
        // Start with the first channel
        self.start_recording_channel(0, cx);
    }

    /// Start recording a single channel
    fn start_recording_channel(&mut self, channel_idx: usize, cx: &mut Context<Self>) {
        use sotf_audio_player::signal_recorder::{
            generate_signal, prepare_signal, write_temp_wav, SignalParams, SignalType,
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

            // Map signal type
            let signal_type = match rec_state.signal_type {
                RecordingSignalType::Sweep => SignalType::Sweep,
                RecordingSignalType::WhiteNoise => SignalType::WhiteNoise,
                RecordingSignalType::PinkNoise => SignalType::PinkNoise,
            };

            // Get output channel from playback config
            let output_ch = rec_state
                .playback_config
                .channel_mappings
                .get(channel_idx)
                .map(|m| m.interface_channel)
                .unwrap_or(0);

            // Get input channel from recording config (use first one for now)
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
                rec_state.playback_config.device_name.clone(),
                rec_state.recording_config.device_name.clone(),
                output_ch as u16,
                input_ch as u16,
                rec_state.playback_config.sample_rate,
                rec_state.mic_calibration_path.clone(),
                channel_name,
            )
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
            SignalType::WhiteNoise | SignalType::PinkNoise => SignalParams::Noise { amp: amplitude },
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
        let prepared_signal = prepare_signal(signal.clone(), sample_rate);

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

        // Create output paths
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let recorded_wav_path = temp_dir.join(format!("recording_ch{}_{}.wav", channel_idx, timestamp));
        let csv_path = temp_dir.join(format!("recording_ch{}_{}.csv", channel_idx, timestamp));

        // Spawn background task for recording
        let state_entity = self.state.clone();
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
            let _ = state_entity.update(&mut cx.clone(), |state, _| {
                match result {
                    Ok(()) => {
                        // Parse the CSV file to get frequency response data
                        if let Ok(csv_data) = std::fs::read_to_string(&csv_path) {
                            let mut frequencies = Vec::new();
                            let mut magnitude_db = Vec::new();
                            let mut phase_deg = Vec::new();

                            for line in csv_data.lines().skip(1) {
                                // Skip header
                                let parts: Vec<&str> = line.split(',').collect();
                                if parts.len() >= 3 {
                                    if let (Ok(f), Ok(m), Ok(p)) = (
                                        parts[0].parse::<f32>(),
                                        parts[1].parse::<f32>(),
                                        parts[2].parse::<f32>(),
                                    ) {
                                        frequencies.push(f);
                                        magnitude_db.push(m);
                                        phase_deg.push(p);
                                    }
                                }
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
                                    wav_path: Some(recorded_wav_path.to_string_lossy().to_string()),
                                    csv_path: Some(csv_path.to_string_lossy().to_string()),
                                    frequencies,
                                    magnitude_db,
                                    phase_deg,
                                });
                            }
                            state.app.recording_state.status_message =
                                format!("Channel {} recording complete", channel_name);
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
                    }
                }
                state.app.recording_state.current_recording_channel = None;
                state.app.recording_state.recording_progress = 1.0;
            });

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

    /// Save recordings to a JSON file
    fn save_recordings(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::ChannelRecording;

        // Get recordings to save
        let recordings: Vec<ChannelRecording> = {
            let state = self.state.read(cx);
            state.app.recording_state.channel_recordings.clone()
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open save dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_file_name("recordings.json")
                .save_file()
                .await;

            if let Some(file) = file {
                // Serialize recordings to JSON
                match serde_json::to_string_pretty(&recordings) {
                    Ok(json) => {
                        // Write to file
                        if let Err(e) = std::fs::write(file.path(), json) {
                            log::error!("Failed to write recordings file: {}", e);
                            let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                state.app.recording_state.status_message =
                                    format!("Failed to save: {}", e);
                            });
                        } else {
                            log::info!("Recordings saved to {:?}", file.path());
                            let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                state.app.recording_state.status_message =
                                    format!("Saved to {}", file.path().display());
                            });
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize recordings: {}", e);
                        let _ = state_entity.update(&mut cx.clone(), |state, _| {
                            state.app.recording_state.status_message =
                                format!("Failed to serialize: {}", e);
                        });
                    }
                }
            }
        })
        .detach();

        log::info!("Save recordings initiated");
    }

    /// Load recordings from a JSON file
    fn load_recordings(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::ChannelRecording;

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
                .await;

            if let Some(file) = file {
                // Read file content
                match std::fs::read_to_string(file.path()) {
                    Ok(json) => {
                        // Deserialize recordings
                        match serde_json::from_str::<Vec<ChannelRecording>>(&json) {
                            Ok(recordings) => {
                                log::info!("Loaded {} recordings from {:?}", recordings.len(), file.path());
                                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                                    state.app.recording_state.channel_recordings = recordings;
                                    state.app.recording_state.status_message =
                                        format!("Loaded from {}", file.path().display());
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

        log::info!("Load recordings initiated");
    }
}
