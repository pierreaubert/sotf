use serde::{Deserialize, Serialize};

/// Data exposed by the A/B comparison plugin for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ABCompareData {
    pub loudness_a_lufs: f64,
    pub loudness_b_lufs: f64,
    pub auto_gain_db: f32,
    pub peak_a: f64,
    pub peak_b: f64,
    pub current_mix: f32,
    pub bypass_active: bool,
}
