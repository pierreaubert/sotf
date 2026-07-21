//! Converters for EQ-style filter plugins.

use super::PluginConfig;
use crate::plugins::PluginSettings;
use serde_json::json;

pub fn convert_linear_phase_eq(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::LinearPhaseEq {
        num_filters,
        fir_length,
        phase_mode,
        auto_gain,
        mix,
        filters,
    } = settings
    else {
        return None;
    };
    let any_soloed = filters.iter().any(|f| f.solo);
    let band_configs: Vec<serde_json::Value> = filters
        .iter()
        .filter(|f| !f.muted && (!any_soloed || f.solo))
        .map(|f| {
            json!({
                "filter_type": f.filter_type.long_name().to_lowercase(),
                "frequency": f.frequency,
                "q": f.q,
                "gain_db": f.gain_db,
                "active": true,
            })
        })
        .collect();
    Some(PluginConfig::new(
        "linear_phase_eq",
        json!({
            "num_filters": *num_filters as usize,
            "fir_length_index": *fir_length as usize,
            "phase_mode_index": *phase_mode as usize,
            "auto_gain": auto_gain,
            "mix": *mix as f32,
            "filters": band_configs,
        }),
    ))
}
