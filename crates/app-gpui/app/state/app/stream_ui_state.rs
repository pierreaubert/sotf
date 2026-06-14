#[derive(Debug, Clone)]
pub struct StreamUiState {
    pub store: sotf_audio_player::SavedStreamStore,
    pub selected_index: usize,
    pub name_input: String,
    pub url_input: String,
    pub format_hint_input: String,
    pub seekable_input: bool,
    pub last_error: Option<String>,
    pub last_status: Option<String>,
}

impl Default for StreamUiState {
    fn default() -> Self {
        Self {
            store: sotf_audio_player::load_saved_streams().unwrap_or_default(),
            selected_index: 0,
            name_input: String::new(),
            url_input: String::new(),
            format_hint_input: String::new(),
            seekable_input: false,
            last_error: None,
            last_status: None,
        }
    }
}

impl StreamUiState {
    pub fn format_hint(&self) -> Option<String> {
        let hint = self.format_hint_input.trim();
        (!hint.is_empty()).then(|| hint.to_string())
    }
}
