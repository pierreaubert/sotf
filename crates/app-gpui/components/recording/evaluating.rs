//! Recording Evaluating Step (Step 3)
//!
//! View and analyze frequency response graphs from recordings.
//! Displays: Magnitude, Phase, Group Delay, and Impulse Response graphs.

use crate::app::types::{PlotSmoothing, RecordingResult};
use crate::components::graphs::response_graphs::{
    CHANNEL_COLORS, ChartConfig, Series, render_line_chart,
};
use crate::components::graphs::spectrum_graphs::{
    SpectrumConfig, SpectrumGrid, render_spectrum_heatmap,
};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::ScaleType;
use gpui_ui_kit::{
    Card, HStack, Select, SelectOption, StackAlign, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};
use sotf_audio::signal_analysis as dsp;

impl PlayerView {
    /// Render the evaluating step UI with frequency response graphs
    pub(crate) fn render_recording_evaluating_step(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Evaluate Recordings")
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new("Review frequency response, phase, group delay, and impulse response measurements.")
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    ),
            )
            .child(self.render_plot_controls(cx))
            .child(self.render_magnitude_plot(cx))
            .child(self.render_phase_plot(cx))
            .child(self.render_group_delay_plot(cx))
            .child(self.render_distortion_plot(cx))
            .child(self.render_rt60_plot(cx))
            .child(self.render_clarity_plot(cx))
            .child(self.render_impulse_response_plot(cx))
            .child(self.render_spectrogram_plot(cx))
            .child(self.render_channel_summary(cx))
    }

    /// Render plot controls (channel selector, smoothing)
    fn render_plot_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;

        let selected_channel = recording_state.plot_selected_channel;
        let smoothing = recording_state.plot_smoothing;
        let channel_dropdown_open = recording_state.plot_channel_dropdown_open;
        let smoothing_dropdown_open = recording_state.plot_smoothing_dropdown_open;

        // Build channel options
        let mut channel_options = vec![SelectOption::new("all", "All Channels")];
        for (idx, rec) in recording_state.channel_recordings.iter().enumerate() {
            if rec.result.is_some() {
                channel_options.push(SelectOption::new(
                    format!("{}", idx),
                    rec.channel_name.clone(),
                ));
            }
        }

        let selected_channel_value = match selected_channel {
            None => "all".to_string(),
            Some(idx) => format!("{}", idx),
        };

        // Build smoothing options
        let smoothing_options = vec![
            SelectOption::new("none", PlotSmoothing::None.as_str()),
            SelectOption::new("1", PlotSmoothing::Octave1.as_str()),
            SelectOption::new("3", PlotSmoothing::Octave3.as_str()),
            SelectOption::new("6", PlotSmoothing::Octave6.as_str()),
            SelectOption::new("24", PlotSmoothing::Octave24.as_str()),
        ];

        let selected_smoothing_value = match smoothing {
            PlotSmoothing::None => "none",
            PlotSmoothing::Octave1 => "1",
            PlotSmoothing::Octave3 => "3",
            PlotSmoothing::Octave6 => "6",
            PlotSmoothing::Octave24 => "24",
        };

        let view = cx.entity().clone();

        Card::new().content(
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Channel:")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div().w(px(160.0)).child(
                                Select::new("plot_channel_select")
                                    .options(channel_options)
                                    .selected(selected_channel_value)
                                    .is_open(channel_dropdown_open)
                                    .theme(theme.to_select_theme())
                                    .on_toggle({
                                        let view = view.clone();
                                        move |open, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_channel_dropdown_open = open;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .on_change({
                                        let view = view.clone();
                                        move |value, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_selected_channel =
                                                        if value.as_ref() == "all" {
                                                            None
                                                        } else {
                                                            value.parse().ok()
                                                        };
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_channel_dropdown_open = false;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                        ),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Smoothing:")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div().w(px(140.0)).child(
                                Select::new("plot_smoothing_select")
                                    .options(smoothing_options)
                                    .selected(selected_smoothing_value)
                                    .is_open(smoothing_dropdown_open)
                                    .theme(theme.to_select_theme())
                                    .on_toggle({
                                        let view = view.clone();
                                        move |open, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_smoothing_dropdown_open = open;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .on_change({
                                        let view = view.clone();
                                        move |value, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_smoothing = match value.as_ref() {
                                                        "1" => PlotSmoothing::Octave1,
                                                        "3" => PlotSmoothing::Octave3,
                                                        "6" => PlotSmoothing::Octave6,
                                                        "24" => PlotSmoothing::Octave24,
                                                        _ => PlotSmoothing::None,
                                                    };
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .plot_smoothing_dropdown_open = false;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                        ),
                ),
        )
    }

    /// Get filtered results based on selected channel
    fn get_filtered_results(
        &self,
        cx: &mut Context<Self>,
    ) -> Vec<(String, usize, RecordingResult)> {
        let state = self.state.read(cx);
        let recording_state = &state.app.measurement_state.recording_state;
        let selected_channel = recording_state.plot_selected_channel;

        recording_state
            .channel_recordings
            .iter()
            .enumerate()
            .filter_map(|(idx, r)| {
                if let Some(selected) = selected_channel
                    && idx != selected
                {
                    return None;
                }
                r.result
                    .as_ref()
                    .map(|res| (r.channel_name.clone(), idx, res.clone()))
            })
            .collect()
    }

    /// Apply octave smoothing to frequency data
    fn apply_smoothing(frequencies: &[f32], values: &[f32], smoothing: PlotSmoothing) -> Vec<f32> {
        if let Some(octaves) = smoothing.octave_fraction() {
            dsp::smooth_response_f32(frequencies, values, octaves)
        } else {
            values.to_vec()
        }
    }

    /// Render magnitude plot
    fn render_magnitude_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("MAGNITUDE (dB)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_magnitude_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render phase plot
    fn render_phase_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("PHASE (degrees)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_phase_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render group delay plot
    fn render_group_delay_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("GROUP DELAY (ms)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_group_delay_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render impulse response plot
    fn render_impulse_response_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("IMPULSE RESPONSE")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_impulse_response_chart(&results, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render distortion plot
    fn render_distortion_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("DISTORTION (THD+N %)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_distortion_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render RT60 plot
    fn render_rt60_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("RT60 DECAY (ms)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_rt60_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render Clarity plot
    fn render_clarity_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let smoothing = state.app.measurement_state.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("CLARITY (C50/C80 dB)")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_clarity_chart(&results, smoothing, &theme)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render Spectrogram plot
    fn render_spectrogram_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let sample_rate = state
            .app
            .measurement_state
            .recording_state
            .playback_config
            .sample_rate as f32;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("SPECTROGRAM")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(if has_results {
                    self.render_spectrogram_chart(&results, &theme, sample_rate)
                        .into_any_element()
                } else {
                    self.render_no_data_placeholder(&theme).into_any_element()
                }),
        )
    }

    /// Render spectrogram chart
    fn render_spectrogram_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        theme: &crate::theme::Theme,
        sample_rate: f32,
    ) -> impl IntoElement {
        // We only visualize the first selected channel's spectrogram for now
        if let Some((_, _, result)) = results.first()
            && let Some(spectrogram) = &result.spectrogram_db
            && !spectrogram.is_empty()
        {
            return self
                .render_spectrogram_canvas(spectrogram, theme, sample_rate)
                .into_any_element();
        }

        div()
            .h(px(300.0))
            .w_full()
            .bg(theme.surface)
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .child(
                Text::new("Spectrogram not available")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
            .into_any_element()
    }

    fn render_spectrogram_canvas(
        &self,
        spectrogram: &[Vec<f32>],
        theme: &crate::theme::Theme,
        sample_rate: f32,
    ) -> impl IntoElement {
        let num_time_slices = spectrogram.len();
        if num_time_slices == 0 {
            return div().child("No data").into_any_element();
        }
        let num_freq_bins = spectrogram[0].len();
        if num_freq_bins == 0 {
            return div().child("No data").into_any_element();
        }

        // Generate Frequency bins (Y axis) - Linear from 0 to Nyquist
        let nyquist = sample_rate / 2.0;
        let y_values: Vec<f64> = (0..num_freq_bins)
            .map(|i| i as f64 * nyquist as f64 / num_freq_bins as f64)
            .collect();

        // Generate Time bins (X axis) - Seconds
        // Backend uses hop_size = 128
        let hop_size = 128.0;
        let x_values: Vec<f64> = (0..num_time_slices)
            .map(|i| i as f64 * hop_size / sample_rate as f64)
            .collect();

        // Flatten data for Heatmap (Row-major: Y then X)
        // Spectrogram is [time][freq] (X then Y columns).
        // We need [Freq0_Time0, Freq0_Time1...], [Freq1_Time0...]
        let mut z_values = Vec::with_capacity(num_time_slices * num_freq_bins);
        for f in 0..num_freq_bins {
            for row in spectrogram.iter().take(num_time_slices) {
                z_values.push(row[f] as f64);
            }
        }

        let grid = SpectrumGrid {
            x_values,
            y_values,
            z_values,
        };

        let config = SpectrumConfig {
            title: None,
            x_label: Some("Time (s)".to_string()),
            y_label: Some("Frequency (Hz)".to_string()),
            x_scale: ScaleType::Linear, // Time is linear
            y_scale: ScaleType::Linear, // Freq bins are linear
            width: 800.0,
            height: 300.0,
            color_scale: Some(gpui_px::ColorScale::Magma),
            ..Default::default()
        };

        render_spectrum_heatmap(grid, config, theme, None).into_any_element()
    }

    fn render_no_data_placeholder(&self, theme: &crate::theme::Theme) -> impl IntoElement {
        div()
            .h(px(200.0))
            .w_full()
            .rounded_md()
            .bg(theme.surface)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                Text::new("No Recordings Available")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Semibold)
                    .color(theme.text_secondary),
            )
            .child(
                Text::new("Go back to the Capture step to record frequency responses")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
    }

    /// Compute average SPL in the 100 Hz - 10 kHz range
    fn compute_average_spl(frequencies: &[f32], magnitude_db: &[f32]) -> Option<f32> {
        let mut sum = 0.0_f32;
        let mut count = 0;
        for (&freq, &mag) in frequencies.iter().zip(magnitude_db.iter()) {
            if (100.0..=10000.0).contains(&freq) {
                sum += mag;
                count += 1;
            }
        }
        if count > 0 {
            Some(sum / count as f32)
        } else {
            None
        }
    }

    /// Render magnitude chart
    fn render_magnitude_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Find channel with MAXIMUM averaged SPL (100 Hz - 10 kHz) as reference
        // This ensures empty/quiet recordings show well below the loudest channel
        let normalization_offset = results
            .iter()
            .filter_map(|(_, _, result)| {
                Self::compute_average_spl(&result.frequencies, &result.magnitude_db)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        // Prepare series
        let series: Vec<Series> = results
            .iter()
            .map(|(name, idx, result)| {
                let normalized: Vec<f32> = result
                    .magnitude_db
                    .iter()
                    .map(|&mag| -(mag - normalization_offset))
                    .collect();
                let smoothed = Self::apply_smoothing(&result.frequencies, &normalized, smoothing);

                let freqs_f64: Vec<f64> = result.frequencies.iter().map(|&v| v as f64).collect();
                let mags_f64: Vec<f64> = smoothed.iter().map(|&v| v as f64).collect();

                Series::new(
                    name.clone(),
                    CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                    freqs_f64,
                    mags_f64,
                )
            })
            .collect();

        // Find y-axis range
        let (min_db, max_db) = series
            .iter()
            .flat_map(|s| s.y_values.iter())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        let y_min = if min_db.is_finite() {
            min_db - 5.0
        } else {
            -30.0
        };
        let y_max = if max_db.is_finite() {
            max_db + 5.0
        } else {
            10.0
        };

        let config = ChartConfig {
            y_label: Some("SPL (dB)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (y_min, y_max),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 250.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Render phase chart
    fn render_phase_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Process results with smoothing
        let series: Vec<Series> = results
            .iter()
            .map(|(name, idx, result)| {
                let smoothed =
                    Self::apply_smoothing(&result.frequencies, &result.phase_deg, smoothing);

                let freqs_f64: Vec<f64> = result.frequencies.iter().map(|&v| v as f64).collect();
                let phases_f64: Vec<f64> = smoothed.iter().map(|&v| v as f64).collect();

                Series::new(
                    name.clone(),
                    CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                    freqs_f64,
                    phases_f64,
                )
            })
            .collect();

        let config = ChartConfig {
            y_label: Some("Phase (degrees)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (-180.0, 180.0),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 200.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Render group delay chart
    fn render_group_delay_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Compute group delay for each result
        let mut series = Vec::new();

        for (name, idx, result) in results {
            let gd = dsp::compute_group_delay(&result.frequencies, &result.phase_deg);
            let smoothed_gd = Self::apply_smoothing(&result.frequencies, &gd, smoothing);

            let freqs_f64: Vec<f64> = result.frequencies.iter().map(|&v| v as f64).collect();
            let gd_f64: Vec<f64> = smoothed_gd.iter().map(|&v| v as f64).collect();

            series.push(Series::new(
                name.clone(),
                CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                freqs_f64.clone(),
                gd_f64,
            ));

            if let Some(excess) = &result.excess_group_delay_ms {
                let smoothed_excess = Self::apply_smoothing(&result.frequencies, excess, smoothing);
                let excess_f64: Vec<f64> = smoothed_excess.iter().map(|&v| v as f64).collect();

                // Add excess as semi-transparent series
                // Note: Labeling it might clutter legend, maybe skip label or append "(Excess)"?
                // gpui_px legend uses labels. If we want it hidden from legend we might need a feature in Shared chart or just empty label?
                // But let's keep it simple.
                series.push(
                    Series::new(
                        format!("{} (Excess)", name),
                        CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                        freqs_f64,
                        excess_f64,
                    )
                    .with_opacity(0.5)
                    .with_width(1.0),
                );
            }
        }

        // Find y-axis range
        let (min_gd, max_gd) = series
            .iter()
            .flat_map(|s| s.y_values.iter())
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        let y_min = if min_gd.is_finite() {
            (min_gd - 5.0).max(-50.0)
        } else {
            -20.0
        };
        let y_max = if max_gd.is_finite() {
            (max_gd + 5.0).min(50.0)
        } else {
            20.0
        };

        let config = ChartConfig {
            y_label: Some("Group Delay (ms)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (y_min, y_max),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 200.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Render distortion chart
    fn render_distortion_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Process results - show all THD data without filtering
        let series: Vec<Series> = results
            .iter()
            .filter_map(|(name, idx, result)| {
                let thd = result.thd_percent.as_ref()?;

                let smoothed = Self::apply_smoothing(&result.frequencies, thd, smoothing);

                let freqs_f64: Vec<f64> = result.frequencies.iter().map(|&v| v as f64).collect();
                let thd_f64: Vec<f64> = smoothed.iter().map(|&v| v as f64).collect();

                Some(Series::new(
                    name.clone(),
                    CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                    freqs_f64,
                    thd_f64,
                ))
            })
            .collect();

        if series.is_empty() {
            return div()
                .h(px(250.0))
                .w_full()
                .bg(theme.surface)
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Text::new("Distortion data not available (use Sweep signal)")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }

        // Y-axis: THD % (0 to max) with adaptive range
        let max_thd = series
            .iter()
            .flat_map(|s| s.y_values.iter())
            .fold(0.0_f64, |max, &v| max.max(v));

        let y_max = if max_thd > 1.0 {
            max_thd.ceil()
        } else if max_thd > 0.1 {
            1.0
        } else if max_thd > 0.01 {
            0.1
        } else if max_thd > 0.001 {
            0.01
        } else {
            0.001 // Show at least 0.001% range
        };

        let config = ChartConfig {
            y_label: Some("THD (%)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (0.0, y_max),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 250.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None).into_any_element()
    }

    /// Render RT60 chart
    fn render_rt60_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Assume 48kHz or get from config if possible (not passed here, using default)
        let sample_rate = 48000.0;

        let series: Vec<Series> = results
            .iter()
            .map(|(name, idx, result)| {
                let rt60 = result.rt60_ms.clone().or_else(|| {
                    result
                        .impulse_response
                        .as_ref()
                        .map(|ir| dsp::compute_rt60_spectrum(ir, sample_rate, &result.frequencies))
                });
                (name, idx, result, rt60)
            })
            .filter_map(|(name, idx, result, rt60_opt)| {
                rt60_opt.map(|rt60| {
                    let smoothed = Self::apply_smoothing(&result.frequencies, &rt60, smoothing);

                    let freqs_f64: Vec<f64> =
                        result.frequencies.iter().map(|&v| v as f64).collect();
                    let rt60_f64: Vec<f64> = smoothed.iter().map(|&v| v as f64).collect();

                    // Check if flat (broadband)
                    let min_val = rt60_f64.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                    let max_val = rt60_f64.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                    let label = if (max_val - min_val).abs() < 1e-3 {
                        format!("{} (Broadband)", name)
                    } else {
                        name.clone()
                    };

                    Series::new(
                        label,
                        CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                        freqs_f64,
                        rt60_f64,
                    )
                })
            })
            .collect();

        // Y-axis: RT60 in ms
        let max_rt60 = series
            .iter()
            .flat_map(|s| s.y_values.iter())
            .fold(0.0_f64, |max, &v| max.max(v));

        let y_max = ((max_rt60 / 100.0).ceil() * 100.0).max(500.0);

        let config = ChartConfig {
            y_label: Some("RT60 (ms)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (0.0, y_max),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 250.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Render Clarity chart
    fn render_clarity_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Assume 48kHz sample rate if missing
        let sample_rate = 48000.0;

        // Process C50 results
        let series: Vec<Series> = results
            .iter()
            .filter_map(|(name, idx, result)| {
                let c50 = result.clarity_c50_db.clone().or_else(|| {
                    result.impulse_response.as_ref().map(|ir| {
                        dsp::compute_clarity_spectrum(ir, sample_rate, &result.frequencies).0
                    })
                })?;

                let smoothed = Self::apply_smoothing(&result.frequencies, &c50, smoothing);

                let freqs_f64: Vec<f64> = result.frequencies.iter().map(|&v| v as f64).collect();
                let c50_f64: Vec<f64> = smoothed.iter().map(|&v| v as f64).collect();

                // Check if flat (broadband)
                let min_val = c50_f64.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max_val = c50_f64.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                let label = if (max_val - min_val).abs() < 0.1 {
                    format!("{} (Broadband)", name)
                } else {
                    name.clone()
                };

                Some(Series::new(
                    label,
                    CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                    freqs_f64,
                    c50_f64,
                ))
            })
            .collect();

        // Compute adaptive y-range from the data
        let (y_min, y_max) = if series.is_empty() {
            (-10.0, 20.0)
        } else {
            let data_min = series
                .iter()
                .flat_map(|s| s.y_values.iter())
                .fold(f64::INFINITY, |a, &b| a.min(b));
            let data_max = series
                .iter()
                .flat_map(|s| s.y_values.iter())
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            // Add some padding
            let range = (data_max - data_min).max(1.0);
            (data_min - range * 0.1, data_max + range * 0.1)
        };

        let config = ChartConfig {
            y_label: Some("Clarity C50 (dB)".to_string()),
            x_range: (20.0, 20000.0),
            y_range: (y_min, y_max),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 250.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Find the peak time in an impulse response
    fn find_ir_peak_time(times: &[f32], impulse: &[f32]) -> f32 {
        impulse
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.abs()
                    .partial_cmp(&b.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| times.get(i).copied().unwrap_or(0.0))
            .unwrap_or(0.0)
    }

    /// Render impulse response chart with first channel as timing reference
    #[allow(clippy::type_complexity)]
    fn render_impulse_response_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Assume 48kHz sample rate for impulse response computation
        let sample_rate = 48000.0_f32;

        // Compute impulse response and find peak time for each channel
        let channel_data: Vec<(String, usize, Vec<f32>, Vec<f32>, f32)> = results
            .iter()
            .map(|(name, idx, result)| {
                let (times, impulse) = dsp::compute_impulse_response_from_fr(
                    &result.frequencies,
                    &result.magnitude_db,
                    &result.phase_deg,
                    sample_rate,
                );
                let peak_time = Self::find_ir_peak_time(&times, &impulse);
                (name.clone(), *idx, times, impulse, peak_time)
            })
            .collect();

        // Use first channel's peak time as reference (t=0)
        // This shows propagation delay between speakers
        let reference_time = channel_data
            .first()
            .map(|(_, _, _, _, peak_time)| *peak_time)
            .unwrap_or(0.0);

        // Build series with time adjusted relative to first channel
        let series: Vec<Series> = channel_data
            .iter()
            .map(|(name, idx, times, impulse, peak_time)| {
                // Shift time so first channel's peak is at t=0, others show relative delay
                let time_offset = peak_time - reference_time;
                let adjusted_times: Vec<f64> =
                    times.iter().map(|&t| (t - time_offset) as f64).collect();
                let impulse_f64: Vec<f64> = impulse.iter().map(|&v| v as f64).collect();

                Series::new(
                    name.clone(),
                    CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()],
                    adjusted_times,
                    impulse_f64,
                )
                .with_width(1.5)
            })
            .collect();

        // Compute x-axis range to show all impulse responses
        // Center around 0 with some padding to show delays
        let (min_time, max_time) = series
            .iter()
            .flat_map(|s| s.x_values.iter())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        let x_min = if min_time.is_finite() {
            min_time.min(-2.0)
        } else {
            -2.0
        };
        let x_max = if max_time.is_finite() {
            max_time.clamp(10.0, 20.0)
        } else {
            10.0
        };

        let config = ChartConfig {
            x_label: Some("Time (ms)".to_string()),
            y_label: Some("Amplitude".to_string()),
            x_range: (x_min, x_max),
            y_range: (-1.0, 1.0),
            x_scale: ScaleType::Linear,
            width: 800.0,
            height: 200.0,
            ..Default::default()
        };

        render_line_chart(series, config, theme, None)
    }

    /// Render a summary of recorded channels
    fn render_channel_summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;

        let recorded_count = recording_state
            .channel_recordings
            .iter()
            .filter(|r| r.result.is_some())
            .count();
        let total_count = recording_state.channel_recordings.len();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("RECORDING SUMMARY")
                        .size(TextSize::Xs)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new("Channels Recorded")
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(format!("{} / {}", recorded_count, total_count))
                                        .size(TextSize::Md)
                                        .weight(TextWeight::Bold)
                                        .color(theme.text_primary),
                                ),
                        )
                        .child(div().w(px(1.0)).h(px(40.0)).bg(theme.border))
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new("Status")
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(if recorded_count == total_count {
                                        "All channels recorded"
                                    } else {
                                        "Some channels missing"
                                    })
                                    .size(TextSize::Xs)
                                    .weight(TextWeight::Semibold)
                                    .color(
                                        if recorded_count == total_count {
                                            theme.success
                                        } else {
                                            theme.warning
                                        },
                                    ),
                                ),
                        ),
                )
                .children(
                    recording_state
                        .channel_recordings
                        .iter()
                        .map(|rec| {
                            let has_result = rec.result.is_some();
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new(if has_result { "+" } else { "-" })
                                        .size(TextSize::Xs)
                                        .color(if has_result {
                                            theme.success
                                        } else {
                                            theme.text_muted
                                        }),
                                )
                                .child(
                                    Text::new(rec.channel_name.clone())
                                        .size(TextSize::Xs)
                                        .color(if has_result {
                                            theme.text_primary
                                        } else {
                                            theme.text_muted
                                        }),
                                )
                                .into_any_element()
                        })
                        .collect::<Vec<_>>(),
                ),
        )
    }
}
