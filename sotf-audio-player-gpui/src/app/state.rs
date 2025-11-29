//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use std::sync::Arc;

use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{
    LoudnessData, MusicLibrary, Player, PluginChain, PluginType, SpectrumData,
};

use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{Theme, ThemeId};

use super::types::{
    ActiveMenu, ChannelFilter, ChannelGroup, ContextMenuState, InputMode, LayoutMode, LetterNode,
    LibrarySortOrder, LibraryViewMode, QueueItem, Screen, ToastMessage,
};

#[derive(Debug)]
pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueItem>,
    pub expanded_queue_items: Vec<bool>, // Track which queue items are expanded
    pub current_screen: Screen,
    pub input_mode: InputMode,

    // UI state
    pub search_query: String,
    pub directory_input: String,
    pub plugin_file_input: String,    // For save/load plugin chain
    pub apo_file_input: String,       // For loading APO EQ files
    pub sofa_file_input: String,      // For loading SOFA HRTF files
    pub headphone_curve_path: String, // Headphone measurement file
    pub headphone_target: String,     // Selected target curve
    pub headphone_params: crate::optimization_params::OptimizationParams, // All optimization parameters
    pub headphone_optimization_running: bool, // Is optimization in progress
    pub headphone_optimization_progress: Vec<(usize, f64)>, // (iteration, fitness)
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub selected_plugin_index: usize,
    pub album_list_offset: usize,
    pub toast_message: Option<ToastMessage>, // Enhanced toast notifications

    // Autocomplete state
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_index: usize,

    // Plugin preset selection
    pub available_plugin_presets: Vec<String>, // List of preset filenames
    pub selected_preset_index: usize,

    // Library tree view
    pub library_view_mode: LibraryViewMode,
    pub letter_tree: Vec<LetterNode>,
    pub selected_tree_index: usize, // Index in flattened tree (letters + visible albums)
    pub library_sort_order: LibrarySortOrder,
    pub channel_filter: ChannelFilter,

    // Pagination for library
    pub library_page: usize,           // Current page (0-indexed)
    pub library_items_per_page: usize, // Items per page

    // Plugin system
    pub plugin_chain: PluginChain,
    pub needs_plugin_update: bool,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize, // Which parameter is selected in edit mode

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,

    // Loudness monitoring
    pub loudness_info: Option<LoudnessData>,

    // Level meters
    pub level_meter_groups: Vec<ChannelGroup>,
    pub selected_level_meter_group: usize,
    pub level_meter_control_selection: usize, // 0 = Mute, 1 = Solo, 2 = Dim

    // Spectrum analyzer
    pub spectrum_visible: bool,
    pub spectrum_info: Option<SpectrumData>,

    // Audio devices
    pub output_devices: Vec<AudioDevice>,
    pub selected_output_device_index: usize,
    pub current_output_device_name: Option<String>,

    // Flags
    pub should_quit: bool,
    pub needs_rescan: bool,

    // Scan progress
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,

    // Last loaded plugin preset name (for config persistence)
    pub last_loaded_preset: Option<String>,

    // Context menu state
    pub context_menu: Option<ContextMenuState>,

    // Theme and i18n
    pub theme_id: ThemeId,
    pub theme: Theme,
    pub language: Language,
    pub translations: Translations,

    // Keybindings
    pub keymap_preset: KeymapPreset,

    // Menu state
    pub active_menu: ActiveMenu,

    // Layout state
    pub layout_mode: LayoutMode,
    pub window_height: f32,

    // Panel layout (resizable)
    pub queue_panel_ratio: f32,
    pub meters_panel_ratio: f32,
    pub is_dragging_queue_divider: bool,
    pub is_dragging_meters_divider: bool,

    // Scan progress for threaded scanning
    pub scan_total_files: usize,

    // Device popup state
    pub show_device_popup: bool,

    // Settings accordion expanded sections
    pub expanded_settings_sections: Vec<String>,
}

/// GPUI-compatible state wrapper
pub struct AppState {
    pub app: App,
    pub player: Arc<parking_lot::Mutex<Player>>,
}

impl App {
    pub fn new() -> Self {
        // Try to create library with database, fallback to simple library
        let library = MusicLibrary::with_database().unwrap_or_else(|e| {
            log::warn!(
                "Failed to initialize database, using in-memory library: {}",
                e
            );
            MusicLibrary::new()
        });

        let mut app = Self {
            library,
            queue: Vec::new(),
            expanded_queue_items: Vec::new(),
            current_screen: Screen::Library,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            directory_input: String::new(),
            plugin_file_input: String::new(),
            apo_file_input: String::new(),
            sofa_file_input: String::new(),
            headphone_curve_path: String::new(),
            headphone_target: String::from("harman-over-ear-2018"),
            headphone_params: crate::optimization_params::OptimizationParams::headphone_defaults(),
            headphone_optimization_running: false,
            headphone_optimization_progress: Vec::new(),
            selected_directory_index: 0,
            selected_queue_index: 0,
            album_list_offset: 0,
            toast_message: None,
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            selected_album_index: 0,
            selected_tree_index: 0,
            selected_plugin_index: 0,
            library_view_mode: LibraryViewMode::Grid, // Default to Grid view
            letter_tree: Vec::new(),
            library_sort_order: LibrarySortOrder::Album,
            channel_filter: ChannelFilter::All,
            library_page: 0,
            library_items_per_page: 50, // Show 50 items per page
            plugin_chain: {
                let mut chain = PluginChain::new();
                // Add default analyzer plugins for LUFS and level meters
                chain.add_plugin(&PluginType::LoudnessMonitor);
                chain
            },
            needs_plugin_update: false,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            muted: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            loudness_info: None,
            level_meter_groups: Vec::new(),
            selected_level_meter_group: 0,
            level_meter_control_selection: 0,
            spectrum_visible: false,
            spectrum_info: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            should_quit: false,
            needs_rescan: false,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            last_loaded_preset: None,
            context_menu: None,
            theme_id: ThemeId::default(),
            theme: Theme::from_id(ThemeId::default()),
            language: Language::default(),
            translations: Translations::for_language(Language::default()),
            keymap_preset: KeymapPreset::default(),
            active_menu: ActiveMenu::None,
            layout_mode: LayoutMode::Compact,
            window_height: 600.0,
            queue_panel_ratio: 0.35,
            meters_panel_ratio: 0.25,
            is_dragging_queue_divider: false,
            is_dragging_meters_divider: false,
            scan_total_files: 0,
            show_device_popup: false,
            expanded_settings_sections: vec!["library".to_string()],
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.rebuild_letter_tree();
        // Update last scan times for directories from database
        self.update_directory_scan_times();
        Ok(())
    }

    /// Update directory scan times from database
    fn update_directory_scan_times(&mut self) {
        self.library.update_directory_scan_times();
    }

    pub fn load_output_devices(&mut self) {
        // Load available output devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices()
            && let Some(output_devices) = devices_map.get("output")
        {
            self.output_devices = output_devices.clone();
            // Find the default device
            if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                self.selected_output_device_index = default_idx;
            }
        }
    }

    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::Config;
        let config = Config::load()?;

        // Restore directories
        self.library.directories = config.directories;

        // Restore theme
        self.theme_id = config.theme;
        self.theme = Theme::from_id(config.theme);

        // Restore language
        self.language = config.language;
        self.translations = Translations::for_language(config.language);

        // Restore keymap preset
        self.keymap_preset = config.keymap_preset;

        // Restore panel layout
        self.queue_panel_ratio = config.panel_layout.queue_ratio;
        self.meters_panel_ratio = config.panel_layout.meters_ratio;

        // Restore volume and muted state
        self.volume = config.volume;
        self.muted = config.muted;

        // Restore plugin presets path if we had a last loaded preset
        if let Some(preset_name) = config.last_loaded_plugin_preset {
            self.last_loaded_preset = Some(preset_name.clone());
            // Load the preset file
            match self.plugin_chain.load_from_file(&preset_name) {
                Ok(_) => {
                    self.needs_plugin_update = true;
                    log::info!("Restored plugin preset: {}", preset_name);
                }
                Err(e) => {
                    log::warn!("Could not restore preset '{}': {}", preset_name, e);
                }
            }
        }

        Ok(())
    }

    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_config_with_geometry(None)
    }

    /// Save config with optional window geometry
    pub fn save_config_with_geometry(
        &self,
        window_geometry: Option<crate::config::WindowGeometry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::{Config, PanelLayout, WindowGeometry};
        let config = Config {
            directories: self.library.directories.clone(),
            last_loaded_plugin_preset: self.last_loaded_preset.clone(),
            theme: self.theme_id,
            language: self.language,
            keymap_preset: self.keymap_preset,
            panel_layout: PanelLayout {
                queue_ratio: self.queue_panel_ratio,
                meters_ratio: self.meters_panel_ratio,
            },
            window_geometry: window_geometry.unwrap_or_else(|| {
                // If no geometry provided, use current saved value or default
                Config::load()
                    .ok()
                    .and_then(|c| Some(c.window_geometry))
                    .unwrap_or_default()
            }),
            volume: self.volume,
            muted: self.muted,
        };
        config.save()?;
        Ok(())
    }

    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.output_devices
            .get(self.selected_output_device_index)
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    /// Check and dismiss expired toast messages
    pub fn update_toast(&mut self) {
        if let Some(ref toast) = self.toast_message {
            if toast.should_dismiss() {
                self.toast_message = None;
            }
        }
    }

    /// Dismiss the current toast message manually
    pub fn dismiss_toast(&mut self) {
        self.toast_message = None;
    }

    /// Cycle to the next theme
    pub fn next_theme(&mut self) {
        self.theme_id = self.theme_id.next();
        self.theme = Theme::from_id(self.theme_id);
    }

    /// Cycle to the next language
    pub fn next_language(&mut self) {
        self.language = self.language.next();
        self.translations = Translations::for_language(self.language);
    }

    /// Set a specific theme
    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.theme_id = theme_id;
        self.theme = Theme::from_id(theme_id);
    }

    /// Set a specific language
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        self.translations = Translations::for_language(language);
    }

    /// Cycle to the next keymap preset
    pub fn next_keymap_preset(&mut self) {
        self.keymap_preset = self.keymap_preset.next();
    }

    /// Set a specific keymap preset
    pub fn set_keymap_preset(&mut self, preset: KeymapPreset) {
        self.keymap_preset = preset;
    }
}
