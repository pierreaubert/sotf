// ============================================================================
// Plugin Chain — ordered list of plugins with add/remove/reorder operations
// ============================================================================

use super::{
    ChannelConflict, Plugin, PluginSettings, PluginType,
    matrix::{resize_matrix, upmixer_output_channels},
};
use crate::engine::PluginConfig;
use serde::{Deserialize, Serialize};

/// Versioned wrapper for plugin presets (used for saving)
#[derive(Debug, Clone, Serialize)]
struct PluginPreset {
    version: u32,
    plugins: Vec<Plugin>,
}

/// Lenient versioned wrapper for plugin presets (used for loading).
/// Plugins are raw JSON values so that individual plugin deserialization
/// failures don't reject the entire file.
#[derive(Debug, Clone, Deserialize)]
struct PluginPresetRaw {
    #[serde(default = "default_plugin_preset_version")]
    version: u32,
    plugins: Vec<serde_json::Value>,
}

fn default_plugin_preset_version() -> u32 {
    2
}

/// Extract a human-readable plugin type name from a raw JSON plugin value.
fn plugin_type_from_raw(raw: &serde_json::Value) -> String {
    // PluginSettings is an externally tagged enum, so the settings field is
    // either a string like "LoudnessMonitor" or an object like {"Gain": {...}}
    if let Some(settings) = raw.get("settings") {
        if let Some(s) = settings.as_str() {
            return s.to_string();
        }
        if let Some(obj) = settings.as_object() {
            if let Some(key) = obj.keys().next() {
                return key.clone();
            }
        }
    }
    "unknown".to_string()
}

#[derive(Debug, Default, Clone)]
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

    /// Add a permanent plugin that cannot be removed
    pub fn add_permanent_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(id, plugin_type));
        id
    }

    /// Add a permanent plugin that starts disabled (passthrough)
    pub fn add_permanent_disabled_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let mut plugin = Plugin::new_permanent(id, plugin_type);
        plugin.enabled = false;
        self.plugins.push(plugin);
        id
    }

    /// Create a default rack with permanent Input Monitor, ReplayGain, Matrix, and Output Monitor
    pub fn with_default_rack() -> Self {
        let mut chain = Self::new();
        // Input monitor (permanent) - monitors input signal
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);
        // ReplayGain (permanent) - applies track/album replay gain correction
        chain.add_permanent_disabled_plugin(&PluginType::Gain);
        // Matrix (permanent) - channel routing
        chain.add_permanent_plugin(&PluginType::Matrix);
        // Output monitor (permanent) - monitors output signal
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);
        chain
    }

    /// Ensure the default rack (input monitor, replay gain, matrix, output monitor) is present.
    /// Adds missing permanent plugins without disturbing existing user plugins.
    /// Call this after loading a preset to guarantee the rack structure.
    pub fn ensure_default_rack(&mut self) {
        let has_permanent_lm = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        let has_permanent_matrix = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::Matrix));
        let has_permanent_gain = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain));

        if has_permanent_lm && has_permanent_matrix && has_permanent_gain {
            // Check we have at least two permanent LoudnessMonitors (input + output)
            let lm_count = self
                .plugins
                .iter()
                .filter(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                .count();
            if lm_count >= 2 {
                return; // Rack is already complete
            }
        }

        // Rebuild: collect user (non-permanent) plugins, then wrap them in the default rack
        let user_plugins: Vec<Plugin> = self.plugins.drain(..).filter(|p| !p.permanent).collect();

        // Build fresh rack
        let input_id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(
            input_id,
            &PluginType::LoudnessMonitor,
        ));

        // ReplayGain (permanent, starts disabled)
        let gain_id = self.next_id;
        self.next_id += 1;
        let mut gain_plugin = Plugin::new_permanent(gain_id, &PluginType::Gain);
        gain_plugin.enabled = false;
        self.plugins.push(gain_plugin);

        // Insert user plugins between replay gain and matrix
        self.plugins.extend(user_plugins);

        let matrix_id = self.next_id;
        self.next_id += 1;
        self.plugins
            .push(Plugin::new_permanent(matrix_id, &PluginType::Matrix));

        let output_id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(
            output_id,
            &PluginType::LoudnessMonitor,
        ));

        log::info!("Ensured default rack: {} plugins total", self.plugins.len());
    }

    /// Find the index where user plugins should be inserted (before Matrix)
    /// Returns the index of the Matrix plugin, or the first permanent plugin after user plugins
    pub fn user_plugin_insert_index(&self) -> usize {
        // Find the Matrix plugin - user plugins go before it
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if plugin.plugin_type() == PluginType::Matrix && plugin.is_permanent() {
                return idx;
            }
        }
        // Fallback: find processing insert index
        self.find_processing_insert_index()
    }

    /// Set the replay gain value on the permanent Gain plugin.
    /// When `gain_db` is `Some`, the plugin is enabled with the given gain.
    /// When `None`, the plugin is disabled (passthrough).
    pub fn set_replay_gain(&mut self, gain_db: Option<f64>) {
        if let Some(plugin) = self
            .plugins
            .iter_mut()
            .find(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain))
        {
            match gain_db {
                Some(db) => {
                    plugin.enabled = true;
                    plugin.settings = PluginSettings::Gain {
                        channels: match &plugin.settings {
                            PluginSettings::Gain { channels, .. } => *channels,
                            _ => 2,
                        },
                        gain_db: db,
                        smoothing_ms: match &plugin.settings {
                            PluginSettings::Gain { smoothing_ms, .. } => *smoothing_ms,
                            _ => {
                                use sotf_plugins::param_specs::{find_by_key, gain};
                                find_by_key(gain::PARAMS, "smoothing_ms").default_f64()
                            }
                        },
                    };
                }
                None => {
                    plugin.enabled = false;
                }
            }
        }
    }

    /// Read the current replay gain value from the permanent Gain plugin.
    /// Returns `None` if the plugin is disabled or not found.
    pub fn replay_gain_db(&self) -> Option<f64> {
        self.plugins
            .iter()
            .find(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain))
            .and_then(|p| {
                if p.enabled {
                    match &p.settings {
                        PluginSettings::Gain { gain_db, .. } => Some(*gain_db),
                        _ => None,
                    }
                } else {
                    None
                }
            })
    }

    pub fn remove_plugin(&mut self, index: usize) -> Option<Plugin> {
        if index < self.plugins.len() {
            // Don't remove permanent plugins
            if self.plugins[index].is_permanent() {
                return None;
            }
            Some(self.plugins.remove(index))
        } else {
            None
        }
    }

    /// Check if a plugin at the given index can be removed
    pub fn can_remove_plugin(&self, index: usize) -> bool {
        if let Some(plugin) = self.plugins.get(index) {
            !plugin.is_permanent()
        } else {
            false
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

    pub fn plugins_mut(&mut self) -> &mut [Plugin] {
        &mut self.plugins
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
        if from >= self.plugins.len()
            || to >= self.plugins.len()
            || self.plugins[from].is_permanent()
            || self.plugins[to].is_permanent()
        {
            return;
        }
        let plugin = self.plugins.remove(from);
        self.plugins.insert(to, plugin);
    }

    /// Check if a plugin at the given index can be moved in the given direction
    pub fn can_move_plugin_up(&self, index: usize) -> bool {
        index > 0
            && index < self.plugins.len()
            && !self.plugins[index].is_permanent()
            && !self.plugins[index - 1].is_permanent()
    }

    /// Check if a plugin at the given index can be moved down
    pub fn can_move_plugin_down(&self, index: usize) -> bool {
        index < self.plugins.len().saturating_sub(1)
            && !self.plugins[index].is_permanent()
            && !self.plugins[index + 1].is_permanent()
    }

    /// Insert a plugin at a specific index
    pub fn insert_plugin(&mut self, index: usize, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let insert_idx = index.min(self.plugins.len());
        self.plugins
            .insert(insert_idx, Plugin::new(id, plugin_type));
        id
    }

    /// Find the index of the first plugin of a given type
    pub fn find_plugin_index(&self, plugin_type: &PluginType) -> Option<usize> {
        self.plugins
            .iter()
            .position(|p| p.plugin_type() == *plugin_type)
    }

    /// Returns true if the plugin at `index` is the input monitor
    /// (first permanent LoudnessMonitor in the chain)
    pub fn is_input_monitor(&self, index: usize) -> bool {
        let first_permanent_lm = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        first_permanent_lm == Some(index)
    }

    /// Returns true if the plugin at `index` is the output monitor
    /// (last permanent LoudnessMonitor in the chain, distinct from the input monitor)
    pub fn is_output_monitor(&self, index: usize) -> bool {
        let last_permanent_lm = self
            .plugins
            .iter()
            .enumerate()
            .rev()
            .find(|(_, p)| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
            .map(|(i, _)| i);
        let first_permanent_lm = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        // Only true if the last permanent LM is different from the first (i.e., there are at least two)
        last_permanent_lm == Some(index) && last_permanent_lm != first_permanent_lm
    }

    /// Check if the chain has an enabled spectrum analyzer plugin
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.plugins
            .iter()
            .any(|p| p.enabled && matches!(p.settings, PluginSettings::SpectrumAnalyzer { .. }))
    }

    /// Find the insertion index for a new processing plugin (before monitoring plugins)
    pub fn find_processing_insert_index(&self) -> usize {
        // Find the first monitoring plugin
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if plugin.plugin_type().is_monitoring() {
                return idx;
            }
        }
        // No monitoring plugins, insert at end
        self.plugins.len()
    }

    /// Map a UI plugin index (from self.plugins) to the index in the engine's processing chain.
    /// Returns None if the plugin is disabled (not in engine).
    ///
    /// The engine order is:
    /// 1. First LoudnessMonitor (input monitor) - index 0
    /// 2. Processing plugins - indices 1..N
    /// 3. Other monitoring plugins (subsequent LoudnessMonitors, Spectrum, etc.) - at the end
    pub fn get_engine_index(&self, ui_index: usize) -> Option<usize> {
        let target_plugin = self.plugins.get(ui_index)?;
        if !target_plugin.enabled || target_plugin.suspended {
            return None;
        }

        // Determine if this is the first permanent LoudnessMonitor (input monitor)
        let first_permanent_loudness_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        let target_is_first_loudness = first_permanent_loudness_idx == Some(ui_index)
            && matches!(target_plugin.plugin_type(), PluginType::LoudnessMonitor);

        if target_is_first_loudness {
            // First permanent LoudnessMonitor is always at engine index 0
            return Some(0);
        }

        let target_is_monitor = target_plugin.plugin_type().is_monitoring();

        // Check if there's an enabled input monitor (counts toward engine offset)
        // An input monitor exists in the engine if the first permanent one is enabled.
        let has_input_monitor = first_permanent_loudness_idx
            .and_then(|idx| self.plugins.get(idx))
            .map(|p| p.enabled && !p.suspended)
            .unwrap_or(false);
        let input_monitor_offset = if has_input_monitor { 1 } else { 0 };

        if !target_is_monitor {
            // Target is a processing plugin.
            // Engine index is input_monitor_offset + count of enabled processing plugins before it.
            let mut engine_idx = input_monitor_offset;
            for (i, p) in self.plugins.iter().enumerate() {
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && !p.suspended && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        } else {
            // Target is a monitoring plugin (but not first permanent LoudnessMonitor).
            // Engine index is input_monitor_offset + (all enabled processing plugins) + (count of enabled monitors before it, excluding first permanent LoudnessMonitor).

            // 1. Count all enabled processing plugins
            let mut engine_idx = input_monitor_offset;
            for p in &self.plugins {
                if p.enabled && !p.suspended && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }

            // 2. Count enabled monitors until we hit target (skip first permanent LoudnessMonitor)
            for (i, p) in self.plugins.iter().enumerate() {
                if Some(i) == first_permanent_loudness_idx {
                    continue; // Skip first permanent LoudnessMonitor
                }
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && !p.suspended && p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        }

        None
    }

    /// Get the engine index of the input loudness monitor (first permanent LoudnessMonitor).
    pub fn input_monitor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the output loudness monitor
    /// (last permanent LoudnessMonitor, if distinct from input).
    pub fn output_monitor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .enumerate()
            .rev()
            .find(|(_, p)| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
            .map(|(i, _)| i)?;
        // Only valid if different from the input monitor
        let first = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        if first == Some(ui_idx) {
            return None;
        }
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the permanent Matrix plugin.
    pub fn matrix_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::Matrix))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the first enabled spectrum analyzer.
    pub fn spectrum_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.enabled && matches!(p.plugin_type(), PluginType::SpectrumAnalyzer))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the first enabled compressor.
    pub fn compressor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.enabled && matches!(p.plugin_type(), PluginType::Compressor))?;
        self.get_engine_index(ui_idx)
    }

    pub fn to_plugin_configs(&self, sample_rate: f64) -> Vec<PluginConfig> {
        // Separate plugins into three categories:
        // 1. Input monitor (the first permanent LoudnessMonitor)
        // 2. Processing plugins - transform the audio
        // 3. Output analyzers (subsequent LoudnessMonitors, Spectrum, etc.)
        let mut input_monitor: Option<PluginConfig> = None;
        let mut processing_plugins = Vec::new();
        let mut analyzer_plugins = Vec::new();

        // Identify which plugin should be the input monitor.
        // It's the first permanent LoudnessMonitor.
        let first_permanent_loudness_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));

        for (idx, plugin) in self.plugins.iter().enumerate() {
            if let Some(config) = plugin.to_plugin_config(sample_rate) {
                match plugin.plugin_type() {
                    PluginType::LoudnessMonitor => {
                        if Some(idx) == first_permanent_loudness_idx {
                            input_monitor = Some(config);
                        } else {
                            analyzer_plugins.push(config);
                        }
                    }
                    // Other analyzer plugins always go at the end
                    PluginType::SpectrumAnalyzer | PluginType::ChannelMuteSolo => {
                        analyzer_plugins.push(config);
                    }
                    // Processing plugins maintain their order
                    _ => {
                        processing_plugins.push(config);
                    }
                }
            }
        }

        // Concatenate: input monitor, then processing, then output analyzers
        let mut result = Vec::new();
        if let Some(monitor) = input_monitor {
            result.push(monitor);
        }
        result.extend(processing_plugins);
        result.extend(analyzer_plugins);
        result
    }

    /// Get the speaker configuration ID from the last enabled upmixer/binaural decoder
    /// Returns None if no channel-changing plugin is active
    pub fn output_speaker_config(&self) -> Option<&str> {
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    return Some(speaker_config.as_str());
                }
                PluginSettings::BinauralDecoder { .. } => {
                    return Some("2.0");
                }
                _ => continue,
            }
        }
        None
    }

    /// Get the speaker configuration string active at a given plugin index
    /// Walks forward through the chain, tracking config changes from upmixer/binaural/downmix/mono-to-stereo
    pub fn speaker_config_at_index(&self, target_index: usize) -> Option<String> {
        let mut config: Option<String> = None;
        for (i, plugin) in self.plugins.iter().enumerate() {
            if i >= target_index {
                break;
            }
            if !plugin.enabled {
                continue;
            }
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AmbisonicsDecoder { target_layout: speaker_config, .. } => {
                    config = Some(speaker_config.clone());
                }
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => {
                    config = Some("2.0".to_string());
                }
                _ => {}
            }
        }
        config
    }

    pub fn output_channels(&self) -> usize {
        self.output_channels_for_input(2)
    }

    /// Returns the output channel count of the plugin chain given the input channel count.
    /// If no channel-changing plugin is found, the input channel count passes through unchanged.
    pub fn output_channels_for_input(&self, input_channels: usize) -> usize {
        // Walk backwards through the chain to find the last channel-count-changing plugin
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled || plugin.suspended {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    return upmixer_output_channels(speaker_config);
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    return upmixer_output_channels(target_layout);
                }
                PluginSettings::BinauralDecoder { .. } => {
                    return 2;
                }
                PluginSettings::Downmix { .. } => {
                    return 2;
                }
                PluginSettings::MonoToStereo { .. } => {
                    return 2;
                }
                PluginSettings::Matrix {
                    output_channels, ..
                } => {
                    return *output_channels;
                }
                _ => continue,
            }
        }

        // No channel-changing plugin found, input channels pass through
        input_channels
    }

    /// Adapt the matrix plugin to match the file's channel count.
    /// When a multichannel file is loaded but the matrix was configured for stereo
    /// (or vice versa), this resizes the matrix and its channel states to match.
    /// Should be called before `to_plugin_configs()` when the file channel count is known.
    pub fn adapt_matrix_to_input(&mut self, file_channels: usize) {
        let mut running_channels = file_channels;
        for plugin in &mut self.plugins {
            if !plugin.enabled || plugin.suspended {
                continue;
            }
            // Track channel changes from plugins before the matrix
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    running_channels = upmixer_output_channels(speaker_config);
                    continue;
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    running_channels = upmixer_output_channels(target_layout);
                    continue;
                }
                PluginSettings::BinauralDecoder { .. } => {
                    running_channels = 2;
                    continue;
                }
                PluginSettings::Downmix { .. } => {
                    running_channels = 2;
                    continue;
                }
                PluginSettings::MonoToStereo { .. } => {
                    running_channels = 2;
                    continue;
                }
                _ => {}
            }
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                channel_states,
            } = &mut plugin.settings
            {
                if *input_channels != running_channels {
                    log::info!(
                        "[PluginChain] Adapting matrix from {}x{} to {}x{} (file={}, after chain)",
                        input_channels,
                        output_channels,
                        running_channels,
                        running_channels,
                        file_channels
                    );
                    resize_matrix(
                        matrix,
                        *input_channels,
                        *output_channels,
                        running_channels,
                        running_channels,
                    );
                    *input_channels = running_channels;
                    *output_channels = running_channels;
                    channel_states.resize(running_channels, sotf_plugins::ChannelState::default());
                }
                break; // Only adapt the first enabled matrix
            }
        }
    }

    /// Find all enabled (non-suspended) plugins incompatible with the given input channel count.
    /// Walks the chain tracking running channel count through channel-changing plugins.
    pub fn find_channel_conflicts(&self, input_channels: usize) -> Vec<ChannelConflict> {
        let mut conflicts = Vec::new();
        let mut running_channels = input_channels;

        for (index, plugin) in self.plugins.iter().enumerate() {
            if !plugin.enabled || plugin.suspended {
                continue;
            }

            if let Some(required) = plugin.settings.required_input_channels()
                && required != running_channels
            {
                conflicts.push(ChannelConflict {
                    index,
                    plugin_type: plugin.plugin_type(),
                    required_channels: required,
                    actual_channels: running_channels,
                });
                continue;
            }

            // Track channel changes through the chain
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    running_channels = upmixer_output_channels(speaker_config);
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    running_channels = upmixer_output_channels(target_layout);
                }
                PluginSettings::BinauralDecoder { .. } => {
                    running_channels = 2;
                }
                PluginSettings::Downmix { .. } => {
                    running_channels = 2;
                }
                PluginSettings::MonoToStereo { .. } => {
                    running_channels = 2;
                }
                PluginSettings::Matrix {
                    output_channels, ..
                } => {
                    running_channels = *output_channels;
                }
                PluginSettings::BandSplit { .. } => {
                    running_channels *= 2;
                }
                PluginSettings::BandMerge { bands, .. } => {
                    running_channels /= if *bands > 0 { *bands } else { 2 };
                }
                _ => {}
            }
        }

        conflicts
    }

    /// Suspend the plugins at the given indices (set suspended = true).
    pub fn suspend_plugins(&mut self, indices: &[usize]) {
        for &idx in indices {
            if let Some(plugin) = self.plugins.get_mut(idx) {
                plugin.suspended = true;
            }
        }
    }

    /// Clear all suspensions (set suspended = false on all plugins).
    pub fn clear_suspensions(&mut self) {
        for plugin in &mut self.plugins {
            plugin.suspended = false;
        }
    }

    /// Returns true if any plugin is currently suspended.
    pub fn has_suspensions(&self) -> bool {
        self.plugins.iter().any(|p| p.suspended)
    }

    /// Save the plugin chain to a JSON file
    ///
    /// # Arguments
    /// * `presets_dir` - Directory to save the preset file
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the extension is not .json or if saving fails
    pub fn save_to_file(
        &self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Load the plugin chain from a JSON file.
    ///
    /// Individual plugins that fail to deserialize are skipped (not fatal).
    /// The returned `Vec<String>` contains warnings about skipped plugins.
    ///
    /// # Arguments
    /// * `presets_dir` - Directory containing the preset files
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * `Ok(warnings)` — chain loaded, possibly with skipped plugins listed in warnings
    /// * `Err` — file not found or entire JSON is unparseable
    pub fn load_from_file(
        &mut self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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

        let full_path = presets_dir.join(&final_filename);
        log::debug!("Full path: {}", full_path.display());

        // Load from file
        let json = std::fs::read_to_string(&full_path)?;
        log::debug!("Read {} bytes from file", json.len());

        // Parse as raw JSON so individual plugin failures don't reject the file.
        let raw_preset: PluginPresetRaw = match serde_json::from_str(&json) {
            Ok(p) => p,
            Err(_) => {
                // Fall back to loading as legacy format (direct JSON array)
                log::info!("Loading legacy plugin preset format (no version field)");
                let plugins: Vec<serde_json::Value> = serde_json::from_str(&json)?;
                PluginPresetRaw {
                    version: 0, // Mark as legacy
                    plugins,
                }
            }
        };

        // Deserialize each plugin individually, skipping failures
        let mut loaded_plugins = Vec::new();
        let mut warnings = Vec::new();

        for (i, raw) in raw_preset.plugins.iter().enumerate() {
            match serde_json::from_value::<Plugin>(raw.clone()) {
                Ok(plugin) => loaded_plugins.push(plugin),
                Err(e) => {
                    let ptype = plugin_type_from_raw(raw);
                    let msg = format!("Plugin {} ('{}') skipped: {}", i, ptype, e);
                    log::warn!("{}", msg);
                    warnings.push(msg);
                }
            }
        }

        // Build a typed preset for migration
        let mut preset = PluginPreset {
            version: raw_preset.version,
            plugins: loaded_plugins,
        };

        // Check if migration is needed
        const LATEST_VERSION: u32 = 2;
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
            self.save_to_file(presets_dir, &final_filename)?;

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

        // Strip spurious LoudnessMonitor at the edges — the default rack
        // already includes input (first) and output (last) monitors, so
        // presets saved with those included would double them up.
        while self.plugins.first().is_some_and(|p| {
            matches!(p.plugin_type(), PluginType::LoudnessMonitor) && !p.permanent
        }) {
            log::info!("Removing spurious leading LoudnessMonitor from loaded preset");
            self.plugins.remove(0);
        }
        while self.plugins.last().is_some_and(|p| {
            matches!(p.plugin_type(), PluginType::LoudnessMonitor) && !p.permanent
        }) {
            log::info!("Removing spurious trailing LoudnessMonitor from loaded preset");
            self.plugins.pop();
        }

        // Ensure the default rack (input monitor, matrix, output monitor) is present
        // even if the saved preset predates the rack system.
        self.ensure_default_rack();

        log::info!(
            "Loaded plugin chain from {} ({} plugins, {} skipped)",
            full_path.display(),
            self.plugins.len(),
            warnings.len()
        );
        Ok(warnings)
    }

    /// Apply all necessary migrations to bring a plugin preset to the latest version
    fn migrate_preset(
        mut preset: PluginPreset,
    ) -> Result<PluginPreset, Box<dyn std::error::Error>> {
        const LATEST_VERSION: u32 = 2;

        // Apply migrations sequentially
        while preset.version < LATEST_VERSION {
            match preset.version {
                // Migration from legacy format (version 0) to version 1
                0 => {
                    log::info!("Applying plugin preset migration: v0 (legacy) -> v1");
                    preset.version = 1;
                }

                // v1 -> v2: Choice params (speaker_config, etc.) stored as integer
                // indices are now stored as strings. The deserialize_with attribute
                // on the fields handles the conversion during loading; this migration
                // just bumps the version so the preset is re-saved with strings.
                1 => {
                    log::info!("Applying plugin preset migration: v1 -> v2 (choice params as strings)");
                    preset.version = 2;
                }

                v => {
                    return Err(format!("Unknown plugin preset version: {}", v).into());
                }
            }
        }

        Ok(preset)
    }

    /// Update input channels for plugins that depend on the output of previous plugins (BinauralDecoder, Matrix)
    /// This should be called after any plugin chain modification (add, remove, move, toggle)
    pub fn update_channel_dependent_plugins(&mut self) {
        let mut current_channels = 2; // Start with stereo

        for i in 0..self.plugins.len() {
            // Update plugins that depend on input channels
            // We use a temporary clone to check if update is needed to avoid borrow checker issues if we modify in place
            // actually we can modify in place if we match &mut settings

            let mut updated_settings = None;

            match &self.plugins[i].settings {
                PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters,
                    tdf2,
                } => {
                    if *channels != current_channels {
                        // If per-channel filters exist but don't match the new channel
                        // count, disable per-channel mode (the per-channel config was
                        // for a different layout and can't be applied here).
                        let ch_filters_match = channel_filters
                            .as_ref()
                            .map_or(true, |cf| cf.len() == current_channels);
                        let (new_channel_filters, new_per_channel_mode) =
                            if *per_channel_mode && !ch_filters_match {
                                (None, false)
                            } else {
                                (channel_filters.clone(), *per_channel_mode)
                            };
                        updated_settings = Some(PluginSettings::EQ {
                            channels: current_channels,
                            filters: filters.clone(),
                            channel_filters: new_channel_filters,
                            per_channel_mode: new_per_channel_mode,
                            max_filters: *max_filters,
                            tdf2: *tdf2,
                        });
                    }
                }
                PluginSettings::Gain { channels, gain_db, smoothing_ms } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::Gain {
                            channels: current_channels,
                            gain_db: *gain_db,
                            smoothing_ms: *smoothing_ms,
                        });
                    }
                }
                PluginSettings::BinauralDecoder {
                    sofa_file,
                    input_channels,
                    enable_optimization,
                    externalization,
                    near_field_strength,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::BinauralDecoder {
                            sofa_file: sofa_file.clone(),
                            input_channels: current_channels,
                            enable_optimization: *enable_optimization,
                            externalization: *externalization,
                            near_field_strength: *near_field_strength,
                        });
                    }
                }
                PluginSettings::Matrix {
                    input_channels,
                    output_channels,
                    matrix,
                    channel_states,
                } => {
                    if *input_channels != current_channels {
                        // Resize matrix to match new input channels (square matrix)
                        // allowing it to act as pass-through/identity by default
                        let mut new_matrix = matrix.clone();
                        resize_matrix(
                            &mut new_matrix,
                            *input_channels,
                            *output_channels,
                            current_channels,
                            current_channels,
                        );

                        updated_settings = Some(PluginSettings::Matrix {
                            input_channels: current_channels,
                            output_channels: current_channels,
                            matrix: new_matrix,
                            channel_states: channel_states.clone(),
                        });
                    }
                }
                PluginSettings::Downmix {
                    input_channels,
                    center_gain_db,
                    surround_gain_db,
                    height_gain_db,
                    lfe_gain_db,
                    phase_coherence,
                    phase_blend_low_hz,
                    phase_blend_high_hz,
                    itu_mode,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::Downmix {
                            input_channels: current_channels,
                            center_gain_db: *center_gain_db,
                            surround_gain_db: *surround_gain_db,
                            height_gain_db: *height_gain_db,
                            lfe_gain_db: *lfe_gain_db,
                            phase_coherence: *phase_coherence,
                            phase_blend_low_hz: *phase_blend_low_hz,
                            phase_blend_high_hz: *phase_blend_high_hz,
                            itu_mode: *itu_mode,
                        });
                    }
                }
                PluginSettings::BandSplit {
                    channels,
                    frequency,
                    crossover_type,
                } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandSplit {
                            channels: current_channels,
                            frequency: *frequency,
                            crossover_type: crossover_type.clone(),
                        });
                    }
                }
                PluginSettings::BandMerge { channels, bands } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandMerge {
                            channels: current_channels,
                            bands: *bands,
                        });
                    }
                }
                _ => {}
            }

            if let Some(new_settings) = updated_settings {
                self.plugins[i].settings = new_settings;
            }

            // Update output channels for next plugin
            if self.plugins[i].enabled && !self.plugins[i].suspended {
                match &self.plugins[i].settings {
                    PluginSettings::Upmixer { speaker_config, .. } => {
                        current_channels = upmixer_output_channels(speaker_config);
                    }
                    PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                        current_channels = upmixer_output_channels(target_layout);
                    }
                    PluginSettings::BinauralDecoder { .. } => {
                        current_channels = 2;
                    }
                    PluginSettings::Matrix {
                        output_channels, ..
                    } => {
                        current_channels = *output_channels;
                    }
                    PluginSettings::Downmix { .. } => {
                        current_channels = 2; // Downmix always produces stereo
                    }
                    PluginSettings::MonoToStereo { .. } => {
                        current_channels = 2; // MonoToStereo always produces stereo
                    }
                    PluginSettings::BandSplit { .. } => {
                        current_channels *= 2; // Split into 2 bands
                    }
                    PluginSettings::BandMerge { bands, .. } => {
                        current_channels /= if *bands > 0 { *bands } else { 2 };
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::matrix::{apply_matrix_preset, available_matrix_presets, detect_matrix_preset};

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
                center_spread: 0.3,
                surround_direct_bleed: 0.15,
                rear_late_reflection: 0.2,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                subharmonic_freq_hz: 56.0,
                subharmonic_attack_ms: 20.0,
                subharmonic_release_ms: 100.0,
                decorrelation_mode: 0,
                decorrelation_lfo_rate_hz: 0.3,
                velvet_noise_duration_ms: 30.0,
                velvet_noise_density: 2000.0,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                height_hf_cap_hz: 8000.0,
                height_transient_reduction: 0.3,
                height_direct_leak: 0.1,
                ambient_boost: 1.0,
                safety_cap_db: 3.0,
                rear_ambient_boost: 1.0,
                dialogue_weight: 0.5,
                voice_freq_min_hz: 300.0,
                voice_freq_max_hz: 3400.0,
                dialogue_centroid_weight: 0.3,
                dialogue_variance_weight: 0.2,
                dialogue_coherence_weight: 0.5,
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
                enable_ml_detection: false,
                multi_source_extraction: false,
                multi_source_threshold: 0.5,
                low_latency: false,
                frequency_resolution: 0,
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
        if let Some(plugin) = chain.get_plugin(1)
            && let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings
        {
            assert_eq!(input_channels, 6); // Default value
        }

        // Update binaural decoder channels
        chain.update_channel_dependent_plugins();

        // Now it should be correctly set to 6 (output of upmixer)
        if let Some(plugin) = chain.get_plugin(1)
            && let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings
        {
            assert_eq!(input_channels, 6);
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
                center_spread: 0.3,
                surround_direct_bleed: 0.15,
                rear_late_reflection: 0.2,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                subharmonic_freq_hz: 56.0,
                subharmonic_attack_ms: 20.0,
                subharmonic_release_ms: 100.0,
                decorrelation_mode: 0,
                decorrelation_lfo_rate_hz: 0.3,
                velvet_noise_duration_ms: 30.0,
                velvet_noise_density: 2000.0,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                height_hf_cap_hz: 8000.0,
                height_transient_reduction: 0.3,
                height_direct_leak: 0.1,
                ambient_boost: 1.0,
                safety_cap_db: 3.0,
                rear_ambient_boost: 1.0,
                dialogue_weight: 0.5,
                voice_freq_min_hz: 300.0,
                voice_freq_max_hz: 3400.0,
                dialogue_centroid_weight: 0.3,
                dialogue_variance_weight: 0.2,
                dialogue_coherence_weight: 0.5,
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
                enable_ml_detection: false,
                multi_source_extraction: false,
                multi_source_threshold: 0.5,
                low_latency: false,
                frequency_resolution: 0,
            };
        }

        // Update binaural decoder channels
        chain.update_channel_dependent_plugins();

        // Now BinauralDecoder should have 8 input channels
        if let Some(plugin) = chain.get_plugin(1)
            && let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings
        {
            assert_eq!(input_channels, 8);
        }

        // Remove the upmixer
        chain.remove_plugin(0);
        chain.update_channel_dependent_plugins();

        // Now BinauralDecoder should have 2 input channels (stereo)
        if let Some(plugin) = chain.get_plugin(0)
            && let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings
        {
            assert_eq!(input_channels, 2);
        }
    }

    #[test]
    fn test_default_rack_structure() {
        let chain = PluginChain::with_default_rack();
        assert_eq!(chain.len(), 4);

        // [InputLM, Gain(disabled), Matrix, OutputLM] - all permanent
        let plugins = chain.plugins();
        assert!(matches!(
            plugins[0].plugin_type(),
            PluginType::LoudnessMonitor
        ));
        assert!(matches!(plugins[1].plugin_type(), PluginType::Gain));
        assert!(!plugins[1].enabled); // ReplayGain starts disabled
        assert!(matches!(plugins[2].plugin_type(), PluginType::Matrix));
        assert!(matches!(
            plugins[3].plugin_type(),
            PluginType::LoudnessMonitor
        ));

        assert!(plugins[0].is_permanent());
        assert!(plugins[1].is_permanent());
        assert!(plugins[2].is_permanent());
        assert!(plugins[3].is_permanent());
    }

    #[test]
    fn test_is_input_output_monitor() {
        let chain = PluginChain::with_default_rack();

        // Index 0 = input monitor
        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_output_monitor(0));

        // Index 1 = Gain (neither)
        assert!(!chain.is_input_monitor(1));
        assert!(!chain.is_output_monitor(1));

        // Index 2 = Matrix (neither)
        assert!(!chain.is_input_monitor(2));
        assert!(!chain.is_output_monitor(2));

        // Index 3 = output monitor
        assert!(!chain.is_input_monitor(3));
        assert!(chain.is_output_monitor(3));
    }

    #[test]
    fn test_default_rack_to_plugin_configs() {
        let chain = PluginChain::with_default_rack();
        let configs = chain.to_plugin_configs(48000.0);

        // Gain is disabled, so it's excluded from configs
        // Engine order: InputLM(0), Matrix(1), OutputLM(2)
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].plugin_type, "loudness_monitor"); // input monitor
        assert_eq!(configs[1].plugin_type, "matrix"); // processing
        assert_eq!(configs[2].plugin_type, "loudness_monitor"); // output monitor
    }

    #[test]
    fn test_default_rack_get_engine_index() {
        let chain = PluginChain::with_default_rack();

        // UI index 0 (input LM) → engine index 0
        assert_eq!(chain.get_engine_index(0), Some(0));
        // UI index 1 (Gain, disabled) → None (not in engine)
        assert_eq!(chain.get_engine_index(1), None);
        // UI index 2 (Matrix) → engine index 1
        assert_eq!(chain.get_engine_index(2), Some(1));
        // UI index 3 (output LM) → engine index 2
        assert_eq!(chain.get_engine_index(3), Some(2));
    }

    #[test]
    fn test_default_rack_with_user_plugin() {
        let mut chain = PluginChain::with_default_rack();

        // Insert a user EQ plugin at the user insert point (before Matrix)
        let insert_idx = chain.user_plugin_insert_index();
        assert_eq!(insert_idx, 2); // Before Matrix (after InputLM and Gain)
        chain.insert_plugin(insert_idx, &PluginType::EQ);

        // Chain should be [InputLM, Gain(disabled), EQ, Matrix, OutputLM]
        assert_eq!(chain.len(), 5);
        assert!(matches!(
            chain.plugins()[0].plugin_type(),
            PluginType::LoudnessMonitor
        ));
        assert!(matches!(chain.plugins()[1].plugin_type(), PluginType::Gain));
        assert!(matches!(chain.plugins()[2].plugin_type(), PluginType::EQ));
        assert!(matches!(
            chain.plugins()[3].plugin_type(),
            PluginType::Matrix
        ));
        assert!(matches!(
            chain.plugins()[4].plugin_type(),
            PluginType::LoudnessMonitor
        ));

        // Monitor identification still correct
        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_input_monitor(2));
        assert!(!chain.is_output_monitor(3));
        assert!(chain.is_output_monitor(4));

        // Gain is disabled, so not in engine configs
        // Engine indices: InputLM(0), EQ(1), Matrix(2), OutputLM(3)
        assert_eq!(chain.get_engine_index(0), Some(0)); // input monitor
        assert_eq!(chain.get_engine_index(1), None); // Gain (disabled)
        assert_eq!(chain.get_engine_index(2), Some(1)); // EQ (processing)
        assert_eq!(chain.get_engine_index(3), Some(2)); // Matrix (processing)
        assert_eq!(chain.get_engine_index(4), Some(3)); // output monitor

        // to_plugin_configs order: InputLM, EQ, Matrix, OutputLM (Gain excluded)
        let configs = chain.to_plugin_configs(48000.0);
        assert_eq!(configs.len(), 4);
        assert_eq!(configs[0].plugin_type, "loudness_monitor");
        assert_eq!(configs[1].plugin_type, "eq");
        assert_eq!(configs[2].plugin_type, "matrix");
        assert_eq!(configs[3].plugin_type, "loudness_monitor");
    }

    #[test]
    fn test_single_loudness_monitor_not_output() {
        // A chain with only one permanent LoudnessMonitor should not be an output monitor
        let mut chain = PluginChain::new();
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);

        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_output_monitor(0));
    }

    #[test]
    fn test_matrix_preset_roundtrip() {
        let presets = ["Identity", "Swap L/R", "Mono Mix"];
        // Test 2x2 (all presets should work)
        for preset in &presets {
            let mut matrix = vec![0.0f32; 4];
            apply_matrix_preset(2, 2, &mut matrix, preset);
            let detected = detect_matrix_preset(2, 2, &matrix);
            assert_eq!(detected, *preset, "2x2 roundtrip failed for {}", preset);
        }
        // Test non-square: 5x2, 2x5
        for (in_ch, out_ch) in [(5, 2), (2, 5), (1, 1), (8, 8)] {
            let mut matrix = vec![0.0f32; in_ch * out_ch];
            apply_matrix_preset(in_ch, out_ch, &mut matrix, "Identity");
            let detected = detect_matrix_preset(in_ch, out_ch, &matrix);
            assert_eq!(
                detected, "Identity",
                "{}x{} identity roundtrip failed",
                in_ch, out_ch
            );
        }
    }

    #[test]
    fn test_matrix_preset_cycling() {
        // Simulate the TUI cycling logic using available_matrix_presets
        for (in_ch, out_ch) in [(2, 2), (3, 3), (5, 2), (2, 5), (1, 1)] {
            let presets = available_matrix_presets(in_ch, out_ch);
            let mut matrix = vec![0.0f32; in_ch * out_ch];
            apply_matrix_preset(in_ch, out_ch, &mut matrix, "Identity");

            // Cycle forward through all presets twice
            let mut seen = Vec::new();
            for _ in 0..presets.len() * 2 {
                let current = detect_matrix_preset(in_ch, out_ch, &matrix);
                seen.push(current.to_string());
                let current_idx = presets.iter().position(|&p| p == current).unwrap_or(0);
                let new_idx = (current_idx + 1) % presets.len();
                apply_matrix_preset(in_ch, out_ch, &mut matrix, presets[new_idx]);
            }

            // Every available preset should be reachable
            for preset in &presets {
                assert!(
                    seen.contains(&preset.to_string()),
                    "{} not reachable for {}x{}, cycle: {:?}",
                    preset,
                    in_ch,
                    out_ch,
                    seen
                );
            }
            // No "Custom" should appear (all valid presets should round-trip)
            assert!(
                !seen.contains(&"Custom".to_string()),
                "Custom appeared in cycle for {}x{}: {:?}",
                in_ch,
                out_ch,
                seen
            );
        }
    }

    // ========================================================================
    // Channel flow tests: output_channels_for_input & adapt_matrix_to_input
    // ========================================================================

    /// Helper: build a chain and set the upmixer's speaker_config.
    fn chain_with_upmixer(speaker_config: &str) -> PluginChain {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Upmixer);
        if let Some(p) = chain.get_plugin_mut(0)
            && let PluginSettings::Upmixer {
                speaker_config: sc, ..
            } = &mut p.settings
        {
            *sc = speaker_config.to_string();
        }
        chain
    }

    // -- output_channels_for_input -----------------------------------------

    #[test]
    fn test_output_channels_passthrough() {
        // Empty chain: input passes through unchanged
        let chain = PluginChain::new();
        assert_eq!(chain.output_channels_for_input(1), 1);
        assert_eq!(chain.output_channels_for_input(2), 2);
        assert_eq!(chain.output_channels_for_input(8), 8);
    }

    #[test]
    fn test_output_channels_non_channel_plugins_passthrough() {
        // Plugins that don't change channels should pass through
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Gain);
        assert_eq!(chain.output_channels_for_input(2), 2);
        assert_eq!(chain.output_channels_for_input(6), 6);
    }

    #[test]
    fn test_output_channels_upmixer_configs() {
        for (config, expected) in [
            ("2.0", 2),
            ("5.0", 5),
            ("5.1", 6),
            ("7.1", 8),
            ("5.1.2", 8),
            ("5.1.4", 10),
            ("7.1.2", 10),
            ("7.1.4", 12),
            ("9.1.4", 14),
            ("9.1.6", 16),
        ] {
            let chain = chain_with_upmixer(config);
            assert_eq!(
                chain.output_channels_for_input(2),
                expected,
                "upmixer {} should output {} channels",
                config,
                expected
            );
        }
    }

    #[test]
    fn test_output_channels_downmix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Downmix);
        assert_eq!(chain.output_channels_for_input(6), 2);
        assert_eq!(chain.output_channels_for_input(10), 2);
    }

    #[test]
    fn test_output_channels_mono_to_stereo() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        assert_eq!(chain.output_channels_for_input(1), 2);
    }

    #[test]
    fn test_output_channels_binaural_decoder() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BinauralDecoder);
        assert_eq!(chain.output_channels_for_input(6), 2);
        assert_eq!(chain.output_channels_for_input(10), 2);
    }

    #[test]
    fn test_output_channels_matrix() {
        // Matrix with custom output size
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        if let Some(p) = chain.get_plugin_mut(0)
            && let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                channel_states,
            } = &mut p.settings
        {
            resize_matrix(matrix, *input_channels, *output_channels, 6, 4);
            *input_channels = 6;
            *output_channels = 4;
            channel_states.resize(4, sotf_plugins::ChannelState::default());
        }
        assert_eq!(chain.output_channels_for_input(6), 4);
    }

    #[test]
    fn test_output_channels_upmixer_then_binaural() {
        // Last channel-changing plugin wins (reverse walk)
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::BinauralDecoder);
        // Binaural is last → output is 2
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_upmixer_then_downmix() {
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Downmix);
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_disabled_plugin_skipped() {
        let mut chain = chain_with_upmixer("5.1");
        // Disable the upmixer → passthrough
        if let Some(p) = chain.get_plugin_mut(0) {
            p.enabled = false;
        }
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_eq_after_upmixer() {
        // EQ doesn't change channels → upmixer still determines output
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::EQ);
        assert_eq!(chain.output_channels_for_input(2), 6);
    }

    // -- adapt_matrix_to_input ---------------------------------------------

    fn get_matrix_dims(chain: &PluginChain) -> Option<(usize, usize)> {
        for p in chain.plugins() {
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } = &p.settings
            {
                return Some((*input_channels, *output_channels));
            }
        }
        None
    }

    #[test]
    fn test_adapt_matrix_stereo_file_no_upmixer() {
        // Matrix alone with stereo input stays 2x2
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_multichannel_file_no_upmixer() {
        // 6-channel file → matrix adapts to 6x6
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_before_matrix() {
        // Stereo file, upmixer 5.1.4 (10ch) before matrix → matrix should be 10x10
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((10, 10)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_various_configs() {
        for (config, expected) in [
            ("5.1", 6),
            ("7.1", 8),
            ("5.1.4", 10),
            ("7.1.4", 12),
            ("9.1.6", 16),
        ] {
            let mut chain = chain_with_upmixer(config);
            chain.add_plugin(&PluginType::Matrix);
            chain.adapt_matrix_to_input(2);
            assert_eq!(
                get_matrix_dims(&chain),
                Some((expected, expected)),
                "upmixer {} → matrix should be {}x{}",
                config,
                expected,
                expected
            );
        }
    }

    #[test]
    fn test_adapt_matrix_downmix_before_matrix() {
        // Downmix before matrix → matrix gets 2x2 regardless of file channels
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Downmix);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_mono_to_stereo_before_matrix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(1);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_binaural_before_matrix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BinauralDecoder);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_then_binaural_then_matrix() {
        // Chain: upmixer(5.1.4=10ch) → binaural(→2ch) → matrix
        // Matrix should see 2 channels (binaural is last before it)
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::BinauralDecoder);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_disabled_upmixer_ignored() {
        // Disabled upmixer should be skipped → matrix uses file channels
        let mut chain = chain_with_upmixer("5.1.4");
        if let Some(p) = chain.get_plugin_mut(0) {
            p.enabled = false;
        }
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_eq_between_upmixer_and_matrix() {
        // EQ doesn't change channels → upmixer output carries through
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((8, 8)));
    }

    #[test]
    fn test_adapt_matrix_noop_when_already_correct() {
        // If matrix already matches, nothing should change
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::Matrix);
        // First adapt: 2x2 → 6x6
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
        // Second adapt: already 6x6 → no change
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
    }

    #[test]
    fn test_adapt_matrix_readapt_on_config_change() {
        // Simulate changing upmixer config and re-adapting
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));

        // Change upmixer to 7.1.4
        if let Some(p) = chain.get_plugin_mut(0)
            && let PluginSettings::Upmixer { speaker_config, .. } = &mut p.settings
        {
            *speaker_config = "7.1.4".to_string();
        }
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((12, 12)));
    }

    // -- update_channel_dependent_plugins ----------------------------------

    #[test]
    fn test_update_channels_upmixer_then_eq() {
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::EQ);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::EQ { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 10,
                    "EQ after 5.1.4 upmixer should have 10 channels"
                );
            } else {
                panic!("expected EQ");
            }
        }
    }

    #[test]
    fn test_update_channels_upmixer_then_gain() {
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 8,
                    "Gain after 7.1 upmixer should have 8 channels"
                );
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_bandsplit_doubles() {
        // BandSplit doubles the channel count
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BandSplit);
        chain.update_channel_dependent_plugins();

        // Default input is 2, split → 4 output channels
        // Check via output_channels_for_input (BandSplit isn't in that fn,
        // but update_channel_dependent_plugins tracks it)
        // Instead check that a Gain after the split gets the doubled count
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 4,
                    "Gain after BandSplit(2ch) should have 4 channels"
                );
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_bandsplit_then_bandmerge() {
        // Split doubles, merge halves → back to original
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BandSplit);
        chain.add_plugin(&PluginType::BandMerge);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "Gain after Split+Merge should be back to 2");
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_upmixer_split_merge_gain() {
        // Upmixer(5.1=6) → Split(→12) → Merge(→6) → Gain(6)
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::BandSplit);
        chain.add_plugin(&PluginType::BandMerge);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        // BandSplit should have 6 channels
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::BandSplit { channels, .. } = &p.settings {
                assert_eq!(*channels, 6, "BandSplit after 5.1 upmixer");
            } else {
                panic!("expected BandSplit");
            }
        }
        // BandMerge should have 12 channels (doubled by split)
        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::BandMerge { channels, .. } = &p.settings {
                assert_eq!(*channels, 12, "BandMerge after BandSplit(6ch)");
            } else {
                panic!("expected BandMerge");
            }
        }
        // Gain should be back to 6
        if let Some(p) = chain.get_plugin(3) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 6, "Gain after Split+Merge should be 6");
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_downmix_then_eq() {
        // Downmix → EQ: EQ should have 2 channels
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Downmix);
        chain.add_plugin(&PluginType::EQ);
        chain.update_channel_dependent_plugins();

        // Downmix input should be set to 8
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Downmix { input_channels, .. } = &p.settings {
                assert_eq!(*input_channels, 8, "Downmix input after 7.1 upmixer");
            } else {
                panic!("expected Downmix");
            }
        }
        // EQ after downmix should be 2
        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::EQ { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "EQ after Downmix should be 2");
            } else {
                panic!("expected EQ");
            }
        }
    }

    #[test]
    fn test_update_channels_mono_to_stereo_then_gain() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "Gain after MonoToStereo");
            } else {
                panic!("expected Gain");
            }
        }
    }
}
