//! Physical controller layout abstraction
//!
//! Describes the physical controls on a MIDI controller in an abstract way,
//! independent of any specific plugin parameter mapping.

use serde::{Deserialize, Serialize};

/// The physical type of a control on the hardware
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalControlKind {
    /// Absolute rotary potentiometer (0-127)
    Pot,
    /// Relative endless encoder (increment/decrement)
    Encoder,
    /// Linear fader (0-127)
    Fader,
    /// Momentary or toggle button (on/off)
    Button,
    /// Encoder with integrated push button
    EncoderWithButton,
}

impl PhysicalControlKind {
    pub fn is_continuous(self) -> bool {
        matches!(
            self,
            Self::Pot | Self::Encoder | Self::Fader | Self::EncoderWithButton
        )
    }
}

/// Identifies a MIDI control by its message type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidiControlId {
    /// Control Change message (channel, cc number)
    CC(u8, u8),
    /// Note message (channel, note number)
    Note(u8, u8),
}

/// A single physical control on a controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalControl {
    /// Unique string identifier (e.g., "pot_1", "fader_3", "btn_top_2")
    pub id: String,
    /// What kind of physical control this is
    pub kind: PhysicalControlKind,
    /// Column position on the controller (0-based)
    pub column: u8,
    /// Row position on the controller (0-based)
    pub row: u8,
    /// Logical group name (e.g., "top_knobs", "faders", "buttons")
    pub group: String,
    /// Human-readable label (e.g., "K1", "F3", "B2")
    pub label: String,
    /// Primary MIDI control ID
    pub midi_id: MidiControlId,
    /// Secondary MIDI control ID (e.g., encoder push button)
    pub secondary_midi_id: Option<MidiControlId>,
}

/// Describes the full physical layout of a MIDI controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerLayout {
    /// Controller name (e.g., "Xone:K2", "Launch Control XL")
    pub name: String,
    /// All physical controls
    pub controls: Vec<PhysicalControl>,
    /// Number of columns in the grid
    pub grid_columns: u8,
    /// Number of rows in the grid
    pub grid_rows: u8,
    /// Control IDs reserved for paging (not available for parameter mapping)
    pub reserved_control_ids: Vec<String>,
    /// Control ID used for page-previous
    pub page_prev_id: Option<String>,
    /// Control ID used for page-next
    pub page_next_id: Option<String>,
}

impl ControllerLayout {
    /// Get all mappable controls (excluding reserved ones)
    pub fn mappable_controls(&self) -> Vec<&PhysicalControl> {
        self.controls
            .iter()
            .filter(|c| !self.reserved_control_ids.contains(&c.id))
            .collect()
    }

    /// Get mappable controls filtered by kind
    pub fn controls_of_kind(&self, kind: PhysicalControlKind) -> Vec<&PhysicalControl> {
        self.mappable_controls()
            .into_iter()
            .filter(|c| c.kind == kind)
            .collect()
    }

    /// Find a control by its MIDI control ID
    pub fn find_by_midi_id(&self, midi_id: &MidiControlId) -> Option<&PhysicalControl> {
        self.controls
            .iter()
            .find(|c| c.midi_id == *midi_id || c.secondary_midi_id.as_ref() == Some(midi_id))
    }

    /// Find a control by its string ID
    pub fn find_by_id(&self, id: &str) -> Option<&PhysicalControl> {
        self.controls.iter().find(|c| c.id == id)
    }

    /// Count of mappable continuous controls (pots, encoders, faders)
    pub fn continuous_control_count(&self) -> usize {
        self.mappable_controls()
            .iter()
            .filter(|c| c.kind.is_continuous())
            .count()
    }

    /// Count of mappable button controls
    pub fn button_count(&self) -> usize {
        self.controls_of_kind(PhysicalControlKind::Button).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> ControllerLayout {
        ControllerLayout {
            name: "Test Controller".to_string(),
            controls: vec![
                PhysicalControl {
                    id: "pot_1".to_string(),
                    kind: PhysicalControlKind::Pot,
                    column: 0,
                    row: 0,
                    group: "knobs".to_string(),
                    label: "K1".to_string(),
                    midi_id: MidiControlId::CC(0, 1),
                    secondary_midi_id: None,
                },
                PhysicalControl {
                    id: "fader_1".to_string(),
                    kind: PhysicalControlKind::Fader,
                    column: 0,
                    row: 1,
                    group: "faders".to_string(),
                    label: "F1".to_string(),
                    midi_id: MidiControlId::CC(0, 44),
                    secondary_midi_id: None,
                },
                PhysicalControl {
                    id: "btn_nav_prev".to_string(),
                    kind: PhysicalControlKind::Button,
                    column: 0,
                    row: 2,
                    group: "nav".to_string(),
                    label: "<".to_string(),
                    midi_id: MidiControlId::Note(0, 57),
                    secondary_midi_id: None,
                },
            ],
            grid_columns: 1,
            grid_rows: 3,
            reserved_control_ids: vec!["btn_nav_prev".to_string()],
            page_prev_id: Some("btn_nav_prev".to_string()),
            page_next_id: None,
        }
    }

    #[test]
    fn test_mappable_controls_excludes_reserved() {
        let layout = test_layout();
        let mappable = layout.mappable_controls();
        assert_eq!(mappable.len(), 2);
        assert!(mappable.iter().all(|c| c.id != "btn_nav_prev"));
    }

    #[test]
    fn test_find_by_midi_id() {
        let layout = test_layout();
        let found = layout.find_by_midi_id(&MidiControlId::CC(0, 44));
        assert_eq!(found.unwrap().id, "fader_1");
    }

    #[test]
    fn test_continuous_count() {
        let layout = test_layout();
        assert_eq!(layout.continuous_control_count(), 2);
    }
}
