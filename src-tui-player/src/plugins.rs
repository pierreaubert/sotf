use autoeq_iir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sotf_audio::engine::PluginConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    EQ,
    Upmixer,
    Compressor,
    Limiter,
    Gate,
    LoudnessCompensation,
}

impl PluginType {
    pub fn name(&self) -> &str {
        match self {
            Self::EQ => "EQ",
            Self::Upmixer => "Upmixer",
            Self::Compressor => "Compressor",
            Self::Limiter => "Limiter",
            Self::Gate => "Gate",
            Self::LoudnessCompensation => "Loudness Compensation",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::EQ => "Parametric Equalizer (10-band)",
            Self::Upmixer => "Stereo to 5.1 Surround",
            Self::Compressor => "Dynamic Range Compressor",
            Self::Limiter => "Peak Limiter",
            Self::Gate => "Noise Gate",
            Self::LoudnessCompensation => "Equal Loudness Compensation",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::EQ,
            Self::Upmixer,
            Self::Compressor,
            Self::Limiter,
            Self::Gate,
            Self::LoudnessCompensation,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQFilter {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

impl EQFilter {
    pub fn new(filter_type: BiquadFilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
        }
    }

    pub fn to_biquad(&self, sample_rate: f64) -> Biquad {
        Biquad::new(
            self.filter_type,
            sample_rate,
            self.frequency,
            self.q,
            self.gain_db,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSettings {
    EQ {
        filters: Vec<EQFilter>,
    },
    Upmixer {
        center_level_db: f64,
        lfe_level_db: f64,
        surround_delay_ms: f64,
    },
    Compressor {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        knee_db: f64,
    },
    Limiter {
        threshold_db: f64,
        release_ms: f64,
    },
    Gate {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
    },
    LoudnessCompensation {
        target_lufs: f64,
        min_gain_db: f64,
        max_gain_db: f64,
    },
}

impl PluginSettings {
    pub fn plugin_type(&self) -> PluginType {
        match self {
            Self::EQ { .. } => PluginType::EQ,
            Self::Upmixer { .. } => PluginType::Upmixer,
            Self::Compressor { .. } => PluginType::Compressor,
            Self::Limiter { .. } => PluginType::Limiter,
            Self::Gate { .. } => PluginType::Gate,
            Self::LoudnessCompensation { .. } => PluginType::LoudnessCompensation,
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> PluginConfig {
        match self {
            Self::EQ { filters } => {
                let filter_configs: Vec<_> = filters
                    .iter()
                    .map(|f| {
                        let bq = f.to_biquad(sample_rate);
                        json!({
                            "filter_type": bq.filter_type.long_name().to_lowercase(),
                            "freq": bq.freq,
                            "q": bq.q,
                            "db_gain": bq.db_gain,
                        })
                    })
                    .collect();

                PluginConfig::new(
                    "eq",
                    json!({
                        "filters": filter_configs,
                    }),
                )
            }
            Self::Upmixer {
                center_level_db,
                lfe_level_db,
                surround_delay_ms,
            } => PluginConfig::new(
                "upmixer",
                json!({
                    "center_level_db": center_level_db,
                    "lfe_level_db": lfe_level_db,
                    "surround_delay_ms": surround_delay_ms,
                }),
            ),
            Self::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
            } => PluginConfig::new(
                "compressor",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "knee_db": knee_db,
                }),
            ),
            Self::Limiter {
                threshold_db,
                release_ms,
            } => PluginConfig::new(
                "limiter",
                json!({
                    "threshold_db": threshold_db,
                    "release_ms": release_ms,
                }),
            ),
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
            } => PluginConfig::new(
                "gate",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                }),
            ),
            Self::LoudnessCompensation {
                target_lufs,
                min_gain_db,
                max_gain_db,
            } => PluginConfig::new(
                "loudness_compensation",
                json!({
                    "target_lufs": target_lufs,
                    "min_gain_db": min_gain_db,
                    "max_gain_db": max_gain_db,
                }),
            ),
        }
    }

    /// Create default settings for a plugin type
    pub fn default_for(plugin_type: &PluginType) -> Self {
        match plugin_type {
            PluginType::EQ => Self::EQ {
                filters: vec![
                    // Default: 10-band flat EQ
                    EQFilter::new(BiquadFilterType::Peak, 32.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 64.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 125.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 250.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 500.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 2000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 4000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 8000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 16000.0, 1.4, 0.0),
                ],
            },
            PluginType::Upmixer => Self::Upmixer {
                center_level_db: 0.0,
                lfe_level_db: 0.0,
                surround_delay_ms: 15.0,
            },
            PluginType::Compressor => Self::Compressor {
                threshold_db: -20.0,
                ratio: 4.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                knee_db: 3.0,
            },
            PluginType::Limiter => Self::Limiter {
                threshold_db: -1.0,
                release_ms: 50.0,
            },
            PluginType::Gate => Self::Gate {
                threshold_db: -40.0,
                ratio: 10.0,
                attack_ms: 1.0,
                release_ms: 100.0,
            },
            PluginType::LoudnessCompensation => Self::LoudnessCompensation {
                target_lufs: -18.0,
                min_gain_db: -6.0,
                max_gain_db: 6.0,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: usize,
    pub enabled: bool,
    pub settings: PluginSettings,
}

impl Plugin {
    pub fn new(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        self.settings.plugin_type()
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> Option<PluginConfig> {
        if self.enabled {
            Some(self.settings.to_plugin_config(sample_rate))
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
pub struct PluginChain {
    plugins: Vec<Plugin>,
    next_id: usize,
}

impl PluginChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new(id, plugin_type));
        id
    }

    pub fn remove_plugin(&mut self, index: usize) -> Option<Plugin> {
        if index < self.plugins.len() {
            Some(self.plugins.remove(index))
        } else {
            None
        }
    }

    pub fn get_plugin(&self, index: usize) -> Option<&Plugin> {
        self.plugins.get(index)
    }

    pub fn get_plugin_mut(&mut self, index: usize) -> Option<&mut Plugin> {
        self.plugins.get_mut(index)
    }

    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        if let Some(plugin) = self.plugins.get_mut(index) {
            plugin.enabled = !plugin.enabled;
        }
    }

    pub fn move_plugin(&mut self, from: usize, to: usize) {
        if from < self.plugins.len() && to < self.plugins.len() {
            let plugin = self.plugins.remove(from);
            self.plugins.insert(to, plugin);
        }
    }

    pub fn to_plugin_configs(&self, sample_rate: f64) -> Vec<PluginConfig> {
        self.plugins
            .iter()
            .filter_map(|p| p.to_plugin_config(sample_rate))
            .collect()
    }

    pub fn output_channels(&self) -> usize {
        // Check if there's an upmixer in the chain
        let has_upmixer = self
            .plugins
            .iter()
            .any(|p| p.enabled && matches!(p.settings, PluginSettings::Upmixer { .. }));

        if has_upmixer {
            6 // 5.1 surround
        } else {
            2 // Stereo
        }
    }

    /// Save the plugin chain to a JSON file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(&self.plugins)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load the plugin chain from a JSON file
    pub fn load_from_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let plugins: Vec<Plugin> = serde_json::from_str(&json)?;

        // Update next_id to be higher than any loaded plugin id
        let max_id = plugins.iter().map(|p| p.id).max().unwrap_or(0);
        self.next_id = max_id + 1;

        self.plugins = plugins;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_chain() {
        let mut chain = PluginChain::new();
        assert_eq!(chain.len(), 0);

        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Upmixer);
        assert_eq!(chain.len(), 2);

        let configs = chain.to_plugin_configs(48000.0);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].plugin_type, "eq");
        assert_eq!(configs[1].plugin_type, "upmixer");
    }

    #[test]
    fn test_output_channels() {
        let mut chain = PluginChain::new();
        assert_eq!(chain.output_channels(), 2);

        chain.add_plugin(&PluginType::Upmixer);
        assert_eq!(chain.output_channels(), 6);
    }
}
