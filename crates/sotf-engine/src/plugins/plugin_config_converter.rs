//! Converter registry: turn a typed [`PluginSettings`] variant into a wire [`PluginConfig`].
//!
//! The end goal is to retire the giant `match` in [`PluginSettings::to_plugin_config`].
//! Each plugin type registers a small converter function here; `to_plugin_config` looks
//! up the converter by the plugin's wire name and delegates to it.
//!
//! Unmigrated variants still fall back to the inline match in `plugin_settings.rs`.

use crate::PluginConfig;
use crate::plugins::{EQFilter, PluginSettings};
use std::collections::HashMap;
use std::sync::OnceLock;

mod dynamics;
mod effects;
mod eq;
mod spatial;

/// Signature for a function that converts one [`PluginSettings`] variant into a [`PluginConfig`].
///
/// Implementations should pattern-match on their specific variant and return `None` for any
/// other variant (this is only a safety net; the registry routes by wire name).
pub type PluginConfigConverter = fn(&PluginSettings, f64) -> Option<PluginConfig>;

/// Registry of converters keyed by the plugin's wire type string.
#[derive(Default)]
pub struct PluginConfigConverterRegistry {
    converters: HashMap<&'static str, PluginConfigConverter>,
}

impl PluginConfigConverterRegistry {
    /// Returns the global, lazily-initialized registry.
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<PluginConfigConverterRegistry> = OnceLock::new();
        GLOBAL.get_or_init(Self::build)
    }

    /// Convert a [`PluginSettings`] value if a converter is registered for its wire type.
    pub fn convert(
        &self,
        plugin_type: &str,
        settings: &PluginSettings,
        sample_rate: f64,
    ) -> Option<PluginConfig> {
        self.converters
            .get(plugin_type)
            .and_then(|c| c(settings, sample_rate))
    }

    fn build() -> Self {
        let mut registry = Self::default();
        registry.register("gain", convert_gain);
        registry.register("eq", convert_eq);
        registry.register("delay", convert_delay);
        registry.register("crossfeed", convert_crossfeed);
        registry.register("aec", effects::convert_aec);
        registry.register("beamformer", spatial::convert_beamformer);
        registry.register("ambisonics_decoder", spatial::convert_ambisonics_decoder);
        registry.register("stereo_imager", effects::convert_stereo_imager);
        registry.register("de_esser", dynamics::convert_de_esser);
        registry.register("transient_shaper", dynamics::convert_transient_shaper);
        registry.register("saturation", effects::convert_saturation);
        registry.register("dynamic_eq", dynamics::convert_dynamic_eq);
        registry.register("linear_phase_eq", eq::convert_linear_phase_eq);
        registry.register("fir_designer", eq::convert_fir_designer);
        registry.register("spectral_compressor", dynamics::convert_spectral_compressor);
        registry.register("upmixer", spatial::convert_upmixer);
        registry.register("compressor", dynamics::convert_compressor);
        registry.register("limiter", dynamics::convert_limiter);
        registry.register("gate", dynamics::convert_gate);
        registry.register("expander", dynamics::convert_expander);
        registry.register(
            "multiband_compressor",
            dynamics::convert_multiband_compressor,
        );
        registry.register("multiband_expander", dynamics::convert_multiband_expander);
        registry.register(
            "loudness_compensation",
            effects::convert_loudness_compensation,
        );
        registry.register("fletcher_munson", effects::convert_fletcher_munson);
        registry.register("binaural_decoder", spatial::convert_binaural_decoder);
        registry.register("convolution", effects::convert_convolution);
        registry.register("loudness_monitor", effects::convert_loudness_monitor);
        registry.register("spectrum_analyzer", effects::convert_spectrum_analyzer);
        registry.register("channel_mute_solo", effects::convert_channel_mute_solo);
        registry.register("matrix", effects::convert_matrix);
        registry.register("xtc", spatial::convert_xtc);
        registry.register("denoiser", effects::convert_denoiser);
        registry.register("declick", effects::convert_declick);
        registry.register("hiss_reducer", effects::convert_hiss_reducer);
        registry.register("speech_denoiser", effects::convert_speech_denoiser);
        registry.register("pnd", effects::convert_pnd);
        registry.register("ab_compare", effects::convert_ab_compare);
        registry.register("crossover", spatial::convert_crossover);
        registry.register("band_split", spatial::convert_band_split);
        registry.register("band_merge", spatial::convert_band_merge);
        registry.register("downmix", spatial::convert_downmix);
        registry.register("mono_to_stereo", spatial::convert_mono_to_stereo);
        registry.register("aae", spatial::convert_aae);
        registry
    }

    fn register(&mut self, plugin_type: &'static str, converter: PluginConfigConverter) {
        self.converters.insert(plugin_type, converter);
    }
}

fn convert_gain(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Gain {
        channels,
        gain_db,
        smoothing_ms,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "gain",
        serde_json::json!({
            "channels": channels,
            "gain_db": gain_db,
            "smoothing_ms": smoothing_ms,
        }),
    ))
}

fn convert_eq(settings: &PluginSettings, sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::EQ {
        channels,
        filters,
        channel_filters,
        per_channel_mode,
        max_filters: _,
        tdf2,
        topology: _,
    } = settings
    else {
        return None;
    };

    let convert_filters = |filters: &[EQFilter]| -> Vec<serde_json::Value> {
        use sotf_plugins::plugin_eq::EqFilterTopology;

        let any_soloed = filters.iter().any(|f| f.solo);
        filters
            .iter()
            .filter(|f| {
                if f.muted {
                    return false;
                }
                if any_soloed && !f.solo {
                    return false;
                }
                true
            })
            .map(|f| {
                let bq = f.to_biquad(sample_rate);
                let mut value = serde_json::json!({
                    "filter_type": bq.filter_type.long_name().to_lowercase(),
                    "freq": bq.freq,
                    "q": bq.q,
                    "db_gain": bq.db_gain,
                });
                if !matches!(f.topology, EqFilterTopology::Biquad) {
                    let obj = value.as_object_mut().expect("json! object");
                    match f.topology {
                        EqFilterTopology::Biquad => unreachable!(),
                        EqFilterTopology::WarpedBiquad => {
                            obj.insert("topology".into(), serde_json::json!("warped_biquad"));
                            if let Some(lambda) = f.lambda {
                                obj.insert("lambda".into(), serde_json::json!(lambda));
                            }
                        }
                        EqFilterTopology::KautzFilter => {
                            obj.insert("topology".into(), serde_json::json!("kautz_filter"));
                            if !f.kautz_sections.is_empty() {
                                obj.insert(
                                    "kautz_sections".into(),
                                    serde_json::to_value(&f.kautz_sections)
                                        .unwrap_or(serde_json::Value::Null),
                                );
                            }
                        }
                    }
                }
                value
            })
            .collect()
    };

    if *per_channel_mode {
        if let Some(ch_filters) = channel_filters {
            let channel_filter_configs: Vec<Vec<serde_json::Value>> =
                ch_filters.iter().map(|f| convert_filters(f)).collect();
            Some(PluginConfig::new(
                "eq",
                serde_json::json!({
                    "channels": channels,
                    "channel_filters": channel_filter_configs,
                    "tdf2": tdf2,
                }),
            ))
        } else {
            let filter_configs = convert_filters(filters);
            Some(PluginConfig::new(
                "eq",
                serde_json::json!({
                    "channels": channels,
                    "filters": filter_configs,
                    "tdf2": tdf2,
                }),
            ))
        }
    } else {
        let filter_configs = convert_filters(filters);
        Some(PluginConfig::new(
            "eq",
            serde_json::json!({
                "channels": channels,
                "filters": filter_configs,
                "tdf2": tdf2,
            }),
        ))
    }
}

fn convert_delay(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Delay {
        delay_ms,
        feedback,
        mix,
        lfo_rate_hz,
        lfo_depth_ms,
        allpass_feedback,
        allpass_coeff,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "delay",
        serde_json::json!({
            "delay_ms": delay_ms,
            "feedback": feedback,
            "mix": mix,
            "lfo_rate_hz": lfo_rate_hz,
            "lfo_depth_ms": lfo_depth_ms,
            "allpass_feedback": allpass_feedback,
            "allpass_coeff": allpass_coeff,
        }),
    ))
}

fn convert_crossfeed(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Crossfeed {
        mode,
        preset,
        enabled,
        mix,
        bauer_fcut_hz,
        bauer_feed_db,
        meier_level,
        mb_low_freq_hz,
        mb_mid_high_freq_hz,
        mb_low_feed_db,
        mb_mid_feed_db,
        mb_high_feed_db,
        itd_delay_ms,
        autogain_enabled,
        autogain_target_lufs,
        autogain_max_gain_db,
        autogain_smoothing_ms,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "crossfeed",
        serde_json::json!({
            "mode": mode,
            "preset": preset,
            "enabled": enabled,
            "mix": mix,
            "bauer_fcut_hz": bauer_fcut_hz,
            "bauer_feed_db": bauer_feed_db,
            "meier_level": meier_level,
            "mb_low_freq_hz": mb_low_freq_hz,
            "mb_mid_high_freq_hz": mb_mid_high_freq_hz,
            "mb_low_feed_db": mb_low_feed_db,
            "mb_mid_feed_db": mb_mid_feed_db,
            "mb_high_feed_db": mb_high_feed_db,
            "itd_delay_ms": itd_delay_ms,
            "autogain_enabled": autogain_enabled,
            "autogain_target_lufs": autogain_target_lufs,
            "autogain_max_gain_db": autogain_max_gain_db,
            "autogain_smoothing_ms": autogain_smoothing_ms,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;

    #[test]
    fn registry_converts_gain() {
        let settings = PluginSettings::Gain {
            channels: 2,
            gain_db: -3.0,
            smoothing_ms: 10.0,
        };
        let config = PluginConfigConverterRegistry::global()
            .convert("gain", &settings, 48_000.0)
            .expect("gain converter registered");
        assert_eq!(config.plugin_type, "gain");
        let params = config.parameters;
        assert_eq!(params["channels"], 2);
        assert_eq!(params["gain_db"], -3.0);
        assert_eq!(params["smoothing_ms"], 10.0);
    }

    #[test]
    fn registry_converts_delay() {
        let settings = PluginSettings::Delay {
            delay_ms: 100.0,
            feedback: 0.3,
            mix: 0.5,
            lfo_rate_hz: 0.0,
            lfo_depth_ms: 0.0,
            allpass_feedback: false,
            allpass_coeff: 0.0,
        };
        let config = PluginConfigConverterRegistry::global()
            .convert("delay", &settings, 48_000.0)
            .expect("delay converter registered");
        assert_eq!(config.plugin_type, "delay");
        assert_eq!(config.parameters["delay_ms"], 100.0);
    }

    #[test]
    fn registry_converts_crossfeed() {
        let settings = PluginSettings::Crossfeed {
            mode: sotf_plugins::CrossfeedMode::Bauer,
            preset: sotf_plugins::CrossfeedPreset::Default,
            enabled: true,
            mix: 0.5,
            bauer_fcut_hz: 700.0,
            bauer_feed_db: 2.0,
            meier_level: 0.5,
            mb_low_freq_hz: 200.0,
            mb_mid_high_freq_hz: 2000.0,
            mb_low_feed_db: 1.0,
            mb_mid_feed_db: 2.0,
            mb_high_feed_db: 3.0,
            itd_delay_ms: 0.0,
            autogain_enabled: false,
            autogain_target_lufs: -14.0,
            autogain_max_gain_db: 6.0,
            autogain_smoothing_ms: 100.0,
        };
        let config = PluginConfigConverterRegistry::global()
            .convert("crossfeed", &settings, 48_000.0)
            .expect("crossfeed converter registered");
        assert_eq!(config.plugin_type, "crossfeed");
        assert_eq!(config.parameters["bauer_fcut_hz"], 700.0);
    }

    #[test]
    fn registry_converts_eq_global() {
        let settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 2.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 5,
            tdf2: false,
            topology: 0.0,
        };
        let config = PluginConfigConverterRegistry::global()
            .convert("eq", &settings, 48_000.0)
            .expect("eq converter registered");
        assert_eq!(config.plugin_type, "eq");
        assert!(config.parameters["filters"].is_array());
    }

    #[test]
    fn registry_returns_none_for_unregistered_type() {
        let settings = PluginSettings::Gain {
            channels: 2,
            gain_db: 0.0,
            smoothing_ms: 0.0,
        };
        assert!(
            PluginConfigConverterRegistry::global()
                .convert("not_a_plugin", &settings, 48_000.0)
                .is_none()
        );
    }

    #[test]
    fn registry_converts_all_plugin_types() {
        use crate::plugins::PluginType;
        for plugin_type in PluginType::all() {
            let settings = PluginSettings::default_for(&plugin_type);
            let wire_type = settings.plugin_type().wire_name();
            let config = PluginConfigConverterRegistry::global()
                .convert(wire_type, &settings, 48_000.0)
                .unwrap_or_else(|| panic!("converter not registered for {}", wire_type));
            assert_eq!(config.plugin_type, wire_type);
        }
    }

    #[test]
    fn registry_converts_legacy_fletcher_munson() {
        let settings = PluginSettings::FletcherMunson {
            playback_volume_db: -10.0,
            reference_level_db: 0.0,
            enabled: true,
            band1_freq: 60.0,
            band1_q: 0.7,
            band1_max_gain: 10.0,
            band1_slope: 1.0,
            band2_freq: 200.0,
            band2_q: 0.7,
            band2_max_gain: 8.0,
            band2_slope: 1.0,
            band3_freq: 4000.0,
            band3_q: 0.7,
            band3_max_gain: 6.0,
            band3_slope: 1.0,
            band4_freq: 12000.0,
            band4_q: 0.7,
            band4_max_gain: 4.0,
            band4_slope: 1.0,
            smoothing_ms: 50.0,
            auto_gain_enabled: false,
            auto_gain_max_db: 6.0,
            auto_gain_smoothing_ms: 100.0,
            auto_gain_loudness_type: 0,
            iso_226: false,
        };
        let config = PluginConfigConverterRegistry::global()
            .convert("fletcher_munson", &settings, 48_000.0)
            .expect("fletcher_munson converter registered");
        assert_eq!(config.plugin_type, "loudness_compensation");
        assert_eq!(config.parameters["mode"], 2);
    }
}
