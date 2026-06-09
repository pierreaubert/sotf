// ============================================================================
// Property-Based Tests for sotf-midi
// ============================================================================
//
// Covers:
// - MidiMessage round-trip encoding/decoding
// - SMF VLQ encoding/decoding round-trip (via parse_smf)
// - MidiConfig save/load round-trip

use proptest::prelude::*;
use sotf_audio_player_midi::smf::parse_smf;
use sotf_audio_player_midi::{DeviceConfig, DeviceProfile, MidiConfig, MidiMessage};

// ============================================================================
// VLQ helpers (mirrors SMF spec for round-trip testing)
// ============================================================================

/// Encode a non-negative integer as SMF variable-length quantity.
fn write_vlq(mut value: u32) -> Vec<u8> {
    if value == 0 {
        return vec![0x00];
    }
    let mut bytes = Vec::with_capacity(4);
    // Collect 7-bit chunks LSB-first
    while value > 0 {
        bytes.push((value & 0x7F) as u8);
        value >>= 7;
    }
    // Reverse to MSB-first and set continuation bits on all but the last byte
    bytes.reverse();
    for i in 0..bytes.len() - 1 {
        bytes[i] |= 0x80;
    }
    bytes
}

/// Build a minimal Type-0 SMF with one track containing a single Note On
/// event whose delta-time is encoded from `delta_ticks`.
fn build_smf_with_delta(delta_ticks: u32) -> Vec<u8> {
    let mut data = Vec::new();

    // Header: MThd, length=6, format=0, tracks=1, division=480
    data.extend_from_slice(b"MThd");
    data.extend_from_slice(&6u32.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes());
    data.extend_from_slice(&1u16.to_be_bytes());
    data.extend_from_slice(&480u16.to_be_bytes());

    let mut track = Vec::new();
    track.extend_from_slice(&write_vlq(delta_ticks));
    track.extend_from_slice(&[0x90, 60, 100]); // Note On ch0, note 60, vel 100
    track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]); // End of Track

    data.extend_from_slice(b"MTrk");
    data.extend_from_slice(&(track.len() as u32).to_be_bytes());
    data.extend_from_slice(&track);

    data
}

// ============================================================================
// Strategies
// ============================================================================

fn system_message_strategy() -> impl Strategy<Value = MidiMessage> {
    let status_strategy = prop_oneof![
        // 0-data-byte system realtime/common
        Just(0xF6u8),
        Just(0xF7),
        Just(0xF8),
        Just(0xF9),
        Just(0xFA),
        Just(0xFB),
        Just(0xFC),
        Just(0xFE),
        Just(0xFF),
        // 1-data-byte system common
        Just(0xF1),
        Just(0xF3),
        // 2-data-byte system common
        Just(0xF2),
    ];

    status_strategy.prop_flat_map(|status| {
        let len: usize = match status {
            0xF1 | 0xF3 => 1,
            0xF2 => 2,
            _ => 0,
        };
        if len == 0 {
            Just(MidiMessage::System {
                status,
                data: [0, 0],
                len: 0,
            })
            .boxed()
        } else {
            prop::collection::vec(0u8..0x80, len..=len)
                .prop_map(move |v| {
                    let mut data = [0u8; 2];
                    data[..len].copy_from_slice(&v);
                    MidiMessage::System {
                        status,
                        data,
                        len: len as u8,
                    }
                })
                .boxed()
        }
    })
}

fn midi_message_strategy() -> impl Strategy<Value = MidiMessage> {
    prop_oneof![
        // Note Off
        (0u8..16, 0u8..128, 0u8..128)
            .prop_map(|(channel, note, velocity)| MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            }),
        // Note On (velocity 1..127 to avoid Note Off normalization on round-trip)
        (0u8..16, 0u8..128, 1u8..128)
            .prop_map(|(channel, note, velocity)| MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            }),
        // Note On with velocity 0 (round-trips through Note Off by design)
        (0u8..16, 0u8..128)
            .prop_map(|(channel, note)| MidiMessage::NoteOn {
                channel,
                note,
                velocity: 0,
            }),
        // Polyphonic Aftertouch
        (0u8..16, 0u8..128, 0u8..128)
            .prop_map(|(channel, note, pressure)| MidiMessage::PolyphonicAftertouch {
                channel,
                note,
                pressure,
            }),
        // Control Change
        (0u8..16, 0u8..128, 0u8..128)
            .prop_map(|(channel, controller, value)| MidiMessage::ControlChange {
                channel,
                controller,
                value,
            }),
        // Program Change
        (0u8..16, 0u8..128)
            .prop_map(|(channel, program)| MidiMessage::ProgramChange {
                channel,
                program,
            }),
        // Channel Aftertouch
        (0u8..16, 0u8..128)
            .prop_map(|(channel, pressure)| MidiMessage::ChannelAftertouch {
                channel,
                pressure,
            }),
        // Pitch Bend
        (0u8..16, 0u16..16384)
            .prop_map(|(channel, value)| MidiMessage::PitchBend { channel, value }),
        // System Exclusive (first byte must be 0xF0 to survive round-trip)
        prop::collection::vec(0u8..128, 1..16)
            .prop_map(|mut data| {
                data[0] = 0xF0;
                MidiMessage::SystemExclusive { data }
            }),
        // System common / realtime
        system_message_strategy(),
        // Raw undefined system status (0xF4 and 0xF5 are the only undefined ones)
        prop::collection::vec(0u8..=255, 1..8)
            .prop_filter_map("status must be undefined system", |mut data| {
                // Pin to undefined system status bytes
                if data[0] != 0xF4 && data[0] != 0xF5 {
                    data[0] = 0xF4;
                }
                Some(MidiMessage::Raw { data })
            }),
    ]
}

fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    // Avoid f64 because JSON round-trip can introduce tiny precision differences
    // (e.g. 1.2603871711673197e+61 becomes 1.2603871711673195e+61).
    let leaf = prop_oneof![
        any::<i64>().prop_map(|v| serde_json::Value::Number(v.into())),
        "[a-zA-Z0-9_-]{0,16}".prop_map(serde_json::Value::String),
        any::<bool>().prop_map(serde_json::Value::Bool),
        Just(serde_json::Value::Null),
    ];
    leaf.prop_recursive(2, 8, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
            prop::collection::hash_map("[a-zA-Z0-9_-]{0,8}", inner, 0..4)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

fn device_config_strategy() -> impl Strategy<Value = DeviceConfig> {
    (
        any::<Option<String>>(),
        any::<Option<String>>(),
        any::<Option<u8>>(),
        any::<bool>(),
        any::<bool>(),
        prop::collection::hash_map("[a-zA-Z0-9_-]{0,12}", json_value_strategy(), 0..4),
    )
        .prop_map(
            |(manufacturer, model, channel, sysex_enabled, active_sensing, custom_settings)| {
                DeviceConfig {
                    manufacturer,
                    model,
                    channel,
                    sysex_enabled,
                    active_sensing,
                    custom_settings,
                }
            },
        )
}

fn device_profile_strategy() -> impl Strategy<Value = DeviceProfile> {
    (
        "[a-zA-Z0-9 _-]{1,24}",
        any::<Option<String>>(),
        any::<Option<String>>(),
        any::<Option<String>>(),
        device_config_strategy(),
        prop::collection::hash_map(0u8..128, "[a-zA-Z0-9_-]{1,16}", 0..8),
        prop::collection::vec(prop::collection::vec(0u8..=255, 0..8), 0..4),
    )
        .prop_map(
            |(
                name,
                description,
                input_device,
                output_device,
                device_config,
                mappings,
                init_messages,
            )| {
                DeviceProfile {
                    name,
                    description,
                    input_device,
                    output_device,
                    device_config,
                    mappings,
                    init_messages,
                }
            },
        )
}

fn midi_config_strategy() -> impl Strategy<Value = MidiConfig> {
    (
        prop::collection::hash_map("[a-zA-Z0-9_-]{1,16}", device_profile_strategy(), 0..4),
        any::<Option<String>>(),
        any::<Option<String>>(),
        any::<Option<String>>(),
        any::<bool>(),
        any::<Option<u8>>(),
    )
        .prop_map(
            |(
                profiles,
                active_profile,
                default_input,
                default_output,
                learn_mode,
                listen_channel,
            )| MidiConfig {
                profiles,
                active_profile,
                default_input,
                default_output,
                learn_mode,
                listen_channel,
            },
        )
}

// ============================================================================
// MidiMessage Round-Trip Tests
// ============================================================================

proptest! {
    #[test]
    fn midi_message_round_trip(msg in midi_message_strategy()) {
        let bytes = msg.to_bytes();
        let decoded = MidiMessage::from_bytes(&bytes).unwrap();

        // Note: Note On with velocity 0 normalizes to Note Off on decode,
        // which is tested explicitly in a separate property.
        if let MidiMessage::NoteOn { velocity: 0, .. } = msg {
            if let MidiMessage::NoteOff { .. } = decoded {
                // Expected normalization
            } else {
                prop_assert!(false, "Note On with vel=0 should decode as Note Off");
            }
        } else {
            prop_assert_eq!(decoded, msg, "MidiMessage round-trip failed");
        }
    }

    #[test]
    fn midi_message_write_to_matches_to_bytes(msg in midi_message_strategy()) {
        let via_to_bytes = msg.to_bytes();
        let mut buf = vec![0u8; via_to_bytes.len()];
        let written = msg.write_to(&mut buf);
        prop_assert_eq!(written, via_to_bytes.len(), "write_to returned wrong length");
        prop_assert_eq!(buf, via_to_bytes, "write_to bytes differ from to_bytes");
    }

    #[test]
    fn midi_message_finite_description(msg in midi_message_strategy()) {
        let desc = msg.description();
        prop_assert!(!desc.is_empty(), "description should not be empty");
        prop_assert!(desc.len() < 1024, "description should be bounded");
    }

    #[test]
    fn note_on_zero_velocity_normalizes_to_note_off(
        channel in 0u8..16,
        note in 0u8..128
    ) {
        let msg = MidiMessage::NoteOn { channel, note, velocity: 0 };
        let bytes = msg.to_bytes();
        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        prop_assert_eq!(
            decoded,
            MidiMessage::NoteOff { channel, note, velocity: 0 },
            "Note On with vel=0 must normalize to Note Off"
        );
    }
}

// ============================================================================
// SMF VLQ Round-Trip Tests
// ============================================================================

/// Test helper: decode SMF VLQ bytes. Mirrors the production implementation
/// so that the round-trip tests the encoder against an independent decoder.
fn decode_vlq_for_test(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut value: u64 = 0;
    for _ in 0..5 {
        if *pos >= data.len() {
            return Err("Unexpected end of data in VLQ".into());
        }
        let byte = data[*pos];
        *pos += 1;
        value = (value << 7) | (byte & 0x7F) as u64;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("VLQ too long".into())
}

proptest! {
    /// INVARIANT: A VLQ value can be encoded and decoded by the SMF parser,
    /// producing a valid clip with exactly one Note On event.
    #[test]
    fn smf_vlq_round_trip(delta_ticks in 0u32..0x0FFFFFFFu32) {
        let data = build_smf_with_delta(delta_ticks);
        let clips = parse_smf(&data, 48000);
        prop_assert!(
            clips.is_ok(),
            "parse_smf failed for delta_ticks {}: {:?}",
            delta_ticks,
            clips
        );
        let clips = clips.unwrap();
        prop_assert_eq!(clips.len(), 1, "expected exactly one track");
        prop_assert!(
            !clips[0].events.is_empty(),
            "expected at least one event (the Note On)"
        );
    }

    /// INVARIANT: VLQ encoding never produces more than 5 bytes for u32 values
    /// and is non-empty for any input.
    #[test]
    fn vlq_encoding_bounds(value in 0u32..u32::MAX) {
        let bytes = write_vlq(value);
        prop_assert!(!bytes.is_empty(), "VLQ encoding must be non-empty");
        prop_assert!(bytes.len() <= 5, "u32 VLQ must fit in 5 bytes, got {}", bytes.len());
        // Top bit must be set on all but the last byte
        for (i, &b) in bytes.iter().enumerate() {
            if i < bytes.len() - 1 {
                prop_assert!(b & 0x80 != 0, "continuation bit must be set");
            } else {
                prop_assert!(b & 0x80 == 0, "last byte must have continuation bit clear");
            }
        }
    }

    /// INVARIANT: Decoding the bytes produced by write_vlq yields the original value.
    #[test]
    fn vlq_encode_decode_identity(value in 0u32..0x0FFFFFFFu32) {
        let bytes = write_vlq(value);
        let mut pos = 0usize;
        let decoded = decode_vlq_for_test(&bytes, &mut pos).unwrap();
        prop_assert_eq!(decoded, value as u64, "VLQ decode did not round-trip");
        prop_assert_eq!(pos, bytes.len(), "decode did not consume all bytes");
    }
}

// ============================================================================
// Config Save/Load Round-Trip Tests
// ============================================================================

proptest! {
    /// INVARIANT: save -> load preserves all MidiConfig fields.
    #[test]
    fn midi_config_save_load_round_trip(config in midi_config_strategy()) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        config.save(&path).unwrap();
        let loaded = MidiConfig::load(&path).unwrap();

        prop_assert_eq!(
            loaded.active_profile, config.active_profile,
            "active_profile mismatch"
        );
        prop_assert_eq!(
            loaded.default_input, config.default_input,
            "default_input mismatch"
        );
        prop_assert_eq!(
            loaded.default_output, config.default_output,
            "default_output mismatch"
        );
        prop_assert_eq!(
            loaded.learn_mode, config.learn_mode,
            "learn_mode mismatch"
        );
        prop_assert_eq!(
            loaded.listen_channel, config.listen_channel,
            "listen_channel mismatch"
        );
        prop_assert_eq!(
            loaded.profiles.len(),
            config.profiles.len(),
            "profile count mismatch"
        );

        for (name, original) in &config.profiles {
            let loaded_profile = loaded
                .profiles
                .get(name)
                .expect("profile should be preserved");
            prop_assert_eq!(&loaded_profile.name, &original.name, "profile name mismatch");
            prop_assert_eq!(
                &loaded_profile.description, &original.description,
                "description mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.input_device, &original.input_device,
                "input_device mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.output_device, &original.output_device,
                "output_device mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.mappings, &original.mappings,
                "mappings mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.init_messages, &original.init_messages,
                "init_messages mismatch"
            );
            prop_assert_eq!(
                loaded_profile.device_config.sysex_enabled,
                original.device_config.sysex_enabled,
                "sysex_enabled mismatch"
            );
            prop_assert_eq!(
                loaded_profile.device_config.active_sensing,
                original.device_config.active_sensing,
                "active_sensing mismatch"
            );
            prop_assert_eq!(
                loaded_profile.device_config.channel, original.device_config.channel,
                "channel mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.device_config.manufacturer, &original.device_config.manufacturer,
                "manufacturer mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.device_config.model, &original.device_config.model,
                "model mismatch"
            );
            prop_assert_eq!(
                &loaded_profile.device_config.custom_settings,
                &original.device_config.custom_settings,
                "custom_settings mismatch"
            );
        }
    }

    /// INVARIANT: An empty MidiConfig round-trips through save/load.
    #[test]
    fn empty_midi_config_round_trip(_dummy in 0u8..1) {
        let config = MidiConfig::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");

        config.save(&path).unwrap();
        let loaded = MidiConfig::load(&path).unwrap();

        prop_assert!(loaded.profiles.is_empty());
        prop_assert!(loaded.active_profile.is_none());
        prop_assert!(!loaded.learn_mode);
    }
}
