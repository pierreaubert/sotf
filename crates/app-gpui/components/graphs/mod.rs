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
#[must_use]
#[expect(
    clippy::float_cmp,
    reason = "Exact integral values select the compact axis-label format"
)]
pub fn format_frequency(freq: f64) -> String {
    if freq >= 1000.0 {
        let khz = freq / 1000.0;
        if khz == khz.floor() {
            format!("{khz:.0} kHz")
        } else {
            format!("{khz:.1} kHz")
        }
    } else if freq == freq.floor() {
        format!("{freq:.0} Hz")
    } else {
        format!("{freq:.1} Hz")
    }
}
