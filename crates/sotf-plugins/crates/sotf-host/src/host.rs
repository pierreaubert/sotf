// ============================================================================
// Plugin Host Trait - Common interface for plugin hosts
// ============================================================================

use crate::plugin::{Plugin, ProcessContext};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

// ============================================================================
// Node Buffer - Simple non-thread-safe buffer for audio data (zero-allocation)
// ============================================================================

struct NodeBuffer {
    data: Vec<f32>,
    actual_len: usize,
    num_channels: usize,
}

impl NodeBuffer {
    fn new(num_frames: usize, num_channels: usize) -> Self {
        Self {
            data: vec![0.0; num_frames * num_channels],
            actual_len: 0,
            num_channels,
        }
    }
    fn write(&mut self, data: &[f32]) {
        if self.data.len() < data.len() {
            self.data.resize(data.len(), 0.0);
        }
        self.data[..data.len()].copy_from_slice(data);
        self.actual_len = data.len();
    }
    fn read(&self) -> &[f32] {
        if self.actual_len == 0 {
            &self.data
        } else {
            &self.data[..self.actual_len]
        }
    }
    fn clear(&mut self) {
        self.actual_len = 0;
    }
    fn ensure_capacity(&mut self, num_frames: usize) {
        let required = num_frames * self.num_channels;
        if self.data.len() < required {
            self.data.resize(required, 0.0);
        }
    }
}

struct ProcessBuffers {
    node_buffers: Vec<Option<NodeBuffer>>,
    scratch_input: Vec<f32>,
    scratch_output: Vec<f32>,
    merge_buffer: Vec<f32>,
    channel_map_buffer: Vec<f32>,
}

pub trait Host {
    fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String>;
    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String>;
    fn plugin_count(&self) -> usize;
    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin>;
    fn input_channels(&self) -> usize;
    fn output_channels(&self) -> usize;
    fn get_plugin_data(&self, _index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
    fn set_plugin_parameter(
        &mut self,
        index: usize,
        param_id: &str,
        value: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let _ = (index, param_id, value);
        Err("set_plugin_parameter not implemented for this host".to_string())
    }
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String>;
    fn reset(&mut self);
    fn total_latency_samples(&self) -> usize;
}

pub type NodeId = usize;

pub struct GraphNode {
    pub id: NodeId,
    pub name: String,
    input_channels: usize,
    output_channels: usize,
}

impl GraphNode {
    pub fn new(id: NodeId, name: String, input_channels: usize, output_channels: usize) -> Self {
        Self {
            id,
            name,
            input_channels,
            output_channels,
        }
    }
    pub fn input_channels(&self) -> usize {
        self.input_channels
    }
    pub fn output_channels(&self) -> usize {
        self.output_channels
    }
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub channel_map: Option<Vec<usize>>,
}

impl GraphEdge {
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
        }
    }
    pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(channels),
        }
    }
}

#[derive(Debug, Clone)]
struct ProcessingStage {
    nodes: Vec<NodeId>,
}

impl ProcessingStage {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }
    fn add_node(&mut self, node_id: NodeId) {
        self.nodes.push(node_id);
    }
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

pub struct DawHost {
    nodes: HashMap<NodeId, GraphNode>,
    /// Plugin storage indexed by NodeId — disjoint from `nodes` for borrow checker.
    /// `process()` can borrow `&self.nodes` (topology) and `&mut self.plugins[nid]` (plugin)
    /// without conflict.
    plugins: Vec<Option<Box<dyn Plugin>>>,
    edges: Vec<GraphEdge>,
    stages: Vec<ProcessingStage>,
    input_nodes: Vec<NodeId>,
    output_nodes: Vec<NodeId>,
    sample_rate: u32,
    next_node_id: NodeId,
    parallel_enabled: bool,
    chain_nodes: Vec<NodeId>,
    initial_input_channels: usize,
    built: bool,
    process_buffers: Option<ProcessBuffers>,
    predecessors: Vec<Vec<GraphEdge>>,
    is_input_node: Vec<bool>,
    is_output_node: Vec<bool>,
    has_variable_frame_plugin: bool,
    /// True if all plugins return input_frames unchanged from output_frames_for_input()
    cached_frames_identity: bool,
    /// True if all plugins return input_rate unchanged from output_sample_rate()
    cached_rate_identity: bool,
    /// Cached per-node output frame ratios for non-identity chains (rare)
    /// Only populated when cached_frames_identity is false
    cached_output_frame_ratios: Vec<(NodeId, f64)>,
    /// Indices of plugins in chain_nodes that have analyzer data (get_data() returns Some)
    analyzer_indices: Vec<usize>,
}

impl DawHost {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            nodes: HashMap::new(),
            plugins: Vec::new(),
            edges: Vec::new(),
            stages: Vec::new(),
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            sample_rate,
            next_node_id: 0,
            parallel_enabled: true,
            chain_nodes: Vec::new(),
            initial_input_channels: channels,
            built: false,
            process_buffers: None,
            predecessors: Vec::new(),
            is_input_node: Vec::new(),
            is_output_node: Vec::new(),
            has_variable_frame_plugin: false,
            cached_frames_identity: true,
            cached_rate_identity: true,
            cached_output_frame_ratios: Vec::new(),
            analyzer_indices: Vec::new(),
        }
    }
    pub fn new_default(sr: u32) -> Self {
        Self::new(2, sr)
    }
    pub fn set_parallel_enabled(&mut self, e: bool) {
        self.parallel_enabled = e;
    }

    pub fn add_node(
        &mut self,
        name: String,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let id = self.next_node_id;
        self.next_node_id += 1;
        plugin.initialize(self.sample_rate)?;
        let input_channels = plugin.input_channels();
        let output_channels = plugin.output_channels();
        self.nodes.insert(
            id,
            GraphNode::new(id, name, input_channels, output_channels),
        );
        // Grow plugins vec to accommodate the new id
        if id >= self.plugins.len() {
            self.plugins.resize_with(id + 1, || None);
        }
        self.plugins[id] = Some(plugin);
        Ok(id)
    }

    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<(), String> {
        if !self.nodes.contains_key(&edge.from_node) || !self.nodes.contains_key(&edge.to_node) {
            return Err("Node not found".into());
        }
        if edge.from_node == edge.to_node {
            return Err("Self-loop".into());
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn build(&mut self) -> Result<(), String> {
        if self.has_cycle() {
            return Err("Cycle".into());
        }
        self.compute_io_nodes();
        self.compute_stages()?;
        let max_id = self.nodes.keys().copied().max().unwrap_or(0);
        let num_slots = if self.nodes.is_empty() { 0 } else { max_id + 1 };
        self.predecessors = vec![Vec::new(); num_slots];
        self.is_input_node = vec![false; num_slots];
        self.is_output_node = vec![false; num_slots];
        for edge in &self.edges {
            self.predecessors[edge.to_node].push(edge.clone());
        }
        for &id in &self.input_nodes {
            self.is_input_node[id] = true;
        }
        for &id in &self.output_nodes {
            self.is_output_node[id] = true;
        }
        let mut node_buffers = (0..num_slots).map(|_| None).collect::<Vec<_>>();
        for (&id, node) in &self.nodes {
            node_buffers[id] = Some(NodeBuffer::new(4096, node.output_channels()));
        }
        self.process_buffers = Some(ProcessBuffers {
            node_buffers,
            scratch_input: vec![0.0; 4096 * 32],
            scratch_output: vec![0.0; 4096 * 32],
            merge_buffer: vec![0.0; 4096 * 32],
            channel_map_buffer: vec![0.0; 4096 * 32],
        });
        // Cache per-frame properties to avoid mutex locks during process()
        self.cached_frames_identity = true;
        self.cached_rate_identity = true;
        self.cached_output_frame_ratios.clear();
        self.analyzer_indices.clear();

        for (chain_idx, &id) in self.chain_nodes.iter().enumerate() {
            let p = self.plugins[id].as_ref().unwrap();
            if p.output_frames_for_input(100) != 100 {
                self.cached_frames_identity = false;
            }
            if p.output_sample_rate(48000) != 48000 {
                self.cached_rate_identity = false;
            }
            if p.get_data().is_some() {
                self.analyzer_indices.push(chain_idx);
            }
        }

        self.has_variable_frame_plugin = self.chain_nodes.iter().any(|&id| {
            let p = self.plugins[id].as_ref().unwrap();
            p.output_frames_for_input(100) != 100 || p.latency_samples() > 0
        });
        self.built = true;
        Ok(())
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let expected = if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[self.chain_nodes.last().unwrap()].output_channels()
        };
        if plugin.input_channels() != expected {
            return Err("mismatch".into());
        }
        let name = format!("plugin_{}", self.next_node_id);
        let id = self.add_node(name, plugin)?;
        if let Some(&prev) = self.chain_nodes.last() {
            self.add_edge(GraphEdge::new(prev, id))?;
        }
        self.chain_nodes.push(id);
        self.built = false;
        Ok(())
    }

    pub fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        if index >= self.chain_nodes.len() {
            return Err("oob".into());
        }
        let id = self.chain_nodes.remove(index);
        self.edges.retain(|e| e.from_node != id && e.to_node != id);
        if index > 0 && index < self.chain_nodes.len() {
            self.add_edge(GraphEdge::new(
                self.chain_nodes[index - 1],
                self.chain_nodes[index],
            ))?;
        }
        self.nodes.remove(&id).unwrap();
        self.built = false;
        Ok(self.plugins[id].take().unwrap())
    }

    pub fn plugin_count(&self) -> usize {
        self.chain_nodes.len()
    }
    pub fn get_plugin(&self, index: usize) -> Option<&dyn Plugin> {
        let &nid = self.chain_nodes.get(index)?;
        self.plugins.get(nid)?.as_deref()
    }
    pub fn input_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[&self.chain_nodes[0]].input_channels()
        }
    }
    pub fn output_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[self.chain_nodes.last().unwrap()].output_channels()
        }
    }
    pub fn output_frames_for_input(&self, f: usize) -> usize {
        if self.cached_frames_identity {
            return f;
        }
        let mut result = f;
        for &id in &self.chain_nodes {
            result = self.plugins[id]
                .as_ref()
                .unwrap()
                .output_frames_for_input(result);
        }
        result
    }
    pub fn output_sample_rate(&self, r: u32) -> u32 {
        if self.cached_rate_identity {
            return r;
        }
        let mut result = r;
        for &id in &self.chain_nodes {
            result = self.plugins[id]
                .as_ref()
                .unwrap()
                .output_sample_rate(result);
        }
        result
    }
    pub fn last_output_frames(&self) -> Option<usize> {
        for &id in self.chain_nodes.iter().rev() {
            if let Some(f) = self.plugins[id].as_ref().unwrap().last_output_frames() {
                return Some(f);
            }
        }
        None
    }
    pub fn set_plugin_parameter(
        &mut self,
        index: usize,
        id: &str,
        val: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.plugins[nid]
            .as_mut()
            .unwrap()
            .set_parameter(super::parameters::ParameterId(id.to_string()), val)
    }

    /// Returns indices of plugins that have analyzer data (get_data() returns Some).
    /// Computed during build() to avoid per-frame discovery.
    pub fn analyzer_indices(&self) -> &[usize] {
        &self.analyzer_indices
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        if !self.built {
            self.build()?;
        }
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        let nf = input.len() / self.input_channels();
        let max_of = self.output_frames_for_input(nf);
        let mut bufs = self.process_buffers.take().unwrap();
        for nb in bufs.node_buffers.iter_mut().flatten() {
            nb.ensure_capacity(nf.max(max_of));
            nb.clear();
        }
        let mut cf = nf;
        for stage in &self.stages {
            let mut stage_cf: Option<usize> = None;
            for &nid in &stage.nodes {
                let node = &self.nodes[&nid];
                let in_len = if self.is_input_node[nid] {
                    bufs.scratch_input[..input.len()].copy_from_slice(input);
                    input.len()
                } else {
                    let il = Self::merge_inputs_into(
                        node,
                        &self.predecessors,
                        &bufs.node_buffers,
                        cf,
                        &mut bufs.merge_buffer,
                        &mut bufs.channel_map_buffer,
                    )?;
                    bufs.scratch_input[..il].copy_from_slice(&bufs.merge_buffer[..il]);
                    il
                };
                let p = self.plugins[nid].as_mut().unwrap();
                let context = ProcessContext {
                    sample_rate: self.sample_rate,
                    num_frames: cf,
                };
                let mof = p.output_frames_for_input(cf);
                let ol = mof * node.output_channels();
                if bufs.scratch_output.len() < ol {
                    bufs.scratch_output.resize(ol, 0.0);
                }
                let aof = p.process(
                    &bufs.scratch_input[..in_len],
                    &mut bufs.scratch_output[..ol],
                    &context,
                )?;
                bufs.node_buffers[nid]
                    .as_mut()
                    .unwrap()
                    .write(&bufs.scratch_output[..aof * node.output_channels()]);
                stage_cf = Some(match stage_cf {
                    Some(prev) => prev.min(aof),
                    None => aof,
                });
            }
            if let Some(scf) = stage_cf {
                cf = scf;
            }
        }
        Self::collect_output_from_buffers(&self.output_nodes, &bufs.node_buffers, output, cf)?;
        if cf < nf && self.has_variable_frame_plugin {
            output[cf * self.output_channels()..].fill(0.0);
            cf = nf;
        }
        self.process_buffers = Some(bufs);
        Ok(cf)
    }

    fn merge_inputs_into(
        n: &GraphNode,
        preds: &[Vec<GraphEdge>],
        nbs: &[Option<NodeBuffer>],
        nf: usize,
        mb: &mut [f32],
        cmb: &mut [f32],
    ) -> Result<usize, String> {
        let is = nf * n.input_channels();
        if mb.len() < is {
            return Err(format!("Merge buffer too small: {} < {}", mb.len(), is));
        }
        mb[..is].fill(0.0);
        for e in &preds[n.id] {
            let sb = nbs[e.from_node].as_ref().unwrap();
            let sd = sb.read();
            if let Some(ref cm) = e.channel_map {
                let ms = nf * cm.len();
                if cmb.len() < ms {
                    return Err(format!(
                        "Channel map buffer too small: {} < {}",
                        cmb.len(),
                        ms
                    ));
                }
                for f in 0..nf {
                    for (di, &si) in cm.iter().enumerate() {
                        cmb[f * cm.len() + di] = sd[f * sb.num_channels + si];
                    }
                }
                for (d, &s) in mb[..is].iter_mut().zip(cmb[..ms].iter()) {
                    *d += s;
                }
            } else {
                for (d, &s) in mb[..is].iter_mut().zip(sd[..is.min(sd.len())].iter()) {
                    *d += s;
                }
            }
        }
        Ok(is)
    }

    fn collect_output_from_buffers(
        ons: &[NodeId],
        nbs: &[Option<NodeBuffer>],
        out: &mut [f32],
        _nf: usize,
    ) -> Result<(), String> {
        if ons.len() == 1 {
            let d = nbs[ons[0]].as_ref().unwrap().read();
            let l = d.len().min(out.len());
            out[..l].copy_from_slice(&d[..l]);
        } else {
            out.fill(0.0);
            for &id in ons {
                let d = nbs[id].as_ref().unwrap().read();
                let l = d.len().min(out.len());
                for (dst, &s) in out[..l].iter_mut().zip(d[..l].iter()) {
                    *dst += s;
                }
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        for &id in self.nodes.keys() {
            if let Some(p) = self.plugins[id].as_mut() {
                p.reset();
            }
        }
    }
    pub fn total_latency_samples(&self) -> usize {
        self.output_nodes
            .iter()
            .map(|&id| self.path_latency(id))
            .max()
            .unwrap_or(0)
    }
    fn path_latency(&self, id: NodeId) -> usize {
        let l = self.plugins[id].as_ref().unwrap().latency_samples();
        let inc: Vec<NodeId> = self
            .edges
            .iter()
            .filter(|e| e.to_node == id)
            .map(|e| e.from_node)
            .collect();
        if inc.is_empty() {
            l
        } else {
            l + inc
                .iter()
                .map(|&pid| self.path_latency(pid))
                .max()
                .unwrap_or(0)
        }
    }
    fn has_cycle(&self) -> bool {
        let mut v = HashSet::new();
        let mut r = HashSet::new();
        for &id in self.nodes.keys() {
            if self.cycle_util(id, &mut v, &mut r) {
                return true;
            }
        }
        false
    }
    fn cycle_util(&self, id: NodeId, v: &mut HashSet<NodeId>, r: &mut HashSet<NodeId>) -> bool {
        if r.contains(&id) {
            return true;
        }
        if v.contains(&id) {
            return false;
        }
        v.insert(id);
        r.insert(id);
        for e in &self.edges {
            if e.from_node == id && self.cycle_util(e.to_node, v, r) {
                return true;
            }
        }
        r.remove(&id);
        false
    }
    fn compute_io_nodes(&mut self) {
        let mut hi = HashSet::new();
        let mut ho = HashSet::new();
        for e in &self.edges {
            hi.insert(e.to_node);
            ho.insert(e.from_node);
        }
        self.input_nodes = self
            .nodes
            .keys()
            .filter(|id| !hi.contains(id))
            .copied()
            .collect();
        self.output_nodes = self
            .nodes
            .keys()
            .filter(|id| !ho.contains(id))
            .copied()
            .collect();
    }
    fn compute_stages(&mut self) -> Result<(), String> {
        let mut deg: HashMap<NodeId, usize> = self.nodes.keys().map(|&id| (id, 0)).collect();
        for e in &self.edges {
            *deg.get_mut(&e.to_node).unwrap() += 1;
        }
        let mut q: VecDeque<NodeId> = deg
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        self.stages.clear();
        let mut count = 0;
        while !q.is_empty() {
            let mut s = ProcessingStage::new();
            for _ in 0..q.len() {
                let id = q.pop_front().unwrap();
                s.add_node(id);
                count += 1;
                for e in &self.edges {
                    if e.from_node == id {
                        let d = deg.get_mut(&e.to_node).unwrap();
                        *d -= 1;
                        if *d == 0 {
                            q.push_back(e.to_node);
                        }
                    }
                }
            }
            if !s.is_empty() {
                self.stages.push(s);
            }
        }
        if count != self.nodes.len() {
            Err("Cycle".into())
        } else {
            Ok(())
        }
    }
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }
    pub fn stage_info(&self, idx: usize) -> Option<Vec<String>> {
        self.stages.get(idx).map(|s| {
            s.nodes
                .iter()
                .filter_map(|id| self.nodes.get(id))
                .map(|n| n.name.clone())
                .collect()
        })
    }
}

impl Host for DawHost {
    fn add_plugin(&mut self, p: Box<dyn Plugin>) -> Result<(), String> {
        self.add_plugin(p)
    }
    fn remove_plugin(&mut self, i: usize) -> Result<Box<dyn Plugin>, String> {
        self.remove_plugin(i)
    }
    fn plugin_count(&self) -> usize {
        self.plugin_count()
    }
    fn get_plugin(&self, i: usize) -> Option<&dyn Plugin> {
        self.get_plugin(i)
    }
    fn input_channels(&self) -> usize {
        self.input_channels()
    }
    fn output_channels(&self) -> usize {
        self.output_channels()
    }
    fn process(&mut self, i: &[f32], o: &mut [f32]) -> Result<usize, String> {
        self.process(i, o)
    }
    fn reset(&mut self) {
        self.reset()
    }
    fn total_latency_samples(&self) -> usize {
        self.total_latency_samples()
    }
    fn get_plugin_data(&self, i: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        let &node_id = self.chain_nodes.get(i)?;
        self.plugins.get(node_id)?.as_ref()?.get_data()
    }
}

// Note: Host integration tests that use plugin types (GainPlugin, UpmixerPlugin, etc.)
// live in the `sotf-plugins` facade crate's tests/ directory since those plugin types
// are not available in `sotf-host`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Plugin;

    #[test]
    fn test_pluginhost_api_empty_graph() {
        let mut g = DawHost::new(2, 48000);
        let i = vec![1.0; 96];
        let mut o = vec![0.0; 96];
        assert!(g.process(&i, &mut o).is_ok());
        assert_eq!(o, i);
    }

    /// Mock variable-frame plugin that returns a configurable output frame count.
    struct VariableFramePlugin {
        channels: usize,
        output_frames: usize,
    }
    impl VariableFramePlugin {
        fn new(channels: usize, output_frames: usize) -> Self {
            Self { channels, output_frames }
        }
    }
    impl Plugin for VariableFramePlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("VariableFrame", "0.1", "test")
        }
        fn input_channels(&self) -> usize { self.channels }
        fn output_channels(&self) -> usize { self.channels }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> { vec![] }
        fn set_parameter(&mut self, _: crate::parameters::ParameterId, _: crate::parameters::ParameterValue) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(&self, _: &crate::parameters::ParameterId) -> Option<crate::parameters::ParameterValue> { None }
        fn process(&mut self, input: &[f32], output: &mut [f32], _ctx: &crate::plugin::ProcessContext) -> Result<usize, String> {
            let out_len = self.output_frames * self.channels;
            for (o, &i) in output[..out_len].iter_mut().zip(input.iter().cycle()) {
                *o = i;
            }
            Ok(self.output_frames)
        }
        fn output_frames_for_input(&self, _: usize) -> usize { self.output_frames }
        fn latency_samples(&self) -> usize { 1 }
    }

    /// Mock plugin that records the ProcessContext.num_frames it receives.
    struct FrameRecorderPlugin {
        channels: usize,
        last_num_frames: std::cell::Cell<usize>,
    }
    impl FrameRecorderPlugin {
        fn new(channels: usize) -> Self {
            Self { channels, last_num_frames: std::cell::Cell::new(0) }
        }
    }
    impl Plugin for FrameRecorderPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("FrameRecorder", "0.1", "test")
        }
        fn input_channels(&self) -> usize { self.channels }
        fn output_channels(&self) -> usize { self.channels }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> { vec![] }
        fn set_parameter(&mut self, _: crate::parameters::ParameterId, _: crate::parameters::ParameterValue) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(&self, _: &crate::parameters::ParameterId) -> Option<crate::parameters::ParameterValue> { None }
        fn process(&mut self, input: &[f32], output: &mut [f32], ctx: &crate::plugin::ProcessContext) -> Result<usize, String> {
            self.last_num_frames.set(ctx.num_frames);
            let len = input.len().min(output.len());
            output[..len].copy_from_slice(&input[..len]);
            Ok(ctx.num_frames)
        }
        fn get_data(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
            Some(Arc::new(self.last_num_frames.get()))
        }
    }

    #[test]
    fn test_parallel_variable_frame_consistent_cf() {
        let mut g = DawHost::new(2, 48000);

        let input_node = g
            .add_node("input".into(), Box::new(VariableFramePlugin::new(2, 256)))
            .unwrap();
        let vf_a = g
            .add_node("vf_a".into(), Box::new(VariableFramePlugin::new(2, 100)))
            .unwrap();
        let vf_b = g
            .add_node("vf_b".into(), Box::new(VariableFramePlugin::new(2, 200)))
            .unwrap();
        let output_node = g
            .add_node("recorder".into(), Box::new(FrameRecorderPlugin::new(2)))
            .unwrap();

        g.add_edge(GraphEdge::new(input_node, vf_a)).unwrap();
        g.add_edge(GraphEdge::new(input_node, vf_b)).unwrap();
        g.add_edge(GraphEdge::new(vf_a, output_node)).unwrap();
        g.add_edge(GraphEdge::new(vf_b, output_node)).unwrap();
        g.build().unwrap();

        let nf = 256;
        let input = vec![0.5_f32; nf * 2];
        let mut output = vec![0.0_f32; nf * 2];
        g.process(&input, &mut output).unwrap();

        let recorded_cf = g.plugins[output_node]
            .as_ref()
            .unwrap()
            .get_data()
            .unwrap()
            .downcast_ref::<usize>()
            .copied()
            .unwrap();

        assert_eq!(
            recorded_cf, 100,
            "Downstream received cf={}, expected 100 (min of parallel outputs)",
            recorded_cf,
        );
    }
}
