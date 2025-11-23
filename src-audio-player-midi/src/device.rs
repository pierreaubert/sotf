//! MIDI device information and management

use serde::{Deserialize, Serialize};

/// Information about a MIDI device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDeviceInfo {
    /// Device index
    pub index: usize,

    /// Device name
    pub name: String,

    /// Device type (input or output)
    pub device_type: MidiDeviceType,

    /// Manufacturer (if available)
    pub manufacturer: Option<String>,

    /// Whether the device is currently connected
    pub is_connected: bool,
}

/// Type of MIDI device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiDeviceType {
    /// MIDI input device (receives MIDI messages)
    Input,

    /// MIDI output device (sends MIDI messages)
    Output,
}

/// Represents a MIDI device (either input or output)
#[derive(Debug)]
pub enum MidiDevice {
    /// An input device with its connection
    Input {
        info: MidiDeviceInfo,
        connection: Option<midir::MidiInputConnection<()>>,
    },

    /// An output device with its connection
    Output {
        info: MidiDeviceInfo,
        connection: Option<midir::MidiOutputConnection>,
    },
}

impl MidiDevice {
    /// Create a new input device
    pub fn new_input(index: usize, name: String) -> Self {
        MidiDevice::Input {
            info: MidiDeviceInfo {
                index,
                name,
                device_type: MidiDeviceType::Input,
                manufacturer: None,
                is_connected: false,
            },
            connection: None,
        }
    }

    /// Create a new output device
    pub fn new_output(index: usize, name: String) -> Self {
        MidiDevice::Output {
            info: MidiDeviceInfo {
                index,
                name,
                device_type: MidiDeviceType::Output,
                manufacturer: None,
                is_connected: false,
            },
            connection: None,
        }
    }

    /// Get device information
    pub fn info(&self) -> &MidiDeviceInfo {
        match self {
            MidiDevice::Input { info, .. } => info,
            MidiDevice::Output { info, .. } => info,
        }
    }

    /// Check if the device is connected
    pub fn is_connected(&self) -> bool {
        match self {
            MidiDevice::Input { connection, .. } => connection.is_some(),
            MidiDevice::Output { connection, .. } => connection.is_some(),
        }
    }

    /// Get device type
    pub fn device_type(&self) -> MidiDeviceType {
        self.info().device_type
    }
}
