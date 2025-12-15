//! Recording Evaluating Step (Step 3)
//!
//! View and analyze frequency response graphs from recordings.
//! Displays: Magnitude, Phase, Group Delay, and Impulse Response graphs.

use crate::app::types::{PlotSmoothing, RecordingResult};
use crate::ui::PlayerView;
use d3rs::axis::{AxisConfig, DefaultAxisTheme, render_axis};
use d3rs::color::D3Color;
use d3rs::grid::{GridConfig, render_grid};
use d3rs::prelude::LogScale;
use d3rs::scale::LinearScale;
use d3rs::shape::{LineConfig, LinePoint, render_line};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Card, HStack, Select, SelectOption, StackAlign, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

/// Channel colors for plotting
const CHANNEL_COLORS: [u32; 6] = [
    0x4285f4, // Blue
    0xea4335, // Red
    0x34a853, // Green
    0xfbbc04, // Yellow
    0x9c27b0, // Purple
    0x00bcd4, // Cyan
];

impl PlayerView {
    /// Render the evaluating step UI with frequency response graphs
    pub(crate) fn render_recording_evaluating_step(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Evaluate Recordings")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new("Review frequency response, phase, group delay, and impulse response measurements.")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    ),
            )
            .child(self.render_plot_controls(cx))
            .child(self.render_magnitude_plot(cx))
            .child(self.render_phase_plot(cx))
            .child(self.render_group_delay_plot(cx))
            .child(self.render_impulse_response_plot(cx))
            .child(self.render_channel_summary(cx))
    }

    /// Render plot controls (channel selector, smoothing)
    fn render_plot_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;

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
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Center)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Channel:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div().w(px(160.0)).child(
                                Select::new("plot_channel_select")
                                    .options(channel_options)
                                    .selected(selected_channel_value)
                                    .is_open(channel_dropdown_open)
                                    .on_toggle({
                                        let view = view.clone();
                                        move |open, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
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
                                                        .recording_state
                                                        .plot_selected_channel =
                                                        if value.as_ref() == "all" {
                                                            None
                                                        } else {
                                                            value.parse().ok()
                                                        };
                                                    state
                                                        .app
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
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Smoothing:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div().w(px(140.0)).child(
                                Select::new("plot_smoothing_select")
                                    .options(smoothing_options)
                                    .selected(selected_smoothing_value)
                                    .is_open(smoothing_dropdown_open)
                                    .on_toggle({
                                        let view = view.clone();
                                        move |open, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
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
                                                    state.app.recording_state.plot_smoothing =
                                                        match value.as_ref() {
                                                            "1" => PlotSmoothing::Octave1,
                                                            "3" => PlotSmoothing::Octave3,
                                                            "6" => PlotSmoothing::Octave6,
                                                            "24" => PlotSmoothing::Octave24,
                                                            _ => PlotSmoothing::None,
                                                        };
                                                    state
                                                        .app
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
        let recording_state = &state.app.recording_state;
        let selected_channel = recording_state.plot_selected_channel;

        recording_state
            .channel_recordings
            .iter()
            .enumerate()
            .filter_map(|(idx, r)| {
                if let Some(selected) = selected_channel {
                    if idx != selected {
                        return None;
                    }
                }
                r.result
                    .as_ref()
                    .map(|res| (r.channel_name.clone(), idx, res.clone()))
            })
            .collect()
    }

    /// Apply octave smoothing to frequency data
    fn apply_smoothing(frequencies: &[f32], values: &[f32], smoothing: PlotSmoothing) -> Vec<f32> {
        let octave_fraction = match smoothing.octave_fraction() {
            Some(f) => f,
            None => return values.to_vec(), // No smoothing
        };

        // Octave smoothing: for each frequency, average values within +/- half the octave bandwidth
        let half_octave_ratio = 2.0_f32.powf(octave_fraction / 2.0);

        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let center_freq = frequencies[i];
                let low_freq = center_freq / half_octave_ratio;
                let high_freq = center_freq * half_octave_ratio;

                let mut sum = 0.0_f32;
                let mut count = 0;

                for (j, &freq) in frequencies.iter().enumerate() {
                    if freq >= low_freq && freq <= high_freq {
                        sum += values[j];
                        count += 1;
                    }
                }

                if count > 0 {
                    sum / count as f32
                } else {
                    values[i]
                }
            })
            .collect()
    }

    /// Render magnitude plot
    fn render_magnitude_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let smoothing = state.app.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("MAGNITUDE (dB)")
                        .size(TextSize::Sm)
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
        let theme = state.app.theme.clone();
        let smoothing = state.app.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("PHASE (degrees)")
                        .size(TextSize::Sm)
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
        let theme = state.app.theme.clone();
        let smoothing = state.app.recording_state.plot_smoothing;

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("GROUP DELAY (ms)")
                        .size(TextSize::Sm)
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
        let theme = state.app.theme.clone();

        let results = self.get_filtered_results(cx);
        let has_results = !results.is_empty();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("IMPULSE RESPONSE")
                        .size(TextSize::Sm)
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
                    .size(TextSize::Md)
                    .weight(TextWeight::Semibold)
                    .color(theme.text_secondary),
            )
            .child(
                Text::new("Go back to the Capture step to record frequency responses")
                    .size(TextSize::Sm)
                    .color(theme.text_muted),
            )
    }

    /// Render magnitude chart
    fn render_magnitude_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 800.0;
        let chart_height: f32 = 250.0;
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

        // Compute normalization offset from first curve (100 Hz - 10 kHz mean)
        let normalization_offset = if let Some((_, _, first_result)) = results.first() {
            let mut sum = 0.0_f32;
            let mut count = 0;
            for (&freq, &mag) in first_result
                .frequencies
                .iter()
                .zip(first_result.magnitude_db.iter())
            {
                if freq >= 100.0 && freq <= 10000.0 {
                    sum += mag;
                    count += 1;
                }
            }
            if count > 0 { sum / count as f32 } else { 0.0 }
        } else {
            0.0
        };

        // Process results with normalization, inversion, and smoothing
        let processed_results: Vec<(String, usize, Vec<f32>, Vec<f32>)> = results
            .iter()
            .map(|(name, idx, result)| {
                let normalized: Vec<f32> = result
                    .magnitude_db
                    .iter()
                    .map(|&mag| -(mag - normalization_offset))
                    .collect();
                let smoothed = Self::apply_smoothing(&result.frequencies, &normalized, smoothing);
                (name.clone(), *idx, result.frequencies.clone(), smoothed)
            })
            .collect();

        // Find y-axis range
        let (min_db, max_db) = processed_results
            .iter()
            .flat_map(|(_, _, _, mag)| mag.iter())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &v| {
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

        let y_scale = LinearScale::new()
            .domain(y_min as f64, y_max as f64)
            .range(plot_height as f64, 0.0);

        let freq_ticks: Vec<f64> = vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        let mag_range = (y_max - y_min) as i32;
        let mag_step = if mag_range > 40 {
            10
        } else if mag_range > 20 {
            5
        } else {
            2
        };
        let mag_ticks: Vec<f64> = ((y_min as i32 / mag_step * mag_step)
            ..=(y_max as i32 / mag_step * mag_step + mag_step))
            .step_by(mag_step as usize)
            .map(|v| v as f64)
            .collect();

        // Build line elements
        let line_elements: Vec<_> = processed_results
            .iter()
            .map(|(_, idx, freqs, mags)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
                let points: Vec<LinePoint> = freqs
                    .iter()
                    .zip(mags.iter())
                    .filter(|&(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &m)| LinePoint {
                        x: f as f64,
                        y: m as f64,
                    })
                    .collect();
                let line_config = LineConfig::new().stroke_color(color).stroke_width(2.0);
                render_line(&x_scale, &y_scale, &points, &line_config).into_any_element()
            })
            .collect();

        // Build legend
        let legend_items: Vec<_> = processed_results
            .iter()
            .map(|(name, idx, _, _)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
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
            .child(div().flex().flex_wrap().gap_4().children(legend_items))
            .child(
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .bg(theme.surface)
                    .rounded_md()
                    .relative()
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
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
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
                            .children(line_elements),
                    )
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

    /// Render phase chart
    fn render_phase_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 800.0;
        let chart_height: f32 = 200.0;
        let margin_left: f32 = 60.0;
        let margin_right: f32 = 20.0;
        let margin_top: f32 = 20.0;
        let margin_bottom: f32 = 40.0;

        let plot_width = chart_width - margin_left - margin_right;
        let plot_height = chart_height - margin_top - margin_bottom;

        let axis_theme = DefaultAxisTheme;

        let x_scale = LogScale::new()
            .domain(20.0, 20000.0)
            .range(0.0, plot_width as f64);

        // Process results with smoothing
        let processed_results: Vec<(String, usize, Vec<f32>, Vec<f32>)> = results
            .iter()
            .map(|(name, idx, result)| {
                let smoothed =
                    Self::apply_smoothing(&result.frequencies, &result.phase_deg, smoothing);
                (name.clone(), *idx, result.frequencies.clone(), smoothed)
            })
            .collect();

        // Phase typically ranges from -180 to +180
        let y_scale = LinearScale::new()
            .domain(-180.0, 180.0)
            .range(plot_height as f64, 0.0);

        let freq_ticks: Vec<f64> = vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        let phase_ticks: Vec<f64> = vec![-180.0, -90.0, 0.0, 90.0, 180.0];

        let line_elements: Vec<_> = processed_results
            .iter()
            .map(|(_, idx, freqs, phases)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
                let points: Vec<LinePoint> = freqs
                    .iter()
                    .zip(phases.iter())
                    .filter(|&(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &p)| LinePoint {
                        x: f as f64,
                        y: p as f64,
                    })
                    .collect();
                let line_config = LineConfig::new().stroke_color(color).stroke_width(2.0);
                render_line(&x_scale, &y_scale, &points, &line_config).into_any_element()
            })
            .collect();

        let legend_items: Vec<_> = processed_results
            .iter()
            .map(|(name, idx, _, _)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
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
            .child(div().flex().flex_wrap().gap_4().children(legend_items))
            .child(
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .bg(theme.surface)
                    .rounded_md()
                    .relative()
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
                                    .with_tick_values(phase_ticks.clone())
                                    .with_formatter(|v| format!("{:.0}°", v)),
                                plot_height,
                                &axis_theme,
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
                            .child(render_grid(
                                &x_scale,
                                &y_scale,
                                &GridConfig::with_lines()
                                    .with_vertical_values(freq_ticks.clone())
                                    .with_horizontal_values(phase_ticks),
                                plot_width,
                                plot_height,
                                &axis_theme,
                            ))
                            .children(line_elements),
                    )
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

    /// Compute group delay from phase data
    /// Group delay = -d(phase)/d(frequency) / (2*pi)
    fn compute_group_delay(frequencies: &[f32], phase_deg: &[f32]) -> Vec<f32> {
        if frequencies.len() < 2 {
            return vec![0.0; frequencies.len()];
        }

        let mut group_delay_ms = Vec::with_capacity(frequencies.len());

        for i in 0..frequencies.len() {
            let delay = if i == 0 {
                // Forward difference at start
                let df = frequencies[1] - frequencies[0];
                let dp = phase_deg[1] - phase_deg[0];
                if df.abs() > 1e-6 {
                    -dp / df / 360.0 * 1000.0 // Convert to ms
                } else {
                    0.0
                }
            } else if i == frequencies.len() - 1 {
                // Backward difference at end
                let df = frequencies[i] - frequencies[i - 1];
                let dp = phase_deg[i] - phase_deg[i - 1];
                if df.abs() > 1e-6 {
                    -dp / df / 360.0 * 1000.0
                } else {
                    0.0
                }
            } else {
                // Central difference
                let df = frequencies[i + 1] - frequencies[i - 1];
                let dp = phase_deg[i + 1] - phase_deg[i - 1];
                if df.abs() > 1e-6 {
                    -dp / df / 360.0 * 1000.0
                } else {
                    0.0
                }
            };
            group_delay_ms.push(delay);
        }

        group_delay_ms
    }

    /// Render group delay chart
    fn render_group_delay_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        smoothing: PlotSmoothing,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 800.0;
        let chart_height: f32 = 200.0;
        let margin_left: f32 = 60.0;
        let margin_right: f32 = 20.0;
        let margin_top: f32 = 20.0;
        let margin_bottom: f32 = 40.0;

        let plot_width = chart_width - margin_left - margin_right;
        let plot_height = chart_height - margin_top - margin_bottom;

        let axis_theme = DefaultAxisTheme;

        let x_scale = LogScale::new()
            .domain(20.0, 20000.0)
            .range(0.0, plot_width as f64);

        // Compute group delay for each result
        let processed_results: Vec<(String, usize, Vec<f32>, Vec<f32>)> = results
            .iter()
            .map(|(name, idx, result)| {
                let gd = Self::compute_group_delay(&result.frequencies, &result.phase_deg);
                let smoothed = Self::apply_smoothing(&result.frequencies, &gd, smoothing);
                (name.clone(), *idx, result.frequencies.clone(), smoothed)
            })
            .collect();

        // Find y-axis range
        let (min_gd, max_gd) = processed_results
            .iter()
            .flat_map(|(_, _, _, gd)| gd.iter())
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &v| {
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

        let y_scale = LinearScale::new()
            .domain(y_min as f64, y_max as f64)
            .range(plot_height as f64, 0.0);

        let freq_ticks: Vec<f64> = vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        let gd_range = (y_max - y_min) as i32;
        let gd_step = if gd_range > 40 {
            10
        } else if gd_range > 20 {
            5
        } else {
            2
        };
        let gd_ticks: Vec<f64> = ((y_min as i32 / gd_step * gd_step)
            ..=(y_max as i32 / gd_step * gd_step + gd_step))
            .step_by(gd_step as usize)
            .map(|v| v as f64)
            .collect();

        let line_elements: Vec<_> = processed_results
            .iter()
            .map(|(_, idx, freqs, gd)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
                let points: Vec<LinePoint> = freqs
                    .iter()
                    .zip(gd.iter())
                    .filter(|&(&f, &g)| f >= 20.0 && f <= 20000.0 && g.is_finite())
                    .map(|(&f, &g)| LinePoint {
                        x: f as f64,
                        y: g as f64,
                    })
                    .collect();
                let line_config = LineConfig::new().stroke_color(color).stroke_width(2.0);
                render_line(&x_scale, &y_scale, &points, &line_config).into_any_element()
            })
            .collect();

        let legend_items: Vec<_> = processed_results
            .iter()
            .map(|(name, idx, _, _)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
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
            .child(div().flex().flex_wrap().gap_4().children(legend_items))
            .child(
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .bg(theme.surface)
                    .rounded_md()
                    .relative()
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
                                    .with_tick_values(gd_ticks.clone())
                                    .with_formatter(|v| format!("{:.1} ms", v)),
                                plot_height,
                                &axis_theme,
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
                            .child(render_grid(
                                &x_scale,
                                &y_scale,
                                &GridConfig::with_lines()
                                    .with_vertical_values(freq_ticks.clone())
                                    .with_horizontal_values(gd_ticks),
                                plot_width,
                                plot_height,
                                &axis_theme,
                            ))
                            .children(line_elements),
                    )
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

    /// Compute impulse response from frequency response using inverse FFT
    fn compute_impulse_response(
        frequencies: &[f32],
        magnitude_db: &[f32],
        phase_deg: &[f32],
        sample_rate: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        use std::f32::consts::PI;

        // For a simple approximation, we'll create a synthetic impulse response
        // In a real implementation, this would use inverse FFT
        let num_samples = 512;
        let time_step = 1.0 / sample_rate;

        let mut impulse = vec![0.0_f32; num_samples];
        let times: Vec<f32> = (0..num_samples)
            .map(|i| i as f32 * time_step * 1000.0) // Convert to ms
            .collect();

        // Simple approximation: sum of sinusoids weighted by magnitude
        // This is not a true inverse FFT but gives a reasonable visualization
        for (i, time) in times.iter().enumerate() {
            let t_sec = time / 1000.0;
            for (j, (&freq, &mag_db)) in frequencies.iter().zip(magnitude_db.iter()).enumerate() {
                if freq > 0.0 && freq < sample_rate / 2.0 {
                    let mag_linear = 10.0_f32.powf(mag_db / 20.0);
                    let phase_rad = phase_deg[j] * PI / 180.0;
                    impulse[i] += mag_linear * (2.0 * PI * freq * t_sec + phase_rad).cos();
                }
            }
        }

        // Normalize
        let max_val = impulse.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        if max_val > 0.0 {
            for v in &mut impulse {
                *v /= max_val;
            }
        }

        (times, impulse)
    }

    /// Render impulse response chart
    fn render_impulse_response_chart(
        &self,
        results: &[(String, usize, RecordingResult)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 800.0;
        let chart_height: f32 = 200.0;
        let margin_left: f32 = 60.0;
        let margin_right: f32 = 20.0;
        let margin_top: f32 = 20.0;
        let margin_bottom: f32 = 40.0;

        let plot_width = chart_width - margin_left - margin_right;
        let plot_height = chart_height - margin_top - margin_bottom;

        let axis_theme = DefaultAxisTheme;

        // Assume 48kHz sample rate for impulse response computation
        let sample_rate = 48000.0_f32;

        // Compute impulse responses
        let processed_results: Vec<(String, usize, Vec<f32>, Vec<f32>)> = results
            .iter()
            .map(|(name, idx, result)| {
                let (times, impulse) = Self::compute_impulse_response(
                    &result.frequencies,
                    &result.magnitude_db,
                    &result.phase_deg,
                    sample_rate,
                );
                (name.clone(), *idx, times, impulse)
            })
            .collect();

        // X-axis: time in ms (first 10ms)
        let x_scale = LinearScale::new()
            .domain(0.0, 10.0)
            .range(0.0, plot_width as f64);

        // Y-axis: normalized amplitude
        let y_scale = LinearScale::new()
            .domain(-1.0, 1.0)
            .range(plot_height as f64, 0.0);

        let time_ticks: Vec<f64> = vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0];
        let amp_ticks: Vec<f64> = vec![-1.0, -0.5, 0.0, 0.5, 1.0];

        let line_elements: Vec<_> = processed_results
            .iter()
            .map(|(_, idx, times, impulse)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
                let points: Vec<LinePoint> = times
                    .iter()
                    .zip(impulse.iter())
                    .filter(|&(&t, _)| t >= 0.0 && t <= 10.0)
                    .map(|(&t, &a)| LinePoint {
                        x: t as f64,
                        y: a as f64,
                    })
                    .collect();
                let line_config = LineConfig::new().stroke_color(color).stroke_width(1.5);
                render_line(&x_scale, &y_scale, &points, &line_config).into_any_element()
            })
            .collect();

        let legend_items: Vec<_> = processed_results
            .iter()
            .map(|(name, idx, _, _)| {
                let color = D3Color::from_hex(CHANNEL_COLORS[*idx % CHANNEL_COLORS.len()]);
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
            .child(div().flex().flex_wrap().gap_4().children(legend_items))
            .child(
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .bg(theme.surface)
                    .rounded_md()
                    .relative()
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
                                    .with_tick_values(amp_ticks.clone())
                                    .with_formatter(|v| format!("{:.1}", v)),
                                plot_height,
                                &axis_theme,
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
                            .child(render_grid(
                                &x_scale,
                                &y_scale,
                                &GridConfig::with_lines()
                                    .with_vertical_values(time_ticks.clone())
                                    .with_horizontal_values(amp_ticks),
                                plot_width,
                                plot_height,
                                &axis_theme,
                            ))
                            .children(line_elements),
                    )
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
                                    .with_tick_values(time_ticks)
                                    .with_formatter(|t| format!("{:.0} ms", t)),
                                plot_width,
                                &axis_theme,
                            )),
                    ),
            )
    }

    /// Render a summary of recorded channels
    fn render_channel_summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;

        let recorded_count = recording_state
            .channel_recordings
            .iter()
            .filter(|r| r.result.is_some())
            .count();
        let total_count = recording_state.channel_recordings.len();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("RECORDING SUMMARY")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .align(StackAlign::Center)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new("Channels Recorded")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(format!("{} / {}", recorded_count, total_count))
                                        .size(TextSize::Lg)
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
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(if recorded_count == total_count {
                                        "All channels recorded"
                                    } else {
                                        "Some channels missing"
                                    })
                                    .size(TextSize::Sm)
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
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new(if has_result { "+" } else { "-" })
                                        .size(TextSize::Sm)
                                        .color(if has_result {
                                            theme.success
                                        } else {
                                            theme.text_muted
                                        }),
                                )
                                .child(
                                    Text::new(rec.channel_name.clone())
                                        .size(TextSize::Sm)
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
