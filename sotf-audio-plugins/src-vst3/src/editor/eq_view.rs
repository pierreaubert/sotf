//! EQ Visualization View
//!
//! egui-based EQ visualization with frequency response graph and band controls.

use crate::eq_params::{FilterType, SotfEqParams, NUM_BANDS};
use autoeq_iir::{Biquad, BiquadFilterType};
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use std::sync::Arc;

/// Sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Number of frequency points for response curve
const NUM_FREQ_POINTS: usize = 256;

/// Band colors (matching the GPUI version)
const BAND_COLORS: [Color32; 4] = [
    Color32::from_rgb(239, 68, 68),   // Red
    Color32::from_rgb(249, 115, 22),  // Orange
    Color32::from_rgb(34, 197, 94),   // Green
    Color32::from_rgb(59, 130, 246),  // Blue
];

/// EQ Editor state
pub struct EqEditorState {
    params: Arc<SotfEqParams>,
    selected_band: usize,
    /// Cached frequency points (log scale from 20Hz to 20kHz)
    freq_points: Vec<f64>,
}

impl EqEditorState {
    pub fn new(params: Arc<SotfEqParams>) -> Self {
        // Pre-compute logarithmically spaced frequency points
        let min_freq = 20.0_f64;
        let max_freq = 20000.0_f64;
        let freq_points: Vec<f64> = (0..NUM_FREQ_POINTS)
            .map(|i| {
                let t = i as f64 / (NUM_FREQ_POINTS - 1) as f64;
                let log_min = min_freq.ln();
                let log_max = max_freq.ln();
                (log_min + t * (log_max - log_min)).exp()
            })
            .collect();

        Self {
            params,
            selected_band: 0,
            freq_points,
        }
    }

    /// Main UI rendering
    pub fn ui(&mut self, ctx: &egui::Context, setter: &ParamSetter) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.heading("SOTF EQ");
                    ui.separator();
                    ui.label("4-Band Parametric Equalizer");
                });

                ui.add_space(10.0);

                // Frequency response graph
                self.draw_frequency_response(ui);

                ui.add_space(10.0);

                // Band selector and controls
                ui.horizontal(|ui| {
                    // Band tabs
                    for band_idx in 0..NUM_BANDS {
                        let is_selected = self.selected_band == band_idx;
                        let color = BAND_COLORS[band_idx];

                        let button = egui::Button::new(format!("Band {}", band_idx + 1))
                            .fill(if is_selected {
                                color
                            } else {
                                Color32::from_rgb(50, 50, 55)
                            });

                        if ui.add(button).clicked() {
                            self.selected_band = band_idx;
                        }
                    }

                    ui.separator();

                    // Output gain control
                    ui.label("Output:");
                    let mut output_gain = self.params.output_gain.value();
                    if ui
                        .add(
                            egui::DragValue::new(&mut output_gain)
                                .range(-24.0..=24.0)
                                .speed(0.1)
                                .suffix(" dB"),
                        )
                        .changed()
                    {
                        setter.set_parameter(&self.params.output_gain, output_gain);
                    }
                });

                ui.add_space(10.0);

                // Selected band controls
                self.draw_band_controls(ui, setter);
            });
        });
    }

    /// Draw the frequency response graph
    fn draw_frequency_response(&self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let graph_height = 250.0;

        let (response, painter) =
            ui.allocate_painter(Vec2::new(available_width, graph_height), egui::Sense::hover());
        let rect = response.rect;

        // Draw background
        painter.rect_filled(rect, 5.0, Color32::from_rgb(20, 20, 25));

        // Draw grid
        self.draw_grid(&painter, rect);

        // Calculate and draw response curves
        self.draw_response_curves(&painter, rect);
    }

    /// Draw the frequency/gain grid
    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let grid_color = Color32::from_rgb(50, 50, 55);
        let text_color = Color32::from_rgb(120, 120, 130);

        // Frequency grid lines (logarithmic)
        let freq_labels = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
        for freq in freq_labels {
            let x = self.freq_to_x(freq, rect);
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, grid_color),
            );

            // Label
            let label = if freq >= 1000.0 {
                format!("{}k", (freq / 1000.0) as i32)
            } else {
                format!("{}", freq as i32)
            };
            painter.text(
                Pos2::new(x, rect.bottom() - 15.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                text_color,
            );
        }

        // Gain grid lines
        let gain_labels = [-24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0, 24.0];
        for gain in gain_labels {
            let y = self.gain_to_y(gain, rect);
            let stroke = if gain == 0.0 {
                Stroke::new(1.5, Color32::from_rgb(80, 80, 85))
            } else {
                Stroke::new(1.0, grid_color)
            };
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                stroke,
            );

            // Label
            if gain.abs() < 0.1 || gain.abs() > 20.0 {
                painter.text(
                    Pos2::new(rect.left() + 20.0, y),
                    egui::Align2::LEFT_CENTER,
                    format!("{:+.0} dB", gain),
                    egui::FontId::proportional(10.0),
                    text_color,
                );
            }
        }
    }

    /// Draw the frequency response curves
    fn draw_response_curves(&self, painter: &egui::Painter, rect: Rect) {
        // Calculate individual band responses
        let mut band_responses: Vec<Vec<f64>> = Vec::with_capacity(NUM_BANDS);
        let mut combined_response: Vec<f64> = vec![0.0; NUM_FREQ_POINTS];

        for band_idx in 0..NUM_BANDS {
            let band = self.params.band(band_idx);
            let enabled = band.enabled.value();

            let response: Vec<f64> = self
                .freq_points
                .iter()
                .map(|&freq| {
                    if enabled {
                        let biquad = Biquad::new(
                            convert_filter_type(band.filter_type.value()),
                            band.frequency.value() as f64,
                            SAMPLE_RATE,
                            band.q.value() as f64,
                            band.gain_db.value() as f64,
                        );
                        biquad.log_result(freq)
                    } else {
                        0.0
                    }
                })
                .collect();

            // Add to combined response
            for (i, &r) in response.iter().enumerate() {
                combined_response[i] += r;
            }

            band_responses.push(response);
        }

        // Draw individual band curves (semi-transparent)
        for (band_idx, response) in band_responses.iter().enumerate() {
            let band = self.params.band(band_idx);
            if !band.enabled.value() {
                continue;
            }

            let color = BAND_COLORS[band_idx];
            let alpha = if band_idx == self.selected_band {
                180
            } else {
                80
            };
            let stroke_width = if band_idx == self.selected_band {
                2.0
            } else {
                1.5
            };

            let points: Vec<Pos2> = self
                .freq_points
                .iter()
                .zip(response.iter())
                .map(|(&freq, &gain)| {
                    Pos2::new(self.freq_to_x(freq, rect), self.gain_to_y(gain, rect))
                })
                .collect();

            for window in points.windows(2) {
                painter.line_segment(
                    [window[0], window[1]],
                    Stroke::new(stroke_width, color.gamma_multiply(alpha as f32 / 255.0)),
                );
            }
        }

        // Draw combined response curve (white, on top)
        let combined_points: Vec<Pos2> = self
            .freq_points
            .iter()
            .zip(combined_response.iter())
            .map(|(&freq, &gain)| {
                Pos2::new(self.freq_to_x(freq, rect), self.gain_to_y(gain, rect))
            })
            .collect();

        for window in combined_points.windows(2) {
            painter.line_segment(
                [window[0], window[1]],
                Stroke::new(2.5, Color32::from_rgb(200, 200, 210)),
            );
        }

        // Draw band center frequency markers
        for band_idx in 0..NUM_BANDS {
            let band = self.params.band(band_idx);
            if !band.enabled.value() {
                continue;
            }

            let freq = band.frequency.value() as f64;
            let x = self.freq_to_x(freq, rect);
            let y = self.gain_to_y(band.gain_db.value() as f64, rect);

            let color = BAND_COLORS[band_idx];
            let radius = if band_idx == self.selected_band {
                8.0
            } else {
                5.0
            };

            painter.circle_filled(Pos2::new(x, y), radius, color);
            painter.circle_stroke(
                Pos2::new(x, y),
                radius,
                Stroke::new(2.0, Color32::WHITE),
            );
        }
    }

    /// Draw controls for the selected band
    fn draw_band_controls(&mut self, ui: &mut egui::Ui, setter: &ParamSetter) {
        let band = self.params.band(self.selected_band);
        let color = BAND_COLORS[self.selected_band];

        egui::Frame::new()
            .fill(Color32::from_rgb(35, 35, 40))
            .corner_radius(8.0)
            .inner_margin(15.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Enabled toggle
                    let mut enabled = band.enabled.value();
                    if ui
                        .add(egui::Checkbox::new(&mut enabled, "Enabled"))
                        .changed()
                    {
                        setter.set_parameter(&band.enabled, enabled);
                    }

                    ui.separator();

                    // Filter type selector
                    ui.label("Type:");
                    let current_type = band.filter_type.value();
                    egui::ComboBox::from_id_salt(format!("filter_type_{}", self.selected_band))
                        .selected_text(filter_type_name(current_type))
                        .show_ui(ui, |ui| {
                            for filter_type in [
                                FilterType::Peak,
                                FilterType::LowShelf,
                                FilterType::HighShelf,
                                FilterType::LowPass,
                                FilterType::HighPass,
                            ] {
                                if ui
                                    .selectable_label(
                                        current_type == filter_type,
                                        filter_type_name(filter_type),
                                    )
                                    .clicked()
                                {
                                    setter.set_parameter(&band.filter_type, filter_type);
                                }
                            }
                        });
                });

                ui.add_space(10.0);

                // Parameter knobs row
                ui.horizontal(|ui| {
                    // Frequency
                    ui.vertical(|ui| {
                        ui.label("Frequency");
                        let mut freq = band.frequency.value();
                        let speed = freq * 0.01; // Compute before borrowing
                        if ui
                            .add(
                                egui::DragValue::new(&mut freq)
                                    .range(20.0..=20000.0)
                                    .speed(speed)
                                    .custom_formatter(|v, _| {
                                        if v >= 1000.0 {
                                            format!("{:.1} kHz", v / 1000.0)
                                        } else {
                                            format!("{:.0} Hz", v)
                                        }
                                    }),
                            )
                            .changed()
                        {
                            setter.set_parameter(&band.frequency, freq);
                        }
                    });

                    ui.add_space(20.0);

                    // Q
                    ui.vertical(|ui| {
                        ui.label("Q");
                        let mut q = band.q.value();
                        if ui
                            .add(
                                egui::DragValue::new(&mut q)
                                    .range(0.1..=10.0)
                                    .speed(0.01)
                                    .max_decimals(2),
                            )
                            .changed()
                        {
                            setter.set_parameter(&band.q, q);
                        }
                    });

                    ui.add_space(20.0);

                    // Gain
                    ui.vertical(|ui| {
                        ui.label("Gain");
                        let mut gain = band.gain_db.value();
                        if ui
                            .add(
                                egui::DragValue::new(&mut gain)
                                    .range(-24.0..=24.0)
                                    .speed(0.1)
                                    .suffix(" dB")
                                    .max_decimals(1),
                            )
                            .changed()
                        {
                            setter.set_parameter(&band.gain_db, gain);
                        }
                    });

                    // Visual indicator
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        let (_, painter) = ui.allocate_painter(Vec2::new(20.0, 20.0), egui::Sense::hover());
                        painter.circle_filled(
                            painter.clip_rect().center(),
                            8.0,
                            color,
                        );
                    });
                });
            });
    }

    /// Convert frequency to X coordinate (logarithmic scale)
    fn freq_to_x(&self, freq: f64, rect: Rect) -> f32 {
        let min_freq = 20.0_f64;
        let max_freq = 20000.0_f64;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();
        let t = (freq.ln() - log_min) / (log_max - log_min);
        rect.left() + (t as f32) * rect.width()
    }

    /// Convert gain to Y coordinate
    fn gain_to_y(&self, gain: f64, rect: Rect) -> f32 {
        let min_gain = -24.0;
        let max_gain = 24.0;
        let t = (gain - max_gain) / (min_gain - max_gain);
        rect.top() + (t as f32) * rect.height()
    }
}

/// Convert our FilterType to autoeq_iir's BiquadFilterType
fn convert_filter_type(filter_type: FilterType) -> BiquadFilterType {
    match filter_type {
        FilterType::Peak => BiquadFilterType::Peak,
        FilterType::LowShelf => BiquadFilterType::Lowshelf,
        FilterType::HighShelf => BiquadFilterType::Highshelf,
        FilterType::LowPass => BiquadFilterType::Lowpass,
        FilterType::HighPass => BiquadFilterType::Highpass,
    }
}

/// Get display name for filter type
fn filter_type_name(filter_type: FilterType) -> &'static str {
    match filter_type {
        FilterType::Peak => "Peak",
        FilterType::LowShelf => "Low Shelf",
        FilterType::HighShelf => "High Shelf",
        FilterType::LowPass => "Low Pass",
        FilterType::HighPass => "High Pass",
    }
}
