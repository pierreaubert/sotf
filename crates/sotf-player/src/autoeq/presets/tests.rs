use super::eq_preset::find_preset;
use super::eq_preset::presets_for;
use super::eq_workflow::default_preset_id;
use super::field::field_warning;
use super::quality::quality_to_optimizer_params;
use super::types::EqWorkflow;

#[test]
fn test_all_presets_have_valid_ids() {
    for workflow in [
        EqWorkflow::Headphone,
        EqWorkflow::Spinorama,
        EqWorkflow::RoomEq,
    ] {
        let presets = presets_for(workflow);
        assert!(!presets.is_empty());
        // Last preset must be "custom"
        assert_eq!(presets.last().unwrap().id, "custom");
        assert!(presets.last().unwrap().is_custom());
    }
}

#[test]
fn test_default_preset_exists() {
    for workflow in [
        EqWorkflow::Headphone,
        EqWorkflow::Spinorama,
        EqWorkflow::RoomEq,
    ] {
        let id = default_preset_id(workflow);
        assert!(
            find_preset(workflow, id).is_some(),
            "default preset '{id}' not found"
        );
    }
}

#[test]
fn test_apply_preset_produces_valid_params() {
    for workflow in [
        EqWorkflow::Headphone,
        EqWorkflow::Spinorama,
        EqWorkflow::RoomEq,
    ] {
        for preset in presets_for(workflow) {
            if let Some(params) = preset.apply() {
                assert!(params.num_filters > 0);
                assert!(params.population > 0);
                assert!(params.maxeval > 0);
                assert!(params.min_freq < params.max_freq);
            }
        }
    }
}

#[test]
fn test_custom_preset_returns_none() {
    let custom = find_preset(EqWorkflow::Headphone, "custom").unwrap();
    assert!(custom.apply().is_none());
}

#[test]
fn test_quality_slider_bounds() {
    let (pop_low, eval_low) = quality_to_optimizer_params(0.0);
    let (pop_high, eval_high) = quality_to_optimizer_params(1.0);
    assert!(pop_low < pop_high);
    assert!(eval_low < eval_high);
    assert_eq!(pop_low, 30);
    assert_eq!(pop_high, 300);
}

#[test]
fn test_field_warnings() {
    assert!(field_warning("num_filters", 15.0).is_some());
    assert!(field_warning("num_filters", 7.0).is_none());
    assert!(field_warning("max_db", 8.0).is_some());
    assert!(field_warning("max_db", 3.0).is_none());
}
