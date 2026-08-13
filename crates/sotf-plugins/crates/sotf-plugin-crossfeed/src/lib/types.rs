use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossfeedMode {
    #[default]
    Off,
    Bauer,
    Meier,
    Mb,
    /// Compact parametric HRTF crossfeed with head-shadow filtering and ITD.
    Hrtf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossfeedPreset {
    #[default]
    Default,
    Cmoy,
    Meier,
    Mb,
    Off,
    Hrtf,
}
