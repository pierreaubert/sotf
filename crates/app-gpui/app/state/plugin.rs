//! Plugin state management.
//!
//! Thin wrapper around `PluginController` from sotf-player, adding GPUI-specific
//! state (graph view, workflow canvas, pending update tracking, etc.).

use std::ops::{Deref, DerefMut};

use crate::app::types::PluginUpdateType;
use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio_player::{ConnectionDrag, GraphSelection, NodeDrag, PluginController, PluginGraph};
use sotf_audio_player_midi::MidiMappingEngine;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PluginState {
    ctrl: PluginController,

    // GPUI-specific: tracks whether chain has been modified since last save
    pub plugin_chain_modified: bool,
    pub pending_plugin_update: Option<PluginUpdateType>,

    // GPUI-specific: UI state
    pub matrix_selected_cell: Option<(usize, usize)>,
    pub plugin_view_mode: PluginViewMode,
    pub plugin_graph: Option<PluginGraph>,
    pub graph_selection: GraphSelection,
    pub graph_connection_drag: Option<ConnectionDrag>,
    pub graph_node_drag: Option<NodeDrag>,
    pub workflow_canvas: Option<Entity<WorkflowCanvas>>,
    pub workflow_node_mapping: Option<WorkflowNodeMapping>,
    pub editing_plugin_node: Option<gpui_ui_kit::workflow::NodeId>,
    /// Dropdown states for AB Compare plugin
    pub ab_compare_dropdowns: ABCompareDropdowns,
    /// File paths for AB Compare loaded configs (for display)
    pub ab_compare_file_a: Option<String>,
    pub ab_compare_file_b: Option<String>,
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

/// Dropdown open states for AB Compare plugin UI
#[derive(Debug, Clone, Copy, Default)]
pub struct ABCompareDropdowns {
    pub path_a_open: bool,
    pub path_b_open: bool,
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
            plugin_chain_modified: false,
            pending_plugin_update: None,
            matrix_selected_cell: None,
            plugin_view_mode: PluginViewMode::Rack,
            plugin_graph: None,
            graph_selection: GraphSelection::default(),
            graph_connection_drag: None,
            graph_node_drag: None,
            workflow_canvas: None,
            workflow_node_mapping: None,
            editing_plugin_node: None,
            ab_compare_dropdowns: ABCompareDropdowns::default(),
            ab_compare_file_a: None,
            ab_compare_file_b: None,
            plugin_ui_view: PluginUiView::UI,
            controller_picker_open: false,
            midi_mapping: MidiMappingEngine::new(),
            plugin_preset_open: None,
            plugin_preset_list: Vec::new(),
            plugin_preset_save_mode: false,
            plugin_preset_input: String::new(),
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
}
