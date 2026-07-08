use super::input_buffer::InputBuffer;
use super::midi_manager::MidiManager;
use crate::message::MidiMessage;

#[test]
fn test_manager_creation() {
    let manager = MidiManager::new();
    assert!(manager.is_ok());
}

#[test]
fn test_device_enumeration() {
    let mut manager = MidiManager::new().unwrap();

    // Should not panic even if no devices are available
    let inputs = manager.list_input_devices();
    if let Err(err) = inputs {
        eprintln!("Skipping MIDI enumeration test: input backend unavailable ({err})");
        return;
    }

    let outputs = manager.list_output_devices();
    if let Err(err) = outputs {
        eprintln!("Skipping MIDI enumeration test: output backend unavailable ({err})");
    }
}

#[test]
fn input_buffer_reassembles_split_sysex() {
    let mut buffer = InputBuffer::new();

    assert!(buffer.parse_message(&[0xF0, 0x7D, 0x01]).is_none());
    let msg = buffer.parse_message(&[0x02, 0x03, 0xF7]).unwrap().unwrap();

    assert_eq!(
        msg,
        MidiMessage::SystemExclusive {
            data: vec![0xF0, 0x7D, 0x01, 0x02, 0x03, 0xF7]
        }
    );
}

#[test]
fn input_buffer_passes_stack_sized_system_messages() {
    let mut buffer = InputBuffer::new();

    let msg = buffer.parse_message(&[0xF8]).unwrap().unwrap();

    assert_eq!(
        msg,
        MidiMessage::System {
            status: 0xF8,
            data: [0, 0],
            len: 0
        }
    );
}

#[test]
fn input_buffer_preserves_sysex_when_realtime_arrives_mid_packet() {
    let mut buffer = InputBuffer::new();

    assert!(buffer.parse_message(&[0xF0, 0x7D, 0x01]).is_none());
    let clock = buffer.parse_message(&[0xF8]).unwrap().unwrap();
    assert!(matches!(clock, MidiMessage::System { status: 0xF8, .. }));

    let sysex = buffer.parse_message(&[0x02, 0xF7]).unwrap().unwrap();
    assert_eq!(
        sysex,
        MidiMessage::SystemExclusive {
            data: vec![0xF0, 0x7D, 0x01, 0x02, 0xF7]
        }
    );
}

fn assert_missing_device_or_skip_backend(result: crate::error::Result<()>, device_kind: &str) {
    match result {
        Err(crate::error::MidiError::ConnectionError(_)) => {}
        Err(crate::error::MidiError::InitError(err)) => {
            eprintln!(
                "Skipping missing {device_kind} device test: MIDI backend unavailable ({err})"
            );
        }
        other => panic!(
            "expected ConnectionError for missing {device_kind} device, got {:?}",
            other
        ),
    }
}

#[test]
fn connect_input_by_name_missing_device_returns_error() {
    let mut manager = MidiManager::new().unwrap();

    let result = manager.connect_input_by_name("Definitely Not Present Input", |_msg| {});
    assert_missing_device_or_skip_backend(result, "input");
}

#[test]
fn connect_output_by_name_missing_device_returns_error() {
    let mut manager = MidiManager::new().unwrap();

    let result = manager.connect_output_by_name("Definitely Not Present Output");
    assert_missing_device_or_skip_backend(result, "output");
}

#[test]
fn manager_with_missing_default_devices_constructs_safely() {
    use crate::config::MidiConfig;

    let config = MidiConfig {
        default_input: Some("Missing Input".to_string()),
        default_output: Some("Missing Output".to_string()),
        ..Default::default()
    };

    let manager = MidiManager::with_config(config).unwrap();
    assert!(!manager.is_input_connected());
    assert!(!manager.is_output_connected());
    assert_eq!(
        manager.config().default_input,
        Some("Missing Input".to_string())
    );
}
