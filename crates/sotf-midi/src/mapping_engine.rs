//! Runtime coordinator: connects MIDI input to plugin parameters
//!
//! Handles the full flow: MIDI message → resolve control → find binding → update parameter.
//! Supports MIDI learn, paging, and LED feedback.

use crate::auto_map;
use crate::layout::{ControllerLayout, MidiControlId};
use crate::mapping::{
    ControlBinding, MidiMapping, MidiOverlay, ParamAssignment, ValueScaling, midi_to_param,
    param_to_midi,
};
use crate::message::MidiMessage;
use crate::templates::TemplateRegistry;
use sotf_host::param_specs::{ParamSpec, ParamType};

/// Result of processing a MIDI message through the mapping engine
#[derive(Debug, Clone)]
pub enum MappingAction {
    /// A parameter should be set to a new value
    SetParam {
        plugin_index: usize,
        param_index: usize,
        value: f64,
    },
    /// A relative parameter adjustment
    AdjustParam {
        plugin_index: usize,
        param_index: usize,
        delta: f64,
    },
    /// Navigate to previous page
    PagePrev,
    /// Navigate to next page
    PageNext,
    /// MIDI learn completed: bound control to param
    LearnComplete {
        control_id: String,
        param_index: usize,
    },
    /// Message was not mapped to anything
    Unmapped,
}

/// MIDI learn state
#[derive(Debug, Clone)]
struct LearnState {
    plugin_index: usize,
    param_index: usize,
}

/// Runtime mapping engine
#[derive(Debug, Clone)]
pub struct MidiMappingEngine {
    layout: Option<ControllerLayout>,
    mapping: Option<MidiMapping>,
    templates: TemplateRegistry,
    learn_state: Option<LearnState>,
}

impl MidiMappingEngine {
    pub fn new() -> Self {
        Self {
            layout: None,
            mapping: None,
            templates: TemplateRegistry::new(),
            learn_state: None,
        }
    }

    /// Set the controller layout
    pub fn set_layout(&mut self, layout: ControllerLayout) {
        self.layout = Some(layout);
    }

    /// Get current layout
    pub fn layout(&self) -> Option<&ControllerLayout> {
        self.layout.as_ref()
    }

    /// Set the template registry
    pub fn set_templates(&mut self, templates: TemplateRegistry) {
        self.templates = templates;
    }

    /// Get current mapping
    pub fn mapping(&self) -> Option<&MidiMapping> {
        self.mapping.as_ref()
    }

    /// Get mutable mapping
    pub fn mapping_mut(&mut self) -> Option<&mut MidiMapping> {
        self.mapping.as_mut()
    }

    /// Called when a plugin gains focus. Rebuilds the mapping using:
    /// 1. Manual overrides (preserved from previous mapping)
    /// 2. Curated template (if available)
    /// 3. Auto-map (fallback)
    pub fn on_plugin_focus(
        &mut self,
        plugin_type: &str,
        params: &[ParamSpec],
        plugin_index: usize,
    ) {
        let layout = match &self.layout {
            Some(l) => l,
            None => return,
        };

        // Try template first
        let mut mapping = if let Some(template) = self.templates.find(&layout.name, plugin_type) {
            match template.to_mapping_checked(plugin_index, params.len()) {
                Ok(mapping) => mapping,
                Err(err) => {
                    log::warn!(
                        "Ignoring stale MIDI template for {} / {}: {}",
                        layout.name,
                        plugin_type,
                        err
                    );
                    auto_map::auto_map(layout, params, plugin_index, plugin_type)
                }
            }
        } else {
            auto_map::auto_map(layout, params, plugin_index, plugin_type)
        };

        // Apply any manual overrides from previous mapping
        if let Some(prev) = &self.mapping {
            for (&param_idx, control_id) in &prev.manual_overrides {
                mapping
                    .manual_overrides
                    .insert(param_idx, control_id.clone());
                // Replace the auto/template binding for this param, and any
                // page-0 binding already using the same physical control, so a
                // manual override never causes one control to fire two params.
                mapping.bindings.retain(|b| {
                    b.plugin_index != plugin_index
                        || (b.param_index != param_idx
                            && !(b.page == 0 && b.control_id == *control_id))
                });
                if let Some(spec) = params.get(param_idx) {
                    mapping.bindings.push(ControlBinding {
                        control_id: control_id.clone(),
                        plugin_index,
                        param_index: param_idx,
                        page: 0, // overrides always on page 0
                        scaling: auto_map::scaling_for_param(spec),
                    });
                }
            }
        }

        self.mapping = Some(mapping);
    }

    /// Process an incoming MIDI message and return the appropriate action
    pub fn handle_midi(&mut self, msg: &MidiMessage, params: &[ParamSpec]) -> MappingAction {
        let layout = match &self.layout {
            Some(l) => l,
            None => return MappingAction::Unmapped,
        };

        // Extract MIDI control ID from message
        let midi_id = match msg {
            MidiMessage::ControlChange {
                channel,
                controller,
                ..
            } => MidiControlId::CC(*channel, *controller),
            MidiMessage::NoteOn { channel, note, .. } => MidiControlId::Note(*channel, *note),
            MidiMessage::NoteOff { channel, note, .. } => MidiControlId::Note(*channel, *note),
            _ => return MappingAction::Unmapped,
        };

        // Find the physical control
        let control = match layout.find_by_midi_id(&midi_id) {
            Some(c) => c,
            None => return MappingAction::Unmapped,
        };

        // Check for page navigation
        if let Some(ref prev_id) = layout.page_prev_id
            && control.id == *prev_id
        {
            if let Some(ref mut mapping) = self.mapping {
                mapping.prev_page();
            }
            return MappingAction::PagePrev;
        }
        if let Some(ref next_id) = layout.page_next_id
            && control.id == *next_id
        {
            if let Some(ref mut mapping) = self.mapping {
                mapping.next_page();
            }
            return MappingAction::PageNext;
        }

        // Handle MIDI learn mode
        if let Some(learn) = self.learn_state.take()
            && let Some(ref mut mapping) = self.mapping
        {
            // Remove any existing binding for this param, plus any binding on
            // the current page that already uses the learned physical control.
            let learned_page = mapping.current_page;
            mapping.bindings.retain(|b| {
                b.plugin_index != learn.plugin_index
                    || (b.param_index != learn.param_index
                        && !(b.page == learned_page && b.control_id == control.id))
            });

            let scaling = params
                .get(learn.param_index)
                .map(auto_map::scaling_for_param)
                .unwrap_or(ValueScaling::Linear);

            mapping.bindings.push(ControlBinding {
                control_id: control.id.clone(),
                plugin_index: learn.plugin_index,
                param_index: learn.param_index,
                page: mapping.current_page,
                scaling,
            });
            mapping
                .manual_overrides
                .insert(learn.param_index, control.id.clone());

            return MappingAction::LearnComplete {
                control_id: control.id.clone(),
                param_index: learn.param_index,
            };
        }

        // Normal operation: find binding and convert value
        let mapping = match &self.mapping {
            Some(m) => m,
            None => return MappingAction::Unmapped,
        };

        let binding = match mapping.binding_for_control(&control.id) {
            Some(b) => b,
            None => return MappingAction::Unmapped,
        };

        let spec = match params.get(binding.param_index) {
            Some(s) => s,
            None => return MappingAction::Unmapped,
        };

        let midi_value = extract_midi_value(msg);

        // For relative encoders, return a delta
        if binding.scaling == ValueScaling::Relative {
            let delta = midi_to_param(midi_value, 0.0, 0.0, ValueScaling::Relative);
            return MappingAction::AdjustParam {
                plugin_index: binding.plugin_index,
                param_index: binding.param_index,
                delta,
            };
        }

        // For absolute controls, convert to parameter value
        let (min, max) = param_range(spec);
        let value = midi_to_param(midi_value, min, max, binding.scaling);

        MappingAction::SetParam {
            plugin_index: binding.plugin_index,
            param_index: binding.param_index,
            value,
        }
    }

    /// Start MIDI learn mode for a specific parameter
    pub fn learn_param(&mut self, plugin_index: usize, param_index: usize) {
        self.learn_state = Some(LearnState {
            plugin_index,
            param_index,
        });
    }

    /// Cancel MIDI learn mode
    pub fn cancel_learn(&mut self) {
        self.learn_state = None;
    }

    /// Whether MIDI learn is active
    pub fn is_learning(&self) -> bool {
        self.learn_state.is_some()
    }

    /// Build a MidiOverlay for UI display
    pub fn build_overlay(&self, _params: &[ParamSpec]) -> MidiOverlay {
        let mut overlay = MidiOverlay::empty();

        if let (Some(mapping), Some(layout)) = (&self.mapping, &self.layout) {
            overlay.controller_name = Some(mapping.controller_name.clone());
            overlay.current_page = mapping.current_page;
            overlay.total_pages = mapping.total_pages;

            for binding in mapping.active_bindings() {
                if let Some(control) = layout.find_by_id(&binding.control_id) {
                    let is_override = mapping.manual_overrides.contains_key(&binding.param_index);
                    overlay.assignments.insert(
                        binding.param_index,
                        ParamAssignment {
                            control_label: control.label.clone(),
                            control_kind: control.kind,
                            is_override,
                            page: binding.page,
                        },
                    );
                }
            }
        }

        if let Some(ref learn) = self.learn_state {
            overlay.learn_target = Some(learn.param_index);
        }

        overlay
    }

    /// Calculate the MIDI value to send for LED feedback given a parameter's
    /// current value. The `plugin_index` argument disambiguates chains containing
    /// multiple instances of the same plugin type.
    pub fn feedback_value(
        &self,
        plugin_index: usize,
        param_index: usize,
        current_value: f64,
        spec: &ParamSpec,
    ) -> Option<(MidiControlId, u8)> {
        let (mapping, layout) = match (&self.mapping, &self.layout) {
            (Some(m), Some(l)) => (m, l),
            _ => return None,
        };

        let binding = mapping.binding_for_param(plugin_index, param_index)?;
        let control = layout.find_by_id(&binding.control_id)?;
        let (min, max) = param_range(spec);
        let midi_val = param_to_midi(current_value, min, max, binding.scaling);

        Some((control.midi_id, midi_val))
    }
}

impl Default for MidiMappingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the value byte from a MIDI message
fn extract_midi_value(msg: &MidiMessage) -> u8 {
    match msg {
        MidiMessage::ControlChange { value, .. } => *value,
        MidiMessage::NoteOn { velocity, .. } => *velocity,
        MidiMessage::NoteOff { .. } => 0,
        _ => 0,
    }
}

/// Get the (min, max) range from a ParamSpec
fn param_range(spec: &ParamSpec) -> (f64, f64) {
    match spec.param_type {
        ParamType::Float { min, max, .. } => (min, max),
        ParamType::Int { min, max, .. } => (min as f64, max as f64),
        ParamType::Bool { .. } => (0.0, 1.0),
        ParamType::Choice { labels, .. } => (0.0, (labels.len() - 1) as f64),
        ParamType::FilePath => (0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ControllerLayout, PhysicalControl, PhysicalControlKind};
    use crate::templates::{MappingTemplate, TemplateBinding};
    use sotf_plugin_multiband_compressor::params as compressor;

    fn tiny_layout() -> ControllerLayout {
        ControllerLayout {
            name: "Tiny".to_string(),
            controls: vec![
                PhysicalControl {
                    id: "pot_1".to_string(),
                    kind: PhysicalControlKind::Pot,
                    column: 0,
                    row: 0,
                    group: "pots".to_string(),
                    label: "P1".to_string(),
                    midi_id: MidiControlId::CC(0, 1),
                    secondary_midi_id: None,
                },
                PhysicalControl {
                    id: "btn_1".to_string(),
                    kind: PhysicalControlKind::Button,
                    column: 0,
                    row: 1,
                    group: "buttons".to_string(),
                    label: "B1".to_string(),
                    midi_id: MidiControlId::Note(0, 24),
                    secondary_midi_id: None,
                },
            ],
            grid_columns: 1,
            grid_rows: 2,
            reserved_control_ids: vec![],
            page_prev_id: None,
            page_next_id: None,
        }
    }

    #[test]
    fn test_handle_midi_sets_param() {
        let mut engine = MidiMappingEngine::new();
        engine.set_layout(tiny_layout());

        let params = compressor::PARAMS;
        engine.on_plugin_focus("Compressor", params, 0);

        // Send a CC message for pot_1 (CC 1)
        let msg = MidiMessage::ControlChange {
            channel: 0,
            controller: 1,
            value: 64,
        };
        let action = engine.handle_midi(&msg, params);

        match action {
            MappingAction::SetParam {
                plugin_index,
                param_index,
                value,
            } => {
                assert_eq!(plugin_index, 0);
                assert_eq!(param_index, 0); // first continuous param = Threshold
                // 64/127 of -60..0 range ≈ -29.76
                assert!(value > -35.0 && value < -25.0);
            }
            other => panic!("Expected SetParam, got {:?}", other),
        }
    }

    #[test]
    fn test_midi_learn() {
        let mut engine = MidiMappingEngine::new();
        engine.set_layout(tiny_layout());

        let params = compressor::PARAMS;
        engine.on_plugin_focus("Compressor", params, 0);

        // Start learn for param 5 (Makeup Gain)
        engine.learn_param(0, 5);
        assert!(engine.is_learning());

        // Send a CC message
        let msg = MidiMessage::ControlChange {
            channel: 0,
            controller: 1,
            value: 100,
        };
        let action = engine.handle_midi(&msg, params);

        match action {
            MappingAction::LearnComplete {
                control_id,
                param_index,
            } => {
                assert_eq!(control_id, "pot_1");
                assert_eq!(param_index, 5);
            }
            other => panic!("Expected LearnComplete, got {:?}", other),
        }

        assert!(!engine.is_learning());

        // Verify the override persists
        let mapping = engine.mapping().unwrap();
        assert!(mapping.manual_overrides.contains_key(&5));
        let learned_bindings: Vec<_> = mapping
            .bindings
            .iter()
            .filter(|b| b.page == 0 && b.control_id == "pot_1")
            .collect();
        assert_eq!(learned_bindings.len(), 1);
        assert_eq!(learned_bindings[0].param_index, 5);
    }

    #[test]
    fn manual_override_removes_existing_control_binding() {
        let mut engine = MidiMappingEngine::new();
        engine.set_layout(tiny_layout());

        let params = compressor::PARAMS;
        engine.on_plugin_focus("Compressor", params, 0);
        engine
            .mapping_mut()
            .unwrap()
            .manual_overrides
            .insert(5, "pot_1".to_string());

        engine.on_plugin_focus("Compressor", params, 0);

        let mapping = engine.mapping().unwrap();
        let pot_bindings: Vec<_> = mapping
            .bindings
            .iter()
            .filter(|b| b.page == 0 && b.control_id == "pot_1")
            .collect();
        assert_eq!(pot_bindings.len(), 1);
        assert_eq!(pot_bindings[0].param_index, 5);
    }

    #[test]
    fn stale_template_falls_back_to_auto_map() {
        let mut engine = MidiMappingEngine::new();
        engine.set_layout(tiny_layout());

        let mut templates = TemplateRegistry::new();
        templates.add(MappingTemplate {
            controller_name: "Tiny".to_string(),
            plugin_type: "Compressor".to_string(),
            bindings: vec![TemplateBinding {
                control_id: "pot_1".to_string(),
                param_index: 999,
                page: 0,
                scaling: ValueScaling::Linear,
            }],
        });
        engine.set_templates(templates);

        let params = compressor::PARAMS;
        engine.on_plugin_focus("Compressor", params, 0);

        let mapping = engine.mapping().unwrap();
        assert_eq!(
            mapping.binding_for_control("pot_1").map(|b| b.param_index),
            Some(0),
            "invalid template should not survive into active mapping"
        );
    }

    #[test]
    fn test_build_overlay() {
        let mut engine = MidiMappingEngine::new();
        engine.set_layout(tiny_layout());

        let params = compressor::PARAMS;
        engine.on_plugin_focus("Compressor", params, 0);

        let overlay = engine.build_overlay(params);
        // pot_1 should be assigned to first continuous param
        assert!(overlay.assignments.contains_key(&0));
        assert_eq!(overlay.assignments[&0].control_label, "P1");
    }
}
