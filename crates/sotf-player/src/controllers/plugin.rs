//! Plugin chain controller.
//!
//! Encapsulates plugin chain management, parameter editing, EQ operations,
//! and preset management. Every mutation returns a `PluginUpdateEffect` so the
//! UI knows whether to do a structural rebuild or a zero-dropout parameter update.

use std::path::Path;

use crate::plugin_graph::PluginGraph;
use crate::{BiquadFilterType, ChannelConflict, EQFilter, Plugin, PluginSettings, PluginType};

// Re-export param_index_to_engine_param as a free function (used by GPUI apply_plugin_update)
pub use super::plugin_param_map::param_index_to_engine_param;

/// Effect returned by plugin mutations, telling the UI what kind of engine update is needed.
#[derive(Debug, Clone)]
pub enum PluginUpdateEffect {
    /// No update needed (e.g., invalid operation)
    None,
    /// Single parameter change — use `set_plugin_parameter()` for zero-dropout update
    Parameter {
        plugin_index: usize,
        param_index: usize,
    },
    /// Structural change (add/remove/reorder/toggle) — full chain rebuild
    Structural,
}

/// Plugin controller owning shared state for plugin editing.
///
/// Uses `PluginGraph` as the sole source of truth for plugin topology.
#[derive(Debug, Clone)]
pub struct PluginController {
    pub graph: PluginGraph,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize,
    pub selected_plugin_index: usize,
    pub selected_eq_band: usize,
    pub selected_eq_channel: usize,
    pub available_presets: Vec<String>,
    pub selected_preset_index: usize,
    pub last_loaded_preset: Option<String>,
}

impl Default for PluginController {
    fn default() -> Self {
        Self {
            graph: PluginGraph::with_default_rack(),
            editing_plugin_index: None,
            plugin_param_selection: 0,
            selected_plugin_index: 0,
            selected_eq_band: 0,
            selected_eq_channel: 0,
            available_presets: Vec::new(),
            selected_preset_index: 0,
            last_loaded_preset: None,
        }
    }
}

impl PluginController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the plugin graph is a simple linear chain (rack view compatible).
    pub fn is_linear(&self) -> bool {
        self.graph.is_linear()
    }

    // ========================================================================
    // Chain management
    // ========================================================================

    /// Add a plugin to the chain. Returns `Structural` effect.
    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> PluginUpdateEffect {
        let insert_idx = self.graph.user_plugin_insert_index();
        if self.graph.insert_plugin(insert_idx, plugin_type).is_ok() {
            self.selected_plugin_index = insert_idx;
            self.graph.update_channel_dependent_plugins();
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Toggle a plugin's enabled state. Returns `Structural` effect.
    pub fn toggle_plugin(&mut self, index: usize) -> PluginUpdateEffect {
        let _ = self.graph.toggle_plugin_by_index(index);
        self.graph.update_channel_dependent_plugins();
        PluginUpdateEffect::Structural
    }

    /// Move a plugin up in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_up(&mut self, index: usize) -> PluginUpdateEffect {
        if self.graph.can_move_up_by_index(index) {
            self.graph.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            self.graph.update_channel_dependent_plugins();
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Move a plugin down in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_down(&mut self, index: usize) -> PluginUpdateEffect {
        if self.graph.can_move_down_by_index(index) {
            self.graph.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            self.graph.update_channel_dependent_plugins();
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Remove a plugin from the chain. Returns `Structural` if removed, `None` otherwise.
    pub fn remove_plugin(&mut self, index: usize) -> PluginUpdateEffect {
        if self.graph.remove_plugin_by_index(index).is_ok() {
            self.graph.update_channel_dependent_plugins();
            if self.selected_plugin_index >= self.graph.len() && self.selected_plugin_index > 0 {
                self.selected_plugin_index = self.graph.len() - 1;
            }
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Select the next plugin in the chain.
    pub fn select_next_plugin(&mut self) {
        if self.graph.len() > 0 {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.graph.len();
        }
    }

    /// Select the previous plugin in the chain.
    pub fn select_previous_plugin(&mut self) {
        if self.graph.len() > 0 {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.graph.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    // ========================================================================
    // Plugin access
    // ========================================================================

    /// Get the currently editing plugin (immutable).
    pub fn get_editing_plugin(&self) -> Option<&Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.graph.get_plugin(idx))
    }

    /// Get the currently editing plugin (mutable).
    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.graph.get_plugin_mut(idx))
    }

    /// Whether the chain has an enabled spectrum analyzer.
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.graph.has_enabled_spectrum_analyzer()
    }

    // ========================================================================
    // Parameter navigation
    // ========================================================================

    /// Select the next parameter.
    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

    /// Select the previous parameter.
    pub fn select_previous_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                if self.plugin_param_selection == 0 {
                    self.plugin_param_selection = param_count - 1;
                } else {
                    self.plugin_param_selection -= 1;
                }
            }
        }
    }

    // ========================================================================
    // Parameter editing
    // ========================================================================

    /// Adjust the currently selected parameter by the given delta.
    /// Returns `(adjusted, effect)`.
    pub fn adjust_selected_param(&mut self, delta: f64) -> (bool, PluginUpdateEffect) {
        let param_idx = self.plugin_param_selection;
        let mut channel_count_changed = false;

        let result = if let Some(plugin) = self.get_editing_plugin_mut() {
            adjust_plugin_param(
                &mut plugin.settings,
                param_idx,
                delta,
                &mut channel_count_changed,
            )
        } else {
            false
        };

        if result && channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        if result {
            let effect = self.determine_update_effect(
                self.editing_plugin_index,
                param_idx,
                channel_count_changed,
            );
            (true, effect)
        } else {
            (false, PluginUpdateEffect::None)
        }
    }

    /// Set a specific parameter value for a plugin.
    pub fn set_plugin_param(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        value: f64,
    ) -> PluginUpdateEffect {
        let mut channel_count_changed = false;
        let mut update_needed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            update_needed = set_plugin_param_value(
                &mut plugin.settings,
                param_idx,
                value,
                &mut channel_count_changed,
            );
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        if update_needed {
            self.determine_update_effect(Some(plugin_idx), param_idx, channel_count_changed)
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Set a string parameter value for a plugin (e.g., file paths).
    pub fn set_plugin_param_string(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        value: String,
    ) -> PluginUpdateEffect {
        let mut update_needed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            match &mut plugin.settings {
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => match param_idx {
                    9 => {
                        *path_a_config = value;
                        update_needed = true;
                    }
                    10 => {
                        *path_b_config = value;
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Convolution { ir_file, .. } => {
                    if param_idx == 0 {
                        *ir_file = value;
                        update_needed = true;
                    }
                }
                PluginSettings::BinauralDecoder { sofa_file, .. } => {
                    if param_idx == 0 {
                        *sofa_file = value;
                        update_needed = true;
                    }
                }
                _ => {}
            }
        }

        if update_needed {
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Set spectrum analyzer tilt correction mode.
    pub fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        tilt: sotf_plugins::SpectralTiltCorrection,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer {
                tilt_correction, ..
            } = &mut plugin.settings
            {
                *tilt_correction = tilt;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    /// Set spectrum analyzer tilt reference frequency.
    pub fn set_spectrum_tilt_reference(
        &mut self,
        plugin_idx: usize,
        reference: sotf_plugins::TiltReferenceFreq,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer { tilt_reference, .. } = &mut plugin.settings {
                *tilt_reference = reference;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    /// Reset a specific parameter to its default value.
    pub fn reset_plugin_param(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
    ) -> PluginUpdateEffect {
        let plugin_type = if let Some(plugin) = self.graph.get_plugin(plugin_idx) {
            plugin.plugin_type()
        } else {
            return PluginUpdateEffect::None;
        };

        let default_settings = PluginSettings::default_for(&plugin_type);
        let default_value = match default_settings.param_value(param_idx) {
            Some(v) => v,
            None => return PluginUpdateEffect::None,
        };

        let mut channel_count_changed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            plugin.settings.set_param_value(param_idx, default_value);

            match &mut plugin.settings {
                PluginSettings::Upmixer { .. } if param_idx == 0 => {
                    channel_count_changed = true;
                }
                PluginSettings::MultibandCompressor {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    // Default active states: 4 bands => band 4 passive, 5 bands => bands 4,5 passive
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 => i < 3,
                            5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                PluginSettings::MultibandExpander {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 => i < 3,
                            5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                _ => {}
            }
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        self.determine_update_effect(Some(plugin_idx), param_idx, channel_count_changed)
    }

    // ========================================================================
    // EQ operations
    // ========================================================================

    /// Load EQ filters from an APO file path.
    pub fn load_apo_filters(&mut self, path: &Path) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        let filters = EQFilter::from_apo_file(path)?;

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                channel_filters,
                per_channel_mode,
                ..
            } = &plugin.settings
            {
                let channels = *channels;
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                };
                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Update SOFA file path for the currently editing binaural decoder plugin.
    pub fn load_sofa_path(&mut self, sofa_path: String) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                return Err("Selected plugin is not a Binaural Decoder".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = sofa_path;
                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Add a new EQ band to the currently editing EQ plugin.
    pub fn add_eq_band(&mut self) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                let new_filter = EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0);
                filters.push(new_filter);

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                };

                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Remove an EQ band from the currently editing EQ plugin.
    pub fn remove_eq_band(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters.remove(band_idx);

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                };

                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Toggle mute state for an EQ band.
    pub fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters[band_idx].muted = !filters[band_idx].muted;

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                };

                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Toggle solo state for an EQ band.
    pub fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters[band_idx].solo = !filters[band_idx].solo;

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                };

                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Set the EQ plugin to per-channel mode or global mode.
    pub fn set_eq_per_channel_mode(
        &mut self,
        plugin_idx: usize,
        per_channel: bool,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                if per_channel && channel_filters.is_none() {
                    let num_channels = *channels;
                    let mut ch_filters = Vec::with_capacity(num_channels);
                    for _ in 0..num_channels {
                        ch_filters.push(filters.clone());
                    }
                    *channel_filters = Some(ch_filters);
                }

                *per_channel_mode = per_channel;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    // ========================================================================
    // Presets
    // ========================================================================

    /// Refresh the list of available plugin presets from the config directory.
    pub fn refresh_presets(&mut self) {
        self.available_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = crate::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            self.available_presets.sort();
        }

        log::info!("Found {} plugin presets", self.available_presets.len());
    }

    /// Save the plugin chain to a file. Returns the filename used.
    pub fn save_to_file(&mut self, presets_dir: &Path, filename: &str) -> Result<String, String> {
        let filename_with_ext = if filename.ends_with(".json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };

        self.graph
            .save_to_file(presets_dir, filename)
            .map_err(|e| format!("Error saving: {}", e))?;

        self.last_loaded_preset = Some(filename_with_ext.clone());
        self.refresh_presets();
        Ok(filename_with_ext)
    }

    /// Load a plugin chain from a file. Returns `Structural` effect on success.
    pub fn load_from_file(
        &mut self,
        presets_dir: &Path,
        filename: &str,
    ) -> Result<(PluginUpdateEffect, String, Vec<String>), String> {
        let warnings = self
            .graph
            .load_from_file(presets_dir, filename)
            .map_err(|e| format!("Error loading: {}", e))?;

        self.graph.update_channel_dependent_plugins();

        let filename_with_ext = if filename.ends_with(".json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };
        self.last_loaded_preset = Some(filename_with_ext.clone());

        Ok((PluginUpdateEffect::Structural, filename_with_ext, warnings))
    }

    /// Save the plugin chain to the selected preset file. Returns the filename used.
    pub fn save_selected_preset(&mut self, presets_dir: &Path) -> Result<String, String> {
        if self.available_presets.is_empty() {
            return Err("No presets available".to_string());
        }

        let preset_filename = self
            .available_presets
            .get(self.selected_preset_index)
            .cloned()
            .ok_or_else(|| "Invalid preset index".to_string())?;

        self.graph
            .save_to_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error saving: {}", e))?;

        self.last_loaded_preset = Some(preset_filename.clone());
        self.refresh_presets();
        Ok(preset_filename)
    }

    /// Load the selected preset. Returns `Structural` effect, preset filename, plugin count,
    /// and any warnings about skipped plugins.
    pub fn load_selected_preset(
        &mut self,
        presets_dir: &Path,
    ) -> Result<(PluginUpdateEffect, String, usize, Vec<String>), String> {
        if self.available_presets.is_empty() {
            return Err("No presets available".to_string());
        }

        let preset_filename = self
            .available_presets
            .get(self.selected_preset_index)
            .cloned()
            .ok_or_else(|| "Invalid preset index".to_string())?;

        let warnings = self
            .graph
            .load_from_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error loading preset: {}", e))?;

        self.graph.update_channel_dependent_plugins();
        self.last_loaded_preset = Some(preset_filename.clone());
        let plugin_count = self.graph.len();

        Ok((
            PluginUpdateEffect::Structural,
            preset_filename,
            plugin_count,
            warnings,
        ))
    }

    /// Select the next preset in the list.
    pub fn select_next_preset(&mut self) {
        if !self.available_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_presets.len();
        }
    }

    /// Select the previous preset in the list.
    pub fn select_previous_preset(&mut self) {
        if !self.available_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }

    // ========================================================================
    // Per-Plugin Presets
    // ========================================================================

    /// Get the per-plugin presets directory for a given plugin type.
    /// Path: `{config_dir}/plugins/{plugin_type_name}/`
    fn plugin_preset_dir(plugin_type: &PluginType) -> Option<std::path::PathBuf> {
        let config_dir = crate::config::get_app_config_dir()?;
        let dir = config_dir
            .join("plugins")
            .join(plugin_type.name().to_lowercase().replace(' ', "_"));
        Some(dir)
    }

    /// List available presets for a specific plugin type.
    /// Returns sorted list of preset names (without .json extension).
    pub fn list_plugin_presets(plugin_type: &PluginType) -> Vec<String> {
        let Some(dir) = Self::plugin_preset_dir(plugin_type) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    /// Save a single plugin's settings as a preset.
    /// Path: `{config_dir}/plugins/{plugin_type_name}/{preset_name}.json`
    pub fn save_plugin_preset(
        &self,
        plugin_idx: usize,
        preset_name: &str,
    ) -> Result<String, String> {
        let plugin = self
            .graph
            .get_plugin(plugin_idx)
            .ok_or_else(|| format!("Plugin index {} out of range", plugin_idx))?;

        let dir = Self::plugin_preset_dir(&plugin.plugin_type())
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Could not create preset directory: {}", e))?;

        let filename = format!("{}.json", preset_name);
        let full_path = dir.join(&filename);

        let json = serde_json::to_string_pretty(&plugin.settings)
            .map_err(|e| format!("Serialization error: {}", e))?;
        std::fs::write(&full_path, json)
            .map_err(|e| format!("Could not write preset file: {}", e))?;

        log::info!(
            "Saved plugin preset '{}' to {}",
            preset_name,
            full_path.display()
        );
        Ok(filename)
    }

    /// Load a preset into a specific plugin slot.
    /// Returns `Structural` effect since settings are fully replaced.
    pub fn load_plugin_preset(
        &mut self,
        plugin_idx: usize,
        preset_name: &str,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin(plugin_idx)
            .ok_or_else(|| format!("Plugin index {} out of range", plugin_idx))?;

        let dir = Self::plugin_preset_dir(&plugin.plugin_type())
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        let filename = if preset_name.ends_with(".json") {
            preset_name.to_string()
        } else {
            format!("{}.json", preset_name)
        };
        let full_path = dir.join(&filename);

        let json = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Could not read preset file: {}", e))?;

        let settings: PluginSettings =
            serde_json::from_str(&json).map_err(|e| format!("Invalid preset file: {}", e))?;

        // Verify the preset is for the same plugin type by creating a temp plugin
        // and comparing types
        let temp = Plugin {
            id: 0,
            enabled: true,
            settings: settings.clone(),
            permanent: false,
            suspended: false,
        };
        let loaded_type = temp.plugin_type();
        let current_type = plugin.plugin_type();
        if loaded_type != current_type {
            return Err(format!(
                "Preset is for {} but plugin is {}",
                loaded_type.name(),
                current_type.name()
            ));
        }

        // Apply the settings
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            plugin.settings = settings;
        }
        self.graph.update_channel_dependent_plugins();

        log::info!(
            "Loaded plugin preset '{}' into slot {}",
            preset_name,
            plugin_idx
        );
        Ok(PluginUpdateEffect::Structural)
    }

    /// Delete a preset for a specific plugin type.
    pub fn delete_plugin_preset(plugin_type: &PluginType, preset_name: &str) -> Result<(), String> {
        let dir = Self::plugin_preset_dir(plugin_type)
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        let filename = if preset_name.ends_with(".json") {
            preset_name.to_string()
        } else {
            format!("{}.json", preset_name)
        };
        let full_path = dir.join(&filename);

        std::fs::remove_file(&full_path).map_err(|e| format!("Could not delete preset: {}", e))?;

        log::info!("Deleted plugin preset '{}'", preset_name);
        Ok(())
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Determine the update effect based on the plugin and parameter.
    fn determine_update_effect(
        &self,
        plugin_idx: Option<usize>,
        param_idx: usize,
        channel_count_changed: bool,
    ) -> PluginUpdateEffect {
        if channel_count_changed {
            return PluginUpdateEffect::Structural;
        }

        if let Some(idx) = plugin_idx {
            if let Some(plugin) = self.graph.get_plugin(idx) {
                if param_index_to_engine_param(&plugin.settings, param_idx).is_some() {
                    return PluginUpdateEffect::Parameter {
                        plugin_index: idx,
                        param_index: param_idx,
                    };
                }
            }
        }

        PluginUpdateEffect::Structural
    }

    // -- Channel conflict detection & suspension --

    pub fn find_channel_conflicts(&self, input_channels: usize) -> Vec<ChannelConflict> {
        self.graph.find_channel_conflicts(input_channels)
    }

    /// Find and suspend all incompatible plugins, then update channel-dependent plugins.
    pub fn suspend_incompatible(&mut self, input_channels: usize) {
        let conflicts = self.graph.find_channel_conflicts(input_channels);
        let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
        self.graph.suspend_plugins(&indices);
        self.graph.update_channel_dependent_plugins();
    }

    /// Clear all suspensions and update channel-dependent plugins.
    pub fn clear_suspensions(&mut self) {
        self.graph.clear_suspensions();
        self.graph.update_channel_dependent_plugins();
    }

    pub fn has_suspensions(&self) -> bool {
        self.graph.has_suspensions()
    }
}

// ============================================================================
// get_param_count — public helper
// ============================================================================

/// Get parameter count for a plugin's settings.
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4,
        _ => settings.param_specs().len(),
    }
}

// ============================================================================
// adjust_plugin_param — per-plugin param adjustment logic
// ============================================================================

/// Adjust a plugin parameter by delta. Returns true if the parameter was adjusted.
///
/// Most plugins delegate to `PluginSettings::adjust_param_value()` (generic path).
/// Only plugins with side effects beyond simple field updates have manual arms.
fn adjust_plugin_param(
    settings: &mut PluginSettings,
    param_idx: usize,
    delta: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        // === EQ: dynamic filter array, param indices map to band/field ===
        PluginSettings::EQ { filters, .. } => {
            if filters.is_empty() {
                return false;
            }

            let total_params = filters.len() * 4;
            if param_idx >= total_params {
                return false;
            }

            let filter_idx = param_idx / 4;
            let field_idx = param_idx % 4;

            if let Some(filter) = filters.get_mut(filter_idx) {
                match field_idx {
                    0 => {
                        filter.frequency = (filter.frequency + delta * 10.0).clamp(20.0, 20_000.0);
                        true
                    }
                    1 => {
                        filter.q = (filter.q + delta * 0.1).clamp(0.1, 10.0);
                        true
                    }
                    2 => {
                        filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0);
                        true
                    }
                    3 => {
                        let types = [
                            BiquadFilterType::Peak,
                            BiquadFilterType::Lowshelf,
                            BiquadFilterType::Highshelf,
                            BiquadFilterType::Lowpass,
                            BiquadFilterType::Highpass,
                            BiquadFilterType::Bandpass,
                            BiquadFilterType::Notch,
                        ];

                        let current_idx = types
                            .iter()
                            .position(|t| *t == filter.filter_type)
                            .unwrap_or(0);
                        let new_idx = if delta > 0.0 {
                            (current_idx + 1) % types.len()
                        } else if current_idx == 0 {
                            types.len() - 1
                        } else {
                            current_idx - 1
                        };
                        filter.filter_type = types[new_idx];
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === SpectrumAnalyzer: no_params_struct — not in the macro, needs manual handling ===
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            tilt_correction,
            tilt_reference,
            ..
        } => match param_idx {
            0 => {
                *num_bins = (*num_bins as i64 + delta as i64).clamp(10, 100) as usize;
                true
            }
            1 => {
                *min_freq = (*min_freq + delta as f32).clamp(10.0, 100.0);
                true
            }
            2 => {
                *max_freq = (*max_freq + delta as f32 * 100.0).clamp(1000.0, 24000.0);
                true
            }
            3 => {
                *smoothing = (*smoothing + delta as f32 * 0.01).clamp(0.0, 1.0);
                true
            }
            4 => {
                use sotf_plugins::SpectralTiltCorrection as STC;
                let modes = [
                    STC::None,
                    STC::ThreeDbPerOctave,
                    STC::SixDbPerOctave,
                    STC::Pink,
                ];
                let current = modes.iter().position(|m| m == tilt_correction).unwrap_or(0);
                let next = if delta > 0.0 {
                    (current + 1) % modes.len()
                } else if current == 0 {
                    modes.len() - 1
                } else {
                    current - 1
                };
                *tilt_correction = modes[next];
                true
            }
            5 => {
                use sotf_plugins::TiltReferenceFreq as TRF;
                let modes = [
                    TRF::Standard,
                    TRF::OneKilohertz,
                    TRF::TwoKilohertz,
                    TRF::MinFreq,
                ];
                let current = modes.iter().position(|m| m == tilt_reference).unwrap_or(0);
                let next = if delta > 0.0 {
                    (current + 1) % modes.len()
                } else if current == 0 {
                    modes.len() - 1
                } else {
                    current - 1
                };
                *tilt_reference = modes[next];
                true
            }
            _ => false,
        },
        // === MultibandCompressor band-level params (idx >= 100) ===
        PluginSettings::MultibandCompressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            bands,
            ..
        } if param_idx >= 100 => {
            use sotf_plugins::param_specs::{
                find_by_key as p, multiband_compressor::BAND_TEMPLATE as BT,
            };
            macro_rules! band_adj {
                ($field:expr, $global:expr, $key:literal, $step:expr) => {{
                    let spec = p(BT, $key);
                    $field = match $field {
                        None => Some(*$global as f32),
                        Some(v) => {
                            let new_v = v + $step;
                            if new_v < spec.min_f64() as f32 {
                                None
                            } else {
                                Some(new_v.clamp(spec.min_f64() as f32, spec.max_f64() as f32))
                            }
                        }
                    };
                    true
                }};
            }
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => band_adj!(band.threshold_db, threshold_db, "threshold", delta as f32),
                    7 => band_adj!(band.ratio, ratio, "ratio", delta as f32 * 0.1),
                    8 => band_adj!(band.attack_ms, attack_ms, "attack", delta as f32 * 0.5),
                    9 => band_adj!(band.release_ms, release_ms, "release", delta as f32 * 5.0),
                    10 => band_adj!(band.knee_db, knee_db, "knee", delta as f32 * 0.1),
                    13 => {
                        let s = p(BT, "makeup_gain");
                        band.makeup_gain_db = (band.makeup_gain_db + delta as f32 * 0.5)
                            .clamp(s.min_f64() as f32, s.max_f64() as f32);
                        true
                    }
                    14 => {
                        band.bypass = !band.bypass;
                        true
                    }
                    15 => {
                        band.solo = !band.solo;
                        true
                    }
                    16 => {
                        band.auto_makeup = !band.auto_makeup;
                        true
                    }
                    17 => {
                        band.active = !band.active;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandExpander band-level params (idx >= 100) ===
        PluginSettings::MultibandExpander {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            knee_db,
            hysteresis_db,
            hold_ms,
            bands,
            ..
        } if param_idx >= 100 => {
            use sotf_plugins::param_specs::{
                find_by_key as p, multiband_expander::BAND_TEMPLATE as BT,
            };
            macro_rules! band_adj {
                ($field:expr, $global:expr, $key:literal, $step:expr) => {{
                    let spec = p(BT, $key);
                    $field = match $field {
                        None => Some(*$global as f32),
                        Some(v) => {
                            let new_v = v + $step;
                            if new_v < spec.min_f64() as f32 {
                                None
                            } else {
                                Some(new_v.clamp(spec.min_f64() as f32, spec.max_f64() as f32))
                            }
                        }
                    };
                    true
                }};
            }
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => band_adj!(band.threshold_db, threshold_db, "threshold", delta as f32),
                    7 => band_adj!(band.ratio, ratio, "ratio", delta as f32 * 0.1),
                    8 => band_adj!(band.attack_ms, attack_ms, "attack", delta as f32 * 0.1),
                    9 => band_adj!(band.release_ms, release_ms, "release", delta as f32 * 10.0),
                    10 => band_adj!(band.range_db, range_db, "range", delta as f32),
                    11 => band_adj!(band.knee_db, knee_db, "knee", delta as f32 * 0.1),
                    12 => band_adj!(
                        band.hysteresis_db,
                        hysteresis_db,
                        "hysteresis",
                        delta as f32 * 0.1
                    ),
                    13 => band_adj!(band.hold_ms, hold_ms, "hold", delta as f32 * 5.0),
                    14 => {
                        band.bypass = !band.bypass;
                        true
                    }
                    15 => {
                        band.solo = !band.solo;
                        true
                    }
                    16 => {
                        band.auto_makeup = !band.auto_makeup;
                        true
                    }
                    17 => {
                        band.active = !band.active;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === Crossfeed preset (idx 1): sets multiple fields atomically ===
        PluginSettings::Crossfeed {
            mode,
            preset,
            bauer_fcut_hz,
            bauer_feed_db,
            meier_level,
            mb_low_freq_hz,
            mb_mid_high_freq_hz,
            mb_low_feed_db,
            mb_mid_feed_db,
            mb_high_feed_db,
            ..
        } if param_idx == 1 => {
            use sotf_plugins::CrossfeedPreset;
            let presets = [
                CrossfeedPreset::Default,
                CrossfeedPreset::Cmoy,
                CrossfeedPreset::Meier,
                CrossfeedPreset::Mb,
                CrossfeedPreset::Off,
            ];
            let current = presets.iter().position(|pr| pr == preset).unwrap_or(0);
            let next = if delta > 0.0 {
                (current + 1) % presets.len()
            } else {
                (current + presets.len() - 1) % presets.len()
            };
            *preset = presets[next];
            let pp = sotf_plugins::CrossfeedPluginParams::from_preset(*preset);
            *mode = pp.mode;
            *bauer_fcut_hz = pp.bauer_fcut_hz as f64;
            *bauer_feed_db = pp.bauer_feed_db as f64;
            *meier_level = pp.meier_level as f64;
            *mb_low_freq_hz = pp.mb_low_freq_hz as f64;
            *mb_mid_high_freq_hz = pp.mb_mid_high_freq_hz as f64;
            *mb_low_feed_db = pp.mb_low_feed_db as f64;
            *mb_mid_feed_db = pp.mb_mid_feed_db as f64;
            *mb_high_feed_db = pp.mb_high_feed_db as f64;
            true
        }
        // === Generic path: all other plugins use adjust_param_value() ===
        other => {
            let adjusted = other.adjust_param_value(param_idx, delta);
            if adjusted {
                apply_structural_side_effects(other, param_idx, channel_count_changed);
            }
            adjusted
        }
    }
}

// ============================================================================
// set_plugin_param_value — per-plugin set logic
// ============================================================================

/// Set a specific parameter value. Returns true if the parameter was set.
///
/// Most plugins delegate to `PluginSettings::set_param_value()` (generic path).
/// Only plugins with side effects beyond simple field updates have manual arms.
fn set_plugin_param_value(
    settings: &mut PluginSettings,
    param_idx: usize,
    value: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        // === EQ: dynamic filter array, param indices map to band/field ===
        PluginSettings::EQ { filters, .. } => {
            let filter_idx = param_idx / 4;
            let field_idx = param_idx % 4;

            if let Some(filter) = filters.get_mut(filter_idx) {
                match field_idx {
                    0 => {
                        filter.frequency = value.clamp(20.0, 20_000.0);
                        true
                    }
                    1 => {
                        filter.q = value.clamp(0.1, 10.0);
                        true
                    }
                    2 => {
                        filter.gain_db = value.clamp(-24.0, 24.0);
                        true
                    }
                    3 => {
                        let types = [
                            BiquadFilterType::Peak,
                            BiquadFilterType::Lowshelf,
                            BiquadFilterType::Highshelf,
                            BiquadFilterType::Lowpass,
                            BiquadFilterType::Highpass,
                            BiquadFilterType::Bandpass,
                            BiquadFilterType::Notch,
                        ];
                        let type_idx = (value as usize).clamp(0, types.len() - 1);
                        filter.filter_type = types[type_idx];
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === SpectrumAnalyzer: no_params_struct — not in the macro, needs manual handling ===
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            tilt_correction,
            tilt_reference,
            ..
        } => match param_idx {
            0 => {
                *num_bins = (value as usize).clamp(10, 256);
                true
            }
            1 => {
                *min_freq = (value as f32).clamp(20.0, 20000.0);
                true
            }
            2 => {
                *max_freq = (value as f32).clamp(20.0, 20000.0);
                true
            }
            3 => {
                *smoothing = (value as f32).clamp(0.0, 1.0);
                true
            }
            4 => {
                use sotf_plugins::SpectralTiltCorrection as STC;
                let modes = [
                    STC::None,
                    STC::ThreeDbPerOctave,
                    STC::SixDbPerOctave,
                    STC::Pink,
                ];
                *tilt_correction = modes[(value as usize).clamp(0, modes.len() - 1)];
                true
            }
            5 => {
                use sotf_plugins::TiltReferenceFreq as TRF;
                let modes = [
                    TRF::Standard,
                    TRF::OneKilohertz,
                    TRF::TwoKilohertz,
                    TRF::MinFreq,
                ];
                *tilt_reference = modes[(value as usize).clamp(0, modes.len() - 1)];
                true
            }
            _ => false,
        },
        // === MultibandCompressor band-level params (idx >= 100) ===
        PluginSettings::MultibandCompressor { bands, .. } if param_idx >= 100 => {
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => {
                        band.threshold_db = Some(value as f32);
                        true
                    }
                    7 => {
                        band.ratio = Some(value as f32);
                        true
                    }
                    8 => {
                        band.attack_ms = Some(value as f32);
                        true
                    }
                    9 => {
                        band.release_ms = Some(value as f32);
                        true
                    }
                    10 => {
                        band.knee_db = Some(value as f32);
                        true
                    }
                    13 => {
                        band.makeup_gain_db = value as f32;
                        true
                    }
                    14 => {
                        band.bypass = value > 0.5;
                        true
                    }
                    15 => {
                        band.solo = value > 0.5;
                        true
                    }
                    16 => {
                        band.auto_makeup = value > 0.5;
                        true
                    }
                    17 => {
                        band.active = value > 0.5;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandExpander band-level params (idx >= 100) ===
        PluginSettings::MultibandExpander { bands, .. } if param_idx >= 100 => {
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => {
                        band.threshold_db = Some(value as f32);
                        true
                    }
                    7 => {
                        band.ratio = Some(value as f32);
                        true
                    }
                    8 => {
                        band.attack_ms = Some(value as f32);
                        true
                    }
                    9 => {
                        band.release_ms = Some(value as f32);
                        true
                    }
                    10 => {
                        band.range_db = Some(value as f32);
                        true
                    }
                    11 => {
                        band.knee_db = Some(value as f32);
                        true
                    }
                    12 => {
                        band.hysteresis_db = Some(value as f32);
                        true
                    }
                    13 => {
                        band.hold_ms = Some(value as f32);
                        true
                    }
                    14 => {
                        band.bypass = value > 0.5;
                        true
                    }
                    15 => {
                        band.solo = value > 0.5;
                        true
                    }
                    16 => {
                        band.auto_makeup = value > 0.5;
                        true
                    }
                    17 => {
                        band.active = value > 0.5;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === Crossfeed preset (idx 1): sets multiple fields atomically ===
        PluginSettings::Crossfeed {
            mode,
            preset,
            bauer_fcut_hz,
            bauer_feed_db,
            meier_level,
            mb_low_freq_hz,
            mb_mid_high_freq_hz,
            mb_low_feed_db,
            mb_mid_feed_db,
            mb_high_feed_db,
            ..
        } if param_idx == 1 => {
            use sotf_plugins::CrossfeedPreset;
            let presets = [
                CrossfeedPreset::Default,
                CrossfeedPreset::Cmoy,
                CrossfeedPreset::Meier,
                CrossfeedPreset::Mb,
                CrossfeedPreset::Off,
            ];
            let idx = (value as usize).min(presets.len() - 1);
            *preset = presets[idx];
            let p_params = sotf_plugins::CrossfeedPluginParams::from_preset(*preset);
            *mode = p_params.mode;
            *bauer_fcut_hz = p_params.bauer_fcut_hz as f64;
            *bauer_feed_db = p_params.bauer_feed_db as f64;
            *meier_level = p_params.meier_level as f64;
            *mb_low_freq_hz = p_params.mb_low_freq_hz as f64;
            *mb_mid_high_freq_hz = p_params.mb_mid_high_freq_hz as f64;
            *mb_low_feed_db = p_params.mb_low_feed_db as f64;
            *mb_mid_feed_db = p_params.mb_mid_feed_db as f64;
            *mb_high_feed_db = p_params.mb_high_feed_db as f64;
            true
        }
        // === Generic path: all other plugins use set_param_value() ===
        other => {
            let specs = other.param_specs();
            if let Some(spec) = specs.get(param_idx) {
                let raw = value / spec.display_scale;
                other.set_param_value(param_idx, spec.clamp_f64(raw));
                apply_structural_side_effects(other, param_idx, channel_count_changed);
                true
            } else {
                false
            }
        }
    }
}

// ============================================================================
// apply_structural_side_effects — shared post-update logic
// ============================================================================

/// Apply structural side effects after a parameter update via the generic path.
///
/// Handles: Upmixer speaker_config (idx 0) sets channel_count_changed,
/// MultibandCompressor/Expander num_bands (idx 0) resizes band arrays.
fn apply_structural_side_effects(
    settings: &mut PluginSettings,
    param_idx: usize,
    channel_count_changed: &mut bool,
) {
    match settings {
        PluginSettings::Upmixer { .. } if param_idx == 0 => {
            *channel_count_changed = true;
        }
        PluginSettings::MultibandCompressor {
            num_bands, bands, ..
        } if param_idx == 0 => {
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate() {
                band.active = match *num_bands {
                    4 => i < 3,
                    5 => i < 3,
                    _ => true,
                };
            }
            *channel_count_changed = true;
        }
        PluginSettings::MultibandExpander {
            num_bands, bands, ..
        } if param_idx == 0 => {
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate() {
                band.active = match *num_bands {
                    4 => i < 3,
                    5 => i < 3,
                    _ => true,
                };
            }
            *channel_count_changed = true;
        }
        _ => {}
    }
}
