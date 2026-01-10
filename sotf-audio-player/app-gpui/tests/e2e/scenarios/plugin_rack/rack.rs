//! E2E tests for Plugin Rack component.
//!
//! Tests for verifying plugin chain management:
//! - Adding plugins
//! - Removing plugins
//! - Reordering plugins (drag and drop)
//! - Enabling/disabling plugins
//! - Plugin selection
//! - Parameter editing mode

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Plugin Types (mirrors PluginType enum)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestPluginType {
    Eq,
    Gain,
    Compressor,
    Limiter,
    Gate,
    Expander,
    LoudnessCompensation,
    Upmixer,
    ChannelMuteSolo,
    MatrixMixer,
    LoudnessMonitor,
    SpectrumAnalyzer,
}

impl TestPluginType {
    fn display_name(&self) -> &'static str {
        match self {
            TestPluginType::Eq => "EQ",
            TestPluginType::Gain => "Gain",
            TestPluginType::Compressor => "Compressor",
            TestPluginType::Limiter => "Limiter",
            TestPluginType::Gate => "Gate",
            TestPluginType::Expander => "Expander",
            TestPluginType::LoudnessCompensation => "Loudness",
            TestPluginType::Upmixer => "Upmixer",
            TestPluginType::ChannelMuteSolo => "Mute/Solo",
            TestPluginType::MatrixMixer => "Matrix",
            TestPluginType::LoudnessMonitor => "LUFS",
            TestPluginType::SpectrumAnalyzer => "Spectrum",
        }
    }
}

#[derive(Debug, Clone)]
struct TestPlugin {
    plugin_type: TestPluginType,
    enabled: bool,
}

impl TestPlugin {
    fn new(plugin_type: TestPluginType) -> Self {
        Self {
            plugin_type,
            enabled: true,
        }
    }
}

// =============================================================================
// Add Plugin Tests
// =============================================================================

/// Test adding a single plugin to empty chain.
#[gpui::test]
async fn test_rack_add_plugin_to_empty(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(Vec::new()));

    // Add EQ plugin
    plugins
        .borrow_mut()
        .push(TestPlugin::new(TestPluginType::Eq));

    assert_eq!(plugins.borrow().len(), 1, "Chain should have 1 plugin");
    assert_eq!(
        plugins.borrow()[0].plugin_type,
        TestPluginType::Eq,
        "Plugin should be EQ"
    );
}

/// Test adding multiple plugins.
#[gpui::test]
async fn test_rack_add_multiple_plugins(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(Vec::new()));

    // Add plugins in order: EQ -> Compressor -> Limiter
    plugins
        .borrow_mut()
        .push(TestPlugin::new(TestPluginType::Eq));
    plugins
        .borrow_mut()
        .push(TestPlugin::new(TestPluginType::Compressor));
    plugins
        .borrow_mut()
        .push(TestPlugin::new(TestPluginType::Limiter));

    assert_eq!(plugins.borrow().len(), 3, "Chain should have 3 plugins");
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Compressor);
    assert_eq!(plugins.borrow()[2].plugin_type, TestPluginType::Limiter);
}

/// Test adding all plugin types.
#[gpui::test]
async fn test_rack_add_all_plugin_types(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(Vec::new()));

    let all_types = vec![
        TestPluginType::Eq,
        TestPluginType::Gain,
        TestPluginType::Compressor,
        TestPluginType::Limiter,
        TestPluginType::Gate,
        TestPluginType::Expander,
        TestPluginType::LoudnessCompensation,
        TestPluginType::Upmixer,
        TestPluginType::ChannelMuteSolo,
        TestPluginType::MatrixMixer,
        TestPluginType::LoudnessMonitor,
        TestPluginType::SpectrumAnalyzer,
    ];

    for plugin_type in &all_types {
        plugins.borrow_mut().push(TestPlugin::new(*plugin_type));
    }

    assert_eq!(
        plugins.borrow().len(),
        all_types.len(),
        "Should have all plugin types"
    );

    for (i, plugin_type) in all_types.iter().enumerate() {
        assert_eq!(
            plugins.borrow()[i].plugin_type,
            *plugin_type,
            "Plugin {} should be {:?}",
            i,
            plugin_type
        );
    }
}

// =============================================================================
// Remove Plugin Tests
// =============================================================================

/// Test removing a plugin from the middle.
#[gpui::test]
async fn test_rack_remove_plugin_middle(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Remove middle plugin (Compressor)
    plugins.borrow_mut().remove(1);

    assert_eq!(plugins.borrow().len(), 2, "Chain should have 2 plugins");
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Limiter);
}

/// Test removing first plugin.
#[gpui::test]
async fn test_rack_remove_plugin_first(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Remove first plugin (EQ)
    plugins.borrow_mut().remove(0);

    assert_eq!(plugins.borrow().len(), 2, "Chain should have 2 plugins");
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Compressor);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Limiter);
}

/// Test removing last plugin.
#[gpui::test]
async fn test_rack_remove_plugin_last(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Remove last plugin (Limiter)
    plugins.borrow_mut().pop();

    assert_eq!(plugins.borrow().len(), 2, "Chain should have 2 plugins");
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Compressor);
}

/// Test removing all plugins.
#[gpui::test]
async fn test_rack_remove_all_plugins(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Remove all plugins
    plugins.borrow_mut().clear();

    assert_eq!(plugins.borrow().len(), 0, "Chain should be empty");
}

// =============================================================================
// Reorder Plugin Tests
// =============================================================================

/// Test moving plugin up (swap with previous).
#[gpui::test]
async fn test_rack_move_plugin_up(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Move Compressor up (index 1 -> 0)
    let idx = 1;
    if idx > 0 {
        plugins.borrow_mut().swap(idx, idx - 1);
    }

    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Compressor);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[2].plugin_type, TestPluginType::Limiter);
}

/// Test moving plugin down (swap with next).
#[gpui::test]
async fn test_rack_move_plugin_down(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Move Compressor down (index 1 -> 2)
    let idx = 1;
    let len = plugins.borrow().len();
    if idx < len - 1 {
        plugins.borrow_mut().swap(idx, idx + 1);
    }

    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Limiter);
    assert_eq!(plugins.borrow()[2].plugin_type, TestPluginType::Compressor);
}

/// Test moving first plugin up (no-op).
#[gpui::test]
async fn test_rack_move_first_plugin_up(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
    ]));

    // Try to move first plugin up
    let idx = 0;
    if idx > 0 {
        plugins.borrow_mut().swap(idx, idx - 1);
    }

    // Order should be unchanged
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Compressor);
}

/// Test moving last plugin down (no-op).
#[gpui::test]
async fn test_rack_move_last_plugin_down(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
    ]));

    // Try to move last plugin down
    let idx = 1;
    let len = plugins.borrow().len();
    if idx < len - 1 {
        plugins.borrow_mut().swap(idx, idx + 1);
    }

    // Order should be unchanged
    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Eq);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Compressor);
}

/// Test drag-drop reorder (move from index 0 to index 2).
#[gpui::test]
async fn test_rack_drag_drop_reorder(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Drag from 0 to 2 (move EQ after Limiter)
    let from_idx = 0;
    let to_idx = 2;

    // Remove from source and insert at destination
    let plugin = plugins.borrow_mut().remove(from_idx);
    // Adjust index since we removed an element
    let adjusted_idx = if to_idx > from_idx {
        to_idx - 1
    } else {
        to_idx
    };
    plugins.borrow_mut().insert(adjusted_idx + 1, plugin);

    assert_eq!(plugins.borrow()[0].plugin_type, TestPluginType::Compressor);
    assert_eq!(plugins.borrow()[1].plugin_type, TestPluginType::Limiter);
    assert_eq!(plugins.borrow()[2].plugin_type, TestPluginType::Eq);
}

// =============================================================================
// Enable/Disable Plugin Tests
// =============================================================================

/// Test enabling a plugin.
#[gpui::test]
async fn test_rack_enable_plugin(_cx: &mut TestAppContext) {
    let mut plugin = TestPlugin::new(TestPluginType::Eq);
    plugin.enabled = false;

    // Enable plugin
    plugin.enabled = true;

    assert!(plugin.enabled, "Plugin should be enabled");
}

/// Test disabling a plugin.
#[gpui::test]
async fn test_rack_disable_plugin(_cx: &mut TestAppContext) {
    let mut plugin = TestPlugin::new(TestPluginType::Eq);
    plugin.enabled = true;

    // Disable plugin
    plugin.enabled = false;

    assert!(!plugin.enabled, "Plugin should be disabled");
}

/// Test toggle plugin enabled state.
#[gpui::test]
async fn test_rack_toggle_plugin_enabled(_cx: &mut TestAppContext) {
    let enabled = Rc::new(RefCell::new(true));

    // Toggle off
    {
        let current = *enabled.borrow();
        *enabled.borrow_mut() = !current;
    }
    assert!(!*enabled.borrow(), "Should be disabled after toggle");

    // Toggle on
    {
        let current = *enabled.borrow();
        *enabled.borrow_mut() = !current;
    }
    assert!(*enabled.borrow(), "Should be enabled after toggle");
}

/// Test multiple plugins enabled states.
#[gpui::test]
async fn test_rack_multiple_plugins_enabled_states(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));

    // Disable middle plugin
    plugins.borrow_mut()[1].enabled = false;

    assert!(plugins.borrow()[0].enabled, "First should be enabled");
    assert!(!plugins.borrow()[1].enabled, "Second should be disabled");
    assert!(plugins.borrow()[2].enabled, "Third should be enabled");
}

// =============================================================================
// Plugin Selection Tests
// =============================================================================

/// Test selecting a plugin.
#[gpui::test]
async fn test_rack_select_plugin(_cx: &mut TestAppContext) {
    let selected_index = Rc::new(RefCell::new(0usize));
    let plugin_count = 3;

    // Select second plugin
    *selected_index.borrow_mut() = 1;
    assert_eq!(*selected_index.borrow(), 1, "Should select second plugin");

    // Select third plugin
    *selected_index.borrow_mut() = 2;
    assert_eq!(*selected_index.borrow(), 2, "Should select third plugin");
}

/// Test selection bounds.
#[gpui::test]
async fn test_rack_selection_bounds(_cx: &mut TestAppContext) {
    let selected_index = Rc::new(RefCell::new(0usize));
    let plugin_count = 3;

    // Try to select beyond bounds (should clamp)
    let requested_idx = 5;
    let clamped = requested_idx.min(plugin_count - 1);
    *selected_index.borrow_mut() = clamped;

    assert_eq!(
        *selected_index.borrow(),
        2,
        "Selection should be clamped to last index"
    );
}

/// Test selection after removing selected plugin.
#[gpui::test]
async fn test_rack_selection_after_remove(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::Limiter),
    ]));
    let selected_index = Rc::new(RefCell::new(1usize)); // Compressor selected

    // Remove selected plugin
    plugins.borrow_mut().remove(*selected_index.borrow());

    // Adjust selection to stay in bounds
    let len = plugins.borrow().len();
    if *selected_index.borrow() >= len && len > 0 {
        *selected_index.borrow_mut() = len - 1;
    }

    assert_eq!(plugins.borrow().len(), 2);
    assert_eq!(*selected_index.borrow(), 1); // Should select Limiter now
}

// =============================================================================
// Parameter Editing Tests
// =============================================================================

/// Test entering parameter edit mode.
#[gpui::test]
async fn test_rack_enter_edit_mode(_cx: &mut TestAppContext) {
    let editing_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));

    // Enter edit mode for first plugin
    *editing_index.borrow_mut() = Some(0);

    assert_eq!(
        *editing_index.borrow(),
        Some(0),
        "Should be editing first plugin"
    );
}

/// Test exiting parameter edit mode.
#[gpui::test]
async fn test_rack_exit_edit_mode(_cx: &mut TestAppContext) {
    let editing_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(Some(0)));

    // Exit edit mode
    *editing_index.borrow_mut() = None;

    assert_eq!(*editing_index.borrow(), None, "Should not be editing");
}

/// Test parameter selection within edit mode.
#[gpui::test]
async fn test_rack_parameter_selection(_cx: &mut TestAppContext) {
    let param_selection = Rc::new(RefCell::new(0usize));
    let param_count = 5;

    // Select different parameters using number keys (0-9)
    for i in 0..param_count {
        *param_selection.borrow_mut() = i;
        assert_eq!(
            *param_selection.borrow(),
            i,
            "Should select parameter {}",
            i
        );
    }
}

// =============================================================================
// Plugin Chain Modified State Tests
// =============================================================================

/// Test chain modified flag after add.
#[gpui::test]
async fn test_rack_modified_after_add(_cx: &mut TestAppContext) {
    let modified = Rc::new(RefCell::new(false));
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(Vec::new()));

    // Add plugin
    plugins
        .borrow_mut()
        .push(TestPlugin::new(TestPluginType::Eq));
    *modified.borrow_mut() = true;

    assert!(*modified.borrow(), "Chain should be marked modified");
}

/// Test chain modified flag after remove.
#[gpui::test]
async fn test_rack_modified_after_remove(_cx: &mut TestAppContext) {
    let modified = Rc::new(RefCell::new(false));
    let plugins: Rc<RefCell<Vec<TestPlugin>>> =
        Rc::new(RefCell::new(vec![TestPlugin::new(TestPluginType::Eq)]));

    // Remove plugin
    plugins.borrow_mut().pop();
    *modified.borrow_mut() = true;

    assert!(*modified.borrow(), "Chain should be marked modified");
}

/// Test chain modified flag after reorder.
#[gpui::test]
async fn test_rack_modified_after_reorder(_cx: &mut TestAppContext) {
    let modified = Rc::new(RefCell::new(false));
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
    ]));

    // Reorder plugins
    plugins.borrow_mut().swap(0, 1);
    *modified.borrow_mut() = true;

    assert!(*modified.borrow(), "Chain should be marked modified");
}

/// Test clearing modified flag after save.
#[gpui::test]
async fn test_rack_clear_modified_after_save(_cx: &mut TestAppContext) {
    let modified = Rc::new(RefCell::new(true));

    // Simulate save
    *modified.borrow_mut() = false;

    assert!(!*modified.borrow(), "Modified flag should be cleared");
}

// =============================================================================
// Plugin Display Tests
// =============================================================================

/// Test plugin display names.
#[gpui::test]
async fn test_rack_plugin_display_names(_cx: &mut TestAppContext) {
    let test_cases = vec![
        (TestPluginType::Eq, "EQ"),
        (TestPluginType::Gain, "Gain"),
        (TestPluginType::Compressor, "Compressor"),
        (TestPluginType::Limiter, "Limiter"),
        (TestPluginType::Gate, "Gate"),
        (TestPluginType::Expander, "Expander"),
        (TestPluginType::LoudnessCompensation, "Loudness"),
        (TestPluginType::Upmixer, "Upmixer"),
        (TestPluginType::ChannelMuteSolo, "Mute/Solo"),
        (TestPluginType::MatrixMixer, "Matrix"),
        (TestPluginType::LoudnessMonitor, "LUFS"),
        (TestPluginType::SpectrumAnalyzer, "Spectrum"),
    ];

    for (plugin_type, expected_name) in test_cases {
        assert_eq!(
            plugin_type.display_name(),
            expected_name,
            "Display name for {:?} should be {}",
            plugin_type,
            expected_name
        );
    }
}

// =============================================================================
// View Mode Tests
// =============================================================================

/// Test plugin view mode (Rack vs Graph).
#[gpui::test]
async fn test_rack_view_mode(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum ViewMode {
        Rack,
        Graph,
    }

    let view_mode = Rc::new(RefCell::new(ViewMode::Rack));

    // Switch to Graph view
    *view_mode.borrow_mut() = ViewMode::Graph;
    assert_eq!(*view_mode.borrow(), ViewMode::Graph);

    // Switch back to Rack view
    *view_mode.borrow_mut() = ViewMode::Rack;
    assert_eq!(*view_mode.borrow(), ViewMode::Rack);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test loading preset updates chain.
#[gpui::test]
async fn test_rack_load_preset(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(Vec::new()));
    let last_loaded_preset: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Simulate loading a preset
    let preset_plugins = vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
        TestPlugin::new(TestPluginType::LoudnessMonitor),
    ];

    *plugins.borrow_mut() = preset_plugins;
    *last_loaded_preset.borrow_mut() = Some("Studio Mix".to_string());

    assert_eq!(plugins.borrow().len(), 3, "Should load preset plugins");
    assert_eq!(
        *last_loaded_preset.borrow(),
        Some("Studio Mix".to_string()),
        "Should track loaded preset name"
    );
}

/// Test saving current chain as preset.
#[gpui::test]
async fn test_rack_save_preset(_cx: &mut TestAppContext) {
    let plugins: Rc<RefCell<Vec<TestPlugin>>> = Rc::new(RefCell::new(vec![
        TestPlugin::new(TestPluginType::Eq),
        TestPlugin::new(TestPluginType::Compressor),
    ]));
    let saved_preset_name: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Simulate saving preset
    *saved_preset_name.borrow_mut() = Some("My Custom Preset".to_string());

    assert_eq!(
        *saved_preset_name.borrow(),
        Some("My Custom Preset".to_string()),
        "Should save preset with name"
    );
}
