// ============================================================================
// A/B Comparison Plugin
// ============================================================================
//
// This plugin allows fair comparison between two audio processing chains
// with automatic loudness matching. Each path (A or B) can be:
// - A single plugin
// - A rack (linear chain of plugins)
// - A graph (full DAG topology)

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::host::{DawHost, GraphEdge};
use crate::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use crate::smoothing::Smoother;
use crate::{
    CompressorPlugin, CompressorPluginParams, DelayPlugin, DelayPluginParams, EqPlugin,
    EqPluginParams, GainPlugin, GainPluginParams, GatePlugin, GatePluginParams,
    InPlacePluginAdapter, LimiterPlugin, LimiterPluginParams,
};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for a processing path (A or B)
/// Can represent a single plugin, a rack (chain), or a graph
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum PathConfig {
    /// Empty path - pass-through
    #[default]
    None,
    /// Single plugin
    Plugin {
        plugin_type: String,
        #[serde(default)]
        parameters: serde_json::Value,
    },
    /// Linear chain of plugins (rack)
    Rack { plugins: Vec<PluginInRack> },
    /// Full graph with nodes and edges
    Graph {
        nodes: Vec<GraphNodeConfig>,
        edges: Vec<GraphEdgeConfig>,
    },
}

/// A plugin in a rack (chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInRack {
    pub plugin_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// A node in a graph configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeConfig {
    pub id: String,
    pub plugin_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// An edge in a graph configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdgeConfig {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub channel_map: Option<Vec<usize>>,
}

/// Mix mode for A/B comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MixMode {
    /// Continuous mix with potentiometer (-1.0 to +1.0)
    #[default]
    Potentiometer,
    /// Binary A/B switch
    Binary,
}

/// Loudness measurement type for auto-gain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoudnessType {
    /// 400ms momentary loudness (faster response)
    #[default]
    Momentary,
    /// 3 second short-term loudness (more stable)
    ShortTerm,
}

/// Configuration parameters for ABComparePlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABComparePluginParams {
    /// Configuration for path A
    #[serde(default)]
    pub path_a: PathConfig,

    /// Configuration for path B
    #[serde(default)]
    pub path_b: PathConfig,

    /// Mix mode (potentiometer or binary switch)
    #[serde(default)]
    pub mix_mode: MixMode,

    /// Mix value: -1.0 = pure A, 0.0 = 50/50, +1.0 = pure B
    #[serde(default)]
    pub mix: f32,

    /// Selected path for binary mode (0 = A, 1 = B)
    #[serde(default)]
    pub selected_path: i32,

    /// Bypass both A and B, output original input
    #[serde(default)]
    pub bypass: bool,

    /// Enable automatic loudness matching
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Loudness measurement type for auto-gain
    #[serde(default)]
    pub loudness_type: LoudnessType,

    /// Gain smoothing time in ms
    #[serde(default = "default_gain_smoothing_ms")]
    pub gain_smoothing_ms: f32,

    /// Maximum auto-gain correction in dB
    #[serde(default = "default_max_auto_gain_db")]
    pub max_auto_gain_db: f32,

    /// Mix transition time in ms
    #[serde(default = "default_mix_transition_ms")]
    pub mix_transition_ms: f32,
}

fn default_auto_gain_enabled() -> bool {
    true
}

fn default_gain_smoothing_ms() -> f32 {
    100.0
}

fn default_max_auto_gain_db() -> f32 {
    12.0
}

fn default_mix_transition_ms() -> f32 {
    50.0
}

impl Default for ABComparePluginParams {
    fn default() -> Self {
        Self {
            path_a: PathConfig::None,
            path_b: PathConfig::None,
            mix_mode: MixMode::Potentiometer,
            mix: 0.0,
            selected_path: 0,
            bypass: false,
            auto_gain_enabled: true,
            loudness_type: LoudnessType::Momentary,
            gain_smoothing_ms: 100.0,
            max_auto_gain_db: 12.0,
            mix_transition_ms: 50.0,
        }
    }
}

/// Data exposed by the A/B Compare plugin for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABCompareData {
    /// Current loudness of path A (LUFS)
    pub loudness_a_lufs: f64,

    /// Current loudness of path B (LUFS)
    pub loudness_b_lufs: f64,

    /// Current auto-gain applied to B (dB)
    pub auto_gain_db: f32,

    /// Peak level of path A (0.0 to 1.0+)
    pub peak_a: f64,

    /// Peak level of path B (after auto-gain) (0.0 to 1.0+)
    pub peak_b: f64,

    /// Current mix value (-1.0 to +1.0)
    pub current_mix: f32,

    /// Whether bypass is active
    pub bypass_active: bool,
}

// ============================================================================
// Plugin Factory
// ============================================================================

/// Create a plugin from type name and parameters
fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    num_channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    match plugin_type.to_lowercase().as_str() {
        "eq" => {
            let params: EqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid EQ params: {}", e))?;
            let plugin = EqPlugin::from_params(num_channels, sample_rate, params)?;
            Ok(Box::new(plugin))
        }
        "gain" => {
            let params: GainPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Gain params: {}", e))?;
            let plugin = GainPlugin::from_params(num_channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "compressor" => {
            let params: CompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Compressor params: {}", e))?;
            let plugin = CompressorPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "limiter" => {
            let params: LimiterPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Limiter params: {}", e))?;
            let plugin = LimiterPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "gate" => {
            let params: GatePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Gate params: {}", e))?;
            let plugin = GatePlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "delay" => {
            let params: DelayPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Delay params: {}", e))?;
            let plugin = DelayPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        _ => Err(format!("Unknown plugin type: {}", plugin_type)),
    }
}

/// Build a DawHost from a PathConfig
fn build_path_from_config(
    config: &PathConfig,
    num_channels: usize,
    sample_rate: u32,
) -> Result<DawHost, String> {
    let mut host = DawHost::new(num_channels, sample_rate);

    match config {
        PathConfig::None => {
            // Empty host = pass-through
        }
        PathConfig::Plugin {
            plugin_type,
            parameters,
        } => {
            let plugin = create_plugin(plugin_type, parameters, num_channels, sample_rate)?;
            host.add_plugin(plugin)?;
        }
        PathConfig::Rack { plugins } => {
            for p in plugins {
                let plugin =
                    create_plugin(&p.plugin_type, &p.parameters, num_channels, sample_rate)?;
                host.add_plugin(plugin)?;
            }
        }
        PathConfig::Graph { nodes, edges } => {
            let mut node_ids: HashMap<String, usize> = HashMap::new();

            // Add all nodes
            for node in nodes {
                let plugin =
                    create_plugin(&node.plugin_type, &node.parameters, num_channels, sample_rate)?;
                let id = host.add_node(node.id.clone(), plugin)?;
                node_ids.insert(node.id.clone(), id);
            }

            // Add all edges
            for edge in edges {
                let from_id = *node_ids
                    .get(&edge.from)
                    .ok_or_else(|| format!("Unknown node id in edge: {}", edge.from))?;
                let to_id = *node_ids
                    .get(&edge.to)
                    .ok_or_else(|| format!("Unknown node id in edge: {}", edge.to))?;

                let graph_edge = match &edge.channel_map {
                    Some(map) => GraphEdge::with_channels(from_id, to_id, map.clone()),
                    None => GraphEdge::new(from_id, to_id),
                };
                host.add_edge(graph_edge)?;
            }

            // Build the graph
            host.build()?;
        }
    }

    Ok(host)
}

// ============================================================================
// Main Plugin Struct
// ============================================================================

/// A/B Comparison Plugin
///
/// Allows fair comparison between two audio processing chains with automatic
/// loudness matching. Each path (A or B) can be a single plugin, a rack
/// (linear chain), or a full graph.
pub struct ABComparePlugin {
    // Configuration
    num_channels: usize,
    sample_rate: u32,

    // Processing paths - use DawHost for flexibility
    host_a: DawHost,
    host_b: DawHost,

    // Path configurations (stored for runtime changes)
    path_a_config: PathConfig,
    path_b_config: PathConfig,

    // Loudness monitors
    loudness_monitor_a: LoudnessMonitor,
    loudness_monitor_b: LoudnessMonitor,

    // State
    mix_mode: MixMode,
    mix: f32,
    mix_smoother: Smoother,
    selected_path: i32,
    bypass: bool,
    auto_gain_enabled: bool,
    loudness_type: LoudnessType,

    // Auto-gain
    auto_gain_db: f32,
    auto_gain_smoother: Smoother,
    max_auto_gain_db: f32,
    gain_smoothing_ms: f32,
    mix_transition_ms: f32,

    // Internal buffers
    buffer_a: Vec<f32>,
    buffer_b: Vec<f32>,

    // Cached loudness values
    last_loudness_a: f64,
    last_loudness_b: f64,
    last_peak_a: f64,
    last_peak_b: f64,
}

impl ABComparePlugin {
    /// Create a new A/B Compare plugin with default settings
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::from_params(num_channels, ABComparePluginParams::default())
    }

    /// Create from parameters
    pub fn from_params(num_channels: usize, params: ABComparePluginParams) -> Result<Self, String> {
        let sample_rate = 48000; // Will be updated in initialize()

        let host_a = build_path_from_config(&params.path_a, num_channels, sample_rate)?;
        let host_b = build_path_from_config(&params.path_b, num_channels, sample_rate)?;

        let loudness_monitor_a = LoudnessMonitor::new(num_channels as u32, sample_rate)?;
        let loudness_monitor_b = LoudnessMonitor::new(num_channels as u32, sample_rate)?;

        let mix_smoother = Smoother::new(params.mix, params.mix_transition_ms, sample_rate);
        let auto_gain_smoother = Smoother::new(0.0, params.gain_smoothing_ms, sample_rate);

        Ok(Self {
            num_channels,
            sample_rate,
            host_a,
            host_b,
            path_a_config: params.path_a,
            path_b_config: params.path_b,
            loudness_monitor_a,
            loudness_monitor_b,
            mix_mode: params.mix_mode,
            mix: params.mix,
            mix_smoother,
            selected_path: params.selected_path,
            bypass: params.bypass,
            auto_gain_enabled: params.auto_gain_enabled,
            loudness_type: params.loudness_type,
            auto_gain_db: 0.0,
            auto_gain_smoother,
            max_auto_gain_db: params.max_auto_gain_db,
            gain_smoothing_ms: params.gain_smoothing_ms,
            mix_transition_ms: params.mix_transition_ms,
            buffer_a: Vec::new(),
            buffer_b: Vec::new(),
            last_loudness_a: f64::NEG_INFINITY,
            last_loudness_b: f64::NEG_INFINITY,
            last_peak_a: 0.0,
            last_peak_b: 0.0,
        })
    }

    /// Rebuild path A from config
    fn rebuild_path_a(&mut self) -> Result<(), String> {
        self.host_a =
            build_path_from_config(&self.path_a_config, self.num_channels, self.sample_rate)?;
        Ok(())
    }

    /// Rebuild path B from config
    fn rebuild_path_b(&mut self) -> Result<(), String> {
        self.host_b =
            build_path_from_config(&self.path_b_config, self.num_channels, self.sample_rate)?;
        Ok(())
    }

    /// Get loudness value based on configured type
    fn get_loudness(&self, monitor: &LoudnessMonitor) -> f64 {
        let info = monitor.get_loudness();
        match self.loudness_type {
            LoudnessType::Momentary => info.momentary_lufs,
            LoudnessType::ShortTerm => info.shortterm_lufs,
        }
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

impl Plugin for ABComparePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "A/B Compare".to_string(),
            version: "1.0.0".to_string(),
            author: "SOTF".to_string(),
            description: "A/B comparison with automatic loudness matching".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("mix", "A/B Mix", 0.0, -1.0, 1.0)
                .with_description("Mix between A and B: -1.0 = A, 0.0 = 50/50, +1.0 = B")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("mix_mode", "Mix Mode", 0, 0, 1)
                .with_description("0 = Potentiometer (continuous), 1 = Binary (A/B switch)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("selected_path", "Selected Path", 0, 0, 1)
                .with_description("0 = A, 1 = B (only used in binary mode)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("bypass", "Bypass", false)
                .with_description("Bypass A/B processing, output original input")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", true)
                .with_description("Automatically match loudness between A and B")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("loudness_type", "Loudness Type", 0, 0, 1)
                .with_description("0 = Momentary (400ms), 1 = Short-term (3s)")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("max_auto_gain_db", "Max Auto Gain", 12.0, 0.0, 24.0)
                .with_description("Maximum loudness correction in dB")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("gain_smoothing_ms", "Gain Smoothing", 100.0, 10.0, 500.0)
                .with_description("Auto-gain smoothing time in milliseconds")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("mix_transition_ms", "Mix Transition", 50.0, 5.0, 500.0)
                .with_description("A/B transition smoothing time in milliseconds")
                .with_group("Timing")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_string(
                "path_a_config",
                "Path A Config",
                r#"{"type":"None"}"#.to_string(),
            )
            .with_description("JSON configuration for path A")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_string(
                "path_b_config",
                "Path B Config",
                r#"{"type":"None"}"#.to_string(),
            )
            .with_description("JSON configuration for path B")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        match id.0.as_str() {
            "mix" => {
                if let ParameterValue::Float(v) = value {
                    self.mix = v.clamp(-1.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                }
            }
            "mix_mode" => {
                if let ParameterValue::Int(v) = value {
                    self.mix_mode = if v == 0 {
                        MixMode::Potentiometer
                    } else {
                        MixMode::Binary
                    };
                }
            }
            "selected_path" => {
                if let ParameterValue::Int(v) = value {
                    self.selected_path = v.clamp(0, 1);
                    // Update mix target for binary mode
                    if self.mix_mode == MixMode::Binary {
                        let target = if self.selected_path == 0 { -1.0 } else { 1.0 };
                        self.mix_smoother.set_target(target);
                    }
                }
            }
            "bypass" => {
                if let ParameterValue::Bool(v) = value {
                    self.bypass = v;
                }
            }
            "auto_gain_enabled" => {
                if let ParameterValue::Bool(v) = value {
                    self.auto_gain_enabled = v;
                    if !v {
                        // Reset auto-gain when disabled
                        self.auto_gain_smoother.set_target(0.0);
                    }
                }
            }
            "loudness_type" => {
                if let ParameterValue::Int(v) = value {
                    self.loudness_type = if v == 0 {
                        LoudnessType::Momentary
                    } else {
                        LoudnessType::ShortTerm
                    };
                }
            }
            "max_auto_gain_db" => {
                if let ParameterValue::Float(v) = value {
                    self.max_auto_gain_db = v.clamp(0.0, 24.0);
                }
            }
            "gain_smoothing_ms" => {
                if let ParameterValue::Float(v) = value {
                    self.gain_smoothing_ms = v.clamp(10.0, 500.0);
                    self.auto_gain_smoother
                        .set_time(self.gain_smoothing_ms, self.sample_rate);
                }
            }
            "mix_transition_ms" => {
                if let ParameterValue::Float(v) = value {
                    self.mix_transition_ms = v.clamp(5.0, 500.0);
                    self.mix_smoother
                        .set_time(self.mix_transition_ms, self.sample_rate);
                }
            }
            "path_a_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path A config JSON: {}", e))?;
                    self.path_a_config = config;
                    self.rebuild_path_a()?;
                }
            }
            "path_b_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path B config JSON: {}", e))?;
                    self.path_b_config = config;
                    self.rebuild_path_b()?;
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id.0)),
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "mix" => Some(ParameterValue::Float(self.mix)),
            "mix_mode" => Some(ParameterValue::Int(match self.mix_mode {
                MixMode::Potentiometer => 0,
                MixMode::Binary => 1,
            })),
            "selected_path" => Some(ParameterValue::Int(self.selected_path)),
            "bypass" => Some(ParameterValue::Bool(self.bypass)),
            "auto_gain_enabled" => Some(ParameterValue::Bool(self.auto_gain_enabled)),
            "loudness_type" => Some(ParameterValue::Int(match self.loudness_type {
                LoudnessType::Momentary => 0,
                LoudnessType::ShortTerm => 1,
            })),
            "max_auto_gain_db" => Some(ParameterValue::Float(self.max_auto_gain_db)),
            "gain_smoothing_ms" => Some(ParameterValue::Float(self.gain_smoothing_ms)),
            "mix_transition_ms" => Some(ParameterValue::Float(self.mix_transition_ms)),
            "path_a_config" => {
                serde_json::to_string(&self.path_a_config)
                    .ok()
                    .map(ParameterValue::String)
            }
            "path_b_config" => {
                serde_json::to_string(&self.path_b_config)
                    .ok()
                    .map(ParameterValue::String)
            }
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Rebuild paths with new sample rate
        self.rebuild_path_a()?;
        self.rebuild_path_b()?;

        // Recreate loudness monitors
        self.loudness_monitor_a = LoudnessMonitor::new(self.num_channels as u32, sample_rate)
            .map_err(|e| format!("Failed to create loudness monitor A: {}", e))?;
        self.loudness_monitor_b = LoudnessMonitor::new(self.num_channels as u32, sample_rate)
            .map_err(|e| format!("Failed to create loudness monitor B: {}", e))?;

        // Reset smoothers with new sample rate
        self.mix_smoother = Smoother::new(self.mix, self.mix_transition_ms, sample_rate);
        self.auto_gain_smoother = Smoother::new(0.0, self.gain_smoothing_ms, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        // Reset hosts
        self.host_a.reset();
        self.host_b.reset();

        // Reset loudness monitors
        let _ = self.loudness_monitor_a.reset();
        let _ = self.loudness_monitor_b.reset();

        // Reset smoothers
        self.mix_smoother.reset(self.mix);
        self.auto_gain_smoother.reset(0.0);

        // Reset state
        self.auto_gain_db = 0.0;
        self.last_loudness_a = f64::NEG_INFINITY;
        self.last_loudness_b = f64::NEG_INFINITY;
        self.last_peak_a = 0.0;
        self.last_peak_b = 0.0;

        // Clear buffers
        self.buffer_a.clear();
        self.buffer_b.clear();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let expected_samples = context.num_frames * self.num_channels;

        // Verify input/output size
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }
        if output.len() != expected_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                expected_samples,
                output.len()
            ));
        }

        // Handle bypass
        if self.bypass {
            output.copy_from_slice(input);
            return Ok(());
        }

        // Resize buffers if needed
        if self.buffer_a.len() != expected_samples {
            self.buffer_a.resize(expected_samples, 0.0);
            self.buffer_b.resize(expected_samples, 0.0);
        }

        // Process path A
        self.host_a.process(input, &mut self.buffer_a)?;

        // Process path B
        self.host_b.process(input, &mut self.buffer_b)?;

        // Measure loudness
        self.loudness_monitor_a.add_frames(&self.buffer_a)?;
        self.loudness_monitor_b.add_frames(&self.buffer_b)?;

        let loudness_a = self.get_loudness(&self.loudness_monitor_a);
        let loudness_b = self.get_loudness(&self.loudness_monitor_b);

        self.last_loudness_a = loudness_a;
        self.last_loudness_b = loudness_b;

        // Get peak values
        let info_a = self.loudness_monitor_a.get_loudness();
        let info_b = self.loudness_monitor_b.get_loudness();
        self.last_peak_a = info_a.peak;
        self.last_peak_b = info_b.peak;

        // Calculate auto-gain
        if self.auto_gain_enabled && loudness_a.is_finite() && loudness_b.is_finite() {
            // Target: make B match A's loudness
            // gain_db = loudness_A - loudness_B
            // If B is louder (loudness_B > loudness_A), gain_db is negative (attenuate)
            // If B is quieter (loudness_B < loudness_A), gain_db is positive (boost)
            let target_gain_db = (loudness_a - loudness_b) as f32;
            let clamped_gain = target_gain_db.clamp(-self.max_auto_gain_db, self.max_auto_gain_db);
            self.auto_gain_smoother.set_target(clamped_gain);
        }

        // Determine target mix value
        let target_mix = match self.mix_mode {
            MixMode::Potentiometer => self.mix,
            MixMode::Binary => {
                if self.selected_path == 0 {
                    -1.0
                } else {
                    1.0
                }
            }
        };
        self.mix_smoother.set_target(target_mix);

        // Process sample-by-sample
        for frame in 0..context.num_frames {
            // Get smoothed values
            let current_mix = self.mix_smoother.next();
            let current_gain_db = self.auto_gain_smoother.next();
            let gain_linear = 10.0_f32.powf(current_gain_db / 20.0);

            // Equal-power crossfade
            // mix: -1 = pure A, +1 = pure B
            // Convert to 0..1 range for angle calculation
            let mix_01 = (current_mix + 1.0) / 2.0; // 0 = A, 1 = B
            let angle = mix_01 * std::f32::consts::FRAC_PI_2; // 0 to PI/2
            let gain_a = angle.cos();
            let gain_b = angle.sin();

            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let sample_a = self.buffer_a[idx];
                let sample_b = self.buffer_b[idx] * gain_linear;
                output[idx] = sample_a * gain_a + sample_b * gain_b;
            }
        }

        self.auto_gain_db = self.auto_gain_smoother.current();

        Ok(())
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let data = ABCompareData {
            loudness_a_lufs: self.last_loudness_a,
            loudness_b_lufs: self.last_loudness_b,
            auto_gain_db: self.auto_gain_db,
            peak_a: self.last_peak_a,
            peak_b: self.last_peak_b,
            current_mix: self.mix_smoother.current(),
            bypass_active: self.bypass,
        };
        Some(Arc::new(data))
    }

    fn latency_samples(&self) -> usize {
        // Total latency is the max of both paths
        let latency_a = self.host_a.total_latency_samples();
        let latency_b = self.host_b.total_latency_samples();
        latency_a.max(latency_b)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ab_compare_creation() {
        let plugin = ABComparePlugin::new(2).unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_bypass_mode() {
        let mut plugin = ABComparePlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();
        plugin
            .set_parameter(ParameterId("bypass".to_string()), ParameterValue::Bool(true))
            .unwrap();

        let input = vec![1.0, 0.5, 0.8, 0.3]; // 2 frames, 2 channels
        let mut output = vec![0.0; 4];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 2,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(input, output, "Bypass should pass through unchanged");
    }

    #[test]
    fn test_path_config_serialization() {
        // Test None
        let none_config = PathConfig::None;
        let json = serde_json::to_string(&none_config).unwrap();
        assert!(json.contains("None"));

        // Test Plugin
        let plugin_config = PathConfig::Plugin {
            plugin_type: "EQ".to_string(),
            parameters: serde_json::json!({"filters": []}),
        };
        let json = serde_json::to_string(&plugin_config).unwrap();
        assert!(json.contains("Plugin"));
        assert!(json.contains("EQ"));

        // Test Rack
        let rack_config = PathConfig::Rack {
            plugins: vec![PluginInRack {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": -6.0}),
            }],
        };
        let json = serde_json::to_string(&rack_config).unwrap();
        assert!(json.contains("Rack"));

        // Test deserialization
        let deserialized: PathConfig = serde_json::from_str(&json).unwrap();
        match deserialized {
            PathConfig::Rack { plugins } => {
                assert_eq!(plugins.len(), 1);
                assert_eq!(plugins[0].plugin_type, "gain");
            }
            _ => panic!("Expected Rack"),
        }
    }

    #[test]
    fn test_mix_pure_a() {
        let params = ABComparePluginParams {
            path_a: PathConfig::Plugin {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": -6.0}),
            },
            path_b: PathConfig::Plugin {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": 6.0}),
            },
            mix: -1.0, // Pure A
            auto_gain_enabled: false,
            ..Default::default()
        };

        let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
        plugin.initialize(48000).unwrap();

        // Process multiple times to let smoothers settle
        let input = vec![1.0; 4800 * 2]; // 100ms at 48kHz
        let mut output = vec![0.0; 4800 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };

        for _ in 0..5 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        // At mix=-1 (pure A) with -6dB gain, output should be ~0.5
        // Check the last samples (smoothers should have settled)
        let last_sample = output[output.len() - 1];
        assert!(
            (last_sample - 0.5).abs() < 0.1,
            "Expected ~0.5, got {}",
            last_sample
        );
    }

    #[test]
    fn test_mix_pure_b() {
        let params = ABComparePluginParams {
            path_a: PathConfig::Plugin {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": 6.0}),
            },
            path_b: PathConfig::Plugin {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": -6.0}),
            },
            mix: 1.0, // Pure B
            auto_gain_enabled: false,
            ..Default::default()
        };

        let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
        plugin.initialize(48000).unwrap();

        // Process multiple times to let smoothers settle
        let input = vec![1.0; 4800 * 2];
        let mut output = vec![0.0; 4800 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };

        for _ in 0..5 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        // At mix=+1 (pure B) with -6dB gain, output should be ~0.5
        let last_sample = output[output.len() - 1];
        assert!(
            (last_sample - 0.5).abs() < 0.1,
            "Expected ~0.5, got {}",
            last_sample
        );
    }

    #[test]
    fn test_binary_mode() {
        let params = ABComparePluginParams {
            mix_mode: MixMode::Binary,
            selected_path: 0, // A
            ..Default::default()
        };

        let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
        plugin.initialize(48000).unwrap();

        // Switch to B
        plugin
            .set_parameter(
                ParameterId("selected_path".to_string()),
                ParameterValue::Int(1),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId("selected_path".to_string()));
        assert_eq!(value, Some(ParameterValue::Int(1)));
    }

    #[test]
    fn test_multichannel_support() {
        // Test with 5 channels
        let mut plugin = ABComparePlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![0.5; 5 * 1024]; // 1024 frames, 5 channels
        let mut output = vec![0.0; 5 * 1024];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Pass-through with no plugins should work
        // Note: smoothers may affect output slightly
    }

    #[test]
    fn test_reset() {
        let mut plugin = ABComparePlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let input = vec![1.0; 1000 * 2];
        let mut output = vec![0.0; 1000 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1000,
        };
        plugin.process(&input, &mut output, &context).unwrap();

        // Reset should not panic
        plugin.reset();

        // Loudness should be reset
        let data = plugin.get_data().unwrap();
        let ab_data = data.downcast_ref::<ABCompareData>().unwrap();
        assert!(
            ab_data.loudness_a_lufs.is_infinite() || ab_data.loudness_a_lufs < -60.0,
            "Loudness should be reset"
        );
    }

    #[test]
    fn test_rack_configuration() {
        let params = ABComparePluginParams {
            path_a: PathConfig::Rack {
                plugins: vec![
                    PluginInRack {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({"gain_db": -3.0}),
                    },
                    PluginInRack {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({"gain_db": -3.0}),
                    },
                ],
            },
            mix: -1.0,
            auto_gain_enabled: false,
            ..Default::default()
        };

        let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
        plugin.initialize(48000).unwrap();

        // Two -3dB gains = -6dB total
        let input = vec![1.0; 4800 * 2];
        let mut output = vec![0.0; 4800 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };

        for _ in 0..5 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        let last_sample = output[output.len() - 1];
        assert!(
            (last_sample - 0.5).abs() < 0.1,
            "Two -3dB gains should give ~0.5, got {}",
            last_sample
        );
    }

    #[test]
    fn test_get_data() {
        let mut plugin = ABComparePlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let input = vec![0.5; 4800 * 2];
        let mut output = vec![0.0; 4800 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };
        plugin.process(&input, &mut output, &context).unwrap();

        let data = plugin.get_data().unwrap();
        let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

        // Verify data structure is populated
        assert!(!ab_data.bypass_active);
    }

    #[test]
    fn test_runtime_path_change() {
        let mut plugin = ABComparePlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Change path A at runtime
        let new_config = r#"{"type": "Plugin", "plugin_type": "gain", "parameters": {"gain_db": -12.0}}"#;
        plugin
            .set_parameter(
                ParameterId("path_a_config".to_string()),
                ParameterValue::String(new_config.to_string()),
            )
            .unwrap();

        // Verify it works
        let input = vec![1.0; 1024 * 2];
        let mut output = vec![0.0; 1024 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();
    }
}
