// ============================================================================
// OBU (Open Bitstream Unit) Module
// ============================================================================

pub mod bitreader;
pub mod parser;

pub use bitreader::BitReader;
pub use parser::{ObuHeader, ObuType, parse_descriptors, parse_temporal_unit};
