//! Launch Control XL physical layout definition
//!
//! 8 columns × 6 rows:
//! - Row 0-2: 24 knobs (3 rows × 8 columns)
//! - Row 3: 8 faders
//! - Row 4: 8 Track Focus buttons (top button row)
//! - Row 5: 8 Track Control buttons (bottom button row)
//!
//! Uses Device/Mute buttons for page navigation (left/right arrows not available
//! as continuous controls on LCXL).

use crate::layout::{ControllerLayout, MidiControlId, PhysicalControl, PhysicalControlKind};
use crate::profiles::launch_control_xl::{LCXLTemplate, LaunchControlXLProfile};

/// Create the Launch Control XL layout using factory template 1 CCs
pub fn lcxl_layout() -> ControllerLayout {
    let template = LCXLTemplate::factory_1();
    let mut controls = Vec::new();

    // 24 knobs: 3 rows × 8 columns
    for row in 0..3u8 {
        for col in 0..8u8 {
            let cc = template.knob_ccs[row as usize][col as usize];
            controls.push(PhysicalControl {
                id: format!("knob_{}_{}", row + 1, col + 1),
                kind: PhysicalControlKind::Pot,
                column: col,
                row,
                group: format!("knobs_row_{}", row + 1),
                label: format!("K{}{}", row + 1, col + 1),
                midi_id: MidiControlId::CC(template.channel, cc),
                secondary_midi_id: None,
            });
        }
    }

    // 8 faders
    for (i, &cc) in template.fader_ccs.iter().enumerate() {
        controls.push(PhysicalControl {
            id: format!("fader_{}", i + 1),
            kind: PhysicalControlKind::Fader,
            column: i as u8,
            row: 3,
            group: "faders".to_string(),
            label: format!("F{}", i + 1),
            midi_id: MidiControlId::CC(template.channel, cc),
            secondary_midi_id: None,
        });
    }

    // 8 Track Focus buttons (top)
    for (i, &note) in LaunchControlXLProfile::TRACK_FOCUS_NOTES.iter().enumerate() {
        controls.push(PhysicalControl {
            id: format!("btn_focus_{}", i + 1),
            kind: PhysicalControlKind::Button,
            column: i as u8,
            row: 4,
            group: "buttons_focus".to_string(),
            label: format!("TF{}", i + 1),
            midi_id: MidiControlId::Note(template.channel, note),
            secondary_midi_id: None,
        });
    }

    // 8 Track Control buttons (bottom)
    for (i, &note) in LaunchControlXLProfile::TRACK_CONTROL_NOTES
        .iter()
        .enumerate()
    {
        controls.push(PhysicalControl {
            id: format!("btn_ctrl_{}", i + 1),
            kind: PhysicalControlKind::Button,
            column: i as u8,
            row: 5,
            group: "buttons_control".to_string(),
            label: format!("TC{}", i + 1),
            midi_id: MidiControlId::Note(template.channel, note),
            secondary_midi_id: None,
        });
    }

    // Page navigation: Device = prev, Record Arm = next
    let page_prev_id = "btn_page_prev".to_string();
    let page_next_id = "btn_page_next".to_string();

    controls.push(PhysicalControl {
        id: page_prev_id.clone(),
        kind: PhysicalControlKind::Button,
        column: 0,
        row: 6,
        group: "nav".to_string(),
        label: "<".to_string(),
        midi_id: MidiControlId::Note(template.channel, LaunchControlXLProfile::DEVICE_NOTE),
        secondary_midi_id: None,
    });
    controls.push(PhysicalControl {
        id: page_next_id.clone(),
        kind: PhysicalControlKind::Button,
        column: 1,
        row: 6,
        group: "nav".to_string(),
        label: ">".to_string(),
        midi_id: MidiControlId::Note(template.channel, LaunchControlXLProfile::RECORD_ARM_NOTE),
        secondary_midi_id: None,
    });

    ControllerLayout {
        name: "Launch Control XL".to_string(),
        controls,
        grid_columns: 8,
        grid_rows: 7,
        reserved_control_ids: vec![page_prev_id.clone(), page_next_id.clone()],
        page_prev_id: Some(page_prev_id),
        page_next_id: Some(page_next_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lcxl_layout() {
        let layout = lcxl_layout();
        assert_eq!(layout.name, "Launch Control XL");
        // 24 knobs + 8 faders + 8 focus + 8 control + 2 nav = 50
        assert_eq!(layout.controls.len(), 50);
        // Mappable: 50 - 2 nav = 48
        assert_eq!(layout.mappable_controls().len(), 48);
        // Continuous: 24 knobs + 8 faders = 32
        assert_eq!(layout.continuous_control_count(), 32);
        // Buttons: 8 focus + 8 control = 16
        assert_eq!(layout.button_count(), 16);
    }
}
