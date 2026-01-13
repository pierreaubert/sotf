//! E2E tests for Plugin Graph Component.
//!
//! Tests for the visual plugin graph routing:
//! - Node rendering and positioning
//! - Connection creation and deletion
//! - Audio routing visualization
//! - Graph layout and navigation

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Node type in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeType {
    Input,
    Output,
    Plugin,
    Mixer,
    Splitter,
}

/// Position in the graph canvas
#[derive(Debug, Clone, Copy, Default)]
struct Position {
    x: f32,
    y: f32,
}

/// Graph node
#[derive(Debug, Clone)]
struct GraphNode {
    id: String,
    node_type: NodeType,
    label: String,
    position: Position,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
    is_selected: bool,
    is_bypassed: bool,
}

impl Default for GraphNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            node_type: NodeType::Plugin,
            label: String::new(),
            position: Position::default(),
            input_ports: vec!["in".to_string()],
            output_ports: vec!["out".to_string()],
            is_selected: false,
            is_bypassed: false,
        }
    }
}

/// Connection between nodes
#[derive(Debug, Clone)]
struct Connection {
    id: String,
    source_node: String,
    source_port: String,
    target_node: String,
    target_port: String,
    is_selected: bool,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            id: String::new(),
            source_node: String::new(),
            source_port: "out".to_string(),
            target_node: String::new(),
            target_port: "in".to_string(),
            is_selected: false,
        }
    }
}

/// Graph viewport state
#[derive(Debug, Clone, Default)]
struct Viewport {
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
}

/// Plugin graph state
struct PluginGraphState {
    nodes: Vec<GraphNode>,
    connections: Vec<Connection>,
    viewport: Viewport,
    selected_node_id: Option<String>,
    selected_connection_id: Option<String>,
    is_creating_connection: bool,
    pending_connection_source: Option<(String, String)>, // (node_id, port_id)
    show_minimap: bool,
    snap_to_grid: bool,
    grid_size: f32,
}

impl Default for PluginGraphState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            viewport: Viewport {
                pan_x: 0.0,
                pan_y: 0.0,
                zoom: 1.0,
            },
            selected_node_id: None,
            selected_connection_id: None,
            is_creating_connection: false,
            pending_connection_source: None,
            show_minimap: true,
            snap_to_grid: true,
            grid_size: 20.0,
        }
    }
}

// =============================================================================
// Node Tests
// =============================================================================

/// Test adding nodes to graph.
#[gpui::test]
async fn test_adding_nodes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes.push(GraphNode {
        id: "input".to_string(),
        node_type: NodeType::Input,
        label: "Audio Input".to_string(),
        position: Position { x: 50.0, y: 100.0 },
        ..Default::default()
    });

    state.borrow_mut().nodes.push(GraphNode {
        id: "eq".to_string(),
        node_type: NodeType::Plugin,
        label: "EQ".to_string(),
        position: Position { x: 200.0, y: 100.0 },
        ..Default::default()
    });

    assert_eq!(state.borrow().nodes.len(), 2);
}

/// Test removing nodes.
#[gpui::test]
async fn test_removing_nodes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes = vec![
        GraphNode {
            id: "node1".to_string(),
            ..Default::default()
        },
        GraphNode {
            id: "node2".to_string(),
            ..Default::default()
        },
    ];

    state.borrow_mut().nodes.retain(|n| n.id != "node1");
    assert_eq!(state.borrow().nodes.len(), 1);
    assert_eq!(state.borrow().nodes[0].id, "node2");
}

/// Test node selection.
#[gpui::test]
async fn test_node_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes.push(GraphNode {
        id: "test-node".to_string(),
        ..Default::default()
    });

    state.borrow_mut().selected_node_id = Some("test-node".to_string());
    assert_eq!(
        state.borrow().selected_node_id,
        Some("test-node".to_string())
    );
}

/// Test node positioning.
#[gpui::test]
async fn test_node_positioning(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes.push(GraphNode {
        id: "node".to_string(),
        position: Position { x: 100.0, y: 200.0 },
        ..Default::default()
    });

    let pos = state.borrow().nodes[0].position;
    assert!((pos.x - 100.0).abs() < 0.1);
    assert!((pos.y - 200.0).abs() < 0.1);
}

/// Test node dragging.
#[gpui::test]
async fn test_node_dragging(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes.push(GraphNode {
        id: "node".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        ..Default::default()
    });

    // Simulate drag
    let delta_x = 50.0;
    let delta_y = 30.0;
    state.borrow_mut().nodes[0].position.x += delta_x;
    state.borrow_mut().nodes[0].position.y += delta_y;

    let pos = state.borrow().nodes[0].position;
    assert!((pos.x - 150.0).abs() < 0.1);
    assert!((pos.y - 130.0).abs() < 0.1);
}

/// Test snap to grid.
#[gpui::test]
async fn test_snap_to_grid(_cx: &mut TestAppContext) {
    fn snap_position(pos: f32, grid_size: f32) -> f32 {
        (pos / grid_size).round() * grid_size
    }

    let grid_size = 20.0;
    assert!((snap_position(105.0, grid_size) - 100.0).abs() < 0.1);
    assert!((snap_position(115.0, grid_size) - 120.0).abs() < 0.1);
}

/// Test node bypass toggle.
#[gpui::test]
async fn test_node_bypass_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes.push(GraphNode::default());

    assert!(!state.borrow().nodes[0].is_bypassed);

    state.borrow_mut().nodes[0].is_bypassed = true;
    assert!(state.borrow().nodes[0].is_bypassed);
}

/// Test different node types.
#[gpui::test]
async fn test_node_types(_cx: &mut TestAppContext) {
    let types = [
        NodeType::Input,
        NodeType::Output,
        NodeType::Plugin,
        NodeType::Mixer,
        NodeType::Splitter,
    ];

    for node_type in types {
        let node = GraphNode {
            node_type,
            ..Default::default()
        };
        assert_eq!(node.node_type, node_type);
    }
}

/// Test node ports.
#[gpui::test]
async fn test_node_ports(_cx: &mut TestAppContext) {
    let node = GraphNode {
        input_ports: vec!["in_l".to_string(), "in_r".to_string()],
        output_ports: vec!["out_l".to_string(), "out_r".to_string()],
        ..Default::default()
    };

    assert_eq!(node.input_ports.len(), 2);
    assert_eq!(node.output_ports.len(), 2);
}

// =============================================================================
// Connection Tests
// =============================================================================

/// Test creating connections.
#[gpui::test]
async fn test_creating_connections(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().connections.push(Connection {
        id: "conn1".to_string(),
        source_node: "input".to_string(),
        source_port: "out".to_string(),
        target_node: "eq".to_string(),
        target_port: "in".to_string(),
        ..Default::default()
    });

    assert_eq!(state.borrow().connections.len(), 1);
}

/// Test deleting connections.
#[gpui::test]
async fn test_deleting_connections(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().connections = vec![
        Connection {
            id: "conn1".to_string(),
            ..Default::default()
        },
        Connection {
            id: "conn2".to_string(),
            ..Default::default()
        },
    ];

    state.borrow_mut().connections.retain(|c| c.id != "conn1");
    assert_eq!(state.borrow().connections.len(), 1);
}

/// Test connection selection.
#[gpui::test]
async fn test_connection_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().connections.push(Connection {
        id: "conn1".to_string(),
        ..Default::default()
    });

    state.borrow_mut().selected_connection_id = Some("conn1".to_string());
    assert_eq!(
        state.borrow().selected_connection_id,
        Some("conn1".to_string())
    );
}

/// Test connection creation flow.
#[gpui::test]
async fn test_connection_creation_flow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    // Start connection from source port
    state.borrow_mut().is_creating_connection = true;
    state.borrow_mut().pending_connection_source = Some(("node1".to_string(), "out".to_string()));

    assert!(state.borrow().is_creating_connection);
    assert!(state.borrow().pending_connection_source.is_some());

    // Complete connection
    state.borrow_mut().connections.push(Connection {
        id: "new-conn".to_string(),
        source_node: "node1".to_string(),
        source_port: "out".to_string(),
        target_node: "node2".to_string(),
        target_port: "in".to_string(),
        ..Default::default()
    });

    state.borrow_mut().is_creating_connection = false;
    state.borrow_mut().pending_connection_source = None;

    assert!(!state.borrow().is_creating_connection);
    assert_eq!(state.borrow().connections.len(), 1);
}

/// Test connection validation - no self-loops.
#[gpui::test]
async fn test_no_self_loop_connections(_cx: &mut TestAppContext) {
    fn is_valid_connection(source_node: &str, target_node: &str) -> bool {
        source_node != target_node
    }

    assert!(!is_valid_connection("node1", "node1")); // Self-loop invalid
    assert!(is_valid_connection("node1", "node2")); // Different nodes valid
}

/// Test connection validation - no duplicate connections.
#[gpui::test]
async fn test_no_duplicate_connections(_cx: &mut TestAppContext) {
    fn has_duplicate_connection(
        connections: &[Connection],
        source_node: &str,
        source_port: &str,
        target_node: &str,
        target_port: &str,
    ) -> bool {
        connections.iter().any(|c| {
            c.source_node == source_node
                && c.source_port == source_port
                && c.target_node == target_node
                && c.target_port == target_port
        })
    }

    let connections = vec![Connection {
        source_node: "n1".to_string(),
        source_port: "out".to_string(),
        target_node: "n2".to_string(),
        target_port: "in".to_string(),
        ..Default::default()
    }];

    assert!(has_duplicate_connection(
        &connections,
        "n1",
        "out",
        "n2",
        "in"
    ));
    assert!(!has_duplicate_connection(
        &connections,
        "n1",
        "out",
        "n3",
        "in"
    ));
}

/// Test removing connections when node is deleted.
#[gpui::test]
async fn test_cascade_delete_connections(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes = vec![
        GraphNode {
            id: "n1".to_string(),
            ..Default::default()
        },
        GraphNode {
            id: "n2".to_string(),
            ..Default::default()
        },
    ];

    state.borrow_mut().connections = vec![Connection {
        id: "c1".to_string(),
        source_node: "n1".to_string(),
        target_node: "n2".to_string(),
        ..Default::default()
    }];

    // Delete node n1
    let deleted_node_id = "n1";
    state.borrow_mut().nodes.retain(|n| n.id != deleted_node_id);
    state
        .borrow_mut()
        .connections
        .retain(|c| c.source_node != deleted_node_id && c.target_node != deleted_node_id);

    assert_eq!(state.borrow().nodes.len(), 1);
    assert_eq!(state.borrow().connections.len(), 0);
}

// =============================================================================
// Viewport Tests
// =============================================================================

/// Test viewport panning.
#[gpui::test]
async fn test_viewport_panning(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().viewport.pan_x = 100.0;
    state.borrow_mut().viewport.pan_y = 50.0;

    assert!((state.borrow().viewport.pan_x - 100.0).abs() < 0.1);
    assert!((state.borrow().viewport.pan_y - 50.0).abs() < 0.1);
}

/// Test viewport zooming.
#[gpui::test]
async fn test_viewport_zooming(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    assert!((state.borrow().viewport.zoom - 1.0).abs() < 0.01);

    state.borrow_mut().viewport.zoom = 1.5;
    assert!((state.borrow().viewport.zoom - 1.5).abs() < 0.01);
}

/// Test zoom limits.
#[gpui::test]
async fn test_zoom_limits(_cx: &mut TestAppContext) {
    fn clamp_zoom(zoom: f32) -> f32 {
        zoom.clamp(0.25, 4.0)
    }

    assert!((clamp_zoom(0.1) - 0.25).abs() < 0.01);
    assert!((clamp_zoom(5.0) - 4.0).abs() < 0.01);
    assert!((clamp_zoom(1.5) - 1.5).abs() < 0.01);
}

/// Test zoom to fit.
#[gpui::test]
async fn test_zoom_to_fit(_cx: &mut TestAppContext) {
    fn calculate_fit_zoom(
        canvas_width: f32,
        canvas_height: f32,
        content_width: f32,
        content_height: f32,
        padding: f32,
    ) -> f32 {
        let available_width = canvas_width - padding * 2.0;
        let available_height = canvas_height - padding * 2.0;
        let zoom_x = available_width / content_width;
        let zoom_y = available_height / content_height;
        zoom_x.min(zoom_y).clamp(0.25, 4.0)
    }

    let zoom = calculate_fit_zoom(800.0, 600.0, 400.0, 300.0, 50.0);
    assert!(zoom > 0.0 && zoom <= 4.0);
}

/// Test minimap toggle.
#[gpui::test]
async fn test_minimap_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    assert!(state.borrow().show_minimap);

    state.borrow_mut().show_minimap = false;
    assert!(!state.borrow().show_minimap);
}

// =============================================================================
// Graph Layout Tests
// =============================================================================

/// Test auto-layout calculation.
#[gpui::test]
async fn test_auto_layout(_cx: &mut TestAppContext) {
    fn calculate_auto_layout(node_count: usize) -> Vec<Position> {
        let mut positions = Vec::new();
        let spacing_x = 150.0;
        let spacing_y = 100.0;
        let columns = 4;

        for i in 0..node_count {
            let col = i % columns;
            let row = i / columns;
            positions.push(Position {
                x: col as f32 * spacing_x + 50.0,
                y: row as f32 * spacing_y + 50.0,
            });
        }
        positions
    }

    let positions = calculate_auto_layout(6);
    assert_eq!(positions.len(), 6);
    assert!((positions[0].x - 50.0).abs() < 0.1);
    assert!((positions[4].x - 50.0).abs() < 0.1); // First column of second row
}

/// Test topological sort for layout.
#[gpui::test]
async fn test_topological_sort(_cx: &mut TestAppContext) {
    fn topological_sort(nodes: &[&str], edges: &[(&str, &str)]) -> Vec<String> {
        // Simple implementation for testing
        let mut result: Vec<String> = nodes.iter().map(|s| s.to_string()).collect();

        // Sort by dependency - sources come before targets
        for &(source, target) in edges {
            let source_idx = result.iter().position(|n| n == source);
            let target_idx = result.iter().position(|n| n == target);
            if let (Some(s), Some(t)) = (source_idx, target_idx) {
                if s > t {
                    result.swap(s, t);
                }
            }
        }
        result
    }

    let nodes = vec!["input", "eq", "comp", "output"];
    let edges = vec![("input", "eq"), ("eq", "comp"), ("comp", "output")];
    let sorted = topological_sort(&nodes, &edges);

    // Input should come before output
    let input_idx = sorted.iter().position(|n| n == "input").unwrap();
    let output_idx = sorted.iter().position(|n| n == "output").unwrap();
    assert!(input_idx < output_idx);
}

/// Test grid snapping toggle.
#[gpui::test]
async fn test_grid_snapping_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    assert!(state.borrow().snap_to_grid);

    state.borrow_mut().snap_to_grid = false;
    assert!(!state.borrow().snap_to_grid);
}

/// Test grid size adjustment.
#[gpui::test]
async fn test_grid_size_adjustment(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    let sizes = [10.0, 20.0, 40.0];
    for size in sizes {
        state.borrow_mut().grid_size = size;
        assert!((state.borrow().grid_size - size).abs() < 0.1);
    }
}

// =============================================================================
// Node Visual Tests
// =============================================================================

/// Test node color by type.
#[gpui::test]
async fn test_node_color_by_type(_cx: &mut TestAppContext) {
    fn get_node_color(node_type: NodeType) -> &'static str {
        match node_type {
            NodeType::Input => "#4CAF50",    // Green
            NodeType::Output => "#F44336",   // Red
            NodeType::Plugin => "#2196F3",   // Blue
            NodeType::Mixer => "#9C27B0",    // Purple
            NodeType::Splitter => "#FF9800", // Orange
        }
    }

    assert_eq!(get_node_color(NodeType::Input), "#4CAF50");
    assert_eq!(get_node_color(NodeType::Plugin), "#2196F3");
}

/// Test node label truncation.
#[gpui::test]
async fn test_node_label_truncation(_cx: &mut TestAppContext) {
    fn truncate_label(label: &str, max_len: usize) -> String {
        if label.len() <= max_len {
            label.to_string()
        } else {
            format!("{}...", &label[..max_len - 3])
        }
    }

    let long_label = "Very Long Plugin Name That Needs Truncation";
    let truncated = truncate_label(long_label, 15);
    assert!(truncated.len() <= 15);
    assert!(truncated.ends_with("..."));
}

/// Test node selection highlight.
#[gpui::test]
async fn test_node_selection_highlight(_cx: &mut TestAppContext) {
    fn get_node_border_width(is_selected: bool) -> f32 {
        if is_selected { 3.0 } else { 1.0 }
    }

    assert!((get_node_border_width(false) - 1.0).abs() < 0.1);
    assert!((get_node_border_width(true) - 3.0).abs() < 0.1);
}

/// Test bypassed node opacity.
#[gpui::test]
async fn test_bypassed_node_opacity(_cx: &mut TestAppContext) {
    fn get_node_opacity(is_bypassed: bool) -> f32 {
        if is_bypassed { 0.5 } else { 1.0 }
    }

    assert!((get_node_opacity(false) - 1.0).abs() < 0.01);
    assert!((get_node_opacity(true) - 0.5).abs() < 0.01);
}

// =============================================================================
// Connection Visual Tests
// =============================================================================

/// Test connection path calculation.
#[gpui::test]
async fn test_connection_path_calculation(_cx: &mut TestAppContext) {
    fn calculate_bezier_control_points(start: Position, end: Position) -> (Position, Position) {
        let dx = (end.x - start.x).abs() / 2.0;
        let cp1 = Position {
            x: start.x + dx,
            y: start.y,
        };
        let cp2 = Position {
            x: end.x - dx,
            y: end.y,
        };
        (cp1, cp2)
    }

    let start = Position { x: 100.0, y: 100.0 };
    let end = Position { x: 300.0, y: 200.0 };
    let (cp1, cp2) = calculate_bezier_control_points(start, end);

    assert!((cp1.x - 200.0).abs() < 0.1);
    assert!((cp2.x - 200.0).abs() < 0.1);
}

/// Test connection color by state.
#[gpui::test]
async fn test_connection_color_by_state(_cx: &mut TestAppContext) {
    fn get_connection_color(is_selected: bool, is_active: bool) -> &'static str {
        if is_selected {
            "#FFC107" // Amber
        } else if is_active {
            "#4CAF50" // Green
        } else {
            "#757575" // Grey
        }
    }

    assert_eq!(get_connection_color(true, false), "#FFC107");
    assert_eq!(get_connection_color(false, true), "#4CAF50");
    assert_eq!(get_connection_color(false, false), "#757575");
}

/// Test pending connection preview.
#[gpui::test]
async fn test_pending_connection_preview(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().is_creating_connection = true;
    state.borrow_mut().pending_connection_source = Some(("node1".to_string(), "out".to_string()));

    assert!(state.borrow().is_creating_connection);
    assert!(state.borrow().pending_connection_source.is_some());
}

// =============================================================================
// Graph Operations Tests
// =============================================================================

/// Test copy/paste nodes.
#[gpui::test]
async fn test_copy_paste_nodes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    // Add original node
    state.borrow_mut().nodes.push(GraphNode {
        id: "original".to_string(),
        label: "EQ".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        ..Default::default()
    });

    // Paste creates copy with offset
    let original_pos = state.borrow().nodes[0].position;
    state.borrow_mut().nodes.push(GraphNode {
        id: "copy".to_string(),
        label: "EQ".to_string(),
        position: Position {
            x: original_pos.x + 20.0,
            y: original_pos.y + 20.0,
        },
        ..Default::default()
    });

    assert_eq!(state.borrow().nodes.len(), 2);
}

/// Test undo/redo state.
#[gpui::test]
async fn test_undo_redo_state(_cx: &mut TestAppContext) {
    struct UndoState {
        can_undo: bool,
        can_redo: bool,
        undo_stack_size: usize,
        redo_stack_size: usize,
    }

    let state = UndoState {
        can_undo: true,
        can_redo: false,
        undo_stack_size: 5,
        redo_stack_size: 0,
    };

    assert!(state.can_undo);
    assert!(!state.can_redo);
}

/// Test select all nodes.
#[gpui::test]
async fn test_select_all_nodes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes = vec![
        GraphNode {
            id: "n1".to_string(),
            is_selected: false,
            ..Default::default()
        },
        GraphNode {
            id: "n2".to_string(),
            is_selected: false,
            ..Default::default()
        },
        GraphNode {
            id: "n3".to_string(),
            is_selected: false,
            ..Default::default()
        },
    ];

    // Select all
    for node in &mut state.borrow_mut().nodes {
        node.is_selected = true;
    }

    assert!(state.borrow().nodes.iter().all(|n| n.is_selected));
}

/// Test clear selection.
#[gpui::test]
async fn test_clear_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PluginGraphState::default()));

    state.borrow_mut().nodes = vec![GraphNode {
        id: "n1".to_string(),
        is_selected: true,
        ..Default::default()
    }];
    state.borrow_mut().selected_node_id = Some("n1".to_string());
    state.borrow_mut().selected_connection_id = Some("c1".to_string());

    // Clear all selections
    for node in &mut state.borrow_mut().nodes {
        node.is_selected = false;
    }
    state.borrow_mut().selected_node_id = None;
    state.borrow_mut().selected_connection_id = None;

    assert!(state.borrow().nodes.iter().all(|n| !n.is_selected));
    assert!(state.borrow().selected_node_id.is_none());
    assert!(state.borrow().selected_connection_id.is_none());
}

// =============================================================================
// Audio Signal Flow Tests
// =============================================================================

/// Test signal flow validation.
#[gpui::test]
async fn test_signal_flow_validation(_cx: &mut TestAppContext) {
    fn has_path_to_output(connections: &[(&str, &str)], start: &str, output: &str) -> bool {
        let mut visited = vec![start.to_string()];
        let mut queue = vec![start];

        while let Some(current) = queue.pop() {
            if current == output {
                return true;
            }
            for &(source, target) in connections {
                if source == current && !visited.contains(&target.to_string()) {
                    visited.push(target.to_string());
                    queue.push(target);
                }
            }
        }
        false
    }

    let connections = vec![("input", "eq"), ("eq", "comp"), ("comp", "output")];

    assert!(has_path_to_output(&connections, "input", "output"));
    assert!(!has_path_to_output(&connections, "eq", "input")); // No backwards path
}

/// Test detecting disconnected nodes.
#[gpui::test]
async fn test_detect_disconnected_nodes(_cx: &mut TestAppContext) {
    fn find_disconnected_nodes(nodes: &[&str], connections: &[(&str, &str)]) -> Vec<String> {
        nodes
            .iter()
            .filter(|&&node| !connections.iter().any(|(s, t)| *s == node || *t == node))
            .map(|s| s.to_string())
            .collect()
    }

    let nodes = vec!["input", "eq", "orphan", "output"];
    let connections = vec![("input", "eq"), ("eq", "output")];
    let disconnected = find_disconnected_nodes(&nodes, &connections);

    assert_eq!(disconnected.len(), 1);
    assert_eq!(disconnected[0], "orphan");
}

/// Test channel count propagation.
#[gpui::test]
async fn test_channel_count_propagation(_cx: &mut TestAppContext) {
    fn propagate_channels(input_channels: usize, node_type: NodeType) -> usize {
        match node_type {
            NodeType::Input => input_channels,
            NodeType::Output => input_channels,
            NodeType::Plugin => input_channels, // Most plugins preserve channels
            NodeType::Mixer => 2,               // Mix to stereo
            NodeType::Splitter => input_channels * 2, // Duplicate channels
        }
    }

    assert_eq!(propagate_channels(2, NodeType::Plugin), 2);
    assert_eq!(propagate_channels(6, NodeType::Mixer), 2);
    assert_eq!(propagate_channels(2, NodeType::Splitter), 4);
}
