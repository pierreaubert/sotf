//! Input state management.
//!
//! Contains all state related to text inputs, autocomplete,
//! and parameter editing.

/// Input state for text fields and autocomplete
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Directory path input (for library scanning)
    pub directory_input: String,
    /// Plugin file path input (for save/load plugin chain)
    pub plugin_file_input: String,
    /// APO EQ file path input (for loading APO EQ files)
    pub apo_file_input: String,
    /// SOFA file path input (for loading SOFA HRTF files)
    pub sofa_file_input: String,

    /// Autocomplete suggestions for current input
    pub autocomplete_suggestions: Vec<String>,
    /// Currently selected autocomplete index
    pub autocomplete_index: usize,

    /// Parameter currently being edited (e.g., "frequency", "gain")
    pub editing_param: Option<String>,
    /// Current value in the editing text field
    pub editing_value: String,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the autocomplete state
    pub fn clear_autocomplete(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;
    }

    /// Select next autocomplete suggestion
    pub fn next_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index = (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
        }
    }

    /// Select previous autocomplete suggestion
    pub fn prev_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            if self.autocomplete_index == 0 {
                self.autocomplete_index = self.autocomplete_suggestions.len() - 1;
            } else {
                self.autocomplete_index -= 1;
            }
        }
    }

    /// Get currently selected autocomplete suggestion
    pub fn selected_suggestion(&self) -> Option<&str> {
        self.autocomplete_suggestions.get(self.autocomplete_index).map(|s| s.as_str())
    }

    /// Clear the parameter editing state
    pub fn clear_editing(&mut self) {
        self.editing_param = None;
        self.editing_value.clear();
    }

    /// Start editing a parameter
    pub fn start_editing(&mut self, param: &str, initial_value: &str) {
        self.editing_param = Some(param.to_string());
        self.editing_value = initial_value.to_string();
    }
}
