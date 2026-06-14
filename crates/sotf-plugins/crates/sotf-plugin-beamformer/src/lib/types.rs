use serde::{Deserialize, Serialize};

/// Beamformer algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamformerType {
    Mvdr,
    Superdirective,
    Gsc,
}
