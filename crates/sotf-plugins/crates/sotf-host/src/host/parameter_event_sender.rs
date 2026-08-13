use super::parameter_event::ParameterEvent;
use super::types::NodeId;
use crate::param_specs::UpdateMode;
use crate::parameters::Parameter;
use crate::parameters::{ParameterId, ParameterValue};
use rtrb::Producer;

/// Single-producer handle for lock-free parameter updates into `DawHost`.
///
/// Move this to the control/UI thread and call `queue_node_parameter()` there;
/// the host drains events during `process()`.
pub struct ParameterEventSender {
    pub(super) producer: Producer<ParameterEvent>,
    pub(super) dropped_events: u64,
    pub(super) chain_nodes: Vec<NodeId>,
    pub(super) parameters: Vec<(NodeId, Vec<Parameter>)>,
}

impl ParameterEventSender {
    /// Queue a parameter update at the start of the next processed block.
    pub fn queue_node_parameter(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), String> {
        self.queue_node_parameter_at(node_id, param_id, value, 0)
    }

    /// Queue a parameter update for `sample_offset` frames into the next block.
    ///
    /// Offsets beyond the current block are applied after that block, before
    /// the next one begins.
    pub fn queue_node_parameter_at(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        self.validate(node_id, &param_id, &value)?;
        let event = ParameterEvent::new(node_id, param_id, value, sample_offset);
        self.producer.push(event).map_err(|err| {
            self.dropped_events = self.dropped_events.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: external parameter event queue full; dropped {} events",
                self.dropped_events
            );
            format!("parameter event queue full: {err:?}")
        })
    }

    /// Queue automation by stable chain index, matching `DawHost` and the
    /// embedded-engine facade without exposing internal graph node IDs.
    pub fn queue_plugin_parameter_at(
        &mut self,
        plugin_index: usize,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        let node_id = *self
            .chain_nodes
            .get(plugin_index)
            .ok_or("plugin index out of bounds")?;
        self.queue_node_parameter_at(node_id, param_id, value, sample_offset)
    }

    pub fn queue_plugin_parameter(
        &mut self,
        plugin_index: usize,
        param_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), String> {
        self.queue_plugin_parameter_at(plugin_index, param_id, value, 0)
    }

    fn validate(
        &self,
        node_id: NodeId,
        param_id: &ParameterId,
        value: &ParameterValue,
    ) -> Result<(), String> {
        let parameters = self
            .parameters
            .iter()
            .find_map(|(id, parameters)| (*id == node_id).then_some(parameters))
            .ok_or("node not found in parameter sender snapshot")?;
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.id == *param_id)
            .ok_or_else(|| format!("Unknown parameter '{param_id}' for node {node_id}"))?;
        if parameter.update_mode == UpdateMode::Structural {
            return Err(format!(
                "Parameter {param_id} requires rebuilding the plugin chain"
            ));
        }
        parameter
            .validate(value)
            .map_err(|reason| format!("Invalid value for parameter '{param_id}': {reason}"))
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}
