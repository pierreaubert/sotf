//! Xone:K2 physical layout definition
//!
//! 4 columns × multiple rows:
//! - Row 0-2: 12 rotary pots (3 rows × 4 columns)
//! - Row 3: 6 encoders (first 2 rows × 3 columns conceptually, placed in row 3-4)
//! - Row 5: 4 linear faders
//! - Buttons: 30 total across various positions
//!
//! Paging: uses navigation buttons (left/right) for page prev/next.

use crate::layout::{ControllerLayout, MidiControlId, PhysicalControl, PhysicalControlKind};
use crate::profiles::xone_k2::XoneK2Profile;

/// Create the Xone:K2 controller layout
pub fn xone_k2_layout() -> ControllerLayout {
    let mut controls = Vec::new();

    // 12 rotary pots: 3 rows × 4 columns
    for (i, &cc) in XoneK2Profile::POT_CC.iter().enumerate() {
        let col = (i % 4) as u8;
        let row = (i / 4) as u8;
        controls.push(PhysicalControl {
            id: format!("pot_{}", i + 1),
            kind: PhysicalControlKind::Pot,
            column: col,
            row,
            group: format!("pots_row_{}", row + 1),
            label: format!("P{}", i + 1),
            midi_id: MidiControlId::CC(XoneK2Profile::DEFAULT_CHANNEL, cc),
            secondary_midi_id: None,
        });
    }

    // 6 encoders with push buttons
    for (i, &cc) in XoneK2Profile::ENCODER_CC.iter().enumerate() {
        let col = (i % 3) as u8;
        let row = 3 + (i / 3) as u8;
        controls.push(PhysicalControl {
            id: format!("enc_{}", i + 1),
            kind: PhysicalControlKind::EncoderWithButton,
            column: col,
            row,
            group: "encoders".to_string(),
            label: format!("E{}", i + 1),
            midi_id: MidiControlId::CC(XoneK2Profile::DEFAULT_CHANNEL, cc),
            secondary_midi_id: Some(MidiControlId::Note(
                XoneK2Profile::DEFAULT_CHANNEL,
                XoneK2Profile::ENCODER_SWITCH_NOTE[i],
            )),
        });
    }

    // 4 faders
    for (i, &cc) in XoneK2Profile::FADER_CC.iter().enumerate() {
        controls.push(PhysicalControl {
            id: format!("fader_{}", i + 1),
            kind: PhysicalControlKind::Fader,
            column: i as u8,
            row: 5,
            group: "faders".to_string(),
            label: format!("F{}", i + 1),
            midi_id: MidiControlId::CC(XoneK2Profile::DEFAULT_CHANNEL, cc),
            secondary_midi_id: None,
        });
    }

    // Top row buttons (8)
    for (i, &note) in XoneK2Profile::BUTTON_TOP_NOTE.iter().enumerate() {
        controls.push(PhysicalControl {
            id: format!("btn_top_{}", i + 1),
            kind: PhysicalControlKind::Button,
            column: (i % 4) as u8,
            row: 6 + (i / 4) as u8,
            group: "buttons_top".to_string(),
            label: format!("BT{}", i + 1),
            midi_id: MidiControlId::Note(XoneK2Profile::DEFAULT_CHANNEL, note),
            secondary_midi_id: None,
        });
    }

    // Bottom row buttons (8)
    for (i, &note) in XoneK2Profile::BUTTON_BOTTOM_NOTE.iter().enumerate() {
        controls.push(PhysicalControl {
            id: format!("btn_bot_{}", i + 1),
            kind: PhysicalControlKind::Button,
            column: (i % 4) as u8,
            row: 8 + (i / 4) as u8,
            group: "buttons_bottom".to_string(),
            label: format!("BB{}", i + 1),
            midi_id: MidiControlId::Note(XoneK2Profile::DEFAULT_CHANNEL, note),
            secondary_midi_id: None,
        });
    }

    // Navigation buttons: left = page prev, right = page next
    let nav_left_id = "btn_nav_left".to_string();
    let nav_right_id = "btn_nav_right".to_string();

    controls.push(PhysicalControl {
        id: nav_left_id.clone(),
        kind: PhysicalControlKind::Button,
        column: 0,
        row: 10,
        group: "nav".to_string(),
        label: "<".to_string(),
        midi_id: MidiControlId::Note(
            XoneK2Profile::DEFAULT_CHANNEL,
            XoneK2Profile::BUTTON_NAV_NOTE[2], // left
        ),
        secondary_midi_id: None,
    });
    controls.push(PhysicalControl {
        id: nav_right_id.clone(),
        kind: PhysicalControlKind::Button,
        column: 1,
        row: 10,
        group: "nav".to_string(),
        label: ">".to_string(),
        midi_id: MidiControlId::Note(
            XoneK2Profile::DEFAULT_CHANNEL,
            XoneK2Profile::BUTTON_NAV_NOTE[3], // right
        ),
        secondary_midi_id: None,
    });

    ControllerLayout {
        name: "Xone:K2".to_string(),
        controls,
        grid_columns: 4,
        grid_rows: 11,
        reserved_control_ids: vec![nav_left_id.clone(), nav_right_id.clone()],
        page_prev_id: Some(nav_left_id),
        page_next_id: Some(nav_right_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xone_k2_layout() {
        let layout = xone_k2_layout();
        assert_eq!(layout.name, "Xone:K2");
        // 12 pots + 6 encoders + 4 faders + 16 buttons + 2 nav = 40
        assert_eq!(layout.controls.len(), 40);
        // Mappable: 40 - 2 nav = 38
        assert_eq!(layout.mappable_controls().len(), 38);
        // Continuous: 12 pots + 6 encoders + 4 faders = 22
        assert_eq!(layout.continuous_control_count(), 22);
    }
}
