use crate::parameters::{ParameterId, ParameterValue};

/// A host parameter change timestamped relative to the current audio block.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterEvent {
    pub sample_offset: usize,
    pub parameter_id: ParameterId,
    pub value: ParameterValue,
}

impl ParameterEvent {
    pub fn new(sample_offset: usize, parameter_id: ParameterId, value: ParameterValue) -> Self {
        Self {
            sample_offset,
            parameter_id,
            value,
        }
    }
}
