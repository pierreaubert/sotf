use super::midi_mapping_engine::MidiMappingEngine;
use super::types::MappingAction;
use crate::layout::{ControllerLayout, MidiControlId};
use crate::mapping::{ControlBinding, ValueScaling};
use crate::message::MidiMessage;
use crate::templates::TemplateRegistry;

use crate::layout::{PhysicalControl, PhysicalControlKind};
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

#[test]
fn unknown_control_id_returns_unmapped() {
    let mut engine = MidiMappingEngine::new();
    engine.set_layout(tiny_layout());

    let params = compressor::PARAMS;
    engine.on_plugin_focus("Compressor", params, 0);

    // CC 99 is not present in the tiny layout.
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 99,
        value: 64,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));
}

#[test]
fn unsupported_message_types_return_unmapped() {
    let mut engine = MidiMappingEngine::new();
    engine.set_layout(tiny_layout());

    let params = compressor::PARAMS;
    engine.on_plugin_focus("Compressor", params, 0);

    // Pitch bend is parsed but not mapped in this layout.
    let pitch_bend = MidiMessage::PitchBend {
        channel: 0,
        value: 8192,
    };
    assert!(matches!(
        engine.handle_midi(&pitch_bend, params),
        MappingAction::Unmapped
    ));

    // Program change is parsed but not mapped.
    let program_change = MidiMessage::ProgramChange {
        channel: 0,
        program: 5,
    };
    assert!(matches!(
        engine.handle_midi(&program_change, params),
        MappingAction::Unmapped
    ));

    // Channel aftertouch is parsed but not mapped.
    let aftertouch = MidiMessage::ChannelAftertouch {
        channel: 0,
        pressure: 100,
    };
    assert!(matches!(
        engine.handle_midi(&aftertouch, params),
        MappingAction::Unmapped
    ));
}

#[test]
fn out_of_range_midi_values_are_rejected_by_parser() {
    // Values outside the MIDI 0-127 range cannot be constructed directly through
    // the typed API, so the parser must reject malformed bytes.
    assert!(MidiMessage::from_bytes(&[0xB0, 0x80, 0x40]).is_err());
    assert!(MidiMessage::from_bytes(&[0xB0, 0x40, 0x80]).is_err());
    assert!(MidiMessage::from_bytes(&[0x90, 0x80, 0x40]).is_err());
}

#[test]
fn mapped_control_on_wrong_channel_returns_unmapped() {
    let mut engine = MidiMappingEngine::new();
    engine.set_layout(tiny_layout());

    let params = compressor::PARAMS;
    engine.on_plugin_focus("Compressor", params, 0);

    // pot_1 is on channel 0, CC 1. Channel 1 should not match.
    let msg = MidiMessage::ControlChange {
        channel: 1,
        controller: 1,
        value: 64,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));
}

#[test]
fn relative_encoder_returns_adjust_param_action() {
    let mut layout = tiny_layout();
    layout.controls[0].kind = PhysicalControlKind::Encoder;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout);

    let params = compressor::PARAMS;
    engine.on_plugin_focus("Compressor", params, 0);

    // Force the pot_1 binding to Relative scaling so the engine emits deltas.
    engine
        .mapping_mut()
        .unwrap()
        .bindings
        .iter_mut()
        .find(|b| b.control_id == "pot_1")
        .unwrap()
        .scaling = ValueScaling::Relative;

    // Increment: value 3 -> +3
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 1,
        value: 3,
    };
    assert!(
        matches!(engine.handle_midi(&msg, params), MappingAction::AdjustParam { delta, .. } if delta > 0.0),
        "expected positive delta for encoder increment"
    );

    // Decrement: value 125 -> -3
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 1,
        value: 125,
    };
    assert!(
        matches!(engine.handle_midi(&msg, params), MappingAction::AdjustParam { delta, .. } if delta < 0.0),
        "expected negative delta for encoder decrement"
    );
}

#[test]
fn stale_param_index_in_mapping_returns_unmapped() {
    let mut engine = MidiMappingEngine::new();
    engine.set_layout(tiny_layout());

    let params = compressor::PARAMS;
    engine.on_plugin_focus("Compressor", params, 0);

    // Replace the existing btn_1 binding with one whose param_index is out of
    // bounds for the focused plugin. The engine must not panic and should
    // report Unmapped.
    let mapping = engine.mapping_mut().unwrap();
    mapping.bindings.retain(|b| b.control_id != "btn_1");
    mapping.bindings.push(ControlBinding {
        control_id: "btn_1".to_string(),
        plugin_index: 0,
        param_index: 9999,
        page: 0,
        scaling: ValueScaling::Linear,
    });

    let msg = MidiMessage::NoteOn {
        channel: 0,
        note: 24,
        velocity: 127,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));
}
