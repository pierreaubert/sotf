// ============================================================================
// Integration tests for sotf-midi
// ============================================================================
//
// These tests exercise the crate's public API as a black box. They avoid
// requiring physical MIDI hardware by using in-memory message parsing,
// configuration round-trips, device snapshots, and the mapping engine.

use sotf_audio_player_midi::{
    ControllerLayout, MidiControlId, MidiManager, PhysicalControl, PhysicalControlKind, auto_map,
    clock::{MIDI_CLOCK_PPQ, clock_tick_interval_samples, schedule_clock_ticks_for_block},
    config::{DeviceProfile, MidiConfig},
    device::{
        MidiDevice, MidiDeviceChangeKind, MidiDeviceInfo, MidiDeviceSnapshot, MidiDeviceType,
    },
    error::MidiError,
    layouts,
    mapping::{ValueScaling, midi_to_param, param_to_midi},
    mapping_engine::{MappingAction, MidiMappingEngine},
    message::MidiMessage,
    profiles::{
        GLMControl, GenelecGLMProfile, LCXLTemplate, LaunchControlXLProfile, RMETotalMixProfile,
        TotalMixControl, TotalMixRow, XoneK2Profile,
    },
    sequencer::{MidiClip, MidiEvent, MidiRegion},
    templates::{MappingTemplate, TemplateBinding, TemplateRegistry},
};
use sotf_host::param_specs::ParamSpec;

// ----------------------------------------------------------------------------
// Shared test fixtures
// ----------------------------------------------------------------------------

/// A small controller layout with faders, pots, buttons and page navigation.
fn test_controller_layout() -> ControllerLayout {
    ControllerLayout {
        name: "Test Controller".to_string(),
        controls: vec![
            PhysicalControl {
                id: "volume".to_string(),
                kind: PhysicalControlKind::Fader,
                column: 0,
                row: 0,
                group: "faders".to_string(),
                label: "Vol".to_string(),
                midi_id: MidiControlId::CC(0, 7),
                secondary_midi_id: None,
            },
            PhysicalControl {
                id: "pan".to_string(),
                kind: PhysicalControlKind::Pot,
                column: 1,
                row: 0,
                group: "pots".to_string(),
                label: "Pan".to_string(),
                midi_id: MidiControlId::CC(0, 10),
                secondary_midi_id: None,
            },
            PhysicalControl {
                id: "bypass".to_string(),
                kind: PhysicalControlKind::Button,
                column: 2,
                row: 0,
                group: "buttons".to_string(),
                label: "Bypass".to_string(),
                midi_id: MidiControlId::Note(0, 60),
                secondary_midi_id: None,
            },
            PhysicalControl {
                id: "page_prev".to_string(),
                kind: PhysicalControlKind::Button,
                column: 0,
                row: 1,
                group: "nav".to_string(),
                label: "<".to_string(),
                midi_id: MidiControlId::CC(0, 20),
                secondary_midi_id: None,
            },
            PhysicalControl {
                id: "page_next".to_string(),
                kind: PhysicalControlKind::Button,
                column: 1,
                row: 1,
                group: "nav".to_string(),
                label: ">".to_string(),
                midi_id: MidiControlId::CC(0, 21),
                secondary_midi_id: None,
            },
        ],
        grid_columns: 3,
        grid_rows: 2,
        reserved_control_ids: vec!["page_prev".to_string(), "page_next".to_string()],
        page_prev_id: Some("page_prev".to_string()),
        page_next_id: Some("page_next".to_string()),
    }
}

/// A tiny plugin parameter set for mapping-engine workflows.
const TEST_PARAMS: &[ParamSpec] = &[
    ParamSpec::float("Volume", "volume", 0.0, -60.0, 12.0, 0.1, "dB", "Main"),
    ParamSpec::float("Pan", "pan", 0.0, -1.0, 1.0, 0.01, "", "Main"),
    ParamSpec::bool_param("Bypass", "bypass", false, "Main"),
];

fn midi_device_info(index: usize, name: &str, device_type: MidiDeviceType) -> MidiDeviceInfo {
    MidiDeviceInfo {
        index,
        name: name.to_string(),
        device_type,
        manufacturer: None,
        is_connected: false,
    }
}

// ----------------------------------------------------------------------------
// MIDI message parsing round-trips
// ----------------------------------------------------------------------------

#[test]
fn channel_voice_messages_roundtrip_through_bytes() {
    let messages = vec![
        MidiMessage::NoteOff {
            channel: 0,
            note: 60,
            velocity: 64,
        },
        MidiMessage::NoteOn {
            channel: 15,
            note: 127,
            velocity: 127,
        },
        MidiMessage::PolyphonicAftertouch {
            channel: 7,
            note: 64,
            pressure: 32,
        },
        MidiMessage::ControlChange {
            channel: 2,
            controller: 7,
            value: 100,
        },
        MidiMessage::ProgramChange {
            channel: 3,
            program: 42,
        },
        MidiMessage::ChannelAftertouch {
            channel: 11,
            pressure: 127,
        },
        MidiMessage::PitchBend {
            channel: 1,
            value: 8192,
        },
    ];

    for msg in messages {
        let bytes = msg.to_bytes();
        let decoded = MidiMessage::from_bytes(&bytes).expect("decode should succeed");
        assert_eq!(decoded, msg, "round-trip failed for {:?}", msg);
    }
}

#[test]
fn system_messages_roundtrip_through_bytes() {
    let messages = vec![
        MidiMessage::System {
            status: 0xF8,
            data: [0, 0],
            len: 0,
        },
        MidiMessage::System {
            status: 0xFA,
            data: [0, 0],
            len: 0,
        },
        MidiMessage::System {
            status: 0xF2,
            data: [0x7F, 0x7F],
            len: 2,
        },
        MidiMessage::System {
            status: 0xF3,
            data: [0x7F, 0],
            len: 1,
        },
        MidiMessage::SystemExclusive {
            data: vec![0xF0, 0x7D, 0x10, 0x01, 0xF7],
        },
    ];

    for msg in messages {
        let bytes = msg.to_bytes();
        let decoded = MidiMessage::from_bytes(&bytes).expect("decode should succeed");
        assert_eq!(decoded, msg, "round-trip failed for {:?}", msg);
    }
}

#[test]
fn note_on_zero_velocity_normalizes_to_note_off() {
    let msg = MidiMessage::NoteOn {
        channel: 4,
        note: 60,
        velocity: 0,
    };
    let bytes = msg.to_bytes();
    let decoded = MidiMessage::from_bytes(&bytes).expect("decode should succeed");
    assert_eq!(
        decoded,
        MidiMessage::NoteOff {
            channel: 4,
            note: 60,
            velocity: 0,
        }
    );
}

#[test]
fn running_status_parsing_works() {
    let msg = MidiMessage::from_bytes_with_status(&[7, 100], Some(0xB0)).unwrap();
    assert_eq!(
        msg,
        MidiMessage::ControlChange {
            channel: 0,
            controller: 7,
            value: 100
        }
    );

    // Without running status the same data-only bytes must fail.
    assert!(MidiMessage::from_bytes_with_status(&[7, 100], None).is_err());
}

#[test]
fn invalid_messages_return_errors() {
    // Empty message
    assert!(matches!(
        MidiMessage::from_bytes(&[]),
        Err(MidiError::InvalidMessage(_))
    ));

    // Data-only bytes without status
    assert!(MidiMessage::from_bytes(&[0x40, 0x7F]).is_err());

    // Too-short channel message
    assert!(MidiMessage::from_bytes(&[0x90, 60]).is_err());

    // High bit set in data byte
    assert!(MidiMessage::from_bytes(&[0x90, 60, 0x80]).is_err());
}

#[test]
fn write_to_matches_to_bytes_and_reports_required_length() {
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 7,
        value: 100,
    };
    let via_to_bytes = msg.to_bytes();

    let mut buf = vec![0u8; via_to_bytes.len()];
    let written = msg.write_to(&mut buf);
    assert_eq!(written, via_to_bytes.len());
    assert_eq!(buf, via_to_bytes);

    // Buffer too small: function reports required length but does not write past end.
    let mut small = [0u8; 2];
    let required = msg.write_to(&mut small);
    assert_eq!(required, 3);
    assert_eq!(small, [0u8; 2]);
}

#[test]
fn description_covers_all_message_variants() {
    let messages = vec![
        MidiMessage::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0,
        },
        MidiMessage::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        },
        MidiMessage::PolyphonicAftertouch {
            channel: 0,
            note: 60,
            pressure: 50,
        },
        MidiMessage::ControlChange {
            channel: 0,
            controller: 7,
            value: 100,
        },
        MidiMessage::ProgramChange {
            channel: 0,
            program: 5,
        },
        MidiMessage::ChannelAftertouch {
            channel: 0,
            pressure: 80,
        },
        MidiMessage::PitchBend {
            channel: 0,
            value: 8192,
        },
        MidiMessage::SystemExclusive { data: vec![0xF0] },
        MidiMessage::System {
            status: 0xF8,
            data: [0, 0],
            len: 0,
        },
        MidiMessage::Raw { data: vec![0xF4] },
    ];

    for msg in messages {
        let desc = msg.description();
        assert!(!desc.is_empty(), "description empty for {:?}", msg);
    }
}

// ----------------------------------------------------------------------------
// Device routing
// ----------------------------------------------------------------------------

#[test]
fn midi_manager_creation_and_default_state() {
    let manager = MidiManager::new().expect("manager should construct");
    assert!(!manager.is_input_connected());
    assert!(!manager.is_output_connected());
}

#[test]
fn midi_manager_with_config_roundtrips_defaults() {
    let config = MidiConfig {
        default_input: Some("test-input".to_string()),
        default_output: Some("test-output".to_string()),
        listen_channel: Some(5),
        ..Default::default()
    };

    let manager = MidiManager::with_config(config.clone()).expect("manager should construct");
    assert_eq!(
        manager.config().default_input,
        Some("test-input".to_string())
    );
    assert_eq!(manager.config().listen_channel, Some(5));
}

#[test]
fn device_snapshot_does_not_require_hardware() {
    let manager = MidiManager::new().expect("manager should construct");
    let snapshot = manager.device_snapshot().expect("snapshot should succeed");
    // On CI there may be no MIDI devices; the API must still return a valid snapshot.
    for (index, input) in snapshot.inputs.iter().enumerate() {
        assert_eq!(input.index, index);
        assert_eq!(input.device_type, MidiDeviceType::Input);
        assert!(!input.is_connected);
    }
    for (index, output) in snapshot.outputs.iter().enumerate() {
        assert_eq!(output.index, index);
        assert_eq!(output.device_type, MidiDeviceType::Output);
        assert!(!output.is_connected);
    }
}

#[test]
fn sending_without_output_connection_returns_not_connected() {
    let manager = MidiManager::new().expect("manager should construct");
    let result = manager.send_message(&MidiMessage::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    assert!(matches!(result, Err(MidiError::NotConnected)));
}

#[test]
fn send_init_messages_with_no_active_profile_succeeds() {
    let manager = MidiManager::new().expect("manager should construct");
    assert!(manager.send_init_messages().is_ok());
}

#[test]
fn device_snapshot_diff_detects_connect_and_disconnect() {
    let before = MidiDeviceSnapshot::new(
        vec![midi_device_info(0, "Keyboard", MidiDeviceType::Input)],
        vec![midi_device_info(0, "Interface", MidiDeviceType::Output)],
    );
    let after = MidiDeviceSnapshot::new(
        vec![midi_device_info(0, "Pads", MidiDeviceType::Input)],
        vec![midi_device_info(0, "Interface", MidiDeviceType::Output)],
    );

    let changes = before.diff(&after);
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().any(|change| {
        change.kind == MidiDeviceChangeKind::Disconnected
            && change.device.name == "Keyboard"
            && change.device.device_type == MidiDeviceType::Input
    }));
    assert!(changes.iter().any(|change| {
        change.kind == MidiDeviceChangeKind::Connected
            && change.device.name == "Pads"
            && change.device.device_type == MidiDeviceType::Input
    }));
}

#[test]
fn device_diff_uses_name_and_type_not_index() {
    let before = MidiDeviceSnapshot::new(
        vec![midi_device_info(1, "Keyboard", MidiDeviceType::Input)],
        Vec::new(),
    );
    let after = MidiDeviceSnapshot::new(
        vec![midi_device_info(3, "Keyboard", MidiDeviceType::Input)],
        Vec::new(),
    );
    assert!(before.diff(&after).is_empty());
}

#[test]
fn midi_device_creation_and_connection_state() {
    let input = MidiDevice::new_input(0, "Test Input".to_string());
    assert_eq!(input.device_type(), MidiDeviceType::Input);
    assert!(!input.is_connected());
    assert_eq!(input.info().name, "Test Input");

    let output = MidiDevice::new_output(1, "Test Output".to_string());
    assert_eq!(output.device_type(), MidiDeviceType::Output);
    assert!(!output.is_connected());
}

#[test]
fn device_profile_and_config_roundtrip() {
    let mut profile = DeviceProfile::new("Test Profile".to_string());
    profile.description = Some("Test".to_string());
    profile.input_device = Some("in".to_string());
    profile.output_device = Some("out".to_string());
    profile.add_mapping(7, "volume".to_string());
    profile.add_mapping(1, "modulation".to_string());
    profile.add_init_message(vec![0xF0, 0x7D, 0x01, 0xF7]);
    profile
        .device_config
        .add_setting("sensitivity".to_string(), serde_json::json!(0.5));

    let mut config = MidiConfig::default();
    config.add_profile("test".to_string(), profile);
    config.set_active_profile("test".to_string());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    config.save(&path).unwrap();
    let loaded = MidiConfig::load(&path).unwrap();

    assert_eq!(loaded.active_profile, Some("test".to_string()));
    let p = loaded.get_profile("test").unwrap();
    assert_eq!(p.get_mapping(7), Some(&"volume".to_string()));
    assert_eq!(p.init_messages, vec![vec![0xF0, 0x7D, 0x01, 0xF7]]);
    assert_eq!(
        p.device_config.get_setting("sensitivity"),
        Some(&serde_json::json!(0.5))
    );
}

// ----------------------------------------------------------------------------
// Controller workflows
// ----------------------------------------------------------------------------

#[test]
fn controller_layout_finds_controls_by_midi_id_and_id() {
    let layout = test_controller_layout();

    let found = layout
        .find_by_midi_id(&MidiControlId::CC(0, 7))
        .expect("volume fader should exist");
    assert_eq!(found.id, "volume");

    let found_by_id = layout.find_by_id("bypass").expect("bypass should exist");
    assert_eq!(found_by_id.kind, PhysicalControlKind::Button);

    assert_eq!(layout.continuous_control_count(), 2);
    // Only "bypass" is a mappable button; page navigation buttons are reserved.
    assert_eq!(layout.button_count(), 1);

    // Reserved page navigation controls are excluded from mappable list.
    let mappable = layout.mappable_controls();
    assert!(
        !mappable
            .iter()
            .any(|c| c.id == "page_prev" || c.id == "page_next")
    );
}

#[test]
fn auto_map_assigns_parameters_to_layout_controls() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mapping = auto_map::auto_map(&layout, params, 0, "TestPlugin");

    assert_eq!(mapping.controller_name, "Test Controller");
    assert_eq!(mapping.plugin_type, "TestPlugin");
    assert!(!mapping.bindings.is_empty());

    // Volume (continuous) should be mapped to a continuous control.
    let volume_binding = mapping
        .bindings
        .iter()
        .find(|b| b.param_index == 0)
        .expect("volume should be mapped");
    assert!(volume_binding.scaling == ValueScaling::Linear);

    // Bypass (bool) should be mapped to a button.
    let bypass_binding = mapping
        .bindings
        .iter()
        .find(|b| b.param_index == 2)
        .expect("bypass should be mapped");
    assert_eq!(bypass_binding.scaling, ValueScaling::Toggle);
}

#[test]
fn mapping_engine_set_param_from_control_change() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout.clone());
    engine.on_plugin_focus("TestPlugin", params, 0);

    // Move volume fader to maximum.
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 7,
        value: 127,
    };
    let action = engine.handle_midi(&msg, params);
    assert!(
        matches!(action, MappingAction::SetParam { plugin_index: 0, param_index: 0, value } if value > 10.0),
        "expected volume near max, got {:?}",
        action
    );
}

#[test]
fn mapping_engine_page_navigation() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout);
    engine.on_plugin_focus("TestPlugin", params, 0);

    // Need more than one page to test navigation; use a tiny layout or increase params.
    if let Some(mapping) = engine.mapping_mut() {
        mapping.total_pages = 3;
    }

    let prev = MidiMessage::ControlChange {
        channel: 0,
        controller: 20,
        value: 127,
    };
    let next = MidiMessage::ControlChange {
        channel: 0,
        controller: 21,
        value: 127,
    };

    assert!(matches!(
        engine.handle_midi(&next, params),
        MappingAction::PageNext
    ));
    assert_eq!(engine.mapping().unwrap().current_page, 1);

    assert!(matches!(
        engine.handle_midi(&prev, params),
        MappingAction::PagePrev
    ));
    assert_eq!(engine.mapping().unwrap().current_page, 0);

    // Prev at page 0 should stay at 0.
    assert!(matches!(
        engine.handle_midi(&prev, params),
        MappingAction::PagePrev
    ));
    assert_eq!(engine.mapping().unwrap().current_page, 0);
}

#[test]
fn mapping_engine_midi_learn_binds_control_to_parameter() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout);
    engine.on_plugin_focus("TestPlugin", params, 0);

    engine.learn_param(0, 1); // Learn Pan (param 1)
    assert!(engine.is_learning());

    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 10,
        value: 64,
    };
    let action = engine.handle_midi(&msg, params);
    assert!(
        matches!(action, MappingAction::LearnComplete { control_id, param_index: 1 } if control_id == "pan")
    );
    assert!(!engine.is_learning());

    // Subsequent movements of the learned control should adjust Pan.
    let action = engine.handle_midi(&msg, params);
    assert!(
        matches!(
            action,
            MappingAction::SetParam {
                plugin_index: 0,
                param_index: 1,
                ..
            }
        ),
        "expected SetParam for pan, got {:?}",
        action
    );
}

#[test]
fn mapping_engine_unmapped_messages_return_unmapped() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout);
    engine.on_plugin_focus("TestPlugin", params, 0);

    // Unknown CC not in layout.
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 99,
        value: 64,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));

    // Pitch bend is not assigned in our test layout.
    let msg = MidiMessage::PitchBend {
        channel: 0,
        value: 8192,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));
}

#[test]
fn mapping_engine_without_layout_returns_unmapped() {
    let params = TEST_PARAMS;
    let mut engine = MidiMappingEngine::new();
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 7,
        value: 64,
    };
    assert!(matches!(
        engine.handle_midi(&msg, params),
        MappingAction::Unmapped
    ));
}

#[test]
fn mapping_engine_feedback_value_roundtrip() {
    let layout = test_controller_layout();
    let params = TEST_PARAMS;

    let mut engine = MidiMappingEngine::new();
    engine.set_layout(layout);
    engine.on_plugin_focus("TestPlugin", params, 0);

    let spec = &params[0]; // Volume
    let feedback = engine.feedback_value(0, 0, 0.0, spec);
    assert!(feedback.is_some());
    let (midi_id, value) = feedback.unwrap();
    assert_eq!(midi_id, MidiControlId::CC(0, 7));
    // 0 dB maps to a mid/high MIDI value via linear scaling from -60..12.
    assert!(value > 0 && value <= 127);
}

#[test]
fn template_registry_add_find_and_convert() {
    let mut registry = TemplateRegistry::new();
    let template = MappingTemplate {
        controller_name: "Test Controller".to_string(),
        plugin_type: "TestPlugin".to_string(),
        bindings: vec![
            TemplateBinding {
                control_id: "volume".to_string(),
                param_index: 0,
                page: 0,
                scaling: ValueScaling::Linear,
            },
            TemplateBinding {
                control_id: "volume".to_string(),
                param_index: 1,
                page: 1,
                scaling: ValueScaling::Linear,
            },
        ],
    };

    registry.add(template);
    assert!(registry.find("Test Controller", "TestPlugin").is_some());
    assert!(registry.find("Other", "TestPlugin").is_none());

    let found = registry.find("Test Controller", "TestPlugin").unwrap();
    let mapping = found.to_mapping(2);
    assert_eq!(mapping.plugin_type, "TestPlugin");
    assert_eq!(mapping.total_pages, 2);
    assert!(mapping.bindings.iter().all(|b| b.plugin_index == 2));
}

#[test]
fn value_scaling_linear_roundtrip() {
    for midi in [0u8, 32, 64, 96, 127] {
        let param = midi_to_param(midi, -60.0, 12.0, ValueScaling::Linear);
        let back = param_to_midi(param, -60.0, 12.0, ValueScaling::Linear);
        assert_eq!(back, midi, "linear roundtrip failed for midi={midi}");
    }
}

#[test]
fn value_scaling_toggle_behavior() {
    assert_eq!(midi_to_param(0, 0.0, 1.0, ValueScaling::Toggle), 0.0);
    assert_eq!(midi_to_param(63, 0.0, 1.0, ValueScaling::Toggle), 0.0);
    assert_eq!(midi_to_param(64, 0.0, 1.0, ValueScaling::Toggle), 1.0);
    assert_eq!(midi_to_param(127, 0.0, 1.0, ValueScaling::Toggle), 1.0);
}

#[test]
fn value_scaling_relative_encoder() {
    assert_eq!(midi_to_param(3, 0.0, 0.0, ValueScaling::Relative), 3.0);
    assert_eq!(midi_to_param(125, 0.0, 0.0, ValueScaling::Relative), -3.0);
    assert_eq!(midi_to_param(0, 0.0, 0.0, ValueScaling::Relative), 0.0);
    assert_eq!(midi_to_param(64, 0.0, 0.0, ValueScaling::Relative), 0.0);
}

#[test]
fn sequencer_clip_and_region_workflow() {
    let mut clip = MidiClip::new(48_000);
    clip.add_event(MidiEvent::note_on(0, 0, 60, 100));
    clip.add_event(MidiEvent::note_off(24_000, 0, 60));
    clip.sort();

    // Query first half of clip.
    let events = clip.events_in_range(0, 24_000);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].message,
        MidiMessage::NoteOn { note: 60, .. }
    ));

    // Place region on timeline and query.
    let region = MidiRegion::new(clip, 96_000);
    assert!(region.overlaps(100_000, 10_000));
    assert!(!region.overlaps(200_000, 10_000));

    let timeline_events = region.events_in_timeline_range(96_000, 24_000);
    assert_eq!(timeline_events.len(), 1);
    assert_eq!(timeline_events[0].0, 0); // relative to query start
}

#[test]
fn midi_clock_tick_scheduling() {
    // 120 BPM, 48000 Hz, 24 PPQ -> 1000 samples per tick.
    assert_eq!(
        clock_tick_interval_samples(120.0, 48_000, MIDI_CLOCK_PPQ),
        Some(1_000.0)
    );

    let offsets = schedule_clock_ticks_for_block(120.0, 48_000, MIDI_CLOCK_PPQ, 0, 2_100);
    assert_eq!(offsets, vec![0, 1_000, 2_000]);

    // Invalid inputs return empty / None.
    assert!(clock_tick_interval_samples(0.0, 48_000, MIDI_CLOCK_PPQ).is_none());
    assert!(schedule_clock_ticks_for_block(120.0, 48_000, MIDI_CLOCK_PPQ, 0, 0).is_empty());
}

#[test]
fn lcxl_profile_identifies_controls() {
    let template = LCXLTemplate::factory_1();
    assert_eq!(template.knob_cc(0, 0), Some(13));
    assert_eq!(template.fader_cc(0), Some(77));

    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 13,
        value: 64,
    };
    assert_eq!(
        LaunchControlXLProfile::identify_control(&msg, &template),
        Some("Top Knob 1".to_string())
    );

    // Wrong channel should not match.
    let msg = MidiMessage::ControlChange {
        channel: 1,
        controller: 13,
        value: 64,
    };
    assert!(LaunchControlXLProfile::identify_control(&msg, &template).is_none());
}

#[test]
fn xone_k2_profile_identifies_controls() {
    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 0,
        value: 64,
    };
    let identified = XoneK2Profile::identify_control(&msg);
    assert!(
        matches!(&identified, Some((_, 64))),
        "expected rotary pot 0 with value 64, got {:?}",
        identified
    );

    let msg = MidiMessage::ControlChange {
        channel: 0,
        controller: 44,
        value: 127,
    };
    let identified = XoneK2Profile::identify_control(&msg);
    assert!(
        matches!(&identified, Some((_, 127))),
        "expected fader 0 with value 127, got {:?}",
        identified
    );
}

#[test]
fn rme_totalmix_fader_routing() {
    assert_eq!(RMETotalMixProfile::cc_for_fader(0), 102);
    assert_eq!(RMETotalMixProfile::cc_for_fader(15), 117);
    assert_eq!(
        RMETotalMixProfile::channel_for_bank(TotalMixRow::Output, 3),
        11
    );
    assert_eq!(RMETotalMixProfile::fader_to_bank(16), (1, 0));

    let press = RMETotalMixProfile::mackie_button_press(0, 16);
    assert_eq!(press.len(), 2);
    assert!(matches!(
        press[0],
        MidiMessage::NoteOn {
            note: 16,
            velocity: 127,
            ..
        }
    ));
    assert!(matches!(
        press[1],
        MidiMessage::NoteOff {
            note: 16,
            velocity: 0,
            ..
        }
    ));
}

#[test]
fn rme_totalmix_control_validation() {
    let mut manager = MidiManager::new().unwrap();
    let totalmix = TotalMixControl::new(&mut manager).unwrap();

    // Without an output connection all sends fail, but validation errors should
    // still be returned for out-of-range arguments before hitting the wire.
    assert!(totalmix.set_fader(TotalMixRow::Input, 4, 0, 100).is_err()); // invalid bank
    assert!(
        totalmix
            .set_fader_global(TotalMixRow::Output, 64, 100)
            .is_err()
    ); // invalid global index
    // Valid arguments still fail because there is no output connection.
    assert!(
        totalmix
            .set_fader_global(TotalMixRow::Output, 0, 100)
            .is_err()
    );
}

#[test]
fn genelec_glm_profile_routing() {
    let profile = GenelecGLMProfile::create_profile();
    assert_eq!(profile.name, "Genelec GLM");
    assert!(profile.get_mapping(GenelecGLMProfile::VOLUME_CC).is_some());

    let custom = GenelecGLMProfile::create_custom_profile(10, 11, 12, 13);
    assert_eq!(custom.get_mapping(10), Some(&"System Volume".to_string()));

    let mut manager = MidiManager::new().unwrap();
    let _glm = GLMControl::new(&mut manager);
}

#[test]
fn built_in_layouts_are_loadable() {
    let lcxl = layouts::lcxl_layout();
    assert!(!lcxl.controls.is_empty());
    assert_eq!(lcxl.name, "Launch Control XL");

    let xone = layouts::xone_k2_layout();
    assert!(!xone.controls.is_empty());
    assert_eq!(xone.name, "Xone:K2");
}
