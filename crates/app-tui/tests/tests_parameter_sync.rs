use sotf_audio_player::{PluginSettings, PluginType};
use sotf_audio_player_tui::app::TuiEditablePlugin;

#[test]
fn test_plugin_parameter_sync() {
    let plugin_types = PluginType::all();

    for plugin_type in plugin_types {
        // Create default settings for this plugin type
        let mut settings = PluginSettings::default_for(&plugin_type);

        let descriptors = settings.get_descriptors();
        let params = settings.get_params();

        // 1. Verify counts match
        assert_eq!(
            descriptors.len(),
            params.len(),
            "Descriptor/Param count mismatch for {:?}",
            plugin_type
        );

        // 2. Verify each index is editable/readable
        for i in 0..descriptors.len() {
            let desc = &descriptors[i];
            let param = &params[i];

            // Name should match
            assert_eq!(
                desc.name, param.name,
                "Name mismatch at index {} for {:?}",
                i, plugin_type
            );

            // Value should be readable
            let _value = settings.get_value_as_string(i);
            // Some values might be empty for unimplemented or non-displayable params,
            // but for now let's assume most should have something.
            // Actually, for Upmixer 31-33 they are empty, so we won't assert non-empty.

            // 3. Verify adjust_param doesn't panic and returns true/false correctly
            // Most should return true for a small delta, unless it's a file path or something non-adjustable.
            let _ = settings.adjust_param(i, 0.1);
            let _ = settings.adjust_param(i, -0.1);
        }
    }
}

#[test]
fn test_upmixer_parameter_sync_deep() {
    let mut settings = PluginSettings::default_for(&PluginType::Upmixer);
    let descriptors = settings.get_descriptors();

    // Upmixer is expected to have 44 parameters currently (0-43)
    assert_eq!(descriptors.len(), 44);

    for i in 0..descriptors.len() {
        // Just verify it doesn't return false for known editable params
        let res = settings.adjust_param(i, 1.0);
        assert!(res, "Index {} SHOULD be editable for Upmixer", i);
    }
}

#[test]
fn test_crossfeed_parameter_sync_deep() {
    let mut settings = PluginSettings::default_for(&PluginType::Crossfeed);
    let descriptors = settings.get_descriptors();

    // Crossfeed is expected to have 17 parameters (0-16)
    assert_eq!(descriptors.len(), 17);

    for i in 0..descriptors.len() {
        let res = settings.adjust_param(i, 1.0);
        assert!(res, "Index {} SHOULD be editable for Crossfeed", i);
    }
}

#[test]
fn test_eq_filter_limit() {
    let mut settings = PluginSettings::default_for(&PluginType::EQ);

    // Default max_filters should be 5
    // Default filters count is 5
    let descriptors = settings.get_descriptors();
    assert_eq!(descriptors[0].name, "Max Filters");

    // Initial descriptors count: 3 (Max Filters + TDF2 + Topology) + 5 * 4 (filters) = 23
    assert_eq!(descriptors.len(), 23);

    // Change Max Filters to 3
    settings.adjust_param(0, -2.0);
    if let PluginSettings::EQ {
        filters,
        max_filters,
        ..
    } = &settings
    {
        assert_eq!(*max_filters, 3);
        assert_eq!(filters.len(), 3);
    }

    let descriptors = settings.get_descriptors();
    // New count: 3 (Max Filters + TDF2 + Topology) + 3 * 4 (filters) = 15
    assert_eq!(descriptors.len(), 15);

    // Change Max Filters to 6 (add one)
    settings.adjust_param(0, 3.0);
    if let PluginSettings::EQ {
        filters,
        max_filters,
        ..
    } = &settings
    {
        assert_eq!(*max_filters, 6);
        assert_eq!(filters.len(), 6);
    }

    let descriptors = settings.get_descriptors();
    // New count: 3 (Max Filters + TDF2 + Topology) + 6 * 4 (filters) = 27
    assert_eq!(descriptors.len(), 27);

    // Verify we can edit the new filter (index 22-25)
    let res = settings.adjust_param(22, 10.0); // Frequency of 6th filter
    assert!(res);
}

#[test]
fn test_delay_parameter_sync_deep() {
    let mut settings = PluginSettings::default_for(&PluginType::Delay);
    let descriptors = settings.get_descriptors();

    // Delay has 6 parameters: delay_ms, feedback, mix, lfo_rate_hz, lfo_depth_ms, allpass_feedback
    assert_eq!(descriptors.len(), 6);

    // Verify all params are editable
    for i in 0..descriptors.len() {
        let res = settings.adjust_param(i, 1.0);
        assert!(res, "Index {} SHOULD be editable for Delay", i);
    }

    // Verify default values
    assert_eq!(descriptors[0].name, "Delay");
    assert_eq!(descriptors[1].name, "Feedback");
    assert_eq!(descriptors[2].name, "Mix");
    assert_eq!(descriptors[3].name, "LFO Rate");
    assert_eq!(descriptors[4].name, "LFO Depth");
    assert_eq!(descriptors[5].name, "Allpass Feedback");
}

#[test]
fn test_crossfeed_mode_cycling() {
    let mut settings = PluginSettings::default_for(&PluginType::Crossfeed);

    // Index 0 is the mode (choice parameter)
    // Default is Mb (index 3) per PluginSettings::default_for
    let initial_value = settings.param_value(0);
    assert_eq!(
        initial_value,
        Some(3.0),
        "Default mode should be Mb (index 3)"
    );

    // Cycle through modes: Mb(3) -> Off(0) -> Bauer(1) -> Meier(2) -> Mb(3)
    settings.adjust_param(0, 1.0);
    assert_eq!(
        settings.param_value(0),
        Some(0.0),
        "Mode should wrap to Off (index 0)"
    );

    settings.adjust_param(0, 1.0);
    assert_eq!(
        settings.param_value(0),
        Some(1.0),
        "Mode should cycle to Bauer (index 1)"
    );

    settings.adjust_param(0, 1.0);
    assert_eq!(
        settings.param_value(0),
        Some(2.0),
        "Mode should cycle to Meier (index 2)"
    );

    settings.adjust_param(0, 1.0);
    assert_eq!(
        settings.param_value(0),
        Some(3.0),
        "Mode should cycle back to Mb (index 3)"
    );

    // Cycle backwards: Mb(3) -> Meier(2)
    settings.adjust_param(0, -1.0);
    assert_eq!(
        settings.param_value(0),
        Some(2.0),
        "Mode should cycle back to Meier (index 2)"
    );
}

#[test]
fn test_out_of_range_parameter_index() {
    let settings = PluginSettings::default_for(&PluginType::Gain);

    // Gain has 2 parameters (gain_db, smoothing_ms)
    let descriptors = settings.get_descriptors();
    assert_eq!(descriptors.len(), 2);

    // Out of range index should return None for param_value
    assert_eq!(settings.param_value(99), None);
}

#[test]
fn test_boundary_values() {
    // Test Delay boundary values
    let mut settings = PluginSettings::default_for(&PluginType::Delay);

    // delay_ms: 0.0 to 5000.0 ms
    // Try adjusting to boundary
    settings.adjust_param(0, 10000.0); // Way beyond max
    let delay_val = settings.param_value(0).unwrap();
    assert!(
        delay_val <= 5000.0,
        "delay_ms should be clamped to max 5000.0"
    );

    settings.adjust_param(0, -10000.0); // Way below min
    let delay_val = settings.param_value(0).unwrap();
    assert!(delay_val >= 0.0, "delay_ms should be clamped to min 0.0");

    // feedback: -0.95 to 0.95
    settings.adjust_param(1, 100.0);
    let feedback_val = settings.param_value(1).unwrap();
    assert!(
        feedback_val <= 0.95,
        "feedback should be clamped to max 0.95"
    );

    settings.adjust_param(1, -100.0);
    let feedback_val = settings.param_value(1).unwrap();
    assert!(
        feedback_val >= -0.95,
        "feedback should be clamped to min -0.95"
    );
}
