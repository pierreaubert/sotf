//! MIDI device information and management

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Information about a MIDI device
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidiDeviceType {
    /// MIDI input device (receives MIDI messages)
    Input,

    /// MIDI output device (sends MIDI messages)
    Output,
}

/// Point-in-time MIDI device list used for polling-based hot-plug detection.
///
/// Snapshots contain metadata only; they do not own input/output connections and
/// can be polled independently of active MIDI I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiDeviceSnapshot {
    pub inputs: Vec<MidiDeviceInfo>,
    pub outputs: Vec<MidiDeviceInfo>,
}

impl MidiDeviceSnapshot {
    pub fn new(inputs: Vec<MidiDeviceInfo>, outputs: Vec<MidiDeviceInfo>) -> Self {
        Self { inputs, outputs }
    }

    /// Return connected/disconnected events needed to move from `self` to `next`.
    pub fn diff(&self, next: &Self) -> Vec<MidiDeviceChange> {
        let old = self.device_keys();
        let new = next.device_keys();
        let mut changes = Vec::new();

        for device in self.inputs.iter().chain(self.outputs.iter()) {
            if !new.contains(&device_key(device)) {
                changes.push(MidiDeviceChange {
                    kind: MidiDeviceChangeKind::Disconnected,
                    device: device.clone(),
                });
            }
        }

        for device in next.inputs.iter().chain(next.outputs.iter()) {
            if !old.contains(&device_key(device)) {
                changes.push(MidiDeviceChange {
                    kind: MidiDeviceChangeKind::Connected,
                    device: device.clone(),
                });
            }
        }

        changes
    }

    fn device_keys(&self) -> HashSet<DeviceKey> {
        self.inputs
            .iter()
            .chain(self.outputs.iter())
            .map(device_key)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiDeviceChange {
    pub kind: MidiDeviceChangeKind,
    pub device: MidiDeviceInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiDeviceChangeKind {
    Connected,
    Disconnected,
}

type DeviceKey = (MidiDeviceType, String);

fn device_key(device: &MidiDeviceInfo) -> DeviceKey {
    (device.device_type, device.name.clone())
}

/// Represents a MIDI device (either input or output)
pub enum MidiDevice {
    /// An input device with its connection
    Input {
        info: MidiDeviceInfo,
        #[cfg(not(target_os = "tvos"))]
        connection: Option<midir::MidiInputConnection<()>>,
        #[cfg(target_os = "tvos")]
        connection: Option<()>,
    },

    /// An output device with its connection
    Output {
        info: MidiDeviceInfo,
        #[cfg(not(target_os = "tvos"))]
        connection: Option<midir::MidiOutputConnection>,
        #[cfg(target_os = "tvos")]
        connection: Option<()>,
    },
}

impl std::fmt::Debug for MidiDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiDevice::Input { info, connection } => f
                .debug_struct("MidiDevice::Input")
                .field("info", info)
                .field("connection", &connection.is_some())
                .finish(),
            MidiDevice::Output { info, connection } => f
                .debug_struct("MidiDevice::Output")
                .field("info", info)
                .field("connection", &connection.is_some())
                .finish(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn device(index: usize, name: &str, device_type: MidiDeviceType) -> MidiDeviceInfo {
        MidiDeviceInfo {
            index,
            name: name.to_string(),
            device_type,
            manufacturer: None,
            is_connected: false,
        }
    }

    #[test]
    fn hotplug_snapshot_diff_reports_connect_disconnect() {
        let before = MidiDeviceSnapshot::new(
            vec![device(0, "Keyboard", MidiDeviceType::Input)],
            vec![device(0, "Interface", MidiDeviceType::Output)],
        );
        let after = MidiDeviceSnapshot::new(
            vec![device(0, "Pads", MidiDeviceType::Input)],
            vec![device(0, "Interface", MidiDeviceType::Output)],
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
    fn hotplug_snapshot_diff_uses_name_and_type_not_index() {
        let before = MidiDeviceSnapshot::new(
            vec![device(1, "Keyboard", MidiDeviceType::Input)],
            Vec::new(),
        );
        let after = MidiDeviceSnapshot::new(
            vec![device(3, "Keyboard", MidiDeviceType::Input)],
            Vec::new(),
        );

        assert!(before.diff(&after).is_empty());
    }
}
