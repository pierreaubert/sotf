use crate::params::{
    default_attack, default_mix, default_output_gain_db, default_sensitivity_db, default_sustain,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransientShaperPluginParams {
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_attack")]
    pub attack: f32,
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_sustain")]
    pub sustain: f32,
    #[serde(default = "default_sensitivity_db")]
    pub sensitivity_db: f32,
    #[serde(default = "default_output_gain_db")]
    pub output_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

impl Default for TransientShaperPluginParams {
    fn default() -> Self {
        Self {
            attack: default_attack(),
            sustain: default_sustain(),
            sensitivity_db: default_sensitivity_db(),
            output_gain_db: default_output_gain_db(),
            mix: default_mix(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PARAMS;
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: TransientShaperPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64() as f32);
        assert_eq!(p.sustain, pk(PARAMS, "sustain").default_f64() as f32);
        assert_eq!(
            p.sensitivity_db,
            pk(PARAMS, "sensitivity").default_f64() as f32
        );
        assert_eq!(
            p.output_gain_db,
            pk(PARAMS, "output_gain").default_f64() as f32
        );
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransientShaperData {
    /// Peak transient level (positive = transient detected)
    pub transient_level: f32,
    /// Peak sustain level
    pub sustain_level: f32,
    /// Current gain applied (linear)
    pub gain: f32,
}
