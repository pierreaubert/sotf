use super::dyn_eq_band_params::DynEqBandParams;
use crate::params::{
    default_attack_ms, default_bands, default_knee, default_link_channels, default_mix,
    default_num_bands, default_ratio, default_release_ms, default_threshold,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEqPluginParams {
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee")]
    pub knee: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_bands")]
    pub bands: Vec<DynEqBandParams>,
}

impl Default for DynamicEqPluginParams {
    fn default() -> Self {
        Self {
            num_bands: default_num_bands(),
            threshold: default_threshold(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            knee: default_knee(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            bands: default_bands(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{BAND_PARAMS, PARAMS};
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: DynamicEqPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.num_bands, pk(PARAMS, "num_bands").default_f64() as usize);
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64() as f32);
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64() as f32);
        assert_eq!(p.attack_ms, pk(PARAMS, "attack").default_f64() as f32);
        assert_eq!(p.release_ms, pk(PARAMS, "release").default_f64() as f32);
        assert_eq!(p.knee, pk(PARAMS, "knee").default_f64() as f32);
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(p.bands.len(), p.num_bands);
        let band = &p.bands[0];
        assert_eq!(
            band.frequency,
            pk(BAND_PARAMS, "frequency").default_f64() as f32
        );
        assert_eq!(band.q, pk(BAND_PARAMS, "q").default_f64() as f32);
        assert_eq!(band.gain, pk(BAND_PARAMS, "gain").default_f64() as f32);
        assert_eq!(
            band.band_threshold,
            pk(BAND_PARAMS, "band_threshold").default_f64() as f32
        );
        assert_eq!(
            band.band_ratio,
            pk(BAND_PARAMS, "band_ratio").default_f64() as f32
        );
        assert_eq!(band.active, pk(BAND_PARAMS, "active").default_bool());
        assert_eq!(band.solo, pk(BAND_PARAMS, "solo").default_bool());
    }
}
