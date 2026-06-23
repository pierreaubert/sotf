use crate::params::{
    default_dual_release, default_feed_forward, default_isp_mode, default_link_amount,
    default_lookahead_ms, default_mix, default_release_ms, default_soft, default_threshold_db,
    default_true_peak,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimiterPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default = "default_soft")]
    pub soft: bool,
    #[serde(default = "default_true_peak")]
    pub true_peak: bool,
    #[serde(default = "default_isp_mode")]
    pub isp_mode: bool,
    #[serde(default = "default_dual_release")]
    pub dual_release: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_feed_forward")]
    pub feed_forward: bool,
    #[serde(default = "default_link_amount")]
    pub link_amount: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PARAMS;
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: LimiterPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold_db, pk(PARAMS, "threshold").default_f64() as f32);
        assert_eq!(p.release_ms, pk(PARAMS, "release").default_f64() as f32);
        assert_eq!(p.lookahead_ms, pk(PARAMS, "lookahead").default_f64() as f32);
        assert_eq!(p.soft, pk(PARAMS, "soft").default_bool());
        assert_eq!(p.true_peak, pk(PARAMS, "true_peak").default_bool());
        assert_eq!(p.isp_mode, pk(PARAMS, "isp_mode").default_bool());
        assert_eq!(p.dual_release, pk(PARAMS, "dual_release").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(p.feed_forward, pk(PARAMS, "feed_forward").default_bool());
        assert_eq!(p.link_amount, pk(PARAMS, "link_amount").default_f64() as f32);
    }
}

/// Data exposed by the limiter for UI monitoring
#[derive(Debug, Clone, Default)]
pub struct LimiterData {
    /// Current gain reduction in dB (positive value, e.g., 6.0 means -6dB gain)
    pub gain_reduction_db: f32,
    /// Peak input level in dB
    pub peak_db: f32,
    /// Whether the limiter is actively limiting
    pub is_limiting: bool,
    /// Per-channel inter-sample true peak in dBTP (empty when true_peak is disabled)
    pub isp_dbtp: Vec<f32>,
}
