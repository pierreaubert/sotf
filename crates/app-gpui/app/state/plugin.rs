//! Plugin state management.
//!
//! Thin wrapper around `PluginController` from sotf-player, adding GPUI-specific
//! state (graph view, workflow canvas, pending update tracking, etc.).

use std::ops::{Deref, DerefMut};

use crate::app::types::PluginUpdateType;
use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio_player::{ConnectionDrag, GraphSelection, NodeDrag, PluginController, PluginGraph};
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
    /// When true, show a simple text-based parameter list instead of the graphical plugin view
    pub simple_view: bool,
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
            simple_view: false,
        }
    }
}

impl PluginState {
    pub fn new() -> Self {
        Self::default()
    }
}
