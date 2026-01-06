//! E2E integration tests for plugin workflow.
//!
//! Tests the complete workflow of:
//! 1. Adding plugins to the rack
//! 2. Configuring plugin parameters
//! 3. Enabling/disabling plugins
//! 4. Reordering plugins
//! 5. Saving and loading presets
//!
//! This test simulates a real user session configuring their
//! audio processing chain.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Plugin Types
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginType {
    Eq,
    Gain,
    Compressor,
    Limiter,
    Spectrum,
    LoudnessMonitor,
}

impl PluginType {
    fn display_name(&self) -> &'static str {
        match self {
            PluginType::Eq => "Parametric EQ",
            PluginType::Gain => "Gain",
            PluginType::Compressor => "Compressor",
            PluginType::Limiter => "Limiter",
            PluginType::Spectrum => "Spectrum Analyzer",
            PluginType::LoudnessMonitor => "Loudness Monitor",
        }
    }
}

// =============================================================================
// Plugin Instance
// =============================================================================

#[derive(Debug, Clone)]
struct PluginInstance {
    id: usize,
    plugin_type: PluginType,
    enabled: bool,
    parameters: serde_json::Value,
}

impl PluginInstance {
    fn new(id: usize, plugin_type: PluginType) -> Self {
        Self {
            id,
            plugin_type,
            enabled: true,
            parameters: Self::default_parameters(plugin_type),
        }
    }

    fn default_parameters(plugin_type: PluginType) -> serde_json::Value {
        match plugin_type {
            PluginType::Eq => serde_json::json!({
                "bands": [],
                "auto_gain": false
            }),
            PluginType::Gain => serde_json::json!({
                "gain_db": 0.0,
                "muted": false
            }),
            PluginType::Compressor => serde_json::json!({
                "threshold_db": -20.0,
                "ratio": 4.0,
                "attack_ms": 10.0,
                "release_ms": 100.0,
                "makeup_db": 0.0
            }),
            PluginType::Limiter => serde_json::json!({
                "threshold_db": -1.0,
                "release_ms": 100.0
            }),
            PluginType::Spectrum => serde_json::json!({
                "fft_size": 2048,
                "smoothing": 0.7
            }),
            PluginType::LoudnessMonitor => serde_json::json!({
                "target_lufs": -14.0
            }),
        }
    }
}

// =============================================================================
// Plugin Rack State
// =============================================================================

#[derive(Debug, Clone)]
struct PluginRackState {
    plugins: Vec<PluginInstance>,
    selected_plugin: Option<usize>,
    next_id: usize,
}

impl Default for PluginRackState {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
            selected_plugin: None,
            next_id: 1,
        }
    }
}

impl PluginRackState {
    fn add_plugin(&mut self, plugin_type: PluginType) -> usize {
        let id = self.next_id;
        self.plugins.push(PluginInstance::new(id, plugin_type));
        self.next_id += 1;
        self.selected_plugin = Some(self.plugins.len() - 1);
        id
    }

    fn remove_plugin(&mut self, index: usize) -> bool {
        if index < self.plugins.len() {
            self.plugins.remove(index);
            // Adjust selection
            if let Some(sel) = self.selected_plugin {
                if sel >= self.plugins.len() {
                    self.selected_plugin = if self.plugins.is_empty() {
                        None
                    } else {
                        Some(self.plugins.len() - 1)
                    };
                }
            }
            true
        } else {
            false
        }
    }

    fn move_plugin(&mut self, from: usize, to: usize) -> bool {
        if from < self.plugins.len() && to < self.plugins.len() {
            let plugin = self.plugins.remove(from);
            self.plugins.insert(to, plugin);
            self.selected_plugin = Some(to);
            true
        } else {
            false
        }
    }

    fn toggle_plugin(&mut self, index: usize) -> bool {
        if let Some(plugin) = self.plugins.get_mut(index) {
            plugin.enabled = !plugin.enabled;
            true
        } else {
            false
        }
    }
}

// =============================================================================
// Preset Structure
// =============================================================================

#[derive(Debug, Clone)]
struct RackPreset {
    name: String,
    plugins: Vec<PluginInstance>,
}

// =============================================================================
// Full Workflow: Add EQ -> Adjust -> Add Compressor -> Save
// =============================================================================

/// Test complete plugin workflow: Add EQ, configure bands, add compressor, save preset.
#[gpui::test]
async fn test_plugin_workflow_complete(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Step 1: Start with empty rack
    assert!(state.borrow().plugins.is_empty(), "Rack should start empty");

    // Step 2: Add EQ plugin
    let eq_id = state.borrow_mut().add_plugin(PluginType::Eq);
    assert_eq!(state.borrow().plugins.len(), 1);
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Eq);

    // Step 3: Configure EQ bands
    {
        let mut s = state.borrow_mut();
        if let Some(plugin) = s.plugins.iter_mut().find(|p| p.id == eq_id) {
            plugin.parameters = serde_json::json!({
                "bands": [
                    {"type": "highpass", "frequency": 80.0, "q": 0.7},
                    {"type": "peak", "frequency": 250.0, "q": 2.0, "gain_db": -3.0},
                    {"type": "peak", "frequency": 3000.0, "q": 1.5, "gain_db": 2.0},
                    {"type": "highshelf", "frequency": 10000.0, "q": 0.7, "gain_db": 1.5}
                ],
                "auto_gain": true
            });
        }
    }

    // Verify EQ configuration
    {
        let s = state.borrow();
        let eq = s.plugins.iter().find(|p| p.id == eq_id).unwrap();
        let bands = eq.parameters["bands"].as_array().unwrap();
        assert_eq!(bands.len(), 4, "Should have 4 EQ bands");
    }

    // Step 4: Add compressor plugin
    let comp_id = state.borrow_mut().add_plugin(PluginType::Compressor);
    assert_eq!(state.borrow().plugins.len(), 2);

    // Step 5: Configure compressor
    {
        let mut s = state.borrow_mut();
        if let Some(plugin) = s.plugins.iter_mut().find(|p| p.id == comp_id) {
            plugin.parameters = serde_json::json!({
                "threshold_db": -18.0,
                "ratio": 3.0,
                "attack_ms": 5.0,
                "release_ms": 50.0,
                "knee_db": 6.0,
                "makeup_db": 2.0
            });
        }
    }

    // Verify compressor configuration
    {
        let s = state.borrow();
        let comp = s.plugins.iter().find(|p| p.id == comp_id).unwrap();
        let threshold = comp.parameters["threshold_db"].as_f64().unwrap();
        assert!((threshold - (-18.0)).abs() < 0.001);
    }

    // Step 6: Add limiter as safety
    let _limiter_id = state.borrow_mut().add_plugin(PluginType::Limiter);
    assert_eq!(state.borrow().plugins.len(), 3);

    // Step 7: Verify plugin order (EQ -> Compressor -> Limiter)
    {
        let s = state.borrow();
        assert_eq!(s.plugins[0].plugin_type, PluginType::Eq);
        assert_eq!(s.plugins[1].plugin_type, PluginType::Compressor);
        assert_eq!(s.plugins[2].plugin_type, PluginType::Limiter);
    }

    // Step 8: Save as preset
    let preset = {
        let s = state.borrow();
        RackPreset {
            name: "Vocal Chain".to_string(),
            plugins: s.plugins.clone(),
        }
    };

    assert_eq!(preset.name, "Vocal Chain");
    assert_eq!(preset.plugins.len(), 3);
}

// =============================================================================
// Plugin Addition Workflow Tests
// =============================================================================

/// Test adding multiple plugins of different types.
#[gpui::test]
async fn test_plugin_workflow_add_multiple_types(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Add one of each type
    let types = vec![
        PluginType::Eq,
        PluginType::Gain,
        PluginType::Compressor,
        PluginType::Limiter,
        PluginType::Spectrum,
        PluginType::LoudnessMonitor,
    ];

    for plugin_type in &types {
        state.borrow_mut().add_plugin(*plugin_type);
    }

    assert_eq!(state.borrow().plugins.len(), 6);

    // Verify each type
    for (i, plugin_type) in types.iter().enumerate() {
        assert_eq!(state.borrow().plugins[i].plugin_type, *plugin_type);
    }
}

/// Test adding duplicate plugin types.
#[gpui::test]
async fn test_plugin_workflow_add_duplicates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Add multiple EQs (valid scenario for serial EQ)
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Eq);

    assert_eq!(state.borrow().plugins.len(), 3);
    assert!(state.borrow().plugins.iter().all(|p| p.plugin_type == PluginType::Eq));

    // Each should have unique ID
    let ids: Vec<usize> = state.borrow().plugins.iter().map(|p| p.id).collect();
    assert_eq!(ids[0], 1);
    assert_eq!(ids[1], 2);
    assert_eq!(ids[2], 3);
}

// =============================================================================
// Plugin Parameter Workflow Tests
// =============================================================================

/// Test modifying plugin parameters.
#[gpui::test]
async fn test_plugin_workflow_modify_parameters(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Gain);

    // Initial state
    {
        let s = state.borrow();
        let gain = s.plugins[0].parameters["gain_db"].as_f64().unwrap();
        assert!(gain.abs() < 0.001, "Initial gain should be 0 dB");
    }

    // Modify gain
    {
        let mut s = state.borrow_mut();
        s.plugins[0].parameters["gain_db"] = serde_json::json!(6.0);
    }

    // Verify modification
    {
        let s = state.borrow();
        let gain = s.plugins[0].parameters["gain_db"].as_f64().unwrap();
        assert!((gain - 6.0).abs() < 0.001, "Gain should be 6 dB");
    }
}

/// Test parameter validation (conceptual).
#[gpui::test]
async fn test_plugin_workflow_parameter_validation(_cx: &mut TestAppContext) {
    fn validate_compressor_params(params: &serde_json::Value) -> Result<(), String> {
        let threshold = params["threshold_db"]
            .as_f64()
            .ok_or("Missing threshold")?;
        let ratio = params["ratio"].as_f64().ok_or("Missing ratio")?;
        let attack = params["attack_ms"].as_f64().ok_or("Missing attack")?;
        let release = params["release_ms"].as_f64().ok_or("Missing release")?;

        if threshold < -60.0 || threshold > 0.0 {
            return Err("Threshold out of range".to_string());
        }
        if ratio < 1.0 || ratio > 20.0 {
            return Err("Ratio out of range".to_string());
        }
        if attack < 0.1 || attack > 100.0 {
            return Err("Attack out of range".to_string());
        }
        if release < 10.0 || release > 1000.0 {
            return Err("Release out of range".to_string());
        }

        Ok(())
    }

    // Valid params
    let valid_params = serde_json::json!({
        "threshold_db": -20.0,
        "ratio": 4.0,
        "attack_ms": 10.0,
        "release_ms": 100.0
    });
    assert!(validate_compressor_params(&valid_params).is_ok());

    // Invalid threshold
    let invalid_params = serde_json::json!({
        "threshold_db": -70.0,
        "ratio": 4.0,
        "attack_ms": 10.0,
        "release_ms": 100.0
    });
    assert!(validate_compressor_params(&invalid_params).is_err());
}

// =============================================================================
// Plugin Enable/Disable Workflow Tests
// =============================================================================

/// Test toggling plugin enabled state.
#[gpui::test]
async fn test_plugin_workflow_toggle_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Compressor);

    // Initially enabled
    assert!(state.borrow().plugins[0].enabled);

    // Disable
    state.borrow_mut().toggle_plugin(0);
    assert!(!state.borrow().plugins[0].enabled);

    // Re-enable
    state.borrow_mut().toggle_plugin(0);
    assert!(state.borrow().plugins[0].enabled);
}

/// Test bypass behavior with multiple plugins.
#[gpui::test]
async fn test_plugin_workflow_bypass_chain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);
    state.borrow_mut().add_plugin(PluginType::Limiter);

    // Disable middle plugin
    state.borrow_mut().toggle_plugin(1);

    // Count enabled plugins
    let enabled_count = state.borrow().plugins.iter().filter(|p| p.enabled).count();
    assert_eq!(enabled_count, 2);

    // Verify which are enabled
    assert!(state.borrow().plugins[0].enabled, "EQ should be enabled");
    assert!(!state.borrow().plugins[1].enabled, "Compressor should be disabled");
    assert!(state.borrow().plugins[2].enabled, "Limiter should be enabled");
}

// =============================================================================
// Plugin Reordering Workflow Tests
// =============================================================================

/// Test moving plugins in the chain.
#[gpui::test]
async fn test_plugin_workflow_reorder(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Compressor);
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Limiter);

    // Initial order: Compressor, EQ, Limiter
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Compressor);
    assert_eq!(state.borrow().plugins[1].plugin_type, PluginType::Eq);

    // Move EQ to front (typical mastering order)
    state.borrow_mut().move_plugin(1, 0);

    // New order: EQ, Compressor, Limiter
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Eq);
    assert_eq!(state.borrow().plugins[1].plugin_type, PluginType::Compressor);
    assert_eq!(state.borrow().plugins[2].plugin_type, PluginType::Limiter);
}

/// Test moving plugin to end of chain.
#[gpui::test]
async fn test_plugin_workflow_move_to_end(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Limiter);
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);

    // Move limiter to end (safety last)
    state.borrow_mut().move_plugin(0, 2);

    // New order: EQ, Compressor, Limiter
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Eq);
    assert_eq!(state.borrow().plugins[1].plugin_type, PluginType::Compressor);
    assert_eq!(state.borrow().plugins[2].plugin_type, PluginType::Limiter);
}

// =============================================================================
// Plugin Removal Workflow Tests
// =============================================================================

/// Test removing plugins from chain.
#[gpui::test]
async fn test_plugin_workflow_remove(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);
    state.borrow_mut().add_plugin(PluginType::Limiter);

    assert_eq!(state.borrow().plugins.len(), 3);

    // Remove middle plugin
    state.borrow_mut().remove_plugin(1);

    assert_eq!(state.borrow().plugins.len(), 2);
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Eq);
    assert_eq!(state.borrow().plugins[1].plugin_type, PluginType::Limiter);
}

/// Test removing all plugins.
#[gpui::test]
async fn test_plugin_workflow_remove_all(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);

    // Remove all
    while !state.borrow().plugins.is_empty() {
        state.borrow_mut().remove_plugin(0);
    }

    assert!(state.borrow().plugins.is_empty());
    assert!(state.borrow().selected_plugin.is_none());
}

// =============================================================================
// Preset Workflow Tests
// =============================================================================

/// Test saving and loading preset.
#[gpui::test]
async fn test_plugin_workflow_preset_save_load(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Configure a chain
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);

    {
        let mut s = state.borrow_mut();
        s.plugins[0].parameters = serde_json::json!({
            "bands": [
                {"type": "peak", "frequency": 1000.0, "q": 1.0, "gain_db": 3.0}
            ]
        });
    }

    // Save preset
    let saved_preset = {
        let s = state.borrow();
        RackPreset {
            name: "Test Preset".to_string(),
            plugins: s.plugins.clone(),
        }
    };

    // Clear current state
    *state.borrow_mut() = PluginRackState::default();
    assert!(state.borrow().plugins.is_empty());

    // Load preset
    {
        let mut s = state.borrow_mut();
        s.plugins = saved_preset.plugins;
        if !s.plugins.is_empty() {
            s.selected_plugin = Some(0);
        }
    }

    // Verify restored state
    assert_eq!(state.borrow().plugins.len(), 2);
    assert_eq!(state.borrow().plugins[0].plugin_type, PluginType::Eq);

    let bands = state.borrow().plugins[0].parameters["bands"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(bands, 1);
}

/// Test preset naming.
#[gpui::test]
async fn test_plugin_workflow_preset_naming(_cx: &mut TestAppContext) {
    fn validate_preset_name(name: &str) -> Result<(), &'static str> {
        if name.is_empty() {
            return Err("Name cannot be empty");
        }
        if name.len() > 50 {
            return Err("Name too long");
        }
        if name.chars().any(|c| c == '/' || c == '\\') {
            return Err("Invalid characters");
        }
        Ok(())
    }

    assert!(validate_preset_name("My Preset").is_ok());
    assert!(validate_preset_name("").is_err());
    assert!(validate_preset_name("a".repeat(60).as_str()).is_err());
    assert!(validate_preset_name("my/preset").is_err());
}

// =============================================================================
// Selection Workflow Tests
// =============================================================================

/// Test plugin selection changes.
#[gpui::test]
async fn test_plugin_workflow_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);
    state.borrow_mut().add_plugin(PluginType::Limiter);

    // Last added is selected
    assert_eq!(state.borrow().selected_plugin, Some(2));

    // Select first
    state.borrow_mut().selected_plugin = Some(0);
    assert_eq!(state.borrow().selected_plugin, Some(0));

    // Select middle
    state.borrow_mut().selected_plugin = Some(1);
    assert_eq!(
        state.borrow().plugins[state.borrow().selected_plugin.unwrap()].plugin_type,
        PluginType::Compressor
    );
}

// =============================================================================
// Error Handling Workflow Tests
// =============================================================================

/// Test removing from empty rack.
#[gpui::test]
async fn test_plugin_workflow_remove_from_empty(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    let result = state.borrow_mut().remove_plugin(0);
    assert!(!result, "Remove from empty should return false");
}

/// Test invalid move operation.
#[gpui::test]
async fn test_plugin_workflow_invalid_move(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    state.borrow_mut().add_plugin(PluginType::Eq);

    let result = state.borrow_mut().move_plugin(0, 5);
    assert!(!result, "Move to invalid index should return false");
}

/// Test toggle on invalid index.
#[gpui::test]
async fn test_plugin_workflow_toggle_invalid(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    let result = state.borrow_mut().toggle_plugin(0);
    assert!(!result, "Toggle on empty should return false");
}

// =============================================================================
// Typical User Scenarios
// =============================================================================

/// Test typical mixing chain setup.
#[gpui::test]
async fn test_plugin_workflow_mixing_chain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Typical mixing chain order
    state.borrow_mut().add_plugin(PluginType::Eq);        // Corrective EQ
    state.borrow_mut().add_plugin(PluginType::Compressor); // Dynamics control
    state.borrow_mut().add_plugin(PluginType::Eq);        // Tonal EQ
    state.borrow_mut().add_plugin(PluginType::Limiter);   // Safety limiter

    assert_eq!(state.borrow().plugins.len(), 4);

    // Verify signal flow
    let types: Vec<PluginType> = state.borrow().plugins.iter().map(|p| p.plugin_type).collect();
    assert_eq!(types, vec![
        PluginType::Eq,
        PluginType::Compressor,
        PluginType::Eq,
        PluginType::Limiter
    ]);
}

/// Test analysis chain setup.
#[gpui::test]
async fn test_plugin_workflow_analysis_chain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginRackState::default()));

    // Analysis plugins at the end (post all processing)
    state.borrow_mut().add_plugin(PluginType::Eq);
    state.borrow_mut().add_plugin(PluginType::Compressor);
    state.borrow_mut().add_plugin(PluginType::Spectrum);
    state.borrow_mut().add_plugin(PluginType::LoudnessMonitor);

    // Verify analyzers are after processing
    let types: Vec<PluginType> = state.borrow().plugins.iter().map(|p| p.plugin_type).collect();
    assert_eq!(types[2], PluginType::Spectrum);
    assert_eq!(types[3], PluginType::LoudnessMonitor);
}
