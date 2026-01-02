//! EQ Plugin View
//!
//! Connects the Slint UI with the plugin parameters and host.

use crate::parameters::{EqParameters, NUM_BANDS};
use plinth_plugin::Host;
use plugin_canvas::{event::EventResponse, Event};
use plugin_canvas_slint::view::PluginView;
use std::rc::Rc;
use std::sync::Arc;

slint::include_modules!();

/// EQ Plugin View wrapping the Slint UI
pub struct EqPluginView {
    plugin_window: PluginWindow,
    parameters: Arc<EqParameters>,
}

impl EqPluginView {
    pub fn new(host: Rc<dyn Host>, parameters: Arc<EqParameters>) -> Self {
        let plugin_window = PluginWindow::new().expect("Failed to create Slint window");

        // Set up parameter change callbacks
        let host_start = host.clone();
        plugin_window.on_start_parameter_change(move |param_id| {
            host_start.start_parameter_change(param_id as u32);
        });

        let host_change = host.clone();
        plugin_window.on_change_parameter_value(move |param_id, normalized_value| {
            host_change.change_parameter_value(param_id as u32, normalized_value as f64);
        });

        let host_end = host.clone();
        plugin_window.on_end_parameter_change(move |param_id| {
            host_end.end_parameter_change(param_id as u32);
        });

        Self {
            plugin_window,
            parameters,
        }
    }

    /// Update UI with current parameter values
    fn update_ui(&self) {
        // Update each band
        for band in 0..NUM_BANDS {
            let freq = self.parameters.band_frequency(band);
            let q = self.parameters.band_q(band);
            let gain = self.parameters.band_gain(band);

            let band_data = BandData {
                enabled: self.parameters.band_enabled(band),
                filter_type: self.parameters.band_filter_type(band).to_index() as i32,
                frequency: freq as f32,
                frequency_normalized: freq_to_normalized(freq) as f32,
                q: q as f32,
                q_normalized: q_to_normalized(q) as f32,
                gain_db: gain as f32,
                gain_normalized: ((gain + 24.0) / 48.0) as f32,
                frequency_text: format_frequency(freq).into(),
                q_text: format!("{:.2}", q).into(),
                gain_text: format!("{:.1} dB", gain).into(),
            };

            match band {
                0 => self.plugin_window.set_band1_data(band_data),
                1 => self.plugin_window.set_band2_data(band_data),
                2 => self.plugin_window.set_band3_data(band_data),
                _ => self.plugin_window.set_band4_data(band_data),
            }
        }

        // Update output gain
        let output_gain = self.parameters.output_gain();
        self.plugin_window.set_output_gain(output_gain as f32);
        self.plugin_window.set_output_gain_normalized(((output_gain + 24.0) / 48.0) as f32);
        self.plugin_window
            .set_output_gain_text(format!("{:.1} dB", output_gain).into());
    }
}

impl PluginView for EqPluginView {
    fn window(&self) -> &slint::Window {
        self.plugin_window.window()
    }

    fn on_event(&self, event: &Event) -> EventResponse {
        match event {
            Event::Draw => {
                self.update_ui();
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        }
    }
}

/// Convert frequency to normalized (0-1) using log scale
fn freq_to_normalized(freq: f64) -> f64 {
    let min_freq = 20.0_f64;
    let max_freq = 20000.0_f64;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    (freq.ln() - log_min) / (log_max - log_min)
}

/// Convert Q to normalized (0-1) using log scale
fn q_to_normalized(q: f64) -> f64 {
    let min_q = 0.1_f64;
    let max_q = 10.0_f64;
    let log_min = min_q.ln();
    let log_max = max_q.ln();
    (q.ln() - log_min) / (log_max - log_min)
}

/// Format frequency for display
fn format_frequency(freq: f64) -> String {
    if freq >= 1000.0 {
        format!("{:.1} kHz", freq / 1000.0)
    } else {
        format!("{:.0} Hz", freq)
    }
}
