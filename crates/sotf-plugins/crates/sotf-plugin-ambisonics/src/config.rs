// ============================================================================
// Ambisonics Decoder Plugin Configuration
// ============================================================================

use serde::{Deserialize, Serialize};

/// Configuration for the Ambisonics Decoder plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbisonicsDecoderConfig {
    /// Ambisonics order (1 = FOA/4ch, 2 = SOA/9ch, 3 = TOA/16ch)
    #[serde(default = "default_order")]
    pub order: usize,

    /// Target speaker layout ID (e.g., "5.1", "7.1.4")
    #[serde(default = "default_target_layout")]
    pub target_layout: String,

    /// Apply max-rE weighting for improved high-frequency energy preservation
    #[serde(default = "default_max_re")]
    pub max_re_weighting: bool,
}

fn default_order() -> usize {
    1
}

fn default_target_layout() -> String {
    "5.1".to_owned()
}

fn default_max_re() -> bool {
    true
}

impl Default for AmbisonicsDecoderConfig {
    fn default() -> Self {
        Self {
            order: default_order(),
            target_layout: default_target_layout(),
            max_re_weighting: default_max_re(),
        }
    }
}
