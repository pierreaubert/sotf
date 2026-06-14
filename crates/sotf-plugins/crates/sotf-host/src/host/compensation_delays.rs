use super::audio_sample::AudioSample;
use super::delay_buffer::DelayBuffer;
use super::graph_edge::GraphEdge;
use super::types::NodeId;

pub(super) struct CompensationDelays<T: AudioSample> {
    #[allow(dead_code)]
    pub(super) edge_keys: Vec<(NodeId, NodeId)>,
    pub(super) delays: Vec<Option<DelayBuffer<T>>>,
}

impl<T: AudioSample> CompensationDelays<T> {
    pub(super) fn new(edges: &[GraphEdge]) -> Self {
        Self {
            edge_keys: edges.iter().map(|e| (e.from_node, e.to_node)).collect(),
            delays: (0..edges.len()).map(|_| None).collect(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn empty() -> Self {
        Self {
            edge_keys: Vec::new(),
            delays: Vec::new(),
        }
    }

    pub(super) fn set(&mut self, edge_id: usize, delay: DelayBuffer<T>) -> Result<(), String> {
        if edge_id < self.delays.len() {
            self.delays[edge_id] = Some(delay);
            Ok(())
        } else {
            Err(format!(
                "edge_id {} out of bounds ({} delays)",
                edge_id,
                self.delays.len()
            ))
        }
    }

    pub(super) fn get_mut_edge(&mut self, edge_id: usize) -> Option<&mut DelayBuffer<T>> {
        self.delays.get_mut(edge_id).and_then(Option::as_mut)
    }

    #[cfg(test)]
    pub(super) fn contains_key(&self, key: &(NodeId, NodeId)) -> bool {
        self.edge_keys
            .iter()
            .position(|candidate| candidate == key)
            .is_some_and(|idx| self.delays.get(idx).is_some_and(Option::is_some))
    }

    #[cfg(test)]
    pub(super) fn get(&self, key: &(NodeId, NodeId)) -> Option<&DelayBuffer<T>> {
        let idx = self
            .edge_keys
            .iter()
            .position(|candidate| candidate == key)?;
        self.delays.get(idx)?.as_ref()
    }

    #[allow(dead_code)]
    pub(super) fn is_empty(&self) -> bool {
        self.delays.iter().all(Option::is_none)
    }
}
