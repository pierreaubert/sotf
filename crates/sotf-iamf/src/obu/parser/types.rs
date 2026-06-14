use super::obu_type::ObuType;
use crate::types::*;

/// Parsed OBU header
#[derive(Debug, Clone)]
pub struct ObuHeader {
    pub obu_type: ObuType,
    pub redundant_copy: bool,
    pub trimming_status: bool,
    pub extension_flag: bool,
    pub payload_size: usize,
    pub trim_start: u32,
    pub trim_end: u32,
}

/// Parsed temporal unit: audio frames + parameter blocks for one time step.
#[derive(Debug)]
pub struct TemporalUnit {
    pub parameter_blocks: Vec<ParameterBlock>,
    pub audio_frames: Vec<AudioFrameObu>,
}
