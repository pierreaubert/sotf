//! EQ Frequency Response Graph Module
//!
//! A graphing library for frequency/SPL visualizations using gpui-px:
//! - Logarithmic frequency axis (20Hz - 20kHz)
//! - Linear SPL axis (configurable dB range)
//! - Built-in grid and axis rendering
//! - Configurable legend
//! - Aspect ratio preservation

pub mod common;

pub mod headphone_graphs;

pub mod response_graphs;

pub mod speaker_graphs;

pub mod spectrum_graphs;

/// Format a frequency value for display (e.g., "1.5 kHz", "200 Hz")
pub fn format_frequency(freq: f64) -> String {
    if freq >= 1000.0 {
        let khz = freq / 1000.0;
        if khz == khz.floor() {
            format!("{:.0} kHz", khz)
        } else {
            format!("{:.1} kHz", khz)
        }
    } else if freq == freq.floor() {
        format!("{:.0} Hz", freq)
    } else {
        format!("{:.1} Hz", freq)
    }
}
