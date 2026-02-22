//! Plugin state management.
//!
//! Contains all state related to audio plugins including the plugin chain,
//! graph view state, and editing state.

use crate::app::types::PluginUpdateType;
use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio_player::{ConnectionDrag, GraphSelection, NodeDrag, PluginChain, PluginGraph};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PluginState {
    pub plugin_chain: PluginChain,
    pub plugin_chain_modified: bool,
    pub pending_plugin_update: Option<PluginUpdateType>,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize,
    pub selected_plugin_index: usize,
    pub selected_eq_band: usize,
    /// Selected channel for per-channel EQ mode (0 = first channel)
    pub selected_eq_channel: usize,
    pub matrix_selected_cell: Option<(usize, usize)>,
    pub plugin_view_mode: PluginViewMode,
    pub plugin_graph: Option<PluginGraph>,
    pub graph_selection: GraphSelection,
    pub graph_connection_drag: Option<ConnectionDrag>,
    pub graph_node_drag: Option<NodeDrag>,
    pub workflow_canvas: Option<Entity<WorkflowCanvas>>,
    pub workflow_node_mapping: Option<WorkflowNodeMapping>,
    pub editing_plugin_node: Option<gpui_ui_kit::workflow::NodeId>,
    pub available_plugin_presets: Vec<String>,
    pub selected_preset_index: usize,
    pub last_loaded_preset: Option<String>,
    /// Dropdown states for AB Compare plugin
    pub ab_compare_dropdowns: ABCompareDropdowns,
    /// When true, show a simple text-based parameter list instead of the graphical plugin view
    pub simple_view: bool,
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

#[derive(Clone, Default, Debug)]
pub struct WorkflowNodeMapping {
    pub node_to_plugin: HashMap<NodeId, usize>,
    pub plugin_to_node: HashMap<usize, NodeId>,
    pub input_node_id: Option<NodeId>,
    pub output_node_id: Option<NodeId>,
}

impl Default for PluginState {
    fn default() -> Self {
        // Create default rack with permanent plugins:
        // Input Monitor -> [user plugins] -> Matrix -> Output Monitor
        let chain = PluginChain::with_default_rack();
        Self {
            plugin_chain: chain,
            plugin_chain_modified: false,
            pending_plugin_update: None,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            selected_plugin_index: 0,
            selected_eq_band: 0,
            selected_eq_channel: 0,
            matrix_selected_cell: None,
            plugin_view_mode: PluginViewMode::Rack,
            plugin_graph: None,
            graph_selection: GraphSelection::default(),
            graph_connection_drag: None,
            graph_node_drag: None,
            workflow_canvas: None,
            workflow_node_mapping: None,
            editing_plugin_node: None,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            last_loaded_preset: None,
            ab_compare_dropdowns: ABCompareDropdowns::default(),
            simple_view: false,
        }
    }
}

impl PluginState {
    pub fn new() -> Self {
        Self::default()
    }
}
