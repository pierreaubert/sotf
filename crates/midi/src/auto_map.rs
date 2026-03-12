//! Rule-based auto-mapper: assigns plugin parameters to physical controls
//!
//! Resolution order: manual overrides > curated templates > auto-map

use crate::layout::{ControllerLayout, PhysicalControl, PhysicalControlKind};
use crate::mapping::{ControlBinding, MidiMapping, ValueScaling};
use sotf_host::param_specs::{ParamSpec, ParamType};

/// Determine which physical control kinds are best suited for a parameter
pub fn control_affinity(spec: &ParamSpec) -> &'static [PhysicalControlKind] {
    match spec.param_type {
        ParamType::Float { min, max, .. } => {
            let range = max - min;
            if range > 100.0 {
                // Wide range (e.g., frequency, long release) → fader or pot
                &[
                    PhysicalControlKind::Fader,
                    PhysicalControlKind::Pot,
                    PhysicalControlKind::EncoderWithButton,
                ]
            } else {
                // Narrow range → pot or encoder
                &[
                    PhysicalControlKind::Pot,
                    PhysicalControlKind::Encoder,
                    PhysicalControlKind::Fader,
                    PhysicalControlKind::EncoderWithButton,
                ]
            }
        }
        ParamType::Int { .. } => &[
            PhysicalControlKind::Encoder,
            PhysicalControlKind::Pot,
            PhysicalControlKind::EncoderWithButton,
        ],
        ParamType::Bool { .. } => &[PhysicalControlKind::Button],
        ParamType::Choice { labels, .. } => {
            if labels.len() <= 2 {
                &[PhysicalControlKind::Button]
            } else {
                &[
                    PhysicalControlKind::Encoder,
                    PhysicalControlKind::Pot,
                    PhysicalControlKind::EncoderWithButton,
                ]
            }
        }
        ParamType::FilePath => {
            // Not mappable to a physical control
            &[]
        }
    }
}

/// Determine the appropriate value scaling for a parameter
pub fn scaling_for_param(spec: &ParamSpec) -> ValueScaling {
    match spec.param_type {
        ParamType::Float { min, max, .. } => {
            // Use log scaling for Hz params with wide range, or ratio params with wide range
            let is_freq = spec.unit == "Hz" || spec.unit == "kHz";
            let is_wide_ratio = spec.unit == ":1" && max / min.max(0.01) > 10.0;
            if (is_freq && max / min.max(0.01) > 50.0) || is_wide_ratio {
                ValueScaling::Logarithmic
            } else {
                ValueScaling::Linear
            }
        }
        ParamType::Int { min, max, .. } => {
            let steps = (max - min) as u8;
            if steps <= 16 {
                ValueScaling::Stepped(steps.max(1) + 1)
            } else {
                ValueScaling::Linear
            }
        }
        ParamType::Bool { .. } => ValueScaling::Toggle,
        ParamType::Choice { labels, .. } => ValueScaling::Stepped(labels.len() as u8),
        ParamType::FilePath => ValueScaling::Linear, // unused, FilePath is unmappable
    }
}

/// Auto-map plugin parameters to a controller layout
///
/// Assigns parameters group-by-group to available controls, respecting
/// affinity rules. Returns a `MidiMapping` with page information.
pub fn auto_map(
    layout: &ControllerLayout,
    params: &[ParamSpec],
    plugin_index: usize,
    plugin_type: &str,
) -> MidiMapping {
    let mut mapping = MidiMapping::new(layout.name.clone(), plugin_type.to_string());

    // Collect mappable controls by kind
    let continuous_controls: Vec<&PhysicalControl> = layout
        .mappable_controls()
        .into_iter()
        .filter(|c| c.kind.is_continuous())
        .collect();
    let button_controls: Vec<&PhysicalControl> =
        layout.controls_of_kind(PhysicalControlKind::Button);

    // Separate params into continuous (Float/Int/Choice-large) and discrete (Bool/Choice-small)
    let mut continuous_params: Vec<(usize, &ParamSpec)> = Vec::new();
    let mut discrete_params: Vec<(usize, &ParamSpec)> = Vec::new();

    for (idx, spec) in params.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            continue; // unmappable
        }
        let affinity = control_affinity(spec);
        if affinity.contains(&PhysicalControlKind::Button)
            && !affinity.contains(&PhysicalControlKind::Pot)
        {
            discrete_params.push((idx, spec));
        } else {
            continuous_params.push((idx, spec));
        }
    }

    // Calculate pages based on continuous controls (the bottleneck)
    let controls_per_page = continuous_controls.len();
    let buttons_per_page = button_controls.len();

    if controls_per_page == 0 && buttons_per_page == 0 {
        return mapping;
    }

    // Assign continuous params to pages
    let mut page = 0;
    let mut slot = 0;
    for (param_idx, spec) in &continuous_params {
        if controls_per_page > 0 && slot >= controls_per_page {
            page += 1;
            slot = 0;
        }
        if controls_per_page > 0 {
            let control = &continuous_controls[slot % controls_per_page];
            mapping.bindings.push(ControlBinding {
                control_id: control.id.clone(),
                plugin_index,
                param_index: *param_idx,
                page,
                scaling: scaling_for_param(spec),
            });
            slot += 1;
        }
    }
    let continuous_pages = page + 1;

    // Assign discrete params to buttons (share pages with continuous)
    let mut btn_slot = 0;
    let mut btn_page = 0;
    for (param_idx, spec) in &discrete_params {
        if buttons_per_page > 0 && btn_slot >= buttons_per_page {
            btn_page += 1;
            btn_slot = 0;
        }
        if buttons_per_page > 0 {
            let control = &button_controls[btn_slot % buttons_per_page];
            mapping.bindings.push(ControlBinding {
                control_id: control.id.clone(),
                plugin_index,
                param_index: *param_idx,
                page: btn_page,
                scaling: scaling_for_param(spec),
            });
            btn_slot += 1;
        }
    }
    let button_pages = if buttons_per_page > 0 {
        btn_page + 1
    } else {
        1
    };

    mapping.total_pages = continuous_pages.max(button_pages);
    mapping
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::param_specs::compressor;

    fn make_xone_k2_like_layout() -> ControllerLayout {
        use crate::layout::MidiControlId;

        let mut controls = Vec::new();
        // 12 pots
        for i in 0..12 {
            controls.push(PhysicalControl {
                id: format!("pot_{}", i + 1),
                kind: PhysicalControlKind::Pot,
                column: i as u8 % 4,
                row: i as u8 / 4,
                group: "pots".to_string(),
                label: format!("P{}", i + 1),
                midi_id: MidiControlId::CC(0, i as u8),
                secondary_midi_id: None,
            });
        }
        // 4 faders
        for i in 0..4 {
            controls.push(PhysicalControl {
                id: format!("fader_{}", i + 1),
                kind: PhysicalControlKind::Fader,
                column: i as u8,
                row: 3,
                group: "faders".to_string(),
                label: format!("F{}", i + 1),
                midi_id: MidiControlId::CC(0, 44 + i as u8),
                secondary_midi_id: None,
            });
        }
        // 4 buttons
        for i in 0..4 {
            controls.push(PhysicalControl {
                id: format!("btn_{}", i + 1),
                kind: PhysicalControlKind::Button,
                column: i as u8,
                row: 4,
                group: "buttons".to_string(),
                label: format!("B{}", i + 1),
                midi_id: MidiControlId::Note(0, 24 + i as u8),
                secondary_midi_id: None,
            });
        }

        ControllerLayout {
            name: "Xone K2-like".to_string(),
            controls,
            grid_columns: 4,
            grid_rows: 5,
            reserved_control_ids: vec![],
            page_prev_id: None,
            page_next_id: None,
        }
    }

    fn make_lcxl_like_layout() -> ControllerLayout {
        use crate::layout::MidiControlId;

        let mut controls = Vec::new();
        // 24 knobs (3 rows × 8)
        for row in 0..3 {
            for col in 0..8 {
                let idx = row * 8 + col;
                controls.push(PhysicalControl {
                    id: format!("knob_{}_{}", row + 1, col + 1),
                    kind: PhysicalControlKind::Pot,
                    column: col as u8,
                    row: row as u8,
                    group: format!("knobs_row_{}", row + 1),
                    label: format!("K{}{}", row + 1, col + 1),
                    midi_id: MidiControlId::CC(0, idx as u8 + 13),
                    secondary_midi_id: None,
                });
            }
        }
        // 8 faders
        for i in 0..8 {
            controls.push(PhysicalControl {
                id: format!("fader_{}", i + 1),
                kind: PhysicalControlKind::Fader,
                column: i as u8,
                row: 3,
                group: "faders".to_string(),
                label: format!("F{}", i + 1),
                midi_id: MidiControlId::CC(0, 77 + i as u8),
                secondary_midi_id: None,
            });
        }
        // 8 buttons (top focus)
        for i in 0..8 {
            controls.push(PhysicalControl {
                id: format!("btn_{}", i + 1),
                kind: PhysicalControlKind::Button,
                column: i as u8,
                row: 4,
                group: "buttons".to_string(),
                label: format!("B{}", i + 1),
                midi_id: MidiControlId::Note(0, 41 + i as u8),
                secondary_midi_id: None,
            });
        }

        ControllerLayout {
            name: "LCXL-like".to_string(),
            controls,
            grid_columns: 8,
            grid_rows: 5,
            reserved_control_ids: vec![],
            page_prev_id: None,
            page_next_id: None,
        }
    }

    #[test]
    fn test_auto_map_compressor_on_xone_k2() {
        let layout = make_xone_k2_like_layout();
        let params = compressor::PARAMS;
        let mapping = auto_map(&layout, params, 0, "Compressor");

        // Compressor has 10 params: 8 continuous (float), 2 discrete (bool)
        assert!(!mapping.bindings.is_empty());
        assert_eq!(mapping.total_pages, 1); // 16 continuous controls, 8 continuous params → 1 page

        // Check all continuous params got assigned
        let continuous_bindings: Vec<_> = mapping
            .bindings
            .iter()
            .filter(|b| !b.control_id.starts_with("btn_"))
            .collect();
        assert_eq!(continuous_bindings.len(), 8);

        // Check bool params got buttons
        let button_bindings: Vec<_> = mapping
            .bindings
            .iter()
            .filter(|b| b.control_id.starts_with("btn_"))
            .collect();
        assert_eq!(button_bindings.len(), 2);
    }

    #[test]
    fn test_auto_map_compressor_on_lcxl() {
        let layout = make_lcxl_like_layout();
        let params = compressor::PARAMS;
        let mapping = auto_map(&layout, params, 0, "Compressor");

        assert!(!mapping.bindings.is_empty());
        // LCXL has 32 continuous controls, compressor has 8 continuous → 1 page
        assert_eq!(mapping.total_pages, 1);
    }

    #[test]
    fn test_paging_with_limited_controls() {
        // Create a tiny layout: 2 pots, 1 button
        use crate::layout::MidiControlId;
        let layout = ControllerLayout {
            name: "Tiny".to_string(),
            controls: vec![
                PhysicalControl {
                    id: "pot_1".to_string(),
                    kind: PhysicalControlKind::Pot,
                    column: 0,
                    row: 0,
                    group: "pots".to_string(),
                    label: "P1".to_string(),
                    midi_id: MidiControlId::CC(0, 1),
                    secondary_midi_id: None,
                },
                PhysicalControl {
                    id: "pot_2".to_string(),
                    kind: PhysicalControlKind::Pot,
                    column: 1,
                    row: 0,
                    group: "pots".to_string(),
                    label: "P2".to_string(),
                    midi_id: MidiControlId::CC(0, 2),
                    secondary_midi_id: None,
                },
                PhysicalControl {
                    id: "btn_1".to_string(),
                    kind: PhysicalControlKind::Button,
                    column: 0,
                    row: 1,
                    group: "buttons".to_string(),
                    label: "B1".to_string(),
                    midi_id: MidiControlId::Note(0, 24),
                    secondary_midi_id: None,
                },
            ],
            grid_columns: 2,
            grid_rows: 2,
            reserved_control_ids: vec![],
            page_prev_id: None,
            page_next_id: None,
        };

        let params = compressor::PARAMS;
        let mapping = auto_map(&layout, params, 0, "Compressor");

        // 8 continuous params / 2 pots per page = 4 pages
        assert_eq!(mapping.total_pages, 4);

        // Page 0 should have 2 continuous + up to 1 button binding
        let page0: Vec<_> = mapping.bindings.iter().filter(|b| b.page == 0).collect();
        assert!(page0.len() >= 2);
    }

    #[test]
    fn test_scaling_for_hz_param() {
        let spec = ParamSpec::float("Freq", "freq", 1000.0, 20.0, 20000.0, 1.0, "Hz", "Test");
        assert_eq!(scaling_for_param(&spec), ValueScaling::Logarithmic);
    }

    #[test]
    fn test_scaling_for_db_param() {
        let spec = ParamSpec::float(
            "Threshold",
            "threshold",
            -20.0,
            -60.0,
            0.0,
            1.0,
            "dB",
            "Test",
        );
        assert_eq!(scaling_for_param(&spec), ValueScaling::Linear);
    }

    #[test]
    fn test_scaling_for_bool_param() {
        let spec = ParamSpec::bool_param("Toggle", "toggle", false, "Test");
        assert_eq!(scaling_for_param(&spec), ValueScaling::Toggle);
    }

    #[test]
    fn test_filepath_unmappable() {
        let affinity = control_affinity(&ParamSpec {
            name: "IR File",
            engine_key: "ir_file",
            param_type: ParamType::FilePath,
            unit: "",
            group: "Test",
            update_mode: sotf_host::param_specs::UpdateMode::Structural,
            display_scale: 1.0,
        });
        assert!(affinity.is_empty());
    }
}
