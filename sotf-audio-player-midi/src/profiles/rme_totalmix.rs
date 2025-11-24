//! RME UFX+ / TotalMix FX MIDI control profile
//!
//! Provides comprehensive control over RME TotalMix FX via MIDI.
//!
//! TotalMix uses a matrix layout with 3 rows:
//! - Top row: Input channels
//! - Middle row: Playback channels
//! - Bottom row: Output channels
//!
//! Each row can have up to 64 faders, controlled via:
//! - CC 102-117 (16 faders per bank)
//! - MIDI channels 1-4 for different banks (4 banks × 16 = 64 faders)
//!
//! # Example
//!
//! ```no_run
//! use sotf_audio_player_midi::profiles::{RMETotalMixProfile, TotalMixControl, TotalMixRow};
//! use sotf_audio_player_midi::MidiManager;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = MidiManager::new()?;
//! let mut totalmix = TotalMixControl::new(&mut manager)?;
//!
//! // Set volume for output channel 1 to 75%
//! totalmix.set_fader(TotalMixRow::Output, 0, 0, 95)?;
//!
//! // Mute input channel 5
//! totalmix.mute_channel(TotalMixRow::Input, 0, 4)?;
//!
//! // Set main output volume
//! totalmix.set_main_volume(100)?;
//! # Ok(())
//! # }
//! ```

use crate::config::DeviceProfile;
use crate::error::{MidiError, Result};
use crate::manager::MidiManager;
use crate::message::MidiMessage;

/// TotalMix row selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotalMixRow {
    /// Input channels (top row)
    Input,
    /// Playback channels (middle row)
    Playback,
    /// Output channels (bottom row)
    Output,
}

impl TotalMixRow {
    /// Get the base MIDI channel for this row
    pub fn base_channel(&self) -> u8 {
        match self {
            TotalMixRow::Input => 0,    // Channels 1-4 (MIDI 0-3)
            TotalMixRow::Playback => 4, // Channels 5-8 (MIDI 4-7)
            TotalMixRow::Output => 8,   // Channels 9-12 (MIDI 8-11)
        }
    }
}

/// RME TotalMix FX profile
pub struct RMETotalMixProfile;

impl RMETotalMixProfile {
    /// CC numbers for fader control (102-117)
    pub const FADER_CC_START: u8 = 102;
    pub const FADER_CC_END: u8 = 117;
    pub const FADERS_PER_BANK: u8 = 16;

    /// Main output volume control
    pub const MAIN_VOLUME_CC: u8 = 7;
    pub const MAIN_VOLUME_CHANNEL: u8 = 0;

    /// Mackie Control Protocol constants
    pub const MACKIE_MUTE_NOTE_START: u8 = 16;
    pub const MACKIE_SOLO_NOTE_START: u8 = 8;
    pub const MACKIE_SELECT_NOTE_START: u8 = 0;
    pub const MACKIE_PAN_CC_START: u8 = 16;

    /// Create a device profile for RME TotalMix FX
    pub fn create_profile() -> DeviceProfile {
        let mut profile = DeviceProfile::new("RME UFX+ TotalMix FX".to_string());
        profile.description = Some("Complete MIDI control for RME TotalMix FX".to_string());

        // Add fader mappings
        for cc in Self::FADER_CC_START..=Self::FADER_CC_END {
            let fader_num = cc - Self::FADER_CC_START;
            profile.add_mapping(cc, format!("Fader {}", fader_num + 1));
        }

        // Add main volume
        profile.add_mapping(Self::MAIN_VOLUME_CC, "Main Volume".to_string());

        // Add pan controls
        for i in 0..8 {
            profile.add_mapping(Self::MACKIE_PAN_CC_START + i, format!("Pan {}", i + 1));
        }

        profile
    }

    /// Calculate MIDI channel for a specific bank within a row
    ///
    /// # Arguments
    /// * `row` - The TotalMix row (Input/Playback/Output)
    /// * `bank` - Bank number (0-3, each bank has 16 faders)
    pub fn channel_for_bank(row: TotalMixRow, bank: u8) -> u8 {
        row.base_channel() + bank.min(3)
    }

    /// Calculate CC number for a fader within its bank
    ///
    /// # Arguments
    /// * `fader_index` - Fader index within the bank (0-15)
    pub fn cc_for_fader(fader_index: u8) -> u8 {
        Self::FADER_CC_START + fader_index.min(15)
    }

    /// Convert a global fader index to (bank, fader_in_bank)
    ///
    /// # Arguments
    /// * `global_index` - Global fader index (0-63)
    ///
    /// # Returns
    /// (bank_number, fader_index_in_bank)
    pub fn fader_to_bank(global_index: u8) -> (u8, u8) {
        let bank = global_index / Self::FADERS_PER_BANK;
        let fader = global_index % Self::FADERS_PER_BANK;
        (bank, fader)
    }
}

/// High-level control interface for TotalMix FX
pub struct TotalMixControl<'a> {
    manager: &'a mut MidiManager,
}

impl<'a> TotalMixControl<'a> {
    /// Create a new TotalMix control interface
    pub fn new(manager: &'a mut MidiManager) -> Result<Self> {
        Ok(Self { manager })
    }

    /// Set a fader value
    ///
    /// # Arguments
    /// * `row` - Which row (Input/Playback/Output)
    /// * `bank` - Bank number (0-3)
    /// * `fader` - Fader index within bank (0-15)
    /// * `value` - Fader value (0-127)
    pub fn set_fader(&self, row: TotalMixRow, bank: u8, fader: u8, value: u8) -> Result<()> {
        if bank > 3 {
            return Err(MidiError::InvalidMessage(format!(
                "Bank must be 0-3, got {}",
                bank
            )));
        }
        if fader > 15 {
            return Err(MidiError::InvalidMessage(format!(
                "Fader must be 0-15, got {}",
                fader
            )));
        }

        let channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let cc = RMETotalMixProfile::cc_for_fader(fader);

        self.manager.send_message(&MidiMessage::ControlChange {
            channel,
            controller: cc,
            value,
        })
    }

    /// Set a fader by global index (0-63)
    ///
    /// # Arguments
    /// * `row` - Which row (Input/Playback/Output)
    /// * `global_index` - Global fader index (0-63)
    /// * `value` - Fader value (0-127)
    pub fn set_fader_global(&self, row: TotalMixRow, global_index: u8, value: u8) -> Result<()> {
        if global_index > 63 {
            return Err(MidiError::InvalidMessage(format!(
                "Global index must be 0-63, got {}",
                global_index
            )));
        }

        let (bank, fader) = RMETotalMixProfile::fader_to_bank(global_index);
        self.set_fader(row, bank, fader, value)
    }

    /// Set main output volume
    ///
    /// # Arguments
    /// * `value` - Volume value (0-127)
    pub fn set_main_volume(&self, value: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: RMETotalMixProfile::MAIN_VOLUME_CHANNEL,
            controller: RMETotalMixProfile::MAIN_VOLUME_CC,
            value,
        })
    }

    /// Mute a channel (Mackie Control protocol)
    ///
    /// # Arguments
    /// * `row` - Which row
    /// * `bank` - Bank number (0-3)
    /// * `channel` - Channel within bank (0-7 for Mackie)
    pub fn mute_channel(&self, row: TotalMixRow, bank: u8, channel: u8) -> Result<()> {
        if channel > 7 {
            return Err(MidiError::InvalidMessage(format!(
                "Mackie channel must be 0-7, got {}",
                channel
            )));
        }

        let midi_channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let note = RMETotalMixProfile::MACKIE_MUTE_NOTE_START + channel;

        self.manager.send_message(&MidiMessage::NoteOn {
            channel: midi_channel,
            note,
            velocity: 127,
        })
    }

    /// Unmute a channel
    pub fn unmute_channel(&self, row: TotalMixRow, bank: u8, channel: u8) -> Result<()> {
        if channel > 7 {
            return Err(MidiError::InvalidMessage(format!(
                "Mackie channel must be 0-7, got {}",
                channel
            )));
        }

        let midi_channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let note = RMETotalMixProfile::MACKIE_MUTE_NOTE_START + channel;

        self.manager.send_message(&MidiMessage::NoteOff {
            channel: midi_channel,
            note,
            velocity: 0,
        })
    }

    /// Solo a channel (Mackie Control protocol)
    pub fn solo_channel(&self, row: TotalMixRow, bank: u8, channel: u8) -> Result<()> {
        if channel > 7 {
            return Err(MidiError::InvalidMessage(format!(
                "Mackie channel must be 0-7, got {}",
                channel
            )));
        }

        let midi_channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let note = RMETotalMixProfile::MACKIE_SOLO_NOTE_START + channel;

        self.manager.send_message(&MidiMessage::NoteOn {
            channel: midi_channel,
            note,
            velocity: 127,
        })
    }

    /// Unsolo a channel
    pub fn unsolo_channel(&self, row: TotalMixRow, bank: u8, channel: u8) -> Result<()> {
        if channel > 7 {
            return Err(MidiError::InvalidMessage(format!(
                "Mackie channel must be 0-7, got {}",
                channel
            )));
        }

        let midi_channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let note = RMETotalMixProfile::MACKIE_SOLO_NOTE_START + channel;

        self.manager.send_message(&MidiMessage::NoteOff {
            channel: midi_channel,
            note,
            velocity: 0,
        })
    }

    /// Set pan for a channel (Mackie Control protocol)
    ///
    /// # Arguments
    /// * `row` - Which row
    /// * `bank` - Bank number (0-3)
    /// * `channel` - Channel within bank (0-7)
    /// * `value` - Pan value (0=left, 64=center, 127=right)
    pub fn set_pan(&self, row: TotalMixRow, bank: u8, channel: u8, value: u8) -> Result<()> {
        if channel > 7 {
            return Err(MidiError::InvalidMessage(format!(
                "Mackie channel must be 0-7, got {}",
                channel
            )));
        }

        let midi_channel = RMETotalMixProfile::channel_for_bank(row, bank);
        let cc = RMETotalMixProfile::MACKIE_PAN_CC_START + channel;

        self.manager.send_message(&MidiMessage::ControlChange {
            channel: midi_channel,
            controller: cc,
            value,
        })
    }

    /// Recall a TotalMix snapshot
    ///
    /// # Arguments
    /// * `snapshot` - Snapshot number (0-127)
    pub fn recall_snapshot(&self, snapshot: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ProgramChange {
            channel: 0,
            program: snapshot,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_channels() {
        assert_eq!(TotalMixRow::Input.base_channel(), 0);
        assert_eq!(TotalMixRow::Playback.base_channel(), 4);
        assert_eq!(TotalMixRow::Output.base_channel(), 8);
    }

    #[test]
    fn test_bank_calculation() {
        assert_eq!(
            RMETotalMixProfile::channel_for_bank(TotalMixRow::Input, 0),
            0
        );
        assert_eq!(
            RMETotalMixProfile::channel_for_bank(TotalMixRow::Input, 1),
            1
        );
        assert_eq!(
            RMETotalMixProfile::channel_for_bank(TotalMixRow::Playback, 0),
            4
        );
        assert_eq!(
            RMETotalMixProfile::channel_for_bank(TotalMixRow::Output, 3),
            11
        );
    }

    #[test]
    fn test_fader_cc() {
        assert_eq!(RMETotalMixProfile::cc_for_fader(0), 102);
        assert_eq!(RMETotalMixProfile::cc_for_fader(15), 117);
    }

    #[test]
    fn test_fader_to_bank() {
        assert_eq!(RMETotalMixProfile::fader_to_bank(0), (0, 0));
        assert_eq!(RMETotalMixProfile::fader_to_bank(15), (0, 15));
        assert_eq!(RMETotalMixProfile::fader_to_bank(16), (1, 0));
        assert_eq!(RMETotalMixProfile::fader_to_bank(63), (3, 15));
    }
}
