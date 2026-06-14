use super::types::NodeId;
use crate::parameters::{ParameterId, ParameterValue};

pub(super) struct ParameterEvent {
    pub(super) node_id: NodeId,
    pub(super) param_id: ParameterId,
    pub(super) value: ParameterValue,
    pub(super) sample_offset: usize,
}

impl ParameterEvent {
    pub(super) fn new(
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Self {
        Self {
            node_id,
            param_id,
            value,
            sample_offset,
        }
    }
}
