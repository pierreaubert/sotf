//! Workflow Canvas - A ReactFlow-like node graph editor
//!
//! Provides a GPU-accelerated canvas for building node-based workflows with:
//! - Draggable nodes with custom content
//! - Directional connections between input/output ports
//! - Selection (single, multi, box selection)
//! - Pan/zoom navigation
//! - Undo/redo history
//! - Copy/paste support
//! - State persistence with versioned JSON

mod bezier;
mod canvas;
mod history;
mod hit_test;
mod node;
mod port;
mod state;
mod theme;

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests;

// Re-export main types
pub use canvas::WorkflowCanvas;
pub use history::{Command, AddNodeCommand, RemoveNodeCommand, MoveNodesCommand, AddConnectionCommand, RemoveConnectionCommand, ChangePortCountsCommand, CompositeCommand, HistoryManager};
pub use hit_test::{HitTestResult, HitTester};
pub use node::{NodeContent, DefaultNodeContent, WorkflowNode};
pub use port::{PortDirection, Port};
pub use state::{BoxSelection, CanvasState, Connection, Position, SelectionState, NodeId, ConnectionId, LinkType, InteractionMode, NodeDragState, ConnectionDrag, BulkConnectDrag, ContextMenuState, ViewportState, WorkflowGraph, WorkflowNodeData};
pub use theme::WorkflowTheme;
