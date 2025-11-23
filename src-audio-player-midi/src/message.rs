//! MIDI message types and encoding/decoding

use crate::error::{MidiError, Result};
use serde::{Deserialize, Serialize};

/// A MIDI message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiMessage {
    /// Note Off: channel (0-15), note (0-127), velocity (0-127)
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },

    /// Note On: channel (0-15), note (0-127), velocity (0-127)
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },

    /// Polyphonic Aftertouch: channel (0-15), note (0-127), pressure (0-127)
    PolyphonicAftertouch {
        channel: u8,
        note: u8,
        pressure: u8,
    },

    /// Control Change: channel (0-15), controller (0-127), value (0-127)
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },

    /// Program Change: channel (0-15), program (0-127)
    ProgramChange { channel: u8, program: u8 },

    /// Channel Aftertouch: channel (0-15), pressure (0-127)
    ChannelAftertouch { channel: u8, pressure: u8 },

    /// Pitch Bend: channel (0-15), value (0-16383, 8192 is center)
    PitchBend { channel: u8, value: u16 },

    /// System Exclusive message
    SystemExclusive { data: Vec<u8> },

    /// Raw MIDI bytes (for unsupported messages)
    Raw { data: Vec<u8> },
}

impl MidiMessage {
    /// Parse a MIDI message from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(MidiError::InvalidMessage("Empty message".to_string()));
        }

        let status = bytes[0];
        let message_type = status & 0xF0;
        let channel = status & 0x0F;

        match message_type {
            0x80 => {
                // Note Off
                if bytes.len() < 3 {
                    return Err(MidiError::InvalidMessage("Note Off too short".to_string()));
                }
                Ok(MidiMessage::NoteOff {
                    channel,
                    note: bytes[1] & 0x7F,
                    velocity: bytes[2] & 0x7F,
                })
            }
            0x90 => {
                // Note On
                if bytes.len() < 3 {
                    return Err(MidiError::InvalidMessage("Note On too short".to_string()));
                }
                let velocity = bytes[2] & 0x7F;
                // Note: velocity 0 is often used as Note Off
                if velocity == 0 {
                    Ok(MidiMessage::NoteOff {
                        channel,
                        note: bytes[1] & 0x7F,
                        velocity: 0,
                    })
                } else {
                    Ok(MidiMessage::NoteOn {
                        channel,
                        note: bytes[1] & 0x7F,
                        velocity,
                    })
                }
            }
            0xA0 => {
                // Polyphonic Aftertouch
                if bytes.len() < 3 {
                    return Err(MidiError::InvalidMessage(
                        "Polyphonic Aftertouch too short".to_string(),
                    ));
                }
                Ok(MidiMessage::PolyphonicAftertouch {
                    channel,
                    note: bytes[1] & 0x7F,
                    pressure: bytes[2] & 0x7F,
                })
            }
            0xB0 => {
                // Control Change
                if bytes.len() < 3 {
                    return Err(MidiError::InvalidMessage(
                        "Control Change too short".to_string(),
                    ));
                }
                Ok(MidiMessage::ControlChange {
                    channel,
                    controller: bytes[1] & 0x7F,
                    value: bytes[2] & 0x7F,
                })
            }
            0xC0 => {
                // Program Change
                if bytes.len() < 2 {
                    return Err(MidiError::InvalidMessage(
                        "Program Change too short".to_string(),
                    ));
                }
                Ok(MidiMessage::ProgramChange {
                    channel,
                    program: bytes[1] & 0x7F,
                })
            }
            0xD0 => {
                // Channel Aftertouch
                if bytes.len() < 2 {
                    return Err(MidiError::InvalidMessage(
                        "Channel Aftertouch too short".to_string(),
                    ));
                }
                Ok(MidiMessage::ChannelAftertouch {
                    channel,
                    pressure: bytes[1] & 0x7F,
                })
            }
            0xE0 => {
                // Pitch Bend
                if bytes.len() < 3 {
                    return Err(MidiError::InvalidMessage("Pitch Bend too short".to_string()));
                }
                let lsb = bytes[1] & 0x7F;
                let msb = bytes[2] & 0x7F;
                let value = ((msb as u16) << 7) | (lsb as u16);
                Ok(MidiMessage::PitchBend { channel, value })
            }
            0xF0 => {
                // System messages
                if status == 0xF0 {
                    // System Exclusive
                    Ok(MidiMessage::SystemExclusive {
                        data: bytes.to_vec(),
                    })
                } else {
                    // Other system messages - store as raw
                    Ok(MidiMessage::Raw {
                        data: bytes.to_vec(),
                    })
                }
            }
            _ => {
                // Unknown message type - store as raw
                Ok(MidiMessage::Raw {
                    data: bytes.to_vec(),
                })
            }
        }
    }

    /// Convert the MIDI message to raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => {
                vec![0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]
            }
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => {
                vec![0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]
            }
            MidiMessage::PolyphonicAftertouch {
                channel,
                note,
                pressure,
            } => {
                vec![0xA0 | (channel & 0x0F), note & 0x7F, pressure & 0x7F]
            }
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => {
                vec![
                    0xB0 | (channel & 0x0F),
                    controller & 0x7F,
                    value & 0x7F,
                ]
            }
            MidiMessage::ProgramChange { channel, program } => {
                vec![0xC0 | (channel & 0x0F), program & 0x7F]
            }
            MidiMessage::ChannelAftertouch { channel, pressure } => {
                vec![0xD0 | (channel & 0x0F), pressure & 0x7F]
            }
            MidiMessage::PitchBend { channel, value } => {
                let lsb = (value & 0x7F) as u8;
                let msb = ((value >> 7) & 0x7F) as u8;
                vec![0xE0 | (channel & 0x0F), lsb, msb]
            }
            MidiMessage::SystemExclusive { data } => data.clone(),
            MidiMessage::Raw { data } => data.clone(),
        }
    }

    /// Get a human-readable description of the message
    pub fn description(&self) -> String {
        match self {
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => {
                format!(
                    "Note Off: ch={}, note={}, vel={}",
                    channel, note, velocity
                )
            }
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => {
                format!("Note On: ch={}, note={}, vel={}", channel, note, velocity)
            }
            MidiMessage::PolyphonicAftertouch {
                channel,
                note,
                pressure,
            } => {
                format!(
                    "Poly Aftertouch: ch={}, note={}, pressure={}",
                    channel, note, pressure
                )
            }
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => {
                format!(
                    "Control Change: ch={}, cc={}, val={}",
                    channel, controller, value
                )
            }
            MidiMessage::ProgramChange { channel, program } => {
                format!("Program Change: ch={}, prog={}", channel, program)
            }
            MidiMessage::ChannelAftertouch { channel, pressure } => {
                format!("Channel Aftertouch: ch={}, pressure={}", channel, pressure)
            }
            MidiMessage::PitchBend { channel, value } => {
                format!("Pitch Bend: ch={}, val={}", channel, value)
            }
            MidiMessage::SystemExclusive { data } => {
                format!("SysEx: {} bytes", data.len())
            }
            MidiMessage::Raw { data } => {
                format!("Raw: {} bytes", data.len())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_on_encoding() {
        let msg = MidiMessage::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        };
        let bytes = msg.to_bytes();
        assert_eq!(bytes, vec![0x90, 60, 100]);

        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_note_off_encoding() {
        let msg = MidiMessage::NoteOff {
            channel: 1,
            note: 64,
            velocity: 0,
        };
        let bytes = msg.to_bytes();
        assert_eq!(bytes, vec![0x81, 64, 0]);

        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_control_change_encoding() {
        let msg = MidiMessage::ControlChange {
            channel: 2,
            controller: 7,
            value: 127,
        };
        let bytes = msg.to_bytes();
        assert_eq!(bytes, vec![0xB2, 7, 127]);

        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_pitch_bend_encoding() {
        let msg = MidiMessage::PitchBend {
            channel: 0,
            value: 8192, // center
        };
        let bytes = msg.to_bytes();
        assert_eq!(bytes, vec![0xE0, 0, 64]);

        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_note_on_zero_velocity_becomes_note_off() {
        let bytes = vec![0x90, 60, 0]; // Note On with velocity 0
        let decoded = MidiMessage::from_bytes(&bytes).unwrap();
        assert_eq!(
            decoded,
            MidiMessage::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0
            }
        );
    }
}
