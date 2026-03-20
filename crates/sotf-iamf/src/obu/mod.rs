// ============================================================================
// OBU (Open Bitstream Unit) Module
// ============================================================================

pub mod parser;

pub use parser::{ObuHeader, ObuType, parse_descriptors, parse_temporal_unit};
