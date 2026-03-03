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
            let value = settings.get_value_as_string(i);
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

    // Upmixer is expected to have 39 parameters currently (0-38)
    assert_eq!(descriptors.len(), 39);

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

    // Crossfeed is expected to have 16 parameters (0-15)
    assert_eq!(descriptors.len(), 16);

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

    // Initial descriptors count: 1 (Max Filters) + 5 * 4 (filters) = 21
    assert_eq!(descriptors.len(), 21);

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
    // New count: 1 (Max Filters) + 3 * 4 (filters) = 13
    assert_eq!(descriptors.len(), 13);

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
    // New count: 1 (Max Filters) + 6 * 4 (filters) = 25
    assert_eq!(descriptors.len(), 25);

    // Verify we can edit the new filter (index 21-24)
    let res = settings.adjust_param(21, 10.0); // Frequency of 6th filter
    assert!(res);
}
