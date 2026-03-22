//! Novation Launch Control XL MIDI controller profile
//!
//! The Launch Control XL is a USB MIDI controller with:
//! - 24 knobs (8 per row × 3 rows)
//! - 8 faders
//! - 24 buttons with RGB LEDs
//! - 16 templates (8 user, 8 factory)
//!
//! Templates determine which MIDI messages are sent.
//! MIDI channel = template number (1-16).
//!
//! # Example
//!
//! ```no_run
//! use sotf_audio_player_midi::profiles::{LaunchControlXLProfile, LCXLTemplate};
//! use sotf_audio_player_midi::MidiManager;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = MidiManager::new()?;
//!
//! // Get factory template 1 CCs
//! let template = LCXLTemplate::factory_1();
//! println!("Knob 1 CC: {}", template.knob_cc(0, 0).unwrap());
//! println!("Fader 1 CC: {}", template.fader_cc(0).unwrap());
//! # Ok(())
//! # }
//! ```

use crate::config::DeviceProfile;
use crate::message::MidiMessage;

/// Launch Control XL template
#[derive(Debug, Clone)]
pub struct LCXLTemplate {
    /// Template number (1-16)
    pub number: u8,
    /// MIDI channel (0-15, usually template_number - 1)
    pub channel: u8,
    /// CC numbers for knobs [row][column]
    pub knob_ccs: [[u8; 8]; 3],
    /// CC numbers for faders \[fader_index\]
    pub fader_ccs: [u8; 8],
    /// Whether this is a factory template
    pub is_factory: bool,
}

impl LCXLTemplate {
    /// Create Factory Template 1 (default Ableton Live mixer control)
    pub fn factory_1() -> Self {
        Self {
            number: 1,
            channel: 0,
            knob_ccs: [
                [13, 14, 15, 16, 17, 18, 19, 20], // Top row (Send A)
                [29, 30, 31, 32, 33, 34, 35, 36], // Middle row (Send B)
                [49, 50, 51, 52, 53, 54, 55, 56], // Bottom row (Pan)
            ],
            fader_ccs: [77, 78, 79, 80, 81, 82, 83, 84], // Faders (Volume)
            is_factory: true,
        }
    }

    /// Create Factory Template 2
    pub fn factory_2() -> Self {
        let mut template = Self::factory_1();
        template.number = 2;
        template.channel = 1;
        template
    }

    /// Create a custom template
    pub fn custom(number: u8, channel: u8, knob_ccs: [[u8; 8]; 3], fader_ccs: [u8; 8]) -> Self {
        Self {
            number,
            channel,
            knob_ccs,
            fader_ccs,
            is_factory: false,
        }
    }

    /// Get CC number for a knob
    ///
    /// # Arguments
    /// * `row` - Row index (0-2: top, middle, bottom)
    /// * `column` - Column index (0-7)
    pub fn knob_cc(&self, row: u8, column: u8) -> Option<u8> {
        self.knob_ccs
            .get(row as usize)?
            .get(column as usize)
            .copied()
    }

    /// Get CC number for a fader
    ///
    /// # Arguments
    /// * `index` - Fader index (0-7)
    pub fn fader_cc(&self, index: u8) -> Option<u8> {
        self.fader_ccs.get(index as usize).copied()
    }
}

/// Button types on Launch Control XL
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LCXLButton {
    /// Track Focus button (top row, 8 buttons)
    TrackFocus(u8),
    /// Track Control button (bottom row, 8 buttons)
    TrackControl(u8),
    /// Device button (left of function buttons)
    Device,
    /// Mute button
    Mute,
    /// Solo button
    Solo,
    /// Record Arm button
    RecordArm,
}

/// Launch Control XL profile
pub struct LaunchControlXLProfile;

impl LaunchControlXLProfile {
    /// Track Focus button note numbers (top row)
    pub const TRACK_FOCUS_NOTES: [u8; 8] = [41, 42, 43, 44, 57, 58, 59, 60];

    /// Track Control button note numbers (bottom row)
    pub const TRACK_CONTROL_NOTES: [u8; 8] = [73, 74, 75, 76, 89, 90, 91, 92];

    /// Function button notes
    pub const DEVICE_NOTE: u8 = 105;
    pub const MUTE_NOTE: u8 = 106;
    pub const SOLO_NOTE: u8 = 107;
    pub const RECORD_ARM_NOTE: u8 = 108;

    /// Arrow button notes
    pub const UP_NOTE: u8 = 104;
    pub const DOWN_NOTE: u8 = 105;
    pub const LEFT_NOTE: u8 = 106;
    pub const RIGHT_NOTE: u8 = 107;

    /// Create a device profile for Launch Control XL
    pub fn create_profile() -> DeviceProfile {
        let mut profile = DeviceProfile::new("Novation Launch Control XL".to_string());
        profile.description = Some(
            "USB MIDI controller with 24 knobs, 8 faders, and 24 RGB buttons. \
             Supports 16 templates with configurable CC assignments."
                .to_string(),
        );

        let template = LCXLTemplate::factory_1();

        // Add knob mappings
        for row in 0..3 {
            for col in 0..8 {
                if let Some(cc) = template.knob_cc(row, col) {
                    let row_name = match row {
                        0 => "Top",
                        1 => "Middle",
                        2 => "Bottom",
                        _ => "Unknown",
                    };
                    profile.add_mapping(cc, format!("{} Knob {}", row_name, col + 1));
                }
            }
        }

        // Add fader mappings
        for i in 0..8 {
            if let Some(cc) = template.fader_cc(i) {
                profile.add_mapping(cc, format!("Fader {}", i + 1));
            }
        }

        profile
    }

    /// Identify which control a MIDI message corresponds to
    pub fn identify_control(msg: &MidiMessage, template: &LCXLTemplate) -> Option<String> {
        match msg {
            MidiMessage::ControlChange {
                controller,
                channel,
                ..
            } => {
                if *channel != template.channel {
                    return None;
                }

                // Check knobs
                for row in 0..3 {
                    for col in 0..8 {
                        if template.knob_cc(row, col) == Some(*controller) {
                            let row_name = match row {
                                0 => "Top",
                                1 => "Middle",
                                2 => "Bottom",
                                _ => "Unknown",
                            };
                            return Some(format!("{} Knob {}", row_name, col + 1));
                        }
                    }
                }

                // Check faders
                for i in 0..8 {
                    if template.fader_cc(i) == Some(*controller) {
                        return Some(format!("Fader {}", i + 1));
                    }
                }

                None
            }
            MidiMessage::NoteOn { note, channel, .. }
            | MidiMessage::NoteOff { note, channel, .. } => {
                if *channel != template.channel {
                    return None;
                }

                // Check track focus buttons
                if let Some(pos) = Self::TRACK_FOCUS_NOTES.iter().position(|&n| n == *note) {
                    return Some(format!("Track Focus {}", pos + 1));
                }

                // Check track control buttons
                if let Some(pos) = Self::TRACK_CONTROL_NOTES.iter().position(|&n| n == *note) {
                    return Some(format!("Track Control {}", pos + 1));
                }

                // Check function buttons
                match *note {
                    Self::DEVICE_NOTE => Some("Device".to_string()),
                    Self::MUTE_NOTE => Some("Mute".to_string()),
                    Self::SOLO_NOTE => Some("Solo".to_string()),
                    Self::RECORD_ARM_NOTE => Some("Record Arm".to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Set button LED color (requires SysEx)
    ///
    /// LED colors:
    /// - 12: Off
    /// - 13: Red (low)
    /// - 15: Red (full)
    /// - 29: Amber (low)
    /// - 63: Amber (full)
    /// - 62: Yellow (full)
    /// - 28: Green (low)
    /// - 60: Green (full)
    ///
    /// Returns the SysEx message bytes
    pub fn set_button_led(button_note: u8, color: u8, template: u8) -> Vec<u8> {
        vec![
            0xF0, // SysEx start
            0x00,
            0x20,
            0x29, // Novation manufacturer ID
            0x02, // Device ID (Launch Control XL)
            0x0A, // Product ID
            0x78, // LED command
            template,
            button_note,
            color,
            0xF7, // SysEx end
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_template() {
        let template = LCXLTemplate::factory_1();
        assert_eq!(template.number, 1);
        assert_eq!(template.channel, 0);
        assert_eq!(template.knob_cc(0, 0), Some(13));
        assert_eq!(template.knob_cc(2, 7), Some(56));
        assert_eq!(template.fader_cc(0), Some(77));
        assert_eq!(template.fader_cc(7), Some(84));
    }

    #[test]
    fn test_identify_control() {
        let template = LCXLTemplate::factory_1();

        let msg = MidiMessage::ControlChange {
            channel: 0,
            controller: 13,
            value: 64,
        };
        let result = LaunchControlXLProfile::identify_control(&msg, &template);
        assert_eq!(result, Some("Top Knob 1".to_string()));

        let msg2 = MidiMessage::ControlChange {
            channel: 0,
            controller: 77,
            value: 100,
        };
        let result2 = LaunchControlXLProfile::identify_control(&msg2, &template);
        assert_eq!(result2, Some("Fader 1".to_string()));
    }

    #[test]
    fn test_led_sysex() {
        let sysex = LaunchControlXLProfile::set_button_led(41, 15, 0);
        assert_eq!(sysex[0], 0xF0);
        assert_eq!(sysex[sysex.len() - 1], 0xF7);
        assert_eq!(sysex[7], 0); // template
        assert_eq!(sysex[8], 41); // button note
        assert_eq!(sysex[9], 15); // color (red full)
    }
}
