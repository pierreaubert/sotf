//! Allen & Heath Xone:K2/K3 MIDI controller profile
//!
//! The Xone:K2 and K3 are versatile MIDI controllers with:
//! - 52 physical controls
//! - 3 layers (171 total MIDI commands)
//! - 12 analogue rotary potentiometers
//! - 6 endless rotary encoders with push switches
//! - 4 linear faders
//! - 30 backlit performance switches
//!
//! **Important:** CC numbers are FIXED and cannot be changed.
//! You can only change the MIDI channel, not individual CC assignments.
//!
//! # Example
//!
//! ```no_run
//! use sotf_audio_player_midi::profiles::XoneK2Profile;
//! use sotf_audio_player_midi::MidiManager;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = MidiManager::new()?;
//!
//! // Listen for encoder changes
//! manager.connect_input(0, |msg| {
//!     match XoneK2Profile::identify_control(&msg) {
//!         Some((control, value)) => {
//!             println!("Control: {:?}, Value: {}", control, value);
//!         }
//!         None => {}
//!     }
//! })?;
//! # Ok(())
//! # }
//! ```

use crate::config::DeviceProfile;
use crate::message::MidiMessage;

/// Xone:K2/K3 control types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K2Control {
    /// Rotary potentiometer (12 total)
    RotaryPot(u8),
    /// Endless rotary encoder (6 total)
    Encoder(u8),
    /// Encoder push switch (6 total)
    EncoderSwitch(u8),
    /// Linear fader (4 total)
    Fader(u8),
    /// Performance button (30 total)
    Button(u8),
}

/// Xone:K2/K3 layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K2Layer {
    /// Layer 1 (default)
    Layer1,
    /// Layer 2 (latching)
    Layer2,
    /// Layer 3 (latching)
    Layer3,
}

/// Xone:K2/K3 profile
pub struct XoneK2Profile;

impl XoneK2Profile {
    /// Default MIDI channel
    pub const DEFAULT_CHANNEL: u8 = 0;

    // Rotary Potentiometer CC numbers (Layer 1)
    pub const POT_CC: [u8; 12] = [
        0, 1, 2, 3, // Top row
        4, 5, 6, 7, // Middle row
        8, 9, 10, 11, // Bottom row
    ];

    // Endless Encoder CC numbers (Layer 1)
    pub const ENCODER_CC: [u8; 6] = [
        12, 13, 14, // Top row
        15, 16, 17, // Bottom row
    ];

    // Encoder Switch Note numbers (Layer 1)
    pub const ENCODER_SWITCH_NOTE: [u8; 6] = [
        48, 49, 50, // Top row
        51, 52, 53, // Bottom row
    ];

    // Linear Fader CC numbers (Layer 1)
    pub const FADER_CC: [u8; 4] = [44, 45, 46, 47];

    // Button Note numbers (Layer 1)
    // Top row (8 buttons above pots)
    pub const BUTTON_TOP_NOTE: [u8; 8] = [24, 25, 26, 27, 28, 29, 30, 31];
    // Bottom row (8 buttons below pots)
    pub const BUTTON_BOTTOM_NOTE: [u8; 8] = [32, 33, 34, 35, 36, 37, 38, 39];
    // Side buttons (4 buttons)
    pub const BUTTON_SIDE_NOTE: [u8; 4] = [40, 41, 42, 43];
    // Encoder layer buttons (3 buttons)
    pub const BUTTON_ENCODER_LAYER_NOTE: [u8; 3] = [54, 55, 56];
    // Navigation buttons (4 buttons: up, down, left, right)
    pub const BUTTON_NAV_NOTE: [u8; 4] = [57, 58, 59, 60];
    // Additional layer buttons
    pub const BUTTON_LAYER_NOTE: [u8; 3] = [61, 62, 63];

    /// Create a device profile for Xone:K2/K3
    pub fn create_profile() -> DeviceProfile {
        let mut profile = DeviceProfile::new("Allen & Heath Xone:K2/K3".to_string());
        profile.description = Some(
            "MIDI controller with 52 controls across 3 layers. \
             CC numbers are fixed and cannot be changed."
                .to_string(),
        );

        // Add rotary pot mappings
        for (i, &cc) in Self::POT_CC.iter().enumerate() {
            profile.add_mapping(cc, format!("Rotary Pot {}", i + 1));
        }

        // Add encoder mappings
        for (i, &cc) in Self::ENCODER_CC.iter().enumerate() {
            profile.add_mapping(cc, format!("Encoder {}", i + 1));
        }

        // Add fader mappings
        for (i, &cc) in Self::FADER_CC.iter().enumerate() {
            profile.add_mapping(cc, format!("Fader {}", i + 1));
        }

        profile
    }

    /// Identify which control a MIDI message corresponds to
    ///
    /// Returns (control_type, value) if the message is from a known control.
    pub fn identify_control(msg: &MidiMessage) -> Option<(K2Control, u8)> {
        match msg {
            MidiMessage::ControlChange {
                controller, value, ..
            } => {
                // Check rotary pots
                if let Some(pos) = Self::POT_CC.iter().position(|&cc| cc == *controller) {
                    return Some((K2Control::RotaryPot(pos as u8), *value));
                }
                // Check encoders
                if let Some(pos) = Self::ENCODER_CC.iter().position(|&cc| cc == *controller) {
                    return Some((K2Control::Encoder(pos as u8), *value));
                }
                // Check faders
                if let Some(pos) = Self::FADER_CC.iter().position(|&cc| cc == *controller) {
                    return Some((K2Control::Fader(pos as u8), *value));
                }
                None
            }
            MidiMessage::NoteOn { note, velocity, .. } => {
                // Check encoder switches
                if let Some(pos) = Self::ENCODER_SWITCH_NOTE.iter().position(|&n| n == *note) {
                    return Some((K2Control::EncoderSwitch(pos as u8), *velocity));
                }
                // Check all button arrays
                let all_buttons = [
                    &Self::BUTTON_TOP_NOTE[..],
                    &Self::BUTTON_BOTTOM_NOTE[..],
                    &Self::BUTTON_SIDE_NOTE[..],
                    &Self::BUTTON_ENCODER_LAYER_NOTE[..],
                    &Self::BUTTON_NAV_NOTE[..],
                    &Self::BUTTON_LAYER_NOTE[..],
                ]
                .concat();

                if let Some(pos) = all_buttons.iter().position(|&n| n == *note) {
                    return Some((K2Control::Button(pos as u8), *velocity));
                }
                None
            }
            _ => None,
        }
    }

    /// Get the CC number for a rotary pot
    pub fn pot_cc(index: u8) -> Option<u8> {
        Self::POT_CC.get(index as usize).copied()
    }

    /// Get the CC number for an encoder
    pub fn encoder_cc(index: u8) -> Option<u8> {
        Self::ENCODER_CC.get(index as usize).copied()
    }

    /// Get the CC number for a fader
    pub fn fader_cc(index: u8) -> Option<u8> {
        Self::FADER_CC.get(index as usize).copied()
    }

    /// Get the note number for an encoder switch
    pub fn encoder_switch_note(index: u8) -> Option<u8> {
        Self::ENCODER_SWITCH_NOTE.get(index as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pot_ccs() {
        assert_eq!(XoneK2Profile::pot_cc(0), Some(0));
        assert_eq!(XoneK2Profile::pot_cc(11), Some(11));
        assert_eq!(XoneK2Profile::pot_cc(12), None);
    }

    #[test]
    fn test_encoder_ccs() {
        assert_eq!(XoneK2Profile::encoder_cc(0), Some(12));
        assert_eq!(XoneK2Profile::encoder_cc(5), Some(17));
        assert_eq!(XoneK2Profile::encoder_cc(6), None);
    }

    #[test]
    fn test_fader_ccs() {
        assert_eq!(XoneK2Profile::fader_cc(0), Some(44));
        assert_eq!(XoneK2Profile::fader_cc(3), Some(47));
        assert_eq!(XoneK2Profile::fader_cc(4), None);
    }

    #[test]
    fn test_identify_control() {
        let msg = MidiMessage::ControlChange {
            channel: 0,
            controller: 0,
            value: 64,
        };
        let result = XoneK2Profile::identify_control(&msg);
        assert_eq!(result, Some((K2Control::RotaryPot(0), 64)));

        let msg2 = MidiMessage::ControlChange {
            channel: 0,
            controller: 44,
            value: 127,
        };
        let result2 = XoneK2Profile::identify_control(&msg2);
        assert_eq!(result2, Some((K2Control::Fader(0), 127)));
    }
}
