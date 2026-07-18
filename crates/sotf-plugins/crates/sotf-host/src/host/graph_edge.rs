use super::types::EdgeType;
use super::types::NodeId;

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub channel_map: Option<Vec<usize>>,
    /// First destination channel written by this edge.
    ///
    /// Source channels selected by `channel_map` are packed consecutively
    /// starting here. A zero offset preserves the historical routing behavior.
    pub destination_offset: usize,
    pub edge_type: EdgeType,
    pub(super) id: usize,
}

impl GraphEdge {
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            destination_offset: 0,
            edge_type: EdgeType::Audio,
            id: usize::MAX,
        }
    }
    pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(channels),
            destination_offset: 0,
            edge_type: EdgeType::Audio,
            id: usize::MAX,
        }
    }
    pub fn with_channel_route(
        from: NodeId,
        to: NodeId,
        source_channels: Vec<usize>,
        destination_offset: usize,
    ) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(source_channels),
            destination_offset,
            edge_type: EdgeType::Audio,
            id: usize::MAX,
        }
    }
    pub fn sidechain(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            destination_offset: 0,
            edge_type: EdgeType::Sidechain,
            id: usize::MAX,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}
