use crate::params::BAND_TEMPLATE as EQ;
use sotf_host::param_specs::find_by_key as pk;

/// Sample rate for filter calculations
pub const SAMPLE_RATE: f64 = sotf_host::DEFAULT_PREVIEW_SAMPLE_RATE;

/// Q handle bar constants
pub const Q_BAR_MIN_WIDTH: f32 = 40.0;

pub const Q_BAR_MAX_WIDTH: f32 = 100.0;

pub(super) const Q_HANDLE_RADIUS: f32 = 5.0;

pub(super) const Q_BAR_HEIGHT: f32 = 3.0;

/// Convert Q value to bar width (inverse: higher Q = narrower bar)
pub fn q_to_bar_width(q: f64) -> f32 {
    let t = ((q - pk(EQ, "q").min_f64()) / (pk(EQ, "q").max_f64() - pk(EQ, "q").min_f64()))
        .clamp(0.0, 1.0) as f32;
    // Inverse mapping: pk(EQ, "q").min_f64() -> max width, pk(EQ, "q").max_f64() -> min width
    Q_BAR_MAX_WIDTH - t * (Q_BAR_MAX_WIDTH - Q_BAR_MIN_WIDTH)
}

/// Band colors for EQ visualization
pub(super) const BAND_COLORS: [u32; 10] = [
    0xef4444, // Red
    0xf97316, // Orange
    0xeab308, // Yellow
    0x22c55e, // Green
    0x14b8a6, // Teal
    0x3b82f6, // Blue
    0x8b5cf6, // Violet
    0xec4899, // Pink
    0x6366f1, // Indigo
    0x06b6d4, // Cyan
];

/// Chart layout constants for control point positioning
/// These MUST match gpui-px line chart margins (see gpui-px/src/line.rs)
/// Left margin = Y-axis total_size() = 60 (base) + 20 (title: font_size 12 + padding 8)
pub const CHART_LEFT_MARGIN: f32 = 80.0; // Y-axis rendered width when y_label is set

pub const CHART_RIGHT_MARGIN: f32 = 20.0; // gpui-px margin_right (no secondary axis)

pub const CHART_TOP_MARGIN: f32 = 0.0;

pub const CHART_BOTTOM_MARGIN: f32 = 30.0;

pub const CHART_HEIGHT: f32 = 300.0;

pub const GPUI_PX_MARGIN_TOP: f32 = 10.0;

pub const MIN_FREQ: f64 = sotf_host::AUDIBLE_MIN_FREQ;

pub const MAX_FREQ: f64 = sotf_host::AUDIBLE_MAX_FREQ;

pub(super) const CONTROL_POINT_RADIUS: f32 = 8.0;

/// Convert frequency (Hz) to x pixel position
pub fn freq_to_x(freq: f64, plot_width: f32) -> f32 {
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    let t = (freq.ln() - log_min) / (log_max - log_min);
    CHART_LEFT_MARGIN + (t as f32) * plot_width
}

/// Convert x pixel position to frequency (Hz)
pub fn x_to_freq(x: f32, plot_width: f32) -> f64 {
    let t = ((x - CHART_LEFT_MARGIN) / plot_width).clamp(0.0, 1.0) as f64;
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    (log_min + t * (log_max - log_min)).exp()
}

/// Convert gain (dB) to y pixel position with dynamic range
pub fn gain_to_y(gain_db: f64, min_db: f64, max_db: f64) -> f32 {
    // gpui-px calculates plot_height = height - margin_top(10) - margin_bottom(30)
    // but renders the plot starting at y=0 (no actual top margin offset)
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = (max_db - gain_db) / (max_db - min_db);
    CHART_TOP_MARGIN + (t as f32) * plot_height
}

/// Convert y pixel position to gain (dB) with dynamic range
pub fn y_to_gain(y: f32, min_db: f64, max_db: f64) -> f64 {
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = ((y - CHART_TOP_MARGIN) / plot_height).clamp(0.0, 1.0) as f64;
    max_db - t * (max_db - min_db)
}
