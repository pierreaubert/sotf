use super::node_position::NodePosition;
use super::types::GraphNodeId;
use super::types::SpecialNodeType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A special I/O node (Input, Output, Split, Merge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialNode {
    pub id: GraphNodeId,
    pub node_type: SpecialNodeType,
    pub position: NodePosition,
    pub channels: usize,
    /// Optional label (e.g., device name for Input/Output nodes)
    #[serde(default)]
    pub label: Option<String>,
}

impl SpecialNode {
    pub fn new(node_type: SpecialNodeType, position: NodePosition, channels: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            position,
            channels,
            label: None,
        }
    }

    /// Create a special node with a label (e.g., device name)
    pub fn with_label(
        node_type: SpecialNodeType,
        position: NodePosition,
        channels: usize,
        label: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            position,
            channels,
            label: Some(label),
        }
    }

    /// Get display name for this node
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else {
            match self.node_type {
                SpecialNodeType::Input => "Audio Input".to_string(),
                SpecialNodeType::Output => "Audio Output".to_string(),
                SpecialNodeType::Split => "Split".to_string(),
                SpecialNodeType::Merge => "Merge".to_string(),
            }
        }
    }

    /// Input port count for this node type
    pub fn input_channels(&self) -> usize {
        match self.node_type {
            SpecialNodeType::Input => 0, // No inputs (source)
            SpecialNodeType::Output => self.channels,
            SpecialNodeType::Split => 1,
            SpecialNodeType::Merge => self.channels,
        }
    }

    /// Output port count for this node type
    pub fn output_channels(&self) -> usize {
        match self.node_type {
            SpecialNodeType::Input => self.channels,
            SpecialNodeType::Output => 0, // No outputs (sink)
            SpecialNodeType::Split => self.channels,
            SpecialNodeType::Merge => 1,
        }
    }
}
