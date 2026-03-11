//! MIDI-to-parameter binding definitions and value conversion

use crate::layout::PhysicalControlKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How a MIDI value (0-127) maps to a parameter value
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ValueScaling {
    /// Linear interpolation between min and max
    Linear,
    /// Logarithmic scaling (good for frequency, wide-range ratio params)
    Logarithmic,
    /// Button toggle: >=64 = true, <64 = false
    Toggle,
    /// Stepped: quantize to N discrete steps
    Stepped(u8),
    /// Relative encoder: value < 64 = decrement, value > 64 = increment
    Relative,
}

/// A single binding from a physical control to a plugin parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlBinding {
    /// ID of the physical control on the layout
    pub control_id: String,
    /// Plugin index in the plugin chain
    pub plugin_index: usize,
    /// Parameter index within the plugin's PARAMS array
    pub param_index: usize,
    /// Page this binding belongs to (0-based)
    pub page: usize,
    /// How to scale the MIDI value
    pub scaling: ValueScaling,
}

/// Complete mapping state for a controller + plugin combination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiMapping {
    /// Controller layout name this mapping is for
    pub controller_name: String,
    /// Plugin type this mapping targets (e.g., "Compressor")
    pub plugin_type: String,
    /// All bindings
    pub bindings: Vec<ControlBinding>,
    /// Currently active page
    pub current_page: usize,
    /// Total number of pages
    pub total_pages: usize,
    /// Manual overrides: param_index -> control_id (take priority over auto-map)
    pub manual_overrides: HashMap<usize, String>,
}

impl MidiMapping {
    pub fn new(controller_name: String, plugin_type: String) -> Self {
        Self {
            controller_name,
            plugin_type,
            bindings: Vec::new(),
            current_page: 0,
            total_pages: 1,
            manual_overrides: HashMap::new(),
        }
    }

    /// Get bindings for the current page
    pub fn active_bindings(&self) -> Vec<&ControlBinding> {
        self.bindings
            .iter()
            .filter(|b| b.page == self.current_page)
            .collect()
    }

    /// Find the binding for a given control ID on the current page
    pub fn binding_for_control(&self, control_id: &str) -> Option<&ControlBinding> {
        self.bindings
            .iter()
            .find(|b| b.page == self.current_page && b.control_id == control_id)
    }

    /// Find the binding for a given parameter index (any page)
    pub fn binding_for_param(&self, plugin_index: usize, param_index: usize) -> Option<&ControlBinding> {
        self.bindings
            .iter()
            .find(|b| b.plugin_index == plugin_index && b.param_index == param_index)
    }

    /// Navigate to next page
    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.total_pages {
            self.current_page += 1;
        }
    }

    /// Navigate to previous page
    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }
}

/// Per-parameter MIDI overlay info for UI display
#[derive(Debug, Clone)]
pub struct MidiOverlay {
    /// Per param_index: assignment info (if mapped)
    pub assignments: HashMap<usize, ParamAssignment>,
    /// If in learn mode, which param_index is waiting for MIDI input
    pub learn_target: Option<usize>,
    /// Name of the connected controller (if any)
    pub controller_name: Option<String>,
    /// Currently active MIDI page (0-based)
    pub current_page: usize,
    /// Total number of MIDI pages
    pub total_pages: usize,
}

/// Info about a MIDI assignment for one parameter
#[derive(Debug, Clone)]
pub struct ParamAssignment {
    /// Short label of the physical control (e.g., "K1", "F3")
    pub control_label: String,
    /// Kind of physical control
    pub control_kind: PhysicalControlKind,
    /// Whether this is a manual override or auto-mapped
    pub is_override: bool,
    /// Page this assignment is on
    pub page: usize,
}

impl MidiOverlay {
    pub fn empty() -> Self {
        Self {
            assignments: HashMap::new(),
            learn_target: None,
            controller_name: None,
            current_page: 0,
            total_pages: 1,
        }
    }

    /// Whether a MIDI controller is connected and has mappings
    pub fn has_controller(&self) -> bool {
        self.controller_name.is_some()
    }
}

// =============================================================================
// Value Conversion
// =============================================================================

/// Convert a MIDI value (0-127) to a parameter value
pub fn midi_to_param(midi_value: u8, min: f64, max: f64, scaling: ValueScaling) -> f64 {
    let norm = midi_value as f64 / 127.0;
    match scaling {
        ValueScaling::Linear => min + norm * (max - min),
        ValueScaling::Logarithmic => {
            // Protect against log(0)
            let log_min = if min > 0.0 { min.ln() } else { 0.0_f64.ln().max(-10.0) };
            let log_max = max.ln();
            (log_min + norm * (log_max - log_min)).exp()
        }
        ValueScaling::Toggle => {
            if midi_value >= 64 { max } else { min }
        }
        ValueScaling::Stepped(n) => {
            let step_index = ((norm * n as f64).floor() as u8).min(n - 1);
            let step_size = (max - min) / (n - 1).max(1) as f64;
            min + step_index as f64 * step_size
        }
        ValueScaling::Relative => {
            // Relative encoding: this returns a delta, not an absolute value.
            // Values 1-63 = positive increment, 65-127 = negative (two's complement style)
            if midi_value == 0 || midi_value == 64 {
                0.0
            } else if midi_value < 64 {
                midi_value as f64
            } else {
                midi_value as f64 - 128.0
            }
        }
    }
}

/// Convert a parameter value to a MIDI value (0-127)
pub fn param_to_midi(param_value: f64, min: f64, max: f64, scaling: ValueScaling) -> u8 {
    match scaling {
        ValueScaling::Linear => {
            let norm = (param_value - min) / (max - min);
            (norm.clamp(0.0, 1.0) * 127.0).round() as u8
        }
        ValueScaling::Logarithmic => {
            let log_min = if min > 0.0 { min.ln() } else { 0.0_f64.ln().max(-10.0) };
            let log_max = max.ln();
            let log_val = if param_value > 0.0 {
                param_value.ln()
            } else {
                log_min
            };
            let norm = (log_val - log_min) / (log_max - log_min);
            (norm.clamp(0.0, 1.0) * 127.0).round() as u8
        }
        ValueScaling::Toggle => {
            if param_value > (min + max) / 2.0 { 127 } else { 0 }
        }
        ValueScaling::Stepped(n) => {
            let step_size = (max - min) / (n - 1).max(1) as f64;
            let step_index = ((param_value - min) / step_size).round() as u8;
            let norm = step_index as f64 / (n - 1).max(1) as f64;
            (norm.clamp(0.0, 1.0) * 127.0).round() as u8
        }
        ValueScaling::Relative => {
            // Relative doesn't have a meaningful absolute→MIDI mapping
            64
        }
    }
}

/// Apply a relative encoder delta to a current parameter value
pub fn apply_relative_delta(current: f64, delta: f64, min: f64, max: f64, step: f64) -> f64 {
    (current + delta * step).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_roundtrip() {
        let min = -60.0;
        let max = 0.0;
        for midi in [0u8, 32, 64, 96, 127] {
            let param = midi_to_param(midi, min, max, ValueScaling::Linear);
            let back = param_to_midi(param, min, max, ValueScaling::Linear);
            assert_eq!(back, midi, "roundtrip failed for midi={midi}");
        }
    }

    #[test]
    fn test_logarithmic_roundtrip() {
        let min = 20.0;
        let max = 20000.0;
        for midi in [0u8, 32, 64, 96, 127] {
            let param = midi_to_param(midi, min, max, ValueScaling::Logarithmic);
            let back = param_to_midi(param, min, max, ValueScaling::Logarithmic);
            assert!((back as i16 - midi as i16).abs() <= 1, "log roundtrip failed for midi={midi}: got {back}");
        }
    }

    #[test]
    fn test_toggle() {
        assert_eq!(midi_to_param(0, 0.0, 1.0, ValueScaling::Toggle), 0.0);
        assert_eq!(midi_to_param(63, 0.0, 1.0, ValueScaling::Toggle), 0.0);
        assert_eq!(midi_to_param(64, 0.0, 1.0, ValueScaling::Toggle), 1.0);
        assert_eq!(midi_to_param(127, 0.0, 1.0, ValueScaling::Toggle), 1.0);
    }

    #[test]
    fn test_stepped() {
        // 4 steps over 0-3 range
        let val = midi_to_param(0, 0.0, 3.0, ValueScaling::Stepped(4));
        assert_eq!(val, 0.0);
        let val = midi_to_param(127, 0.0, 3.0, ValueScaling::Stepped(4));
        assert_eq!(val, 3.0);
    }

    #[test]
    fn test_relative() {
        // Increment
        let delta = midi_to_param(3, 0.0, 1.0, ValueScaling::Relative);
        assert_eq!(delta, 3.0);
        // Decrement
        let delta = midi_to_param(125, 0.0, 1.0, ValueScaling::Relative);
        assert_eq!(delta, -3.0);
    }

    #[test]
    fn test_mapping_pages() {
        let mut mapping = MidiMapping::new("Test".to_string(), "Compressor".to_string());
        mapping.total_pages = 3;
        assert_eq!(mapping.current_page, 0);
        mapping.next_page();
        assert_eq!(mapping.current_page, 1);
        mapping.next_page();
        assert_eq!(mapping.current_page, 2);
        mapping.next_page(); // should not exceed
        assert_eq!(mapping.current_page, 2);
        mapping.prev_page();
        assert_eq!(mapping.current_page, 1);
    }
}
