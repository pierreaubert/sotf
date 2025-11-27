//! Label management for frequency/SPL graphs
//!
//! Provides formatting and rendering of axis labels.

use super::axis::{FrequencyAxis, SplAxis};
use crate::theme::Theme;
use gpui::*;

/// Label configuration for graph axes
#[derive(Debug, Clone)]
pub struct LabelConfig {
    /// Text size in pixels
    pub font_size: f32,
    /// Padding from axis edge
    pub padding: f32,
    /// Whether to show units (Hz, dB)
    pub show_units: bool,
}

impl Default for LabelConfig {
    fn default() -> Self {
        Self {
            font_size: 10.0,
            padding: 4.0,
            show_units: true,
        }
    }
}

/// Format frequency value for display
/// Examples: 20 -> "20", 100 -> "100", 1000 -> "1k", 2000 -> "2k", 10000 -> "10k", 20000 -> "20k"
pub fn format_frequency(freq: f64) -> String {
    if freq >= 1000.0 {
        let k = freq / 1000.0;
        if k.fract() < 0.001 {
            format!("{}k", k as i32)
        } else {
            format!("{:.1}k", k)
        }
    } else {
        format!("{:.0}", freq)
    }
}

/// Format frequency with Hz unit
pub fn format_frequency_with_unit(freq: f64) -> String {
    if freq >= 1000.0 {
        let k = freq / 1000.0;
        if k.fract() < 0.001 {
            format!("{}kHz", k as i32)
        } else {
            format!("{:.1}kHz", k)
        }
    } else {
        format!("{:.0}Hz", freq)
    }
}

/// Format dB value for display
pub fn format_db(db: f64) -> String {
    if db > 0.0 {
        format!("+{:.0}", db)
    } else if db < 0.0 {
        format!("{:.0}", db)
    } else {
        "0".to_string()
    }
}

/// Format dB with unit
pub fn format_db_with_unit(db: f64) -> String {
    if db > 0.0 {
        format!("+{:.0}dB", db)
    } else if db < 0.0 {
        format!("{:.0}dB", db)
    } else {
        "0dB".to_string()
    }
}

/// Render horizontal frequency labels below the graph
pub fn render_freq_labels_horizontal(
    freq_axis: &FrequencyAxis,
    config: &LabelConfig,
    theme: &Theme,
) -> impl IntoElement {
    let tick_freqs = freq_axis.tick_frequencies();

    div()
        .w_full()
        .h(px(config.font_size + config.padding * 2.0))
        .relative()
        .children(tick_freqs.iter().map(|&freq| {
            let pos = freq_axis.freq_to_normalized(freq);
            let label = if config.show_units && (freq == 20.0 || freq == 20000.0) {
                format_frequency_with_unit(freq)
            } else {
                format_frequency(freq)
            };

            div()
                .absolute()
                .left(relative(pos as f32))
                .top(px(config.padding))
                .text_xs()
                .text_color(theme.text_muted)
                .child(label)
        }))
}

/// Render vertical dB labels to the left of the graph
pub fn render_db_labels_vertical(
    spl_axis: &SplAxis,
    config: &LabelConfig,
    theme: &Theme,
) -> impl IntoElement {
    let tick_values = spl_axis.tick_values();

    div()
        .h_full()
        .w(px(32.0))
        .relative()
        .children(tick_values.iter().map(|&db| {
            let pos = spl_axis.db_to_normalized(db);
            let label = if config.show_units && db == 0.0 {
                format_db_with_unit(db)
            } else {
                format_db(db)
            };

            div()
                .absolute()
                .right(px(config.padding))
                .top(relative(pos as f32))
                .text_xs()
                .text_color(theme.text_muted)
                .child(label)
        }))
}

/// Get the width needed for dB labels based on range
pub fn db_label_width(spl_axis: &SplAxis) -> f32 {
    // Calculate max width based on largest label
    let max_val = spl_axis.max_db.abs().max(spl_axis.min_db.abs());
    if max_val >= 100.0 {
        40.0 // "+100dB"
    } else if max_val >= 10.0 {
        32.0 // "+24dB"
    } else {
        28.0 // "+9dB"
    }
}

/// Get the height needed for frequency labels
pub fn freq_label_height(_freq_axis: &FrequencyAxis) -> f32 {
    20.0 // Standard height for frequency labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_frequency() {
        assert_eq!(format_frequency(20.0), "20");
        assert_eq!(format_frequency(50.0), "50");
        assert_eq!(format_frequency(100.0), "100");
        assert_eq!(format_frequency(200.0), "200");
        assert_eq!(format_frequency(500.0), "500");
        assert_eq!(format_frequency(1000.0), "1k");
        assert_eq!(format_frequency(2000.0), "2k");
        assert_eq!(format_frequency(5000.0), "5k");
        assert_eq!(format_frequency(10000.0), "10k");
        assert_eq!(format_frequency(20000.0), "20k");
    }

    #[test]
    fn test_format_db() {
        assert_eq!(format_db(24.0), "+24");
        assert_eq!(format_db(12.0), "+12");
        assert_eq!(format_db(0.0), "0");
        assert_eq!(format_db(-12.0), "-12");
        assert_eq!(format_db(-24.0), "-24");
    }
}
