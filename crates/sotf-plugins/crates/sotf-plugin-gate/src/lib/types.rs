use crate::params::{
    default_attack_ms, default_detection_mode, default_hold_ms, default_hysteresis_db,
    default_knee_db, default_link_channels, default_lookahead_ms, default_mix, default_range_db,
    default_ratio, default_release_ms, default_sidechain_external, default_sidechain_hpf_hz,
    default_sidechain_hpf_order, default_threshold_db,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
    #[serde(default = "default_sidechain_hpf_order")]
    pub sidechain_hpf_order: String,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "default_sidechain_external")]
    pub sidechain_external: bool,
    /// Maximum attenuation in dB (0 = unlimited). Caps how much the gate attenuates.
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    /// Hysteresis in dB. Close threshold = threshold - hysteresis.
    #[serde(default = "default_hysteresis_db")]
    pub hysteresis_db: f32,
    /// Soft knee width in dB (0 = hard knee).
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    /// Lookahead delay in ms (0 = off, max 20ms). Delays audio so gain is computed from non-delayed signal.
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{DETECTION_MODES, HPF_ORDERS, PARAMS};
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: GatePluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.threshold_db, pk(PARAMS, "threshold").default_f64() as f32);
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64() as f32);
        assert_eq!(p.attack_ms, pk(PARAMS, "attack").default_f64() as f32);
        assert_eq!(p.hold_ms, pk(PARAMS, "hold").default_f64() as f32);
        assert_eq!(p.release_ms, pk(PARAMS, "release").default_f64() as f32);
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(
            p.sidechain_hpf_hz,
            pk(PARAMS, "sidechain_hpf_hz").default_f64() as f32
        );
        assert_eq!(p.sidechain_hpf_order, HPF_ORDERS[0]);
        assert_eq!(p.detection_mode, DETECTION_MODES[0]);
        assert_eq!(
            p.sidechain_external,
            pk(PARAMS, "sidechain_external").default_bool()
        );
        assert_eq!(p.range_db, pk(PARAMS, "range_db").default_f64() as f32);
        assert_eq!(
            p.hysteresis_db,
            pk(PARAMS, "hysteresis_db").default_f64() as f32
        );
        assert_eq!(p.knee_db, pk(PARAMS, "knee_db").default_f64() as f32);
        assert_eq!(
            p.lookahead_ms,
            pk(PARAMS, "lookahead_ms").default_f64() as f32
        );
    }
}
