use crate::params::{
    default_allpass_coeff, default_allpass_feedback, default_delay_ms, default_feedback,
    default_lfo_depth_ms, default_lfo_rate_hz, default_mix,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayPluginParams {
    #[serde(default = "default_delay_ms")]
    pub delay_ms: f32,
    #[serde(default = "default_feedback")]
    pub feedback: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_lfo_rate_hz")]
    pub lfo_rate_hz: f32,
    #[serde(default = "default_lfo_depth_ms")]
    pub lfo_depth_ms: f32,
    #[serde(default = "default_allpass_feedback")]
    pub allpass_feedback: bool,
    #[serde(default = "default_allpass_coeff")]
    pub allpass_coeff: f32,
    /// Per-channel delay times in milliseconds. When non-empty, takes
    /// precedence over the scalar `delay_ms` and switches the plugin into
    /// per-channel mode (one independent delay per channel).
    #[serde(default)]
    pub channel_delays_ms: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PARAMS;
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: DelayPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.delay_ms, pk(PARAMS, "delay_ms").default_f64() as f32);
        assert_eq!(p.feedback, pk(PARAMS, "feedback").default_f64() as f32);
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(
            p.lfo_rate_hz,
            pk(PARAMS, "lfo_rate_hz").default_f64() as f32
        );
        assert_eq!(
            p.lfo_depth_ms,
            pk(PARAMS, "lfo_depth_ms").default_f64() as f32
        );
        assert_eq!(
            p.allpass_feedback,
            pk(PARAMS, "allpass_feedback").default_bool()
        );
        assert_eq!(
            p.allpass_coeff,
            pk(PARAMS, "allpass_coeff").default_f64() as f32
        );
        assert!(p.channel_delays_ms.is_empty());
    }
}
