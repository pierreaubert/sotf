use super::misc::default_channels;
use serde::{Deserialize, Serialize};

/// Configuration parameters for HalInputPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalInputPluginParams {
    /// Number of output channels (default: 2 for stereo)
    #[serde(default = "default_channels")]
    pub channels: usize,
}
