use super::app_impl::App;

impl App {

    /// Generate autocomplete suggestions for the current directory input
    pub fn generate_autocomplete_suggestions(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        let input = if self.directory_input.is_empty() {
            "./"
        } else {
            &self.directory_input
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
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
    }

    /// Apply the current autocomplete suggestion to the directory input
    pub fn apply_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.directory_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion
    pub fn next_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete();
        }
    }

    /// Clear autocomplete suggestions
    pub fn clear_autocomplete(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;
    }

    /// Generate autocomplete suggestions for saving presets (restricted to preset directory)
    /// This filters available presets by the current input and provides suggestions
    pub fn generate_autocomplete_suggestions_for_save_preset(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        // Get the current input (without .json extension if present)
        let input = self
            .plugin_file_input
            .trim_end_matches(".json")
            .to_lowercase();

        // Filter available presets by prefix match
        for preset in &self.available_plugin_presets {
            let preset_without_ext = preset.trim_end_matches(".json");
            if preset_without_ext.to_lowercase().starts_with(&input) {
                // Add suggestion without .json extension (save_to_file will add it)
                self.autocomplete_suggestions
                    .push(preset_without_ext.to_string());
            }
        }

        // Sort suggestions alphabetically
        self.autocomplete_suggestions.sort();
    }

    /// Generate autocomplete suggestions for plugin file input
    pub fn generate_autocomplete_suggestions_for_plugin_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.plugin_file_input.clone());
    }

    /// Apply autocomplete to plugin file input
    pub fn apply_autocomplete_to_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.plugin_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for plugin file input
    pub fn next_autocomplete_for_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_plugin_file();
        }
    }

    /// Generate autocomplete suggestions for APO file input
    pub fn generate_autocomplete_suggestions_for_apo_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.apo_file_input.clone());
    }

    /// Apply autocomplete to APO file input
    pub fn apply_autocomplete_to_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.apo_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for APO file input
    pub fn next_autocomplete_for_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_apo_file();
        }
    }

    /// Generate autocomplete suggestions for SOFA file input
    pub fn generate_autocomplete_suggestions_for_sofa_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.sofa_file_input.clone());
    }

    /// Apply autocomplete to SOFA file input
    pub fn apply_autocomplete_to_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.sofa_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for SOFA file input
    pub fn next_autocomplete_for_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_sofa_file();
        }
    }

    /// Generic autocomplete suggestions generator for any file input
    fn generate_autocomplete_suggestions_for_input(&mut self, input: &str) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

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
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
    }

}
