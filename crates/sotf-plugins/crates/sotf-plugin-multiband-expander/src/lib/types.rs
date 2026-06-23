use super::band_expander_params::BandExpanderParams;
use crate::params::{
    default_attack_ms, default_auto_makeup, default_crossover_frequencies,
    default_crossover_preset, default_detection_mode, default_hold_ms, default_hysteresis_db,
    default_knee_db, default_link_channels, default_lookahead_ms, default_measured_auto_makeup,
    default_mix, default_num_bands, default_processing_mode, default_range_db, default_ratio,
    default_release_ms, default_sidechain_hpf_hz, default_threshold_db,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibandExpanderPluginParams {
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    #[serde(default = "default_crossover_preset")]
    pub crossover_preset: i32,
    #[serde(default = "default_crossover_frequencies")]
    pub crossover_frequencies: Vec<f32>,
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    #[serde(default = "default_hysteresis_db")]
    pub hysteresis_db: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default)]
    pub bands: Vec<BandExpanderParams>,
    /// Processing mode: "time_domain" (default) or "spectral"
    #[serde(default = "default_processing_mode")]
    pub processing_mode: String,
    /// Single-band alias: auto-compensate for gain reduction (applied to band 0)
    #[serde(default = "default_auto_makeup")]
    pub auto_makeup: Option<bool>,
    /// Single-band alias: measured auto-makeup (applied to band 0)
    #[serde(default = "default_measured_auto_makeup")]
    pub measured_auto_makeup: Option<bool>,
    /// Sidechain high-pass frequency (single-band compatibility)
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: Option<f32>,
}

impl Default for MultibandExpanderPluginParams {
    fn default() -> Self {
        Self {
            num_bands: default_num_bands(),
            crossover_preset: default_crossover_preset(),
            crossover_frequencies: default_crossover_frequencies(),
            threshold_db: default_threshold_db(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            knee_db: default_knee_db(),
            range_db: default_range_db(),
            hysteresis_db: default_hysteresis_db(),
            hold_ms: default_hold_ms(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            detection_mode: default_detection_mode(),
            lookahead_ms: default_lookahead_ms(),
            bands: Vec::new(),
            processing_mode: default_processing_mode(),
            auto_makeup: default_auto_makeup(),
            measured_auto_makeup: default_measured_auto_makeup(),
            sidechain_hpf_hz: default_sidechain_hpf_hz(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum GateState {
    Open,
    Hold,
    Closing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{DETECTION_MODES, GLOBAL_PARAMS, PARAMS};
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: MultibandExpanderPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.num_bands,
            pk(GLOBAL_PARAMS, "num_bands").default_f64() as usize
        );
        assert_eq!(
            p.crossover_preset,
            pk(GLOBAL_PARAMS, "crossover_preset").default_f64() as i32
        );
        assert_eq!(
            p.crossover_frequencies,
            vec![
                pk(GLOBAL_PARAMS, "crossover_freq_1").default_f64() as f32,
                pk(GLOBAL_PARAMS, "crossover_freq_2").default_f64() as f32,
                pk(GLOBAL_PARAMS, "crossover_freq_3").default_f64() as f32,
                pk(GLOBAL_PARAMS, "crossover_freq_4").default_f64() as f32,
            ]
        );
        assert_eq!(p.threshold_db, pk(PARAMS, "threshold").default_f64() as f32);
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64() as f32);
        assert_eq!(p.attack_ms, pk(PARAMS, "attack").default_f64() as f32);
        assert_eq!(p.release_ms, pk(PARAMS, "release").default_f64() as f32);
        assert_eq!(p.knee_db, pk(PARAMS, "knee").default_f64() as f32);
        assert_eq!(p.range_db, pk(PARAMS, "range").default_f64() as f32);
        assert_eq!(
            p.hysteresis_db,
            pk(PARAMS, "hysteresis").default_f64() as f32
        );
        assert_eq!(p.hold_ms, pk(PARAMS, "hold").default_f64() as f32);
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(p.detection_mode, DETECTION_MODES[0]);
        assert_eq!(
            p.lookahead_ms,
            pk(PARAMS, "lookahead_ms").default_f64() as f32
        );
        assert_eq!(p.processing_mode, "time_domain");
        assert_eq!(
            p.auto_makeup,
            Some(pk(PARAMS, "auto_makeup").default_bool())
        );
        assert_eq!(
            p.measured_auto_makeup,
            Some(pk(PARAMS, "measured_auto_makeup").default_bool())
        );
        assert_eq!(
            p.sidechain_hpf_hz,
            Some(pk(PARAMS, "sidechain_hpf_hz").default_f64() as f32)
        );
    }
}
