use super::app_impl::App;

/// Compute the longest common prefix of all suggestions.
fn common_prefix(suggestions: &[String]) -> String {
    let Some(first) = suggestions.first() else {
        return String::new();
    };
    let mut prefix_len = first.len();
    for s in &suggestions[1..] {
        prefix_len = first
            .chars()
            .zip(s.chars())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count()
            .min(prefix_len);
    }
    // Use byte-safe slicing via chars
    first.chars().take(prefix_len).collect()
}

/// Which autocomplete generator to use.
pub enum AutocompleteKind {
    /// Filesystem path completion from `generate_autocomplete_suggestions_for_input`
    FilePath,
    /// Preset name completion from `generate_autocomplete_suggestions_for_save_preset`
    PresetName,
}

impl App {
    // ========================================================================
    // Zsh-style Tab completion — unified entry point
    // ========================================================================

    /// Handle a Tab press with zsh-style behavior.
    ///
    /// - First Tab: generate suggestions. If none → "no matches". If one → apply it.
    ///   If many → complete to common prefix, show menu, highlight first item.
    /// - Subsequent Tabs: cycle forward through the menu.
    ///
    /// `get_input` reads the current input, `set_input` writes it back.
    /// `kind` selects which generator to use.
    pub fn zsh_tab_complete(
        &mut self,
        get_input: fn(&Self) -> &str,
        set_input: fn(&mut Self, String),
        kind: AutocompleteKind,
    ) {
        if self.autocomplete.menu_active {
            // Menu is showing — check if the user has already selected a suggestion
            // (inline refresh shows the menu but doesn't set the input to a suggestion)
            let current_input = get_input(self).to_string();
            let already_selected = self
                .autocomplete
                .suggestions
                .get(self.autocomplete.index)
                .is_some_and(|s| *s == current_input);
            if already_selected {
                // Cycle forward to next suggestion
                self.autocomplete.index =
                    (self.autocomplete.index + 1) % self.autocomplete.suggestions.len();
            }
            // Apply current suggestion
            let value = self.autocomplete.suggestions[self.autocomplete.index].clone();
            set_input(self, value);
            return;
        }

        // First Tab: generate suggestions
        let input = get_input(self).to_string();
        match kind {
            AutocompleteKind::FilePath => {
                self.generate_autocomplete_suggestions_for_input(&input);
            }
            AutocompleteKind::PresetName => {
                self.generate_autocomplete_suggestions_for_save_preset_from(&input);
            }
        }

        match self.autocomplete.suggestions.len() {
            0 => {
                // No matches
                self.ui.status_message = Some("No matches".to_string());
            }
            1 => {
                // Single match — apply directly, no menu
                let value = self.autocomplete.suggestions[0].clone();
                set_input(self, value);
                self.clear_autocomplete();
            }
            _ => {
                // Multiple matches — complete to common prefix, show menu
                let prefix = common_prefix(&self.autocomplete.suggestions);
                self.autocomplete.menu_active = true;
                self.autocomplete.index = 0;

                // If common prefix is longer than input, apply it but don't select any item yet
                if prefix.len() > input.len() {
                    set_input(self, prefix);
                } else {
                    // Common prefix doesn't extend input — highlight first item
                    let value = self.autocomplete.suggestions[self.autocomplete.index].clone();
                    set_input(self, value);
                }
            }
        }
    }

    /// Handle Shift+Tab (BackTab) — cycle backward through menu.
    pub fn zsh_backtab_complete(&mut self, set_input: fn(&mut Self, String)) {
        if !self.autocomplete.menu_active || self.autocomplete.suggestions.is_empty() {
            return;
        }
        if self.autocomplete.index == 0 {
            self.autocomplete.index = self.autocomplete.suggestions.len() - 1;
        } else {
            self.autocomplete.index -= 1;
        }
        let value = self.autocomplete.suggestions[self.autocomplete.index].clone();
        set_input(self, value);
    }

    /// Move autocomplete selection down (next suggestion).
    /// Returns true if the event was consumed.
    pub fn autocomplete_down(&mut self, set_input: fn(&mut Self, String)) -> bool {
        if !self.autocomplete.menu_active || self.autocomplete.suggestions.is_empty() {
            return false;
        }
        self.autocomplete.index =
            (self.autocomplete.index + 1) % self.autocomplete.suggestions.len();
        let value = self.autocomplete.suggestions[self.autocomplete.index].clone();
        set_input(self, value);
        true
    }

    /// Move autocomplete selection up (previous suggestion).
    /// Returns true if the event was consumed.
    pub fn autocomplete_up(&mut self, set_input: fn(&mut Self, String)) -> bool {
        if !self.autocomplete.menu_active || self.autocomplete.suggestions.is_empty() {
            return false;
        }
        if self.autocomplete.index == 0 {
            self.autocomplete.index = self.autocomplete.suggestions.len() - 1;
        } else {
            self.autocomplete.index -= 1;
        }
        let value = self.autocomplete.suggestions[self.autocomplete.index].clone();
        set_input(self, value);
        true
    }

    // ========================================================================
    // Generators
    // ========================================================================

    /// Generate filesystem-based autocomplete suggestions from an input path.
    fn generate_autocomplete_suggestions_for_input(&mut self, input: &str) {
        self.autocomplete.suggestions.clear();
        self.autocomplete.index = 0;

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

                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.autocomplete.suggestions.push(suggestion);
                    }
                }
            }
        }

        self.autocomplete.suggestions.sort();
    }

    /// Generate preset name suggestions filtered by prefix.
    fn generate_autocomplete_suggestions_for_save_preset_from(&mut self, input: &str) {
        self.autocomplete.suggestions.clear();
        self.autocomplete.index = 0;

        let input_lower = input.trim_end_matches(".json").to_lowercase();

        for preset in &self.plugin_rack.available_presets {
            let preset_without_ext = preset.trim_end_matches(".json");
            if preset_without_ext.to_lowercase().starts_with(&input_lower) {
                self.autocomplete
                    .suggestions
                    .push(preset_without_ext.to_string());
            }
        }

        self.autocomplete.suggestions.sort();
    }

    // ========================================================================
    // State management
    // ========================================================================

    /// Clear autocomplete suggestions and menu state.
    pub fn clear_autocomplete(&mut self) {
        self.autocomplete.suggestions.clear();
        self.autocomplete.index = 0;
        self.autocomplete.menu_active = false;
    }

    /// Refresh autocomplete suggestions inline (as-you-type).
    ///
    /// Regenerates the suggestion list from the current input without
    /// modifying the input text. The dropdown is shown if any matches
    /// exist, but Tab-cycling state is reset so the next Tab still works
    /// as a first-press.
    pub fn refresh_autocomplete_inline(
        &mut self,
        get_input: fn(&Self) -> &str,
        kind: AutocompleteKind,
    ) {
        let input = get_input(self).to_string();
        if input.is_empty() {
            self.clear_autocomplete();
            return;
        }
        match kind {
            AutocompleteKind::FilePath => {
                self.generate_autocomplete_suggestions_for_input(&input);
            }
            AutocompleteKind::PresetName => {
                self.generate_autocomplete_suggestions_for_save_preset_from(&input);
            }
        }
        self.autocomplete.menu_active = !self.autocomplete.suggestions.is_empty();
        self.autocomplete.index = 0;
    }
}

// ============================================================================
// Input accessor/setter helpers for each field
// ============================================================================

// These are free functions matching `fn(&App) -> &str` and `fn(&mut App, String)`.

pub fn get_directory_input(app: &App) -> &str {
    &app.library_view.directory_input
}
pub fn set_directory_input(app: &mut App, val: String) {
    app.library_view.directory_input = val;
}

pub fn get_plugin_file_input(app: &App) -> &str {
    &app.plugin_rack.file_input
}
pub fn set_plugin_file_input(app: &mut App, val: String) {
    app.plugin_rack.file_input = val;
}

pub fn get_apo_file_input(app: &App) -> &str {
    &app.plugin_rack.apo_input
}
pub fn set_apo_file_input(app: &mut App, val: String) {
    app.plugin_rack.apo_input = val;
}

pub fn get_sofa_file_input(app: &App) -> &str {
    &app.plugin_rack.sofa_input
}
pub fn set_sofa_file_input(app: &mut App, val: String) {
    app.plugin_rack.sofa_input = val;
}

pub fn get_headphone_measurement_path(app: &App) -> &str {
    &app.headphone_eq.model.measurement_path
}
pub fn set_headphone_measurement_path(app: &mut App, val: String) {
    app.headphone_eq.model.measurement_path = val;
}

pub fn get_headphone_custom_target_path(app: &App) -> &str {
    &app.headphone_eq.model.custom_target_path
}
pub fn set_headphone_custom_target_path(app: &mut App, val: String) {
    app.headphone_eq.model.custom_target_path = val;
}

pub fn get_room_eq_file_path(app: &App) -> &str {
    &app.room_eq.file_path
}
pub fn set_room_eq_file_path(app: &mut App, val: String) {
    app.room_eq.file_path = val;
}

pub fn get_room_eq_export_path(app: &App) -> &str {
    &app.room_eq.export_path
}
pub fn set_room_eq_export_path(app: &mut App, val: String) {
    app.room_eq.export_path = val;
}

pub fn get_recording_output_dir(app: &App) -> &str {
    &app.recording.output_directory
}
pub fn set_recording_output_dir(app: &mut App, val: String) {
    app.recording.output_directory = val;
}

pub fn get_recording_mic_cal_path(app: &App) -> &str {
    app.recording.active_mic_cal_path()
}
pub fn set_recording_mic_cal_path(app: &mut App, val: String) {
    app.recording.set_active_mic_cal_path(val);
}
