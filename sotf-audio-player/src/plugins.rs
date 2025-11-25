use autoeq_iir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sotf_audio::engine::PluginConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    EQ,
    Gain,
    Upmixer,
    Compressor,
    Limiter,
    Gate,
    LoudnessCompensation,
    BinauralDecoder,
    Convolution,
    LoudnessMonitor,
    SpectrumAnalyzer,
    ChannelMuteSolo,
}

impl PluginType {
    pub fn name(&self) -> &str {
        match self {
            Self::EQ => "[1] EQ",
            Self::Gain => "[g] Gain",
            Self::Upmixer => "[2] Upmixer",
            Self::Compressor => "[3] Compressor",
            Self::Gate => "[4] Gate",
            Self::Limiter => "[5] Limiter",
            Self::LoudnessCompensation => "[6] Loudness Compensation",
            Self::BinauralDecoder => "[7] Binaural Decoder",
            Self::Convolution => "[8] Convolution",
            Self::LoudnessMonitor => "[9] Loudness Monitor",
            Self::SpectrumAnalyzer => "[0] Spectrum Analyzer",
            Self::ChannelMuteSolo => "[m] Channel Mute/Solo",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::EQ => "Parametric Equalizer IIR",
            Self::Gain => "Simple Volume/Gain Control",
            Self::Upmixer => "Stereo to Surround 5.1 to 9.1.6",
            Self::Compressor => "Dynamic Range Compressor",
            Self::Limiter => "Peak Limiter",
            Self::Gate => "Noise Gate",
            Self::LoudnessCompensation => "Equal Loudness Compensation",
            Self::BinauralDecoder => "Multi-channel to Binaural (HRTF)",
            Self::Convolution => "FFT-based Convolution (IR Processing)",
            Self::LoudnessMonitor => "Real-time EBU R128 loudness monitoring",
            Self::SpectrumAnalyzer => "Real-time frequency spectrum analysis",
            Self::ChannelMuteSolo => "Mute or solo individual channels",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::EQ,
            Self::Gain,
            Self::Upmixer,
            Self::Compressor,
            Self::Limiter,
            Self::Gate,
            Self::LoudnessCompensation,
            Self::BinauralDecoder,
            Self::Convolution,
            Self::LoudnessMonitor,
            Self::SpectrumAnalyzer,
            Self::ChannelMuteSolo,
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

    /// Parse a single APO filter line
    /// Format: "Filter N: ON FILTERTYPE Fc FREQ Hz Gain GAIN dB Q QVAL"
    /// Example: "Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41"
    pub fn from_apo_line(line: &str) -> Result<Self, String> {
        let line = line.trim();

        // Skip if filter is OFF
        if line.contains("OFF") {
            return Err("Filter is disabled".to_string());
        }

        // Parse filter type
        let filter_type = if line.contains(" PK ") || line.contains(" PEQ ") {
            BiquadFilterType::Peak
        } else if line.contains(" LSC ") || line.contains(" LOW_SHELF ") || line.contains(" LS ") {
            BiquadFilterType::Lowshelf
        } else if line.contains(" HSC ") || line.contains(" HIGH_SHELF ") || line.contains(" HS ") {
            BiquadFilterType::Highshelf
        } else if line.contains(" LP ") || line.contains(" LPQ ") {
            BiquadFilterType::Lowpass
        } else if line.contains(" HP ") || line.contains(" HPQ ") {
            BiquadFilterType::Highpass
        } else if line.contains(" NO ") || line.contains(" NOTCH ") {
            BiquadFilterType::Notch
        } else if line.contains(" BP ") {
            BiquadFilterType::Bandpass
        } else {
            return Err(format!("Unknown filter type in line: {}", line));
        };

        // Parse frequency (look for "Fc" followed by number)
        let frequency = line
            .split_whitespace()
            .skip_while(|&s| s != "Fc")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| format!("Could not parse frequency from line: {}", line))?;

        // Parse gain (look for "Gain" followed by number)
        let gain_db = line
            .split_whitespace()
            .skip_while(|&s| s != "Gain")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0); // Default to 0 dB if not found (for LP/HP/BP/NO filters)

        // Parse Q (look for "Q" followed by number)
        let q = line
            .split_whitespace()
            .skip_while(|&s| s != "Q")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.707); // Default Q value

        Ok(Self::new(filter_type, frequency, q, gain_db))
    }

    /// Parse APO format file and return a vector of EQ filters
    /// Format:
    /// ```text
    /// Preamp: -6.0 dB
    /// Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41
    /// Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71
    /// ```
    pub fn from_apo_file(path: &std::path::Path) -> Result<Vec<Self>, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        let mut filters = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Skip preamp lines for now
            if line.to_lowercase().starts_with("preamp:") {
                continue;
            }

            // Try to parse as filter line
            if line.to_lowercase().contains("filter") && line.contains(':') {
                match Self::from_apo_line(line) {
                    Ok(filter) => filters.push(filter),
                    Err(e) => log::warn!("Skipping line '{}': {}", line, e),
                }
            }
        }

        if filters.is_empty() {
            Err("No valid filters found in APO file".to_string())
        } else {
            Ok(filters)
        }
    }
}

fn default_limiter_mix() -> f64 {
    0.95
}

fn default_gate_mix() -> f64 {
    0.95
}

fn default_gate_link_channels() -> bool {
    true
}

fn default_gate_sidechain_hpf_hz() -> f64 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSettings {
    EQ {
        filters: Vec<EQFilter>,
    },
    Gain {
        gain_db: f64,
    },
    Upmixer {
        speaker_config: String,
        gain_front_direct: f64,
        gain_front_ambient: f64,
        gain_rear_ambient: f64,
        lfe_cutoff_hz: f64,
        stereo_width: f64,
        bandpass_hz: f64,
        height_gain: f64,
        lfe_gain: f64,
        enable_subharmonic_synth: bool,
        subharmonic_gain: f64,
        enable_hr_direct: bool,
        hr_sharpen: f64,
        safety_cap_db: f64,
        decorrelation_mode: usize,
    },
    Compressor {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        knee_db: f64,
        makeup_gain_db: f64,
        mix: f64,
        auto_makeup: bool,
        link_channels: bool,
        sidechain_hpf_hz: f64,
    },
    Limiter {
        threshold_db: f64,
        release_ms: f64,
        #[serde(default = "default_limiter_mix")]
        mix: f64,
    },
    Gate {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        #[serde(default = "default_gate_mix")]
        mix: f64,
        #[serde(default = "default_gate_link_channels")]
        link_channels: bool,
        #[serde(default = "default_gate_sidechain_hpf_hz")]
        sidechain_hpf_hz: f64,
    },
    LoudnessCompensation {
        target_lufs: f64,
        min_gain_db: f64,
        max_gain_db: f64,
    },
    BinauralDecoder {
        sofa_file: String,
        input_channels: usize,
        enable_optimization: bool,
        externalization: f64,
        near_field_strength: f64,
    },
    Convolution {
        ir_file: String,
        mix: f64,
        gain_db: f64,
    },
    LoudnessMonitor,
    SpectrumAnalyzer {
        num_bins: usize,
        min_freq: f32,
        max_freq: f32,
        smoothing: f32,
    },
    ChannelMuteSolo {
        enabled: bool,
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
}

impl PluginSettings {
    pub fn plugin_type(&self) -> PluginType {
        match self {
            Self::EQ { .. } => PluginType::EQ,
            Self::Gain { .. } => PluginType::Gain,
            Self::Upmixer { .. } => PluginType::Upmixer,
            Self::Compressor { .. } => PluginType::Compressor,
            Self::Limiter { .. } => PluginType::Limiter,
            Self::Gate { .. } => PluginType::Gate,
            Self::LoudnessCompensation { .. } => PluginType::LoudnessCompensation,
            Self::BinauralDecoder { .. } => PluginType::BinauralDecoder,
            Self::Convolution { .. } => PluginType::Convolution,
            Self::LoudnessMonitor => PluginType::LoudnessMonitor,
            Self::SpectrumAnalyzer { .. } => PluginType::SpectrumAnalyzer,
            Self::ChannelMuteSolo { .. } => PluginType::ChannelMuteSolo,
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
            Self::Gain { gain_db } => PluginConfig::new(
                "gain",
                json!({
                    "gain_db": gain_db,
                }),
            ),
            Self::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                lfe_cutoff_hz,
                stereo_width,
                bandpass_hz,
                height_gain,
                lfe_gain,
                enable_subharmonic_synth,
                subharmonic_gain,
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db,
                decorrelation_mode,
            } => PluginConfig::new(
                "upmixer",
                json!({
                    "speaker_config": speaker_config,
                    "gain_front_direct": gain_front_direct,
                    "gain_front_ambient": gain_front_ambient,
                    "gain_rear_ambient": gain_rear_ambient,
                    "lfe_cutoff_hz": lfe_cutoff_hz,
                    "stereo_width": stereo_width,
                    "bandpass_hz": bandpass_hz,
                    "height_gain": height_gain,
                    "lfe_gain": lfe_gain,
                    "enable_subharmonic_synth": enable_subharmonic_synth,
                    "subharmonic_gain": subharmonic_gain,
                    "enable_hr_direct": enable_hr_direct,
                    "hr_sharpen": hr_sharpen,
                    "safety_cap_db": safety_cap_db,
                    "decorrelation_mode": decorrelation_mode,
                }),
            ),
            Self::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => PluginConfig::new(
                "compressor",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "knee_db": knee_db,
                    "makeup_gain_db": makeup_gain_db,
                    "mix": mix,
                    "auto_makeup": auto_makeup,
                    "link_channels": link_channels,
                    "sidechain_hpf_hz": sidechain_hpf_hz,
                }),
            ),
            Self::Limiter {
                threshold_db,
                release_ms,
                mix,
            } => PluginConfig::new(
                "limiter",
                json!({
                    "threshold_db": threshold_db,
                    "release_ms": release_ms,
                    "mix": mix,
                }),
            ),
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => PluginConfig::new(
                "gate",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "mix": mix,
                    "link_channels": link_channels,
                    "sidechain_hpf_hz": sidechain_hpf_hz,
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
            Self::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => PluginConfig::new(
                "binaural_decoder",
                json!({
                    "sofa_file": sofa_file,
                    "input_channels": input_channels,
                    "enable_optimization": enable_optimization,
                    "externalization": externalization,
                    "near_field_strength": near_field_strength,
                }),
            ),
            Self::Convolution {
                ir_file,
                mix,
                gain_db,
            } => PluginConfig::new(
                "convolution",
                json!({
                    "ir_file": ir_file,
                    "mix": mix,
                    "gain_db": gain_db,
                }),
            ),
            Self::LoudnessMonitor => PluginConfig::new("loudness_monitor", json!({})),
            Self::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
            } => PluginConfig::new(
                "spectrum_analyzer",
                json!({
                    "num_bins": num_bins,
                    "min_freq": min_freq,
                    "max_freq": max_freq,
                    "smoothing": smoothing,
                }),
            ),
            Self::ChannelMuteSolo {
                enabled,
                channel_states,
            } => PluginConfig::new(
                "channel_mute_solo",
                json!({
                    "enabled": enabled,
                    "channel_states": channel_states,
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
            PluginType::Gain => Self::Gain { gain_db: 0.0 },
            PluginType::Upmixer => Self::Upmixer {
                speaker_config: "5.1".to_string(),
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 1.0,
                lfe_cutoff_hz: 120.0,
                stereo_width: 0.5,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                safety_cap_db: 3.0,
                decorrelation_mode: 0,
            },
            PluginType::Compressor => Self::Compressor {
                threshold_db: -20.0,
                ratio: 4.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                knee_db: 3.0,
                makeup_gain_db: 0.0,
                mix: 0.95,
                auto_makeup: false,
                link_channels: true,
                sidechain_hpf_hz: 80.0,
            },
            PluginType::Limiter => Self::Limiter {
                threshold_db: -1.0,
                release_ms: 50.0,
                mix: default_limiter_mix(),
            },
            PluginType::Gate => Self::Gate {
                threshold_db: -40.0,
                ratio: 10.0,
                attack_ms: 1.0,
                release_ms: 100.0,
                mix: default_gate_mix(),
                link_channels: default_gate_link_channels(),
                sidechain_hpf_hz: default_gate_sidechain_hpf_hz(),
            },
            PluginType::LoudnessCompensation => Self::LoudnessCompensation {
                target_lufs: -18.0,
                min_gain_db: -6.0,
                max_gain_db: 6.0,
            },
            PluginType::BinauralDecoder => Self::BinauralDecoder {
                sofa_file: String::new(),
                input_channels: 6, // Default to 5.1
                enable_optimization: true,
                externalization: 0.0,
                near_field_strength: 0.0,
            },
            PluginType::Convolution => Self::Convolution {
                ir_file: String::new(),
                mix: 1.0,
                gain_db: 0.0,
            },
            PluginType::LoudnessMonitor => Self::LoudnessMonitor,
            PluginType::SpectrumAnalyzer => Self::SpectrumAnalyzer {
                num_bins: 30,
                min_freq: 20.0,
                max_freq: 20000.0,
                smoothing: 0.7,
            },
            PluginType::ChannelMuteSolo => Self::ChannelMuteSolo {
                enabled: false,
                channel_states: vec![],
            },
        }
    }
}

/// Versioned wrapper for plugin presets
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginPreset {
    #[serde(default = "default_plugin_preset_version")]
    version: u32,
    plugins: Vec<Plugin>,
}

fn default_plugin_preset_version() -> u32 {
    1
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
        // Walk backwards through the chain to find the last channel-count-changing plugin
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    // Map speaker config to channel count
                    return match speaker_config.as_str() {
                        "2.0" => 2,
                        "5.0" => 5,
                        "5.1" => 6,
                        "7.1" => 8,
                        "5.1.2" => 8,
                        "5.1.4" => 10,
                        "7.1.2" => 10,
                        "7.1.4" => 12,
                        "9.1.4" => 14,
                        "9.1.6" => 16,
                        _ => {
                            log::warn!(
                                "Unknown speaker config '{}', defaulting to 5.1 (6 channels)",
                                speaker_config
                            );
                            6
                        }
                    };
                }
                PluginSettings::BinauralDecoder { .. } => {
                    // Binaural decoder always outputs stereo
                    return 2;
                }
                _ => continue,
            }
        }

        // No channel-changing plugin found, return stereo
        2
    }

    /// Save the plugin chain to a JSON file in the plugin_presets directory
    ///
    /// # Arguments
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the extension is not .json or if saving fails
    pub fn save_to_file(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Validate extension - must be .json or none
        let path = std::path::Path::new(filename);
        let extension = path.extension().and_then(|ext| ext.to_str());

        // Check if user specified a non-json extension
        if let Some(ext) = extension
            && ext != "json"
        {
            return Err(format!(
                "Only .json files are supported. Please use .json extension instead of .{}",
                ext
            )
            .into());
        }

        // Auto-append .json if no extension provided
        let filename = if extension.is_none() {
            format!("{}.json", filename)
        } else {
            filename.to_string()
        };

        // Get plugin_presets directory
        let presets_dir = crate::config::get_plugin_presets_dir()
            .ok_or("Could not access plugin presets directory")?;

        let full_path = presets_dir.join(&filename);

        // Wrap plugins in versioned preset
        let preset = PluginPreset {
            version: default_plugin_preset_version(),
            plugins: self.plugins.clone(),
        };

        // Save to file
        let json = serde_json::to_string_pretty(&preset)?;
        std::fs::write(&full_path, json)?;

        log::info!("Saved plugin chain to {}", full_path.display());
        Ok(())
    }

    /// Load the plugin chain from a JSON file in the plugin_presets directory
    ///
    /// # Arguments
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the file doesn't exist or loading fails
    pub fn load_from_file(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Auto-append .json if not already present
        let path = std::path::Path::new(filename);
        let final_filename = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };

        log::debug!(
            "Loading plugin chain from filename: {} (original: {})",
            final_filename,
            filename
        );

        // Get plugin_presets directory
        let presets_dir = crate::config::get_plugin_presets_dir()
            .ok_or("Could not access plugin presets directory")?;

        let full_path = presets_dir.join(&final_filename);
        log::debug!("Full path: {}", full_path.display());

        // Load from file
        let json = std::fs::read_to_string(&full_path)?;
        log::debug!("Read {} bytes from file", json.len());

        // Try to load as versioned preset first
        let mut preset: PluginPreset = match serde_json::from_str(&json) {
            Ok(p) => p,
            Err(_) => {
                // Fall back to loading as legacy format (direct Vec<Plugin>)
                log::info!("Loading legacy plugin preset format (no version field)");
                let plugins: Vec<Plugin> = serde_json::from_str(&json)?;
                PluginPreset {
                    version: 0, // Mark as legacy
                    plugins,
                }
            }
        };

        // Check if migration is needed
        const LATEST_VERSION: u32 = 1;
        let original_version = preset.version;

        if preset.version < LATEST_VERSION {
            log::info!(
                "Migrating plugin preset from version {} to {}",
                original_version,
                LATEST_VERSION
            );

            // Apply migrations
            preset = Self::migrate_preset(preset)?;

            // Save upgraded preset back to disk
            self.plugins = preset.plugins.clone();
            self.save_to_file(&final_filename)?;

            log::info!(
                "Successfully migrated plugin preset from version {} to {}",
                original_version,
                LATEST_VERSION
            );
        }

        log::debug!("Deserialized {} plugins", preset.plugins.len());

        // Update next_id to be higher than any loaded plugin id
        let max_id = preset.plugins.iter().map(|p| p.id).max().unwrap_or(0);
        self.next_id = max_id + 1;

        self.plugins = preset.plugins;

        log::info!(
            "Loaded plugin chain from {} ({} plugins)",
            full_path.display(),
            self.plugins.len()
        );
        Ok(())
    }

    /// Apply all necessary migrations to bring a plugin preset to the latest version
    fn migrate_preset(
        mut preset: PluginPreset,
    ) -> Result<PluginPreset, Box<dyn std::error::Error>> {
        const LATEST_VERSION: u32 = 1;

        // Apply migrations sequentially
        while preset.version < LATEST_VERSION {
            match preset.version {
                // Migration from legacy format (version 0) to version 1
                0 => {
                    log::info!("Applying plugin preset migration: v0 (legacy) -> v1");
                    // No structural changes needed for now
                    // Future migrations might need to transform plugin settings
                    preset.version = 1;
                }

                // Example migration from version 1 to 2:
                // 1 => {
                //     log::info!("Applying plugin preset migration: v1 -> v2");
                //     // Apply migration logic here
                //     // e.g., transform plugin parameters, rename fields, etc.
                //     preset.version = 2;
                // }

                // If we reach here with no match, we have an unknown version
                v => {
                    return Err(format!("Unknown plugin preset version: {}", v).into());
                }
            }
        }

        Ok(preset)
    }

    /// Update BinauralDecoder input_channels based on the output of plugins before them
    /// This should be called after any plugin chain modification (add, remove, move, toggle)
    pub fn update_binaural_decoder_channels(&mut self) {
        for i in 0..self.plugins.len() {
            if let PluginSettings::BinauralDecoder { sofa_file, .. } = &self.plugins[i].settings {
                // Calculate output channels from all plugins before this one
                let input_channels = if i == 0 {
                    2 // Stereo input by default
                } else {
                    // Create a temporary view of plugins before this one
                    let mut channels = 2; // Start with stereo
                    for j in 0..i {
                        if !self.plugins[j].enabled {
                            continue;
                        }
                        match &self.plugins[j].settings {
                            PluginSettings::Upmixer { speaker_config, .. } => {
                                channels = match speaker_config.as_str() {
                                    "2.0" => 2,
                                    "5.0" => 5,
                                    "5.1" => 6,
                                    "7.1" => 8,
                                    "5.1.2" => 8,
                                    "5.1.4" => 10,
                                    "7.1.2" => 10,
                                    "7.1.4" => 12,
                                    "9.1.4" => 14,
                                    "9.1.6" => 16,
                                    _ => 6, // Default to 5.1
                                };
                            }
                            PluginSettings::BinauralDecoder { .. } => {
                                channels = 2; // Binaural outputs stereo
                            }
                            _ => {} // Other plugins don't change channel count
                        }
                    }
                    channels
                };

                // Update the BinauralDecoder with the calculated input channels
                // Preserve existing settings when updating input channels
                if let PluginSettings::BinauralDecoder {
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    ..
                } = &self.plugins[i].settings
                {
                    let sofa_file = sofa_file.clone();
                    self.plugins[i].settings = PluginSettings::BinauralDecoder {
                        sofa_file,
                        input_channels,
                        enable_optimization: *enable_optimization,
                        externalization: *externalization,
                        near_field_strength: *near_field_strength,
                    };
                }
            }
        }
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

        // Add default upmixer (5.1 = 6 channels)
        chain.add_plugin(&PluginType::Upmixer);
        assert_eq!(chain.output_channels(), 6);

        // Test that speaker_config is correctly mapped
        let idx = 0;
        if let Some(plugin) = chain.get_plugin_mut(idx) {
            plugin.settings = PluginSettings::Upmixer {
                speaker_config: "7.1".to_string(),
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 1.0,
                lfe_cutoff_hz: 120.0,
                stereo_width: 0.5,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                safety_cap_db: 3.0,
                decorrelation_mode: 0,
            };
        }
        assert_eq!(chain.output_channels(), 8);
    }

    #[test]
    fn test_binaural_decoder_channel_update() {
        let mut chain = PluginChain::new();

        // Add upmixer (5.1 = 6 channels) and binaural decoder
        chain.add_plugin(&PluginType::Upmixer);
        chain.add_plugin(&PluginType::BinauralDecoder);

        // Initially, BinauralDecoder should have default 6 channels (from default_for)
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 6); // Default value
            }
        }

        // Update binaural decoder channels
        chain.update_binaural_decoder_channels();

        // Now it should be correctly set to 6 (output of upmixer)
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 6);
            }
        }

        // Change upmixer to 7.1 (8 channels)
        if let Some(plugin) = chain.get_plugin_mut(0) {
            plugin.settings = PluginSettings::Upmixer {
                speaker_config: "7.1".to_string(),
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 1.0,
                lfe_cutoff_hz: 120.0,
                stereo_width: 0.5,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                safety_cap_db: 3.0,
                decorrelation_mode: 0,
            };
        }

        // Update binaural decoder channels
        chain.update_binaural_decoder_channels();

        // Now BinauralDecoder should have 8 input channels
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 8);
            }
        }

        // Remove the upmixer
        chain.remove_plugin(0);
        chain.update_binaural_decoder_channels();

        // Now BinauralDecoder should have 2 input channels (stereo)
        if let Some(plugin) = chain.get_plugin(0) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 2);
            }
        }
    }
}
