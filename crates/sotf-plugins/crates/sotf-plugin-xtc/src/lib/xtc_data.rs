use serde::{Deserialize, Serialize};
use sotf_host::auto_gain::AutoGainData;

/// Diagnostic data from XTC plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtcData {
    pub auto_gain: AutoGainData,
    pub limiter_envelope: f32,
}

impl Default for XtcData {
    fn default() -> Self {
        Self {
            auto_gain: AutoGainData::default(),
            limiter_envelope: 1.0,
        }
    }
}
