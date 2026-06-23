use crate::params::{
    default_attack_ms, default_frequency, default_mix, default_mode, default_q, default_ratio,
    default_release_ms, default_threshold,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeEsserPluginParams {
    #[serde(default = "default_frequency")]
    pub frequency: f32,
    #[serde(default = "default_q")]
    pub q: f32,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PARAMS;
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: DeEsserPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.frequency, pk(PARAMS, "frequency").default_f64() as f32);
        assert_eq!(p.q, pk(PARAMS, "q").default_f64() as f32);
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64() as f32);
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64() as f32);
        assert_eq!(p.attack_ms, pk(PARAMS, "attack").default_f64() as f32);
        assert_eq!(p.release_ms, pk(PARAMS, "release").default_f64() as f32);
        assert_eq!(p.mode, crate::params::MODES[1]);
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
    }
}
