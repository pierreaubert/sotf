use super::app_impl::{App, get_param_count};
use super::parameters::TuiEditablePlugin;
use super::types::InputMode;
use sotf_audio_player::PluginType;

impl App {
    // Plugin management

    /// Request a plugin update and reset retry state
    /// This should be called whenever the plugin chain is modified
    pub fn request_plugin_update(&mut self) {
        self.needs_plugin_update = true;
        self.plugin_update_retry_count = 0;
        self.plugin_update_in_progress = false;
    }

    pub fn add_plugin(&mut self, plugin_type: &PluginType) {
        let insert_idx = self.plugin_graph.user_plugin_insert_index();
        self.plugin_graph
            .insert_plugin(insert_idx, plugin_type)
            .ok();
        // Update BinauralDecoder input channels after adding
        self.plugin_graph.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn remove_plugin(&mut self, index: usize) {
        self.plugin_graph.remove_plugin_by_index(index).ok();
        if self.selected_plugin_index >= self.plugin_graph.len() && self.selected_plugin_index > 0 {
            self.selected_plugin_index = self.plugin_graph.len() - 1;
        }
        // Update BinauralDecoder input channels after removal
        self.plugin_graph.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_graph.toggle_plugin_by_index(index).ok();
        // Update BinauralDecoder input channels after toggle
        self.plugin_graph.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if self.plugin_graph.can_move_up_by_index(index) {
            self.plugin_graph.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_graph.update_channel_dependent_plugins();
            self.request_plugin_update();
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if self.plugin_graph.can_move_down_by_index(index) {
            self.plugin_graph.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_graph.update_channel_dependent_plugins();
            self.request_plugin_update();
        }
    }

    pub fn select_next_plugin(&mut self) {
        if !self.plugin_graph.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.plugin_graph.len();
        }
    }

    pub fn select_previous_plugin(&mut self) {
        if !self.plugin_graph.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.plugin_graph.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    // Plugin parameter editing
    pub fn enter_plugin_edit_mode(&mut self) {
        if self.selected_plugin_index < self.plugin_graph.len() {
            self.editing_plugin_index = Some(self.selected_plugin_index);
            self.plugin_param_selection = 0;
            self.input_mode = InputMode::EditPlugin;
        }
    }

    pub fn exit_plugin_edit_mode(&mut self) {
        self.editing_plugin_index = None;
        self.plugin_param_selection = 0;
        self.input_mode = InputMode::Normal;
    }

    pub fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_graph.get_plugin(idx))
    }

    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_graph.get_plugin_mut(idx))
    }

    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

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

    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    pub fn adjust_selected_param(&mut self, delta: f64) -> bool {
        let param_idx = self.plugin_param_selection;

        let success = if let Some(plugin) = self.get_editing_plugin_mut() {
            plugin.settings.adjust_param(param_idx, delta)
        } else {
            false
        };

        if success {
            // Always propagate channel counts — a parameter change (e.g., upmixer speaker config)
            // may change intermediate channel counts that downstream plugins depend on
            self.plugin_graph.update_channel_dependent_plugins();
        }

        success
    }

    // ========================================================================
    // Matrix Editor Methods
    // ========================================================================

    /// Get the dimensions of the currently editing Matrix plugin
    pub fn get_matrix_dimensions(&self) -> Option<(usize, usize)> {
        use sotf_audio_player::PluginSettings;
        if let Some(plugin) = self.get_editing_plugin()
            && let PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } = &plugin.settings
        {
            return Some((*input_channels, *output_channels));
        }
        None
    }

    /// Adjust the selected matrix header parameter (input channels, output channels, or preset)
    /// Returns true if adjustment was made
    pub fn adjust_matrix_header(&mut self, delta: i32) -> bool {
        use sotf_audio_player::{PluginSettings, apply_matrix_preset, resize_matrix};

        // Read selection before mutable borrow
        let header_selection = self.matrix_header_selection;

        // Track whether we need to clamp grid selection and the new dimensions
        let mut clamp_col_to: Option<usize> = None;
        let mut clamp_row_to: Option<usize> = None;

        let result = {
            let Some(plugin) = self.get_editing_plugin_mut() else {
                return false;
            };

            let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                ..
            } = &mut plugin.settings
            else {
                return false;
            };

            match header_selection {
                0 => {
                    // Input channels: 1-16
                    let old_in = *input_channels;
                    let new_in = (*input_channels as i32 + delta).clamp(1, 16) as usize;
                    if new_in != old_in {
                        resize_matrix(matrix, old_in, *output_channels, new_in, *output_channels);
                        *input_channels = new_in;
                        clamp_col_to = Some(new_in);
                        true
                    } else {
                        false
                    }
                }
                1 => {
                    // Output channels: 1-16
                    let old_out = *output_channels;
                    let new_out = (*output_channels as i32 + delta).clamp(1, 16) as usize;
                    if new_out != old_out {
                        resize_matrix(matrix, *input_channels, old_out, *input_channels, new_out);
                        *output_channels = new_out;
                        clamp_row_to = Some(new_out);
                        true
                    } else {
                        false
                    }
                }
                2 => {
                    // Preset: cycle through presets valid for current channel config
                    let in_ch = *input_channels;
                    let out_ch = *output_channels;
                    let presets = sotf_audio_player::available_matrix_presets(in_ch, out_ch);
                    let current = sotf_audio_player::detect_matrix_preset(in_ch, out_ch, matrix);
                    let current_idx = presets.iter().position(|&p| p == current).unwrap_or(0);
                    let new_idx = if delta > 0 {
                        (current_idx + 1) % presets.len()
                    } else {
                        (current_idx + presets.len() - 1) % presets.len()
                    };
                    apply_matrix_preset(in_ch, out_ch, matrix, presets[new_idx]);
                    true
                }
                _ => false,
            }
        }; // Mutable borrow ends here

        // Clamp grid selection after borrow is released
        if let Some(max_col) = clamp_col_to
            && self.matrix_grid_col >= max_col
        {
            self.matrix_grid_col = max_col.saturating_sub(1);
        }
        if let Some(max_row) = clamp_row_to
            && self.matrix_grid_row >= max_row
        {
            self.matrix_grid_row = max_row.saturating_sub(1);
        }

        result
    }

    /// Adjust the selected matrix cell gain by dB amount
    /// Returns true if adjustment was made
    pub fn adjust_matrix_cell(&mut self, delta_db: f32) -> bool {
        use sotf_audio_player::{PluginSettings, db_to_linear};

        // Read grid position before mutable borrow
        let grid_row = self.matrix_grid_row;
        let grid_col = self.matrix_grid_col;

        let Some(plugin) = self.get_editing_plugin_mut() else {
            return false;
        };

        let PluginSettings::Matrix {
            input_channels,
            matrix,
            ..
        } = &mut plugin.settings
        else {
            return false;
        };

        let idx = grid_row * *input_channels + grid_col;
        if idx >= matrix.len() {
            return false;
        }

        let current = matrix[idx];
        // Convert to dB, adjust, convert back
        let current_db = if current < 0.001 {
            -60.0 // Treat as -60 dB for adjustment
        } else {
            20.0 * current.log10()
        };
        let new_db = (current_db + delta_db).clamp(-60.0, 6.0);
        let new_linear = if new_db <= -60.0 {
            0.0 // Silence
        } else {
            db_to_linear(new_db)
        };
        matrix[idx] = new_linear;
        true
    }

    /// Set the selected matrix cell to a specific linear gain value
    /// Returns true if adjustment was made
    pub fn set_matrix_cell(&mut self, linear_gain: f32) -> bool {
        use sotf_audio_player::PluginSettings;

        // Read grid position before mutable borrow
        let grid_row = self.matrix_grid_row;
        let grid_col = self.matrix_grid_col;

        let Some(plugin) = self.get_editing_plugin_mut() else {
            return false;
        };

        let PluginSettings::Matrix {
            input_channels,
            matrix,
            ..
        } = &mut plugin.settings
        else {
            return false;
        };

        let idx = grid_row * *input_channels + grid_col;
        if idx >= matrix.len() {
            return false;
        }

        matrix[idx] = linear_gain.clamp(0.0, 2.0);
        true
    }

    // ========================================================================

    /// Save plugin chain to file
    pub fn save_plugins(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Check if file exists and show warning if overwriting
        let filename_with_ext = if self.plugin_file_input.ends_with(".json") {
            self.plugin_file_input.clone()
        } else {
            format!("{}.json", self.plugin_file_input)
        };

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
            let full_path = presets_dir.join(&filename_with_ext);
            if full_path.exists() {
                self.status_message = Some(format!(
                    "Warning: Overwriting existing preset: {}",
                    filename_with_ext
                ));
                log::warn!("Overwriting existing preset: {}", filename_with_ext);
            }
        }

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.status_message = Some("Error: Could not find presets directory".to_string());
            return;
        };
        match self
            .plugin_graph
            .save_to_file(&presets_dir, &self.plugin_file_input)
        {
            Ok(_) => {
                self.status_message = Some(format!("Saved preset: {}", filename_with_ext));
                self.last_loaded_preset = Some(filename_with_ext);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.status_message = Some(format!("Error saving: {}", e));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    /// Save plugin chain to selected preset file (overwrite confirmation shown in UI)
    pub fn save_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            // Pass filename as-is; save_to_file handles .json extension correctly
            // Save using the plugin chain's own save method
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.status_message = Some("Error: Could not find presets directory".to_string());
                return;
            };
            match self
                .plugin_graph
                .save_to_file(&presets_dir, &preset_filename)
            {
                Ok(_) => {
                    self.status_message = Some(format!("Overwritten preset: {}", preset_filename));
                    self.last_loaded_preset = Some(preset_filename);
                    // Refresh presets list
                    self.refresh_plugin_presets();
                }
                Err(e) => {
                    self.status_message = Some(format!("Error saving: {}", e));
                    log::error!("Failed to save plugin chain: {}", e);
                }
            }
        }
    }

    /// Load plugin chain from file
    pub fn load_plugins(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Load using the plugin chain's own load method (handles path, extension, etc.)
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.status_message = Some("Error: Could not find presets directory".to_string());
            return;
        };
        match self
            .plugin_graph
            .load_from_file(&presets_dir, &self.plugin_file_input)
        {
            Ok(warnings) => {
                // Update BinauralDecoder input channels after loading
                self.plugin_graph.update_channel_dependent_plugins();

                // Get the final filename (with .json appended if needed)
                let filename = if self.plugin_file_input.ends_with(".json") {
                    self.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.plugin_file_input)
                };

                if warnings.is_empty() {
                    self.status_message = Some(format!("Loaded preset: {}", filename));
                } else {
                    self.status_message = Some(format!(
                        "Loaded preset: {} ({} plugin(s) skipped)",
                        filename,
                        warnings.len()
                    ));
                    for w in &warnings {
                        log::warn!("{}", w);
                    }
                }
                self.request_plugin_update();
                self.last_loaded_preset = Some(filename);
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading: {}", e));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    /// Refresh the list of available plugin presets from the config directory
    pub fn refresh_plugin_presets(&mut self) {
        self.available_plugin_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_plugin_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            // Sort presets alphabetically
            self.available_plugin_presets.sort();
        }

        log::info!(
            "Found {} plugin presets",
            self.available_plugin_presets.len()
        );
    }

    /// Select the next preset in the list
    pub fn select_next_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_plugin_presets.len();
        }
    }

    /// Select the previous preset in the list
    pub fn select_previous_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_plugin_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }

    /// Load the currently selected preset
    pub fn load_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            log::warn!("No presets available to load");
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            log::info!(
                "Loading preset: {} (index {})",
                preset_filename,
                self.selected_preset_index
            );
            // Use the plugin chain's own load method (handles path construction)
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.status_message = Some("Error: Could not find presets directory".to_string());
                return;
            };
            match self
                .plugin_graph
                .load_from_file(&presets_dir, &preset_filename)
            {
                Ok(warnings) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_graph.update_channel_dependent_plugins();

                    if warnings.is_empty() {
                        log::info!(
                            "Successfully loaded preset: {} ({} plugins)",
                            preset_filename,
                            self.plugin_graph.len()
                        );
                        self.status_message = Some(format!("Loaded preset: {}", preset_filename));
                    } else {
                        log::warn!(
                            "Loaded preset: {} ({} plugins, {} skipped)",
                            preset_filename,
                            self.plugin_graph.len(),
                            warnings.len()
                        );
                        for w in &warnings {
                            log::warn!("  {}", w);
                        }
                        self.status_message = Some(format!(
                            "Loaded preset: {} ({} plugin(s) skipped)",
                            preset_filename,
                            warnings.len()
                        ));
                    }
                    self.request_plugin_update();
                    self.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.status_message = Some(format!("Error loading preset: {}", e));
                    log::error!("Failed to load preset {}: {}", preset_filename, e);
                }
            }
        } else {
            log::error!(
                "Failed to get preset at index {}",
                self.selected_preset_index
            );
        }
    }
}
