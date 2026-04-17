//! Plugin state management.
//!
//! Thin wrapper around `PluginController` from sotf-player, adding GPUI-specific
//! state (graph view, workflow canvas, pending update tracking, etc.).

use std::ops::{Deref, DerefMut};

use crate::app::types::PluginUpdateType;
use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio_player::{ConnectionDrag, GraphSelection, NodeDrag, PluginController};
use sotf_audio_player_midi::MidiMappingEngine;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PluginState {
    ctrl: PluginController,

    // GPUI-specific: tracks whether chain has been modified since last save
    pub plugin_graph_modified: bool,
    pub pending_plugin_update: Option<PluginUpdateType>,

    // GPUI-specific: UI state
    pub matrix_selected_cell: Option<(usize, usize)>,
    pub plugin_view_mode: PluginViewMode,
    pub graph_selection: GraphSelection,
    pub graph_connection_drag: Option<ConnectionDrag>,
    pub graph_node_drag: Option<NodeDrag>,
    pub workflow_canvas: Option<Entity<WorkflowCanvas>>,
    pub workflow_node_mapping: Option<WorkflowNodeMapping>,
    pub editing_plugin_node: Option<gpui_ui_kit::workflow::NodeId>,
    /// The `GraphNodeId` (UUID) of the plugin being edited in the graph modal.
    /// When set, `set_plugin_param` redirects to the node-ID-based path so
    /// parameter edits work even in non-linear graphs.
    pub editing_graph_node_uuid: Option<sotf_audio_player::GraphNodeId>,
    /// Dropdown states for AB Compare plugin
    pub ab_compare_dropdowns: ABCompareDropdowns,
    /// File paths for AB Compare loaded configs (for display)
    pub ab_compare_file_a: Option<String>,
    pub ab_compare_file_b: Option<String>,
    /// Parsed sub-rack contents for A/B paths (kept in sync with engine JSON)
    pub ab_path_a: Vec<sotf_audio_player::controllers::ab_compare_path::PluginInRack>,
    pub ab_path_b: Vec<sotf_audio_player::controllers::ab_compare_path::PluginInRack>,
    /// Selected sub-plugin index within each A/B path
    pub ab_path_a_selected: Option<usize>,
    pub ab_path_b_selected: Option<usize>,
    /// Which path's "add plugin" menu is currently open
    pub ab_add_menu_target: Option<ABPathTarget>,
    /// Which plugin UI view mode to show
    pub plugin_ui_view: PluginUiView,
    /// Whether the controller picker dropdown is open
    pub controller_picker_open: bool,
    /// MIDI controller → plugin parameter mapping engine
    pub midi_mapping: MidiMappingEngine,
    /// Per-plugin preset picker state
    pub plugin_preset_open: Option<usize>, // Some(plugin_idx) when open
    pub plugin_preset_list: Vec<String>, // Available presets for the open plugin
    pub plugin_preset_save_mode: bool,   // True when in save mode (text input)
    pub plugin_preset_input: String,     // Save preset name input

    /// Pending confirmation for destructive actions
    pub confirm_remove_plugin: Option<usize>, // Some(plugin_idx) awaiting confirmation
    pub confirm_delete_preset: Option<(usize, String)>, // Some((plugin_idx, preset_name)) awaiting confirmation

    // Chain-level state
    /// When true, all plugins are bypassed (audio passes through unchanged)
    pub chain_bypass: bool,
    /// Chain-level auto-gain toggle
    pub chain_autogain: bool,
    /// Index of the currently soloed plugin (None = no solo active)
    pub soloed_plugin_index: Option<usize>,
    /// Saved enabled states before solo was activated (to restore on un-solo)
    pub pre_solo_enabled_states: Vec<bool>,
}

impl Deref for PluginState {
    type Target = PluginController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for PluginState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl PluginState {
    /// The effective view mode: returns `Graph` when the graph topology is
    /// non-linear (rack can't represent it), otherwise returns the user's choice.
    pub fn effective_view_mode(&self) -> PluginViewMode {
        if !self.ctrl.is_linear() {
            PluginViewMode::Graph
        } else {
            self.plugin_view_mode
        }
    }

    /// Whether the rack view is available (graph must be linear).
    pub fn is_rack_available(&self) -> bool {
        self.ctrl.is_linear()
    }
}

/// Dropdown open states for AB Compare plugin UI
#[derive(Debug, Clone, Copy, Default)]
pub struct ABCompareDropdowns {
    pub path_a_open: bool,
    pub path_b_open: bool,
}

/// Which A/B path the add menu targets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ABPathTarget {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PluginViewMode {
    #[default]
    Rack,
    Graph,
}

/// Which view to show for a plugin's UI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PluginUiView {
    /// Graphical plugin-specific UI (EQ graph, compressor curves, etc.)
    #[default]
    UI,
    /// Hardware controller view with MIDI mappings for a specific controller layout
    Controller(String),
    /// Simple text-based parameter table
    Simple,
}

impl PluginUiView {
    pub fn is_simple(&self) -> bool {
        matches!(self, Self::Simple)
    }

    pub fn is_controller(&self) -> bool {
        matches!(self, Self::Controller(_))
    }

    pub fn is_ui(&self) -> bool {
        matches!(self, Self::UI)
    }
}

/// Returns the list of available controller layout names
pub fn available_controllers() -> Vec<(&'static str, &'static str)> {
    vec![("xone_k2", "Xone:K2"), ("lcxl", "Launch Control XL")]
}

#[derive(Clone, Default, Debug)]
pub struct WorkflowNodeMapping {
    pub node_to_plugin: HashMap<NodeId, usize>,
    pub plugin_to_node: HashMap<usize, NodeId>,
    pub input_node_id: Option<NodeId>,
    pub output_node_id: Option<NodeId>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            ctrl: PluginController::new(),
            plugin_graph_modified: false,
            pending_plugin_update: None,
            matrix_selected_cell: None,
            plugin_view_mode: PluginViewMode::Rack,
            graph_selection: GraphSelection::default(),
            graph_connection_drag: None,
            graph_node_drag: None,
            workflow_canvas: None,
            workflow_node_mapping: None,
            editing_plugin_node: None,
            editing_graph_node_uuid: None,
            ab_compare_dropdowns: ABCompareDropdowns::default(),
            ab_compare_file_a: None,
            ab_compare_file_b: None,
            ab_path_a: Vec::new(),
            ab_path_b: Vec::new(),
            ab_path_a_selected: None,
            ab_path_b_selected: None,
            ab_add_menu_target: None,
            plugin_ui_view: PluginUiView::UI,
            controller_picker_open: false,
            midi_mapping: MidiMappingEngine::new(),
            plugin_preset_open: None,
            plugin_preset_list: Vec::new(),
            plugin_preset_save_mode: false,
            plugin_preset_input: String::new(),
            confirm_remove_plugin: None,
            confirm_delete_preset: None,
            chain_bypass: false,
            chain_autogain: false,
            soloed_plugin_index: None,
            pre_solo_enabled_states: Vec::new(),
        }
    }
}

impl PluginState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear pending destructive-action confirmations.
    /// Called on any structural change (toggle, reorder, screen switch)
    /// to prevent stale confirmation state from persisting.
    pub fn clear_confirmations(&mut self) {
        self.confirm_remove_plugin = None;
        self.confirm_delete_preset = None;
    }

    /// Sync the parsed A/B path state from the engine-side JSON config strings.
    /// Call this when selecting an AB Compare plugin or after loading a preset.
    pub fn sync_ab_path_state(&mut self, path_a_json: &str, path_b_json: &str) {
        use sotf_audio_player::controllers::ab_compare_path::parse_path_config;
        self.ab_path_a = parse_path_config(path_a_json);
        self.ab_path_b = parse_path_config(path_b_json);
        // Clamp selections to valid range
        if let Some(sel) = self.ab_path_a_selected
            && sel >= self.ab_path_a.len()
        {
            self.ab_path_a_selected = None;
        }
        if let Some(sel) = self.ab_path_b_selected
            && sel >= self.ab_path_b.len()
        {
            self.ab_path_b_selected = None;
        }
    }

    /// Clear all A/B path state (called when an AB Compare plugin is removed).
    pub fn clear_ab_path_state(&mut self) {
        self.ab_path_a.clear();
        self.ab_path_b.clear();
        self.ab_path_a_selected = None;
        self.ab_path_b_selected = None;
        self.ab_add_menu_target = None;
    }
}
