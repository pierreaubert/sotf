//! Plugin chain controller.
//!
//! Encapsulates plugin chain management, parameter editing, EQ operations,
//! and preset management. Every mutation returns a `PluginUpdateEffect` so the
//! UI knows whether to do a structural rebuild or a zero-dropout parameter update.

use std::path::Path;

use crate::{
    BiquadFilterType, ChannelConflict, EQFilter, Plugin, PluginChain, PluginSettings, PluginType,
};

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

/// Plugin chain controller owning shared state for plugin editing.
#[derive(Debug, Clone)]
pub struct PluginController {
    pub chain: PluginChain,
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
            chain: PluginChain::with_default_rack(),
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

    // ========================================================================
    // Chain management
    // ========================================================================

    /// Add a plugin to the chain. Returns `Structural` effect.
    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> PluginUpdateEffect {
        let insert_idx = self.chain.user_plugin_insert_index();
        self.chain.insert_plugin(insert_idx, plugin_type);
        self.selected_plugin_index = insert_idx;
        self.chain.update_channel_dependent_plugins();
        PluginUpdateEffect::Structural
    }

    /// Toggle a plugin's enabled state. Returns `Structural` effect.
    pub fn toggle_plugin(&mut self, index: usize) -> PluginUpdateEffect {
        self.chain.toggle_plugin(index);
        self.chain.update_channel_dependent_plugins();
        PluginUpdateEffect::Structural
    }

    /// Move a plugin up in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_up(&mut self, index: usize) -> PluginUpdateEffect {
        if self.chain.can_move_plugin_up(index) {
            self.chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            self.chain.update_channel_dependent_plugins();
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Move a plugin down in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_down(&mut self, index: usize) -> PluginUpdateEffect {
        if self.chain.can_move_plugin_down(index) {
            self.chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            self.chain.update_channel_dependent_plugins();
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Remove a plugin from the chain. Returns `Structural` if removed, `None` otherwise.
    pub fn remove_plugin(&mut self, index: usize) -> PluginUpdateEffect {
        if index < self.chain.len() {
            self.chain.remove_plugin(index);
            self.chain.update_channel_dependent_plugins();
            if self.selected_plugin_index >= self.chain.len() && self.selected_plugin_index > 0 {
                self.selected_plugin_index = self.chain.len() - 1;
            }
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Select the next plugin in the chain.
    pub fn select_next_plugin(&mut self) {
        if !self.chain.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.chain.len();
        }
    }

    /// Select the previous plugin in the chain.
    pub fn select_previous_plugin(&mut self) {
        if !self.chain.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.chain.len() - 1;
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
            .and_then(|idx| self.chain.get_plugin(idx))
    }

    /// Get the currently editing plugin (mutable).
    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.chain.get_plugin_mut(idx))
    }

    /// Whether the chain has an enabled spectrum analyzer.
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.chain.has_enabled_spectrum_analyzer()
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
            self.chain.update_channel_dependent_plugins();
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

        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
            update_needed = set_plugin_param_value(
                &mut plugin.settings,
                param_idx,
                value,
                &mut channel_count_changed,
            );
        }

        if channel_count_changed {
            self.chain.update_channel_dependent_plugins();
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

        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
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
        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
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
        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
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
        let plugin_type = if let Some(plugin) = self.chain.get_plugin(plugin_idx) {
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

        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
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
            self.chain.update_channel_dependent_plugins();
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
        if let Some(plugin) = self.chain.get_plugin_mut(plugin_idx) {
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

        self.chain
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
    ) -> Result<(PluginUpdateEffect, String), String> {
        self.chain
            .load_from_file(presets_dir, filename)
            .map_err(|e| format!("Error loading: {}", e))?;

        self.chain.update_channel_dependent_plugins();

        let filename_with_ext = if filename.ends_with(".json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };
        self.last_loaded_preset = Some(filename_with_ext.clone());

        Ok((PluginUpdateEffect::Structural, filename_with_ext))
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

        self.chain
            .save_to_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error saving: {}", e))?;

        self.last_loaded_preset = Some(preset_filename.clone());
        self.refresh_presets();
        Ok(preset_filename)
    }

    /// Load the selected preset. Returns `Structural` effect and preset filename on success.
    pub fn load_selected_preset(
        &mut self,
        presets_dir: &Path,
    ) -> Result<(PluginUpdateEffect, String, usize), String> {
        if self.available_presets.is_empty() {
            return Err("No presets available".to_string());
        }

        let preset_filename = self
            .available_presets
            .get(self.selected_preset_index)
            .cloned()
            .ok_or_else(|| "Invalid preset index".to_string())?;

        self.chain
            .load_from_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error loading preset: {}", e))?;

        self.chain.update_channel_dependent_plugins();
        self.last_loaded_preset = Some(preset_filename.clone());
        let plugin_count = self.chain.len();

        Ok((
            PluginUpdateEffect::Structural,
            preset_filename,
            plugin_count,
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
            .chain
            .plugins()
            .get(plugin_idx)
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
            .chain
            .plugins()
            .get(plugin_idx)
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
        self.chain.plugins_mut()[plugin_idx].settings = settings;
        self.chain.update_channel_dependent_plugins();

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
            if let Some(plugin) = self.chain.get_plugin(idx) {
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
        self.chain.find_channel_conflicts(input_channels)
    }

    /// Find and suspend all incompatible plugins, then update channel-dependent plugins.
    pub fn suspend_incompatible(&mut self, input_channels: usize) {
        let conflicts = self.chain.find_channel_conflicts(input_channels);
        let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
        self.chain.suspend_plugins(&indices);
        self.chain.update_channel_dependent_plugins();
    }

    /// Clear all suspensions and update channel-dependent plugins.
    pub fn clear_suspensions(&mut self) {
        self.chain.clear_suspensions();
        self.chain.update_channel_dependent_plugins();
    }

    pub fn has_suspensions(&self) -> bool {
        self.chain.has_suspensions()
    }
}

// ============================================================================
// get_param_count — public helper
// ============================================================================

/// Get parameter count for a plugin's settings.
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4,
        PluginSettings::Convolution { .. } => 2,
        PluginSettings::SpectrumAnalyzer { .. } => 4,
        _ => settings.param_specs().len(),
    }
}

// ============================================================================
// adjust_plugin_param — per-plugin param adjustment logic
// ============================================================================

/// Adjust a plugin parameter by delta. Returns true if the parameter was adjusted.
fn adjust_plugin_param(
    settings: &mut PluginSettings,
    param_idx: usize,
    delta: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            stereo_width,
            center_spread,
            surround_direct_bleed,
            rear_late_reflection,
            lfe_cutoff_hz,
            lfe_gain,
            bandpass_hz,
            enable_subharmonic_synth,
            subharmonic_gain,
            subharmonic_freq_hz,
            subharmonic_attack_ms,
            subharmonic_release_ms,
            decorrelation_mode,
            decorrelation_lfo_rate_hz,
            velvet_noise_duration_ms,
            velvet_noise_density,
            enable_hr_direct,
            hr_sharpen,
            height_hf_cap_hz,
            height_transient_reduction,
            height_direct_leak,
            ambient_boost,
            safety_cap_db,
            rear_ambient_boost,
            dialogue_weight,
            voice_freq_min_hz,
            voice_freq_max_hz,
            dialogue_centroid_weight,
            dialogue_variance_weight,
            dialogue_coherence_weight,
            bypass_decorrelation,
            bypass_transient_detection,
            bypass_all_processing,
            enable_ml_detection,
            ..
        } => {
            use sotf_plugins::param_specs::{find_by_key as p, upmixer::PARAMS as UP};
            macro_rules! adj {
                ($field:expr, $key:literal) => {{
                    *$field = p(UP, $key).adjust_f64(*$field, delta);
                    true
                }};
            }
            match param_idx {
                0 => {
                    let configs = p(UP, "speaker_config").choice_labels();
                    let current_idx = configs
                        .iter()
                        .position(|&c| c == speaker_config.as_str())
                        .unwrap_or(0);
                    let new_idx = if delta > 0.0 {
                        (current_idx + 1) % configs.len()
                    } else if current_idx == 0 {
                        configs.len() - 1
                    } else {
                        current_idx - 1
                    };
                    *speaker_config = configs[new_idx].to_string();
                    *channel_count_changed = true;
                    true
                }
                1 => adj!(gain_front_direct, "gain_front_direct"),
                2 => adj!(gain_front_ambient, "gain_front_ambient"),
                3 => adj!(gain_rear_ambient, "gain_rear_ambient"),
                4 => adj!(height_gain, "height_gain"),
                5 => adj!(lfe_gain, "lfe_gain"),
                6 => adj!(lfe_cutoff_hz, "lfe_cutoff_hz"),
                7 => adj!(stereo_width, "stereo_width"),
                8 => adj!(center_spread, "center_spread"),
                9 => adj!(bandpass_hz, "bandpass_hz"),
                10 => {
                    *enable_subharmonic_synth = !*enable_subharmonic_synth;
                    true
                }
                11 => adj!(subharmonic_gain, "subharmonic_gain"),
                12 => {
                    *enable_hr_direct = !*enable_hr_direct;
                    true
                }
                13 => adj!(hr_sharpen, "hr_sharpen"),
                14 => adj!(safety_cap_db, "safety_cap_db"),
                15 => {
                    if delta.abs() > 0.1 {
                        *decorrelation_mode = if *decorrelation_mode == 0 { 1 } else { 0 };
                    }
                    true
                }
                16 => adj!(subharmonic_freq_hz, "subharmonic_freq_hz"),
                17 => adj!(subharmonic_attack_ms, "subharmonic_attack_ms"),
                18 => adj!(subharmonic_release_ms, "subharmonic_release_ms"),
                19 => adj!(decorrelation_lfo_rate_hz, "decorrelation_lfo_rate_hz"),
                20 => adj!(velvet_noise_duration_ms, "velvet_noise_duration_ms"),
                21 => adj!(velvet_noise_density, "velvet_noise_density"),
                22 => adj!(height_hf_cap_hz, "height_hf_cap_hz"),
                23 => adj!(height_transient_reduction, "height_transient_reduction"),
                24 => adj!(height_direct_leak, "height_direct_leak"),
                25 => adj!(surround_direct_bleed, "surround_direct_bleed"),
                26 => adj!(rear_ambient_boost, "rear_ambient_boost"),
                27 => adj!(rear_late_reflection, "rear_late_reflection"),
                28 => adj!(ambient_boost, "ambient_boost"),
                29 => adj!(dialogue_weight, "dialogue_weight"),
                30 => adj!(voice_freq_min_hz, "voice_freq_min_hz"),
                31 => adj!(voice_freq_max_hz, "voice_freq_max_hz"),
                32 => adj!(dialogue_centroid_weight, "dialogue_centroid_weight"),
                33 => adj!(dialogue_variance_weight, "dialogue_variance_weight"),
                34 => adj!(dialogue_coherence_weight, "dialogue_coherence_weight"),
                35 => {
                    *bypass_decorrelation = !*bypass_decorrelation;
                    true
                }
                36 => {
                    *bypass_transient_detection = !*bypass_transient_detection;
                    true
                }
                37 => {
                    *bypass_all_processing = !*bypass_all_processing;
                    true
                }
                38 => {
                    *enable_ml_detection = !*enable_ml_detection;
                    true
                }
                _ => false,
            }
        }
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
                        use crate::BiquadFilterType;

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
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
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
            _ => false,
        },
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
        PluginSettings::FletcherMunson {
            reference_level_db,
            smoothing_ms,
            band1_freq,
            band1_q,
            band1_max_gain,
            band1_slope,
            band2_freq,
            band2_q,
            band2_max_gain,
            band2_slope,
            band3_freq,
            band3_q,
            band3_max_gain,
            band3_slope,
            band4_freq,
            band4_q,
            band4_max_gain,
            band4_slope,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            auto_gain_loudness_type,
            ..
        } => match param_idx {
            0 => {
                *reference_level_db = (*reference_level_db + delta).clamp(-40.0, 0.0);
                true
            }
            1 => {
                *smoothing_ms = (*smoothing_ms + delta * 5.0).clamp(1.0, 200.0);
                true
            }
            2 => {
                *band1_freq = (*band1_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                true
            }
            3 => {
                *band1_q = (*band1_q + delta * 0.1).clamp(0.1, 10.0);
                true
            }
            4 => {
                *band1_max_gain = (*band1_max_gain + delta).clamp(0.0, 24.0);
                true
            }
            5 => {
                *band1_slope = (*band1_slope + delta * 0.05).clamp(0.0, 1.0);
                true
            }
            6 => {
                *band2_freq = (*band2_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                true
            }
            7 => {
                *band2_q = (*band2_q + delta * 0.1).clamp(0.1, 10.0);
                true
            }
            8 => {
                *band2_max_gain = (*band2_max_gain + delta).clamp(0.0, 24.0);
                true
            }
            9 => {
                *band2_slope = (*band2_slope + delta * 0.05).clamp(0.0, 1.0);
                true
            }
            10 => {
                *band3_freq = (*band3_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                true
            }
            11 => {
                *band3_q = (*band3_q + delta * 0.1).clamp(0.1, 10.0);
                true
            }
            12 => {
                *band3_max_gain = (*band3_max_gain + delta).clamp(0.0, 24.0);
                true
            }
            13 => {
                *band3_slope = (*band3_slope + delta * 0.05).clamp(0.0, 1.0);
                true
            }
            14 => {
                *band4_freq = (*band4_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                true
            }
            15 => {
                *band4_q = (*band4_q + delta * 0.1).clamp(0.1, 10.0);
                true
            }
            16 => {
                *band4_max_gain = (*band4_max_gain + delta).clamp(0.0, 24.0);
                true
            }
            17 => {
                *band4_slope = (*band4_slope + delta * 0.05).clamp(0.0, 1.0);
                true
            }
            18 => {
                *auto_gain_enabled = !*auto_gain_enabled;
                true
            }
            19 => {
                *auto_gain_max_db = (*auto_gain_max_db + delta).clamp(0.0, 24.0);
                true
            }
            20 => {
                *auto_gain_smoothing_ms =
                    (*auto_gain_smoothing_ms + delta * 10.0).clamp(10.0, 500.0);
                true
            }
            21 => {
                *auto_gain_loudness_type = if *auto_gain_loudness_type == 0 { 1 } else { 0 };
                true
            }
            _ => false,
        },
        PluginSettings::BandSplit {
            frequency,
            crossover_type,
            ..
        } => match param_idx {
            0 => {
                *frequency = (*frequency * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                true
            }
            1 => {
                *crossover_type = if crossover_type == "LR24" {
                    "LR48".to_string()
                } else {
                    "LR24".to_string()
                };
                true
            }
            _ => false,
        },
        PluginSettings::Crossfeed {
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
        } => {
            use sotf_plugins::param_specs::{crossfeed::PARAMS as CF, find_by_key as p};
            use sotf_plugins::{CrossfeedMode, CrossfeedPreset};
            macro_rules! adj {
                ($field:expr, $key:literal) => {{
                    *$field = p(CF, $key).adjust_f64(*$field, delta);
                    true
                }};
            }
            match param_idx {
                0 => {
                    let modes = [
                        CrossfeedMode::Off,
                        CrossfeedMode::Bauer,
                        CrossfeedMode::Meier,
                        CrossfeedMode::Mb,
                    ];
                    let current = modes.iter().position(|m| m == mode).unwrap_or(0);
                    let next = if delta > 0.0 {
                        (current + 1) % modes.len()
                    } else {
                        (current + modes.len() - 1) % modes.len()
                    };
                    *mode = modes[next];
                    true
                }
                1 => {
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
                2 => {
                    *enabled = !*enabled;
                    true
                }
                3 => adj!(mix, "mix"),
                4 => adj!(bauer_fcut_hz, "bauer_fcut_hz"),
                5 => adj!(bauer_feed_db, "bauer_feed_db"),
                6 => adj!(meier_level, "meier_level"),
                7 => adj!(mb_low_freq_hz, "mb_low_freq_hz"),
                8 => adj!(mb_mid_high_freq_hz, "mb_mid_high_freq_hz"),
                9 => adj!(mb_low_feed_db, "mb_low_feed_db"),
                10 => adj!(mb_mid_feed_db, "mb_mid_feed_db"),
                11 => adj!(mb_high_feed_db, "mb_high_feed_db"),
                12 => {
                    *autogain_enabled = !*autogain_enabled;
                    true
                }
                13 => adj!(autogain_target_lufs, "autogain_target_lufs"),
                14 => adj!(autogain_max_gain_db, "autogain_max_gain_db"),
                15 => adj!(autogain_smoothing_ms, "autogain_smoothing_ms"),
                _ => false,
            }
        }
        // Generic: all other plugins use ParamSpec
        other => {
            let specs = other.param_specs();
            if let Some(spec) = specs.get(param_idx) {
                if let Some(current) = other.param_value(param_idx) {
                    let new_value = spec.adjust_f64(current, delta);
                    other.set_param_value(param_idx, new_value);
                    match other {
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
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
    }
}

// ============================================================================
// set_plugin_param_value — per-plugin set logic
// ============================================================================

/// Set a specific parameter value. Returns true if the parameter was set.
fn set_plugin_param_value(
    settings: &mut PluginSettings,
    param_idx: usize,
    value: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            stereo_width,
            center_spread,
            surround_direct_bleed,
            rear_late_reflection,
            lfe_cutoff_hz,
            lfe_gain,
            bandpass_hz,
            enable_subharmonic_synth,
            subharmonic_gain,
            subharmonic_freq_hz,
            subharmonic_attack_ms,
            subharmonic_release_ms,
            decorrelation_mode,
            decorrelation_lfo_rate_hz,
            velvet_noise_duration_ms,
            velvet_noise_density,
            enable_hr_direct,
            hr_sharpen,
            height_hf_cap_hz,
            height_transient_reduction,
            height_direct_leak,
            ambient_boost,
            safety_cap_db,
            rear_ambient_boost,
            dialogue_weight,
            voice_freq_min_hz,
            voice_freq_max_hz,
            dialogue_centroid_weight,
            dialogue_variance_weight,
            dialogue_coherence_weight,
            bypass_decorrelation,
            bypass_transient_detection,
            bypass_all_processing,
            enable_ml_detection,
            ..
        } => {
            use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP};
            macro_rules! set {
                ($field:expr, $key:literal) => {{
                    *$field = pk(UP, $key).clamp_f64(value);
                    true
                }};
            }
            let result = match param_idx {
                0 => {
                    let configs = pk(UP, "speaker_config").choice_labels();
                    let idx = (value as usize).clamp(0, configs.len() - 1);
                    *speaker_config = configs[idx].to_string();
                    *channel_count_changed = true;
                    true
                }
                1 => set!(gain_front_direct, "gain_front_direct"),
                2 => set!(gain_front_ambient, "gain_front_ambient"),
                3 => set!(gain_rear_ambient, "gain_rear_ambient"),
                4 => set!(height_gain, "height_gain"),
                5 => set!(lfe_gain, "lfe_gain"),
                6 => set!(lfe_cutoff_hz, "lfe_cutoff_hz"),
                7 => {
                    *enable_subharmonic_synth = value > 0.5;
                    true
                }
                8 => set!(subharmonic_gain, "subharmonic_gain"),
                9 => set!(subharmonic_freq_hz, "subharmonic_freq_hz"),
                10 => set!(subharmonic_attack_ms, "subharmonic_attack_ms"),
                11 => set!(subharmonic_release_ms, "subharmonic_release_ms"),
                12 => set!(stereo_width, "stereo_width"),
                13 => set!(center_spread, "center_spread"),
                14 => set!(bandpass_hz, "bandpass_hz"),
                15 => {
                    *enable_hr_direct = value > 0.5;
                    true
                }
                16 => set!(hr_sharpen, "hr_sharpen"),
                17 => set!(ambient_boost, "ambient_boost"),
                18 => {
                    *decorrelation_mode = if value > 0.5 { 1 } else { 0 };
                    true
                }
                19 => set!(decorrelation_lfo_rate_hz, "decorrelation_lfo_rate_hz"),
                20 => set!(velvet_noise_duration_ms, "velvet_noise_duration_ms"),
                21 => set!(velvet_noise_density, "velvet_noise_density"),
                22 => set!(height_hf_cap_hz, "height_hf_cap_hz"),
                23 => set!(height_transient_reduction, "height_transient_reduction"),
                24 => set!(height_direct_leak, "height_direct_leak"),
                25 => set!(surround_direct_bleed, "surround_direct_bleed"),
                26 => set!(rear_ambient_boost, "rear_ambient_boost"),
                27 => set!(rear_late_reflection, "rear_late_reflection"),
                28 => set!(dialogue_weight, "dialogue_weight"),
                29 => set!(voice_freq_min_hz, "voice_freq_min_hz"),
                30 => set!(voice_freq_max_hz, "voice_freq_max_hz"),
                31 => set!(dialogue_centroid_weight, "dialogue_centroid_weight"),
                32 => set!(dialogue_variance_weight, "dialogue_variance_weight"),
                33 => set!(dialogue_coherence_weight, "dialogue_coherence_weight"),
                34 => set!(safety_cap_db, "safety_cap_db"),
                35 => {
                    *bypass_decorrelation = value > 0.5;
                    true
                }
                36 => {
                    *bypass_transient_detection = value > 0.5;
                    true
                }
                37 => {
                    *bypass_all_processing = value > 0.5;
                    true
                }
                38 => {
                    *enable_ml_detection = value > 0.5;
                    true
                }
                _ => false,
            };
            if param_idx == 0 {
                *channel_count_changed = true;
            }
            result
        }
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
                        use crate::BiquadFilterType;
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
        PluginSettings::Convolution { mix, gain_db, .. } => {
            use sotf_plugins::param_specs::{convolution::PARAMS as CV, find_by_key as pk};
            match param_idx {
                // 0 = ir_file (handled by set_plugin_param_string)
                1 => {
                    *mix = pk(CV, "mix").clamp_f64(value);
                    true
                }
                2 => {
                    *gain_db = pk(CV, "gain_db").clamp_f64(value);
                    true
                }
                _ => false,
            }
        }
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
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
            _ => false,
        },
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
        PluginSettings::FletcherMunson {
            playback_volume_db,
            reference_level_db,
            enabled,
            smoothing_ms,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            auto_gain_loudness_type,
            band1_freq,
            band1_q,
            band1_max_gain,
            band1_slope,
            band2_freq,
            band2_q,
            band2_max_gain,
            band2_slope,
            band3_freq,
            band3_q,
            band3_max_gain,
            band3_slope,
            band4_freq,
            band4_q,
            band4_max_gain,
            band4_slope,
        } => {
            use sotf_plugins::param_specs::{find_by_key as pk, fletcher_munson::PARAMS as FM};
            match param_idx {
                0 => {
                    *playback_volume_db = value.clamp(-80.0, 0.0);
                    true
                }
                1 => {
                    *reference_level_db = pk(FM, "reference_level_db").clamp_f64(value);
                    true
                }
                2 => {
                    *enabled = value > 0.5;
                    true
                }
                3 => {
                    *smoothing_ms = pk(FM, "smoothing_ms").clamp_f64(value);
                    true
                }
                4 => {
                    *auto_gain_enabled = value > 0.5;
                    true
                }
                5 => {
                    *auto_gain_max_db = pk(FM, "auto_gain_max_db").clamp_f64(value);
                    true
                }
                6 => {
                    *auto_gain_smoothing_ms = pk(FM, "auto_gain_smoothing_ms").clamp_f64(value);
                    true
                }
                7 => {
                    *auto_gain_loudness_type = (value as i32).clamp(0, 1);
                    true
                }
                _ => {
                    if (8..24).contains(&param_idx) {
                        let rel_idx = param_idx - 8;
                        let band_idx = (rel_idx / 4) + 1;
                        let field_idx = rel_idx % 4;
                        let band_prefix = match band_idx {
                            1 => "band1",
                            2 => "band2",
                            3 => "band3",
                            4 => "band4",
                            _ => return false,
                        };
                        let (freq, q, max_gain, slope) = match band_idx {
                            1 => (band1_freq, band1_q, band1_max_gain, band1_slope),
                            2 => (band2_freq, band2_q, band2_max_gain, band2_slope),
                            3 => (band3_freq, band3_q, band3_max_gain, band3_slope),
                            4 => (band4_freq, band4_q, band4_max_gain, band4_slope),
                            _ => return false,
                        };
                        let keys = [
                            format!("{}_freq", band_prefix),
                            format!("{}_q", band_prefix),
                            format!("{}_max_gain", band_prefix),
                            format!("{}_slope", band_prefix),
                        ];
                        match field_idx {
                            0 => {
                                *freq = pk(FM, &keys[0]).clamp_f64(value);
                                true
                            }
                            1 => {
                                *q = pk(FM, &keys[1]).clamp_f64(value);
                                true
                            }
                            2 => {
                                *max_gain = pk(FM, &keys[2]).clamp_f64(value);
                                true
                            }
                            3 => {
                                *slope = pk(FM, &keys[3]).clamp_f64(value);
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
            }
        }
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
        // Generic: all other plugins use ParamSpec
        other => {
            let specs = other.param_specs();
            if let Some(spec) = specs.get(param_idx) {
                let raw = value / spec.display_scale;
                other.set_param_value(param_idx, spec.clamp_f64(raw));
                match other {
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
                true
            } else {
                false
            }
        }
    }
}
