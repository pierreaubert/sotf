//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{
    ConnectionDrag, GraphSelection, LoudnessData, MusicLibrary, NodeDrag, Player, PluginChain,
    PluginGraph, PluginType, SpectrumData,
};

use crate::app::SettingsTab;
use crate::app::types::ReplayGainMode;
use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{Theme, ThemeId};

use crate::app::types::{
    ActiveMenu, ChannelFilter, ChannelGroup, ContextMenuState, HeadphoneEqState, InputMode,
    LayoutMode, LibrarySortOrder, LibraryStats, MeasureState, MeterDisplayMode,
    OptimizationUiState, PlaybackSource, PluginViewMode, QueueItem, RecordingState, RoomEqState,
    Screen, SpinoramaEqState, ToastMessage,
};

/// Mapping between workflow NodeIds and plugin indices / special nodes
#[derive(Clone, Default, Debug)]
pub struct WorkflowNodeMapping {
    pub node_to_plugin: HashMap<NodeId, usize>,
    pub plugin_to_node: HashMap<usize, NodeId>,
    /// Workflow node ID for the Input special node
    pub input_node_id: Option<NodeId>,
    /// Workflow node ID for the Output special node
    pub output_node_id: Option<NodeId>,
}

/// Which divider is being dragged
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DividerType {
    InputMeter,
    OutputMeter,
}

/// State for tracking divider drag operations
#[derive(Debug, Clone)]
pub struct DividerDragState {
    pub divider_type: DividerType,
    pub start_x: f32,
    pub start_width: f32,
}

#[derive(Debug)]
pub struct App {
    pub library: MusicLibrary,
    /// Cached library statistics (artists, tracks, genres, years, etc.)
    /// Call invalidate_library_stats() when library changes, get_library_stats() to access
    pub library_stats: LibraryStats,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,
    pub queue: Vec<QueueItem>,
    pub expanded_queue_items: Vec<bool>, // Track which queue items are expanded
    pub current_screen: Screen,
    pub last_screen: Screen, // Track previous screen for Back/Close logic
    pub input_mode: InputMode,

    // UI state
    pub search_query: String,
    pub directory_input: String,
    pub plugin_file_input: String, // For save/load plugin chain
    pub apo_file_input: String,    // For loading APO EQ files
    pub sofa_file_input: String,   // For loading SOFA HRTF files

    // Speaker Optimization State
    pub speaker_model: String, // Selected speaker model name (e.g. "KEF LS50 Meta")
    pub speaker_params: sotf_audio_player::autoeq::OptimizationParams,
    pub speaker_optimization_running: bool,
    pub speaker_optimization_progress: Vec<(usize, f64)>,
    pub speaker_optimization_result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    pub speaker_export_format: String,
    pub speaker_opt_ui: OptimizationUiState, // UI state (dropdowns)

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

    // Library sort and filter
    pub library_sort_order: LibrarySortOrder,
    pub channel_filter: ChannelFilter,
    // Selection filters for each sort mode (None = show selection UI)
    pub selected_genre: Option<String>,
    pub selected_decade: Option<(i32, i32)>, // Decade range (start, end) e.g., (2020, 2029)
    pub selected_year: Option<i32>,
    pub selected_artist_letter: Option<char>, // First letter filter for artists
    pub selected_artist: Option<String>,
    pub selected_composer_letter: Option<char>, // First letter filter for composers
    pub selected_composer: Option<String>,
    pub selected_album_letter: Option<char>,
    pub selected_track_range: Option<(usize, usize)>, // (min, max) track count range

    // Pagination for library
    pub library_items_per_page: usize, // Items per page
    pub library_columns: usize,        // Number of columns in grid

    // Plugin system
    pub plugin_chain: PluginChain,
    pub plugin_chain_modified: bool, // Track if plugins changed since last save
    /// Pending plugin update to sync to audio engine (None = no update needed)
    pub pending_plugin_update: Option<crate::app::types::PluginUpdateType>,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize, // Which parameter is selected in edit mode
    pub selected_eq_band: usize,       // Currently selected EQ band for display (0-indexed)
    pub matrix_selected_cell: Option<(usize, usize)>, // Currently selected matrix cell (input, output)

    // Plugin graph system (alternative to linear plugin chain)
    pub plugin_view_mode: PluginViewMode,
    pub plugin_graph: Option<PluginGraph>,
    pub graph_selection: GraphSelection,
    pub graph_connection_drag: Option<ConnectionDrag>,
    pub graph_node_drag: Option<NodeDrag>,

    // Workflow canvas (for WorkflowCanvas from gpui-ui-kit)
    pub workflow_canvas: Option<Entity<WorkflowCanvas>>,
    pub workflow_node_mapping: Option<WorkflowNodeMapping>,
    /// The node ID being edited in the plugin modal (if any)
    pub editing_plugin_node: Option<gpui_ui_kit::workflow::NodeId>,

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
    /// Cached channel count to avoid rebuilding meter groups every frame
    pub level_meter_last_channel_count: usize,
    /// Cached speaker config to avoid rebuilding meter groups every frame
    pub level_meter_last_speaker_config: Option<String>,

    // Spectrum analyzer
    pub spectrum_visible: bool,
    pub spectrum_info: Option<SpectrumData>,

    // Compressor data (for real-time gain reduction display)
    pub compressor_info: Option<sotf_plugins::CompressorData>,

    // Audio devices
    // Audio devices
    pub output_devices: Vec<AudioDevice>,
    pub selected_output_device_index: usize,
    pub current_output_device_name: Option<String>,

    pub input_devices: Vec<AudioDevice>,
    pub selected_input_device_index: usize,
    pub current_input_device_name: Option<String>,

    /// Audio source mode (File player or HAL device input)
    pub playback_source: PlaybackSource,

    // Measurement state
    pub measure_state: Option<MeasureState>,

    // Recording screen state
    pub recording_state: RecordingState,

    // Room EQ screen state
    pub room_eq_state: RoomEqState,
    /// Applied room EQ plugins (ready to be sent to audio engine)
    pub room_eq_applied_plugins: Option<Vec<sotf_audio::PluginConfig>>,

    // Headphone EQ screen state
    pub headphone_eq_state: HeadphoneEqState,

    // Spinorama EQ screen state
    pub spinorama_eq_state: SpinoramaEqState,

    // Flags
    pub should_quit: bool,
    pub needs_rescan: bool,

    // Scan progress
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,

    // Scan progress modal (for library, bliss, waveform, replaygain scans)
    pub scan_progress_modal: Option<crate::app::types::ScanProgressModal>,

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
    pub window_width: f32,

    // Panel layout (resizable)
    pub queue_panel_ratio: f32, // Height ratio for Queue section in split view (Library on top, Queue on bottom)
    pub queue_list_ratio: f32,  // Width ratio for queue list in Queue screen
    pub meters_panel_ratio: f32, // Width ratio for level meters panel in Queue screen
    pub lufs_panel_ratio: f32,  // Width ratio for LUFS panel in Queue screen (4-col mode)
    pub lufs_visible: bool,     // Whether LUFS panel is visible (when separated from meters)
    pub meter_display_mode: MeterDisplayMode, // Which meter to show (LUFS or Levels)
    pub is_dragging_queue_divider: bool,
    pub is_dragging_queue_list_divider: bool,
    pub is_dragging_meters_divider: bool,
    pub is_dragging_lufs_divider: bool,
    pub divider_click_start: Option<std::time::Instant>,

    // Scan progress for threaded scanning
    pub scan_total_files: usize,

    // Device popup state
    pub show_device_popup: bool,

    // Studio menu state
    pub show_studio_menu: bool,

    // active settings tab
    pub active_settings_tab: SettingsTab,

    // Filter menu state in library view
    pub filter_menu_open: bool,

    // Volume drag state
    pub is_dragging_volume: bool,
    pub volume_drag_start_y: Option<f32>,
    pub volume_drag_start_value: f32,

    // Plugin knob/slider drag state
    pub is_dragging_knob: bool,
    pub knob_drag_plugin_idx: usize,
    pub knob_drag_param_idx: usize,
    pub knob_drag_start_y: Option<f32>,
    pub knob_drag_start_value: f64,
    pub knob_drag_min: f64,
    pub knob_drag_max: f64,

    // Settings accordion expanded sections
    pub expanded_settings_sections: Vec<String>,

    // Waveform scanner manager
    pub waveform_manager: sotf_audio_player::WaveformScanManager,

    // ReplayGain scanner manager
    pub replay_gain_manager: sotf_audio_player::ReplayGainScanManager,

    // Bliss audio analysis scanner manager
    pub bliss_manager: sotf_audio_player::BlissScanManager,

    // Parameter editing state
    pub editing_param: Option<String>,
    pub editing_value: String,

    // ReplayGain settings
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: ReplayGainMode,
    pub replay_gain_preamp: f32,

    // Flag for closing studio after save
    pub pending_studio_close: bool,

    // Plugin UI states
    pub upmixer_config_open: bool,

    // Rack panel collapse states
    pub rack_detail_collapsed: bool, // Horizontal divider between rack and detail
    pub input_meter_collapsed: bool, // Left meter panel
    pub output_meter_collapsed: bool, // Right meter panel

    // Rack panel widths (for resizing)
    pub input_meter_width: f32,  // Width of input meter panel
    pub output_meter_width: f32, // Width of output meter panel

    // Divider drag state
    pub dragging_divider: Option<DividerDragState>,

    // Startup database check state
    /// Whether we've performed the initial database check on startup
    pub startup_db_check_done: bool,
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
            library_stats: LibraryStats::default(),
            library_scanner: None,
            queue: Vec::new(),
            expanded_queue_items: Vec::new(),
            current_screen: Screen::Library,
            last_screen: Screen::Library,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            directory_input: String::new(),
            plugin_file_input: String::new(),
            apo_file_input: String::new(),
            sofa_file_input: String::new(),

            // Speaker State Init
            speaker_model: String::new(),
            speaker_params: sotf_audio_player::autoeq::OptimizationParams::speaker_defaults(),
            speaker_optimization_running: false,
            speaker_optimization_progress: Vec::new(),
            speaker_optimization_result: None,
            speaker_export_format: String::from("json"),
            speaker_opt_ui: OptimizationUiState::default(),

            selected_directory_index: 0,
            selected_queue_index: 0,
            album_list_offset: 0,
            toast_message: None,
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            selected_album_index: 0,
            selected_plugin_index: 0,
            selected_eq_band: 0,
            library_sort_order: LibrarySortOrder::Album,
            channel_filter: ChannelFilter::All,
            selected_genre: None,
            selected_decade: None,
            selected_year: None,
            selected_artist_letter: None,
            selected_artist: None,
            selected_composer_letter: None,
            selected_composer: None,
            selected_album_letter: None,
            selected_track_range: None,
            library_items_per_page: 50, // Show 50 items per page
            library_columns: 4,
            plugin_chain: {
                let mut chain = PluginChain::new();
                // Add default analyzer plugins for LUFS and level meters
                chain.add_plugin(&PluginType::LoudnessMonitor);
                chain
            },
            plugin_chain_modified: false,
            pending_plugin_update: None,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            matrix_selected_cell: None,
            plugin_view_mode: PluginViewMode::Rack,
            plugin_graph: None,
            graph_selection: GraphSelection::default(),
            graph_connection_drag: None,
            graph_node_drag: None,
            workflow_canvas: None,
            workflow_node_mapping: None,
            editing_plugin_node: None,
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
            level_meter_last_channel_count: 0,
            level_meter_last_speaker_config: None,
            spectrum_visible: false,
            spectrum_info: None,
            compressor_info: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            input_devices: Vec::new(),
            selected_input_device_index: 0,
            current_input_device_name: None,
            playback_source: PlaybackSource::default(),
            measure_state: None,
            recording_state: RecordingState::default(),
            room_eq_state: RoomEqState::default(),
            room_eq_applied_plugins: None,
            headphone_eq_state: HeadphoneEqState::default(),
            spinorama_eq_state: SpinoramaEqState::default(),
            should_quit: false,
            needs_rescan: false,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            scan_progress_modal: None,
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
            window_width: 800.0,
            queue_panel_ratio: 0.35,
            queue_list_ratio: 0.30,
            meters_panel_ratio: 0.25,
            lufs_panel_ratio: 0.25,
            lufs_visible: true,
            meter_display_mode: MeterDisplayMode::default(),
            is_dragging_queue_divider: false,
            is_dragging_queue_list_divider: false,
            is_dragging_meters_divider: false,
            is_dragging_lufs_divider: false,
            divider_click_start: None,
            scan_total_files: 0,
            show_device_popup: false,
            show_studio_menu: false,
            active_settings_tab: SettingsTab::Library,
            filter_menu_open: false,
            is_dragging_volume: false,
            volume_drag_start_y: None,
            volume_drag_start_value: 0.0,
            is_dragging_knob: false,
            knob_drag_plugin_idx: 0,
            knob_drag_param_idx: 0,
            knob_drag_start_y: None,
            knob_drag_start_value: 0.0,
            knob_drag_min: 0.0,
            knob_drag_max: 1.0,
            expanded_settings_sections: vec!["library".to_string()],
            waveform_manager: sotf_audio_player::WaveformScanManager::new(),
            replay_gain_manager: sotf_audio_player::ReplayGainScanManager::new(),
            bliss_manager: sotf_audio_player::BlissScanManager::new(),
            editing_param: None,
            editing_value: String::new(),
            replay_gain_enabled: true,
            replay_gain_mode: ReplayGainMode::Track,
            replay_gain_preamp: 0.0,
            pending_studio_close: false,
            upmixer_config_open: false,
            rack_detail_collapsed: false,
            input_meter_collapsed: false,
            output_meter_collapsed: false,
            input_meter_width: 80.0,  // Default width for input meter panel
            output_meter_width: 140.0, // Default width for output meter panel
            dragging_divider: None,
            startup_db_check_done: false,
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        // Update last scan times for directories from database
        self.update_directory_scan_times();
        // Invalidate cached stats since library content changed
        self.invalidate_library_stats();
        Ok(())
    }

    /// Perform the initial database check on startup.
    /// Called from the UI update loop after the first render.
    /// Shows appropriate modal/toast based on database state.
    pub fn check_library_on_startup(&mut self) {
        // Only run once
        if self.startup_db_check_done {
            return;
        }
        self.startup_db_check_done = true;

        // Try to load from database
        if let Err(e) = self.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
        }

        // Check if library is empty
        if self.library.albums.is_empty() {
            // Show modal prompting to scan for music
            self.input_mode = InputMode::EmptyLibraryPrompt;
        } else {
            // Show toast with album count
            let album_count = self.library.albums.len();
            let message = format!("Loaded {} albums from database", album_count);
            self.toast_message = Some(ToastMessage::info(message));
        }
    }

    /// Update directory scan times from database
    fn update_directory_scan_times(&mut self) {
        self.library.update_directory_scan_times();
    }

    pub fn load_audio_devices(&mut self) {
        // Load available devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                self.output_devices = output_devices.clone();
                // Find the default device
                if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                    self.selected_output_device_index = default_idx;
                    // Initialize recording state playback device if not already set
                    if self.recording_state.playback_config.device_name.is_empty() {
                        let device = &output_devices[default_idx];
                        self.recording_state.playback_config.device_name = device.name.clone();
                        self.recording_state.playback_config.device_id = device.name.clone(); // Use name as ID
                        // Get num_channels from default_config
                        if let Some(ref config) = device.default_config {
                            self.recording_state.playback_config.num_channels =
                                config.channels as usize;
                        }
                        // Set sample rates
                        self.recording_state.playback_config.available_sample_rates =
                            device.available_sample_rates.clone();

                        let rates = &device.available_sample_rates;
                        let default_rate = if rates.contains(&48000) {
                            48000
                        } else if rates.contains(&44100) {
                            44100
                        } else {
                            device
                                .default_config
                                .as_ref()
                                .map(|c| c.sample_rate)
                                .unwrap_or(rates.first().copied().unwrap_or(48000))
                        };
                        self.recording_state.playback_config.sample_rate = default_rate;
                    }
                }
            }
            if let Some(input_devices) = devices_map.get("input") {
                self.input_devices = input_devices.clone();
                // Find the default device
                if let Some(default_idx) = input_devices.iter().position(|d| d.is_default) {
                    self.selected_input_device_index = default_idx;
                    // Initialize recording state recording device if not already set
                    if self.recording_state.recording_config.device_name.is_empty() {
                        let device = &input_devices[default_idx];
                        self.recording_state.recording_config.device_name = device.name.clone();
                        self.recording_state.recording_config.device_id = device.name.clone(); // Use name as ID
                        // Get num_channels from default_config
                        if let Some(ref config) = device.default_config {
                            self.recording_state.recording_config.num_channels =
                                config.channels as usize;
                        }
                        // Set sample rates
                        self.recording_state.recording_config.available_sample_rates =
                            device.available_sample_rates.clone();

                        let rates = &device.available_sample_rates;
                        let default_rate = if rates.contains(&48000) {
                            48000
                        } else if rates.contains(&44100) {
                            44100
                        } else {
                            device
                                .default_config
                                .as_ref()
                                .map(|c| c.sample_rate)
                                .unwrap_or(rates.first().copied().unwrap_or(48000))
                        };
                        self.recording_state.recording_config.sample_rate = default_rate;
                    }
                }
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
        self.queue_list_ratio = config.panel_layout.queue_list_ratio;
        self.lufs_panel_ratio = config.panel_layout.lufs_ratio;

        // Restore volume and muted state
        // self.volume = config.volume; // Always start at default (10%) per requirement
        self.muted = config.muted;

        // Restore recording config
        if !config.recording_config.playback.device_name.is_empty() {
            self.recording_state.playback_config = config.recording_config.playback;
        }
        if !config.recording_config.recording.device_name.is_empty() {
            self.recording_state.recording_config = config.recording_config.recording;
        }

        self.recording_state.signal_type = config.recording_config.signal_type;
        self.recording_state.signal_duration_secs = config.recording_config.signal_duration_secs;
        self.recording_state.signal_level_db = config.recording_config.signal_level_db;
        self.recording_state.mic_calibration_path = config.recording_config.mic_calibration_path;
        self.recording_state.recording_directory = config.recording_config.recording_directory;
        self.recording_state.recording_base_directory =
            config.recording_config.recording_base_directory;

        // Reload calibration data if path exists
        if let Some(ref path) = self.recording_state.mic_calibration_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.recording_state.mic_calibration_data =
                    crate::app::types::CalibrationData::parse(&content);
            }
        }

        // Restore plugin presets path if we had a last loaded preset
        if let Some(preset_name) = config.last_loaded_plugin_preset {
            self.last_loaded_preset = Some(preset_name.clone());
            // Load the preset file
            match self.plugin_chain.load_from_file(&preset_name) {
                Ok(_) => {
                    self.pending_plugin_update =
                        Some(crate::app::types::PluginUpdateType::Structural);
                    self.sync_spectrum_visible();
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
        use crate::config::{Config, PanelLayout};
        let config = Config {
            directories: self.library.directories.clone(),
            last_loaded_plugin_preset: self.last_loaded_preset.clone(),
            theme: self.theme_id,
            language: self.language,
            keymap_preset: self.keymap_preset,
            panel_layout: PanelLayout {
                queue_ratio: self.queue_panel_ratio,
                meters_ratio: self.meters_panel_ratio,
                queue_list_ratio: self.queue_list_ratio,
                lufs_ratio: self.lufs_panel_ratio,
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
            recording_config: crate::app::config::RecordingConfigState {
                playback: self.recording_state.playback_config.clone(),
                recording: self.recording_state.recording_config.clone(),
                signal_type: self.recording_state.signal_type,
                signal_duration_secs: self.recording_state.signal_duration_secs,
                signal_level_db: self.recording_state.signal_level_db,
                mic_calibration_path: self.recording_state.mic_calibration_path.clone(),
                recording_directory: self.recording_state.recording_directory.clone(),
                recording_base_directory: self.recording_state.recording_base_directory.clone(),
            },
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

    /// Invalidate cached library statistics. Call this when the library changes
    /// (albums added/removed, tracks modified, etc.)
    pub fn invalidate_library_stats(&mut self) {
        self.library_stats.valid = false;
    }

    /// Get library statistics, computing them if not cached.
    /// This is an O(n) operation when stats are invalid, but returns cached
    /// values on subsequent calls until invalidate_library_stats() is called.
    pub fn get_library_stats(&mut self) -> &LibraryStats {
        if !self.library_stats.valid {
            self.compute_library_stats();
        }
        &self.library_stats
    }

    /// Compute library statistics from scratch.
    /// This is expensive - O(n) over all albums and tracks.
    fn compute_library_stats(&mut self) {
        use std::collections::{HashMap, HashSet};

        let mut artists: HashSet<String> = HashSet::new();
        let mut composers: HashSet<String> = HashSet::new();
        let mut genres: HashSet<String> = HashSet::new();
        let mut genre_counts: HashMap<String, usize> = HashMap::new();
        let mut year_counts: HashMap<i32, usize> = HashMap::new();
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut artist_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut composer_counts: HashMap<String, usize> = HashMap::new();
        let mut composer_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut album_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut track_count_distribution: HashMap<usize, usize> = HashMap::new();
        let mut total_tracks = 0usize;
        let mut min_year = i32::MAX;
        let mut max_year = 0i32;
        let mut mono_count = 0usize;
        let mut stereo_count = 0usize;
        let mut surround_count = 0usize;
        let mut surround71_count = 0usize;
        let mut surround_plus_count = 0usize;

        for album in &self.library.albums {
            // Count channels
            if let Some(channels) = album.uniform_channel_count() {
                match channels {
                    1 => mono_count += 1,
                    2 => stereo_count += 1,
                    5 | 6 => surround_count += 1,
                    8 => surround71_count += 1,
                    n if n > 8 => surround_plus_count += 1,
                    _ => {} // 3, 4, 7 channels - rare, skip
                }
            }

            // Track year range and count per year
            if let Some(y) = album.year {
                let y = y as i32;
                if y > 0 {
                    if y < min_year {
                        min_year = y;
                    }
                    if y > max_year {
                        max_year = y;
                    }
                    *year_counts.entry(y).or_insert(0) += 1;
                }
            }

            // Count albums per first letter
            if let Some(first_char) = album.title.chars().next() {
                let letter = first_char.to_ascii_uppercase();
                let key = if letter.is_ascii_alphabetic() {
                    letter
                } else {
                    '#' // Group non-alphabetic titles
                };
                *album_letter_counts.entry(key).or_insert(0) += 1;
            }

            // Count track distribution
            let track_count = album.tracks.len();
            *track_count_distribution.entry(track_count).or_insert(0) += 1;

            // Get album artist for artist counts and artist letter counts
            let album_artist = album.artist();
            if !album_artist.is_empty() {
                *artist_counts.entry(album_artist.to_string()).or_insert(0) += 1;
                // Count by first letter
                if let Some(first_char) = album_artist.chars().next() {
                    let letter = first_char.to_ascii_uppercase();
                    let key = if letter.is_ascii_alphabetic() {
                        letter
                    } else {
                        '#'
                    };
                    *artist_letter_counts.entry(key).or_insert(0) += 1;
                }
            }

            // Get album genre (from first track) for genre counts
            if let Some(first_track) = album.tracks.first() {
                if let Some(genre) = &first_track.genre {
                    if !genre.is_empty() {
                        *genre_counts.entry(genre.clone()).or_insert(0) += 1;
                    }
                }
                // Get album composer for composer counts and composer letter counts
                if let Some(composer) = &first_track.composer {
                    if !composer.is_empty() {
                        *composer_counts.entry(composer.clone()).or_insert(0) += 1;
                        // Count by first letter
                        if let Some(first_char) = composer.chars().next() {
                            let letter = first_char.to_ascii_uppercase();
                            let key = if letter.is_ascii_alphabetic() {
                                letter
                            } else {
                                '#'
                            };
                            *composer_letter_counts.entry(key).or_insert(0) += 1;
                        }
                    }
                }
            }

            // Count artists, composers, genres, tracks
            for track in &album.tracks {
                total_tracks += 1;
                if let Some(artist) = &track.artist {
                    if !artist.is_empty() {
                        artists.insert(artist.to_lowercase());
                    }
                }
                if let Some(composer) = &track.composer {
                    if !composer.is_empty() {
                        composers.insert(composer.to_lowercase());
                    }
                }
                if let Some(genre) = &track.genre {
                    if !genre.is_empty() {
                        genres.insert(genre.to_lowercase());
                    }
                }
            }
        }

        // Handle case where no albums have years
        if min_year == i32::MAX {
            min_year = 0;
        }

        // Build track range counts (group into ranges)
        let track_range_counts = Self::build_track_ranges(&track_count_distribution);

        // Build decade counts from year_counts
        let decade_counts = Self::build_decade_counts(&year_counts);

        self.library_stats = LibraryStats {
            artists_count: artists.len(),
            composers_count: composers.len(),
            total_tracks,
            genres_count: genres.len(),
            genre_counts,
            year_counts,
            decade_counts,
            artist_counts,
            artist_letter_counts,
            composer_counts,
            composer_letter_counts,
            album_letter_counts,
            track_range_counts,
            min_year,
            max_year,
            mono_count,
            stereo_count,
            surround_count,
            surround71_count,
            surround_plus_count,
            valid: true,
        };
    }

    /// Build decade counts from year counts
    fn build_decade_counts(
        year_counts: &std::collections::HashMap<i32, usize>,
    ) -> Vec<(i32, i32, usize)> {
        use std::collections::HashMap;

        let mut decade_map: HashMap<i32, usize> = HashMap::new();

        for (year, count) in year_counts {
            let decade_start = (*year / 10) * 10;
            *decade_map.entry(decade_start).or_insert(0) += count;
        }

        let mut decades: Vec<(i32, i32, usize)> = decade_map
            .into_iter()
            .map(|(start, count)| (start, start + 9, count))
            .collect();

        // Sort by decade descending (most recent first)
        decades.sort_by(|a, b| b.0.cmp(&a.0));

        decades
    }

    /// Build track count ranges from distribution
    fn build_track_ranges(
        distribution: &std::collections::HashMap<usize, usize>,
    ) -> Vec<(usize, usize, usize)> {
        // Define meaningful ranges
        let ranges = [
            (1, 5, "1-5 tracks"),
            (6, 10, "6-10 tracks"),
            (11, 15, "11-15 tracks"),
            (16, 20, "16-20 tracks"),
            (21, 30, "21-30 tracks"),
            (31, 50, "31-50 tracks"),
            (51, usize::MAX, "51+ tracks"),
        ];

        ranges
            .iter()
            .filter_map(|(min, max, _label)| {
                let count: usize = distribution
                    .iter()
                    .filter(|(tracks, _)| **tracks >= *min && **tracks <= *max)
                    .map(|(_, count)| count)
                    .sum();
                if count > 0 {
                    Some((*min, *max, count))
                } else {
                    None
                }
            })
            .collect()
    }

    // ============== Dynamic Text Truncation ==============

    /// Calculate the maximum characters for queue list album title based on panel width.
    /// Returns a reasonable limit that adapts to window size.
    pub fn max_chars_queue_list_title(&self) -> usize {
        // Queue list panel width = window_width * queue_list_ratio
        // Subtract padding (16px on each side = 32px)
        // Account for ellipsis (~24px) and some margin
        // Average char width for text_sm is ~7px
        let panel_width = self.window_width * self.queue_list_ratio;
        let available_width = (panel_width - 32.0 - 24.0).max(50.0);
        let char_width = 7.0;
        let max_chars = (available_width / char_width) as usize;
        max_chars.clamp(15, 100) // Min 15, max 100 characters
    }

    /// Calculate the maximum characters for queue list artist based on panel width.
    pub fn max_chars_queue_list_artist(&self) -> usize {
        // Same calculation as title, but artist uses text_xs (~6px)
        let panel_width = self.window_width * self.queue_list_ratio;
        let available_width = (panel_width - 32.0 - 24.0).max(50.0);
        let char_width = 6.0;
        let max_chars = (available_width / char_width) as usize;
        max_chars.clamp(15, 120) // Min 15, max 120 characters
    }

    /// Calculate the maximum characters for Now Playing album title.
    /// The center panel uses remaining width after queue list and meters panels.
    pub fn max_chars_now_playing_title(&self) -> usize {
        // Center panel width = window_width * (1.0 - queue_list_ratio - meters_panel_ratio)
        // Subtract album art (120px), gaps (16px), padding (16px), dividers (~40px)
        // text_lg uses ~9px per character
        let center_ratio = 1.0 - self.queue_list_ratio - self.meters_panel_ratio;
        let center_width = self.window_width * center_ratio;
        let available_width = (center_width - 120.0 - 40.0 - 32.0).max(100.0);
        let char_width = 9.0;
        let max_chars = (available_width / char_width) as usize;
        max_chars.clamp(20, 150) // Min 20, max 150 characters
    }

    /// Calculate the maximum characters for Now Playing artist.
    pub fn max_chars_now_playing_artist(&self) -> usize {
        // Same as title but text_sm uses ~7px
        let center_ratio = 1.0 - self.queue_list_ratio - self.meters_panel_ratio;
        let center_width = self.window_width * center_ratio;
        let available_width = (center_width - 120.0 - 40.0 - 32.0).max(100.0);
        let char_width = 7.0;
        let max_chars = (available_width / char_width) as usize;
        max_chars.clamp(20, 180) // Min 20, max 180 characters
    }

    /// Calculate the maximum characters for track titles in the track list.
    /// Accounts for track number column and duration column.
    pub fn max_chars_track_title(&self) -> usize {
        // Center panel minus album info section, track number (24px), duration (~48px), padding
        let center_ratio = 1.0 - self.queue_list_ratio - self.meters_panel_ratio;
        let center_width = self.window_width * center_ratio;
        // Available for track title = center_width - track_num(24) - duration(48) - padding(32) - gaps(16)
        let available_width = (center_width - 24.0 - 48.0 - 48.0).max(100.0);
        let char_width = 7.0; // text_sm
        let max_chars = (available_width / char_width) as usize;
        max_chars.clamp(20, 200) // Min 20, max 200 characters
    }
}
