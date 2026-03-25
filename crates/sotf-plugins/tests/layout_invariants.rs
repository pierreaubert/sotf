//! Layout Structural Invariant Tests
//!
//! Property-based assertions that hold for ALL plugins with a PluginLayout.
//! These verify structural correctness regardless of specific layout details.
//!
//! Run: cargo test -p sotf-plugins --test layout_invariants

use sotf_host::param_specs::{ParamSpec, ParamType};
use sotf_host::plugin_layout::{ColumnRole, ControlSpec, ControlType, PluginLayout};
use sotf_host::plugin_params::PluginParamDef;

/// Verify all structural invariants for a plugin's layout.
fn assert_layout_invariants<P: PluginParamDef>() {
    let layout = match P::LAYOUT {
        Some(l) => l,
        None => return, // no layout = nothing to check
    };
    let params = P::PARAMS;
    let plugin_type = P::PLUGIN_TYPE_KEY;

    // 1. Every control references a valid param index (or usize::MAX for meters)
    assert_valid_param_indices(plugin_type, layout, params);

    // 2. Control types are compatible with param types
    assert_control_type_compatibility(plugin_type, layout, params);

    // 3. No duplicate param indices in the same column
    assert_no_duplicate_params(plugin_type, layout);

    // 4. Column constraints include Main (which never collapses)
    assert_has_main_column(plugin_type, layout);

}

fn all_controls(layout: &PluginLayout) -> Vec<(&'static str, &ControlSpec)> {
    let mut controls = Vec::new();
    for spec in layout.config {
        controls.push(("config", spec));
    }
    for group in layout.main {
        for spec in group.controls {
            controls.push(("main", spec));
        }
    }
    for spec in layout.output {
        controls.push(("output", spec));
    }
    for tab in layout.tabs {
        for spec in tab.controls {
            controls.push(("tab", spec));
        }
    }
    controls
}

fn assert_valid_param_indices(plugin_type: &str, layout: &PluginLayout, params: &[ParamSpec]) {
    for (column, spec) in all_controls(layout) {
        if spec.param_index == usize::MAX {
            continue; // meter placeholder
        }
        assert!(
            spec.param_index < params.len(),
            "[{plugin_type}] {column} control references param index {} but PARAMS has {} entries",
            spec.param_index,
            params.len()
        );
    }
}

fn assert_control_type_compatibility(
    plugin_type: &str,
    layout: &PluginLayout,
    params: &[ParamSpec],
) {
    for (column, spec) in all_controls(layout) {
        if spec.param_index >= params.len() {
            continue;
        }
        let param = &params[spec.param_index];

        match (&spec.control_type, &param.param_type) {
            // Toggle should only be used with Bool params
            (ControlType::Toggle, ParamType::Bool { .. }) => {}
            (ControlType::Toggle, _) => {
                panic!(
                    "[{plugin_type}] {column} control for '{}' (idx {}) is Toggle but param is {:?}",
                    param.name, spec.param_index, param.param_type
                );
            }
            // Selector should be used with Choice params
            (ControlType::Selector, ParamType::Choice { .. }) => {}
            (ControlType::Selector, _) => {
                panic!(
                    "[{plugin_type}] {column} control for '{}' (idx {}) is Selector but param is {:?}",
                    param.name, spec.param_index, param.param_type
                );
            }
            // FilePicker should be used with FilePath params
            (ControlType::FilePicker, ParamType::FilePath) => {}
            (ControlType::FilePicker, _) => {
                panic!(
                    "[{plugin_type}] {column} control for '{}' (idx {}) is FilePicker but param is {:?}",
                    param.name, spec.param_index, param.param_type
                );
            }
            // ButtonSet should be used with Choice or Bool params
            (ControlType::ButtonSet { .. }, ParamType::Choice { .. }) => {}
            (ControlType::ButtonSet { .. }, ParamType::Bool { .. }) => {}
            (ControlType::ButtonSet { .. }, _) => {
                panic!(
                    "[{plugin_type}] {column} control for '{}' (idx {}) is ButtonSet but param is {:?}",
                    param.name, spec.param_index, param.param_type
                );
            }
            // Knobs, sliders, labels are flexible — they work with any numeric param
            _ => {}
        }
    }
}

fn assert_no_duplicate_params(plugin_type: &str, layout: &PluginLayout) {
    // Check per-column uniqueness (same param in one column is always a bug)
    let check_unique = |name: &str, controls: &[ControlSpec]| {
        let mut seen = std::collections::HashSet::new();
        for spec in controls {
            if spec.param_index == usize::MAX {
                continue;
            }
            assert!(
                seen.insert(spec.param_index),
                "[{plugin_type}] {name} has duplicate param index {}",
                spec.param_index
            );
        }
    };

    check_unique("config", layout.config);
    for group in layout.main {
        check_unique(&format!("main/{}", group.title), group.controls);
    }
    check_unique("output", layout.output);
    for tab in layout.tabs {
        check_unique(&format!("tab/{}", tab.name), tab.controls);
    }
}

fn assert_has_main_column(plugin_type: &str, layout: &PluginLayout) {
    if layout.column_constraints.is_empty() {
        return; // no constraints = solver uses defaults
    }
    let has_main = layout
        .column_constraints
        .iter()
        .any(|c| c.role == ColumnRole::Main);
    assert!(
        has_main,
        "[{plugin_type}] column_constraints must include a Main column"
    );
}


// Generate invariant tests for all plugins
macro_rules! invariant_test {
    ($test_name:ident, $params_type:ty) => {
        #[test]
        fn $test_name() {
            assert_layout_invariants::<$params_type>();
        }
    };
}

invariant_test!(invariants_ab_compare, sotf_plugin_ab_compare::params::Params);
invariant_test!(invariants_aec, sotf_plugin_aec::params::Params);
invariant_test!(invariants_ambisonics, sotf_plugin_ambisonics::params::Params);
invariant_test!(invariants_band_merge, sotf_plugin_band_merge::params::Params);
invariant_test!(invariants_band_split, sotf_plugin_band_split::params::Params);
invariant_test!(invariants_beamformer, sotf_plugin_beamformer::params::Params);
invariant_test!(invariants_binaural, sotf_plugin_binaural::params::Params);
invariant_test!(invariants_channel_mute_solo, sotf_plugin_channel_mute_solo::params::Params);

invariant_test!(invariants_convolution, sotf_plugin_convolution::params::Params);
invariant_test!(invariants_crossfeed, sotf_plugin_crossfeed::params::Params);
invariant_test!(invariants_delay, sotf_plugin_delay::params::Params);
invariant_test!(invariants_denoiser, sotf_plugin_denoiser::params::Params);
invariant_test!(invariants_dither, sotf_plugin_dither::params::Params);
invariant_test!(invariants_downmix, sotf_plugin_downmix::params::Params);

invariant_test!(invariants_fletcher_munson, sotf_plugin_fletcher_munson::params::Params);
invariant_test!(invariants_gain, sotf_plugin_gain::params::Params);
invariant_test!(invariants_gate, sotf_plugin_gate::params::Params);
invariant_test!(invariants_limiter, sotf_plugin_limiter::params::Params);
invariant_test!(invariants_loudness_compensation, sotf_plugin_loudness_compensation::params::Params);
invariant_test!(invariants_matrix, sotf_plugin_matrix::params::Params);
invariant_test!(invariants_mono_to_stereo, sotf_plugin_mono_to_stereo::params::Params);
invariant_test!(invariants_pnd, sotf_plugin_pnd::params::Params);
invariant_test!(invariants_upmixer, sotf_plugin_upmixer::params::Params);
invariant_test!(invariants_xtc, sotf_plugin_xtc::params::Params);
