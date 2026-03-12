//! Autocomplete methods.
//!
//! Contains methods for directory and plugin file autocomplete.

use super::state::App;

impl App {
    // Directory autocomplete methods

    /// Generate autocomplete suggestions for the current directory input
    pub fn generate_autocomplete_suggestions(&mut self) {
        self.input_state.autocomplete_suggestions.clear();
        self.input_state.autocomplete_index = 0;

        let input = if self.input_state.directory_input.is_empty() {
            "./"
        } else {
            &self.input_state.directory_input
        };

        // Expand tilde to home directory
        let expanded_input = if input.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                input.replacen('~', &home, 1)
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        };

        let path = std::path::Path::new(&expanded_input);

        // Determine the directory to search and the prefix to match
        let (search_dir, prefix) = if path.is_dir() && expanded_input.ends_with('/') {
            // User typed a complete directory with trailing slash
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            // User is typing a partial name
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            // Fallback to current directory
            (std::path::PathBuf::from("."), expanded_input.clone())
        };

        // Read directory and find matching entries
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip hidden files unless prefix starts with '.'
                    if file_name.starts_with('.') && !prefix.starts_with('.') {
                        continue;
                    }

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.input_state.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.input_state.autocomplete_suggestions.sort();
    }

    /// Apply the current autocomplete suggestion to the directory input
    pub fn apply_autocomplete(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            let suggestion =
                &self.input_state.autocomplete_suggestions[self.input_state.autocomplete_index];
            self.input_state.directory_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion
    pub fn next_autocomplete(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            self.input_state.autocomplete_index = (self.input_state.autocomplete_index + 1)
                % self.input_state.autocomplete_suggestions.len();
            self.apply_autocomplete();
        }
    }

    /// Clear autocomplete suggestions
    pub fn clear_autocomplete(&mut self) {
        self.input_state.autocomplete_suggestions.clear();
        self.input_state.autocomplete_index = 0;
    }

    /// Generate autocomplete suggestions for plugin file save (preset names only)
    pub fn generate_autocomplete_suggestions_for_save_preset(&mut self) {
        self.input_state.autocomplete_suggestions.clear();
        self.input_state.autocomplete_index = 0;

        let prefix = self.input_state.plugin_file_input.to_lowercase();

        for preset in &self.plugin_state.available_presets {
            // Strip .json extension for suggestion
            let name = preset.strip_suffix(".json").unwrap_or(preset);
            if name.to_lowercase().starts_with(&prefix) {
                self.input_state
                    .autocomplete_suggestions
                    .push(name.to_string());
            }
        }

        self.input_state.autocomplete_suggestions.sort();
    }

    /// Generate autocomplete suggestions for plugin file load (file paths)
    pub fn generate_autocomplete_suggestions_for_plugin_file(&mut self) {
        self.input_state.autocomplete_suggestions.clear();
        self.input_state.autocomplete_index = 0;

        let input = &self.input_state.plugin_file_input;

        // Check if it's a path or just a preset name
        if input.contains('/') || input.contains('\\') {
            // Full path autocomplete
            let path = std::path::Path::new(input);
            let (search_dir, prefix) = if path.is_dir() {
                (path.to_path_buf(), String::new())
            } else {
                let parent = path.parent().unwrap_or(std::path::Path::new("."));
                let prefix = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                (parent.to_path_buf(), prefix)
            };

            if let Ok(entries) = std::fs::read_dir(&search_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_name) = entry.file_name().into_string()
                        && file_name.to_lowercase().starts_with(&prefix.to_lowercase())
                    {
                        let full_path = search_dir.join(&file_name);
                        self.input_state
                            .autocomplete_suggestions
                            .push(full_path.to_string_lossy().to_string());
                    }
                }
            }
        } else {
            // Preset name autocomplete
            let prefix = input.to_lowercase();
            for preset in &self.plugin_state.available_presets {
                let name = preset.strip_suffix(".json").unwrap_or(preset);
                if name.to_lowercase().starts_with(&prefix) {
                    self.input_state
                        .autocomplete_suggestions
                        .push(name.to_string());
                }
            }
        }

        self.input_state.autocomplete_suggestions.sort();
    }

    /// Apply the current autocomplete suggestion to the plugin file input
    pub fn apply_autocomplete_to_plugin_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            let suggestion =
                &self.input_state.autocomplete_suggestions[self.input_state.autocomplete_index];
            self.input_state.plugin_file_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion for plugin file
    pub fn next_autocomplete_for_plugin_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            self.input_state.autocomplete_index = (self.input_state.autocomplete_index + 1)
                % self.input_state.autocomplete_suggestions.len();
            self.apply_autocomplete_to_plugin_file();
        }
    }

    // APO file autocomplete methods

    /// Generate autocomplete suggestions for APO file input (file paths with .txt extension filter)
    pub fn generate_autocomplete_suggestions_for_apo_file(&mut self) {
        self.generate_file_autocomplete_suggestions(
            &self.input_state.apo_file_input.clone(),
            Some(&["txt"]),
        );
    }

    /// Apply the current autocomplete suggestion to the APO file input
    pub fn apply_autocomplete_to_apo_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            let suggestion =
                &self.input_state.autocomplete_suggestions[self.input_state.autocomplete_index];
            self.input_state.apo_file_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion for APO file
    pub fn next_autocomplete_for_apo_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            self.input_state.autocomplete_index = (self.input_state.autocomplete_index + 1)
                % self.input_state.autocomplete_suggestions.len();
            self.apply_autocomplete_to_apo_file();
        }
    }

    // SOFA file autocomplete methods

    /// Generate autocomplete suggestions for SOFA file input (file paths with .sofa extension filter)
    pub fn generate_autocomplete_suggestions_for_sofa_file(&mut self) {
        self.generate_file_autocomplete_suggestions(
            &self.input_state.sofa_file_input.clone(),
            Some(&["sofa"]),
        );
    }

    /// Apply the current autocomplete suggestion to the SOFA file input
    pub fn apply_autocomplete_to_sofa_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            let suggestion =
                &self.input_state.autocomplete_suggestions[self.input_state.autocomplete_index];
            self.input_state.sofa_file_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion for SOFA file
    pub fn next_autocomplete_for_sofa_file(&mut self) {
        if !self.input_state.autocomplete_suggestions.is_empty() {
            self.input_state.autocomplete_index = (self.input_state.autocomplete_index + 1)
                % self.input_state.autocomplete_suggestions.len();
            self.apply_autocomplete_to_sofa_file();
        }
    }

    // Generic file autocomplete helper

    /// Generate file path autocomplete suggestions with optional extension filter
    /// If extensions is Some, only files with those extensions (and directories) are shown.
    fn generate_file_autocomplete_suggestions(&mut self, input: &str, extensions: Option<&[&str]>) {
        self.input_state.autocomplete_suggestions.clear();
        self.input_state.autocomplete_index = 0;

        let input = if input.is_empty() { "./" } else { input };

        // Expand tilde to home directory
        let expanded_input = if input.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                input.replacen('~', &home, 1)
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        };

        let path = std::path::Path::new(&expanded_input);

        // Determine the directory to search and the prefix to match
        let (search_dir, prefix) = if path.is_dir() && expanded_input.ends_with('/') {
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            (std::path::PathBuf::from("."), expanded_input.clone())
        };

        // Read directory and find matching entries
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip hidden files unless prefix starts with '.'
                    if file_name.starts_with('.') && !prefix.starts_with('.') {
                        continue;
                    }

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let entry_path = entry.path();
                        let is_dir = entry_path.is_dir();

                        // Apply extension filter for files (not directories)
                        if !is_dir && let Some(exts) = extensions {
                            let has_valid_ext = entry_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| exts.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
                                .unwrap_or(false);
                            if !has_valid_ext {
                                continue;
                            }
                        }

                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if is_dir {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.input_state.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        self.input_state.autocomplete_suggestions.sort();
    }
}
