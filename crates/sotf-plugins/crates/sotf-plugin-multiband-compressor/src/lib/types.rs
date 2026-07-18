use super::band_compressor_params::BandCompressorParams;
use crate::params::{
    default_attack_ms, default_auto_makeup, default_crossover_frequencies,
    default_crossover_preset, default_detection_mode, default_knee_db, default_link_amount,
    default_link_channels, default_lookahead_ms, default_makeup_gain, default_measured_auto_makeup,
    default_mix, default_ms_mode, default_num_bands, default_per_band_lookahead_ms,
    default_program_dependent_release, default_ratio, default_release_ms,
    default_sidechain_external, default_sidechain_hpf_hz, default_sidechain_hpf_order,
    default_sidechain_tilt_db, default_threshold_db,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibandCompressorPluginParams {
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
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_per_band_lookahead_ms")]
    pub per_band_lookahead_ms: f32,
    #[serde(default = "default_ms_mode")]
    pub ms_mode: bool,
    #[serde(default)]
    pub bands: Vec<BandCompressorParams>,
    #[serde(default = "default_sidechain_tilt_db")]
    pub sidechain_tilt_db: f32,
    #[serde(default = "default_link_amount")]
    pub link_amount: f32,
    /// Single-band alias: makeup gain applied to band 0
    #[serde(default = "default_makeup_gain")]
    pub makeup_gain: Option<f32>,
    /// Single-band alias: auto-compensate for gain reduction (applied to band 0)
    #[serde(default = "default_auto_makeup")]
    pub auto_makeup: Option<bool>,
    /// Single-band alias: measured auto-makeup (applied to band 0)
    #[serde(default = "default_measured_auto_makeup")]
    pub measured_auto_makeup: Option<bool>,
    /// Sidechain high-pass frequency (single-band compatibility)
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: Option<f32>,
    /// Sidechain HPF order (single-band compatibility): "2nd" or "4th"
    #[serde(default = "default_sidechain_hpf_order")]
    pub sidechain_hpf_order: Option<String>,
    /// Detection mode (single-band compatibility): "peak" or "rms"
    #[serde(default = "default_detection_mode")]
    pub detection_mode: Option<String>,
    /// Lookahead alias (single-band uses "lookahead_ms", multiband uses "per_band_lookahead_ms")
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: Option<f32>,
    /// Program-dependent release (single-band compatibility)
    #[serde(default = "default_program_dependent_release")]
    pub program_dependent_release: Option<bool>,
    /// External sidechain (single-band compatibility)
    #[serde(default = "default_sidechain_external")]
    pub sidechain_external: Option<bool>,
}

impl Default for MultibandCompressorPluginParams {
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
            link_channels: default_link_channels(),
            mix: default_mix(),
            per_band_lookahead_ms: default_per_band_lookahead_ms(),
            ms_mode: default_ms_mode(),
            bands: Vec::new(),
            sidechain_tilt_db: default_sidechain_tilt_db(),
            link_amount: default_link_amount(),
            makeup_gain: default_makeup_gain(),
            auto_makeup: default_auto_makeup(),
            measured_auto_makeup: default_measured_auto_makeup(),
            sidechain_hpf_hz: default_sidechain_hpf_hz(),
            sidechain_hpf_order: default_sidechain_hpf_order(),
            detection_mode: default_detection_mode(),
            lookahead_ms: default_lookahead_ms(),
            program_dependent_release: default_program_dependent_release(),
            sidechain_external: default_sidechain_external(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::params::{GLOBAL_PARAMS, PARAMS};
    use sotf_host::param_specs::find_by_key as pk;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: MultibandCompressorPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.num_bands, pk(GLOBAL_PARAMS, "num_bands").default_usize());
        assert_eq!(
            p.crossover_preset,
            pk(GLOBAL_PARAMS, "crossover_preset").default_i32()
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
        assert_eq!(p.link_channels, pk(PARAMS, "link_channels").default_bool());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64() as f32);
        assert_eq!(
            p.per_band_lookahead_ms,
            pk(GLOBAL_PARAMS, "per_band_lookahead_ms").default_f64() as f32
        );
        assert_eq!(p.ms_mode, pk(GLOBAL_PARAMS, "ms_mode").default_bool());
        assert!(p.bands.is_empty());
        assert_eq!(
            p.sidechain_tilt_db,
            pk(GLOBAL_PARAMS, "sidechain_tilt_db").default_f64() as f32
        );
        assert_eq!(
            p.link_amount,
            pk(GLOBAL_PARAMS, "link_amount").default_f64() as f32
        );
        // Single-band alias fields are absent by default so they do not override
        // the multiband canonical parameters or per-band settings.
        assert_eq!(p.makeup_gain, None);
        assert_eq!(p.auto_makeup, None);
        assert_eq!(p.measured_auto_makeup, None);
        assert_eq!(p.sidechain_hpf_hz, None);
        assert_eq!(p.sidechain_hpf_order, None);
        assert_eq!(p.detection_mode, None);
        assert_eq!(p.lookahead_ms, None);
        assert_eq!(p.program_dependent_release, None);
        assert_eq!(p.sidechain_external, None);
    }
}

pub(super) struct BandCompressor {
    pub(super) envelope: Vec<f32>,
    pub(super) attack_coeff: f32,
    pub(super) release_coeff: f32,
}
