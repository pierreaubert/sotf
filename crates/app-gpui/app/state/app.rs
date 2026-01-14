//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_ui_kit::workflow::NodeId;
use sotf_audio_player::Player;

use crate::app::types::ReplayGainMode;
use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{Theme, ThemeId};

use crate::app::debug::StateHistory;
use crate::app::types::{
    ChannelGroup, InputMode,
    LibraryStats, MeterDisplayMode, OptimizationUiState, QueueItem,
    ToastMessage,
};

use super::{InputState, LibraryState, PlaybackState, PluginState, UIState};
use crate::app::manager::{Manager, ManagerError};
use crate::app::state::library::LibraryEvent;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;

/// Messages that can be dispatched to the App
#[derive(Debug, Clone)]
pub enum AppMessage {
    Library(LibraryEvent),
}

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
    // Library state - now managed via library_state
    /// Cached library statistics (artists, tracks, genres, years, etc.)
    /// Call invalidate_library_stats() when library changes, get_library_stats() to access
    pub library_stats: LibraryStats,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,

    // Queue state
    pub queue: Vec<QueueItem>,
    pub expanded_queue_items: Vec<bool>, // Track which queue items are expanded

    // Speaker Optimization State
    pub speaker_model: String, // Selected speaker model name (e.g. "KEF LS50 Meta")
    pub speaker_params: sotf_audio_player::autoeq::OptimizationParams,
    pub speaker_optimization_running: bool,
    pub speaker_optimization_progress: Vec<(usize, f64)>,
    pub speaker_optimization_result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    pub speaker_export_format: String,
    pub speaker_opt_ui: OptimizationUiState, // UI state (dropdowns)

    // Selection indices
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub album_list_offset: usize,

    // Level meters
    pub level_meter_groups: Vec<ChannelGroup>,
    pub selected_level_meter_group: usize,
    pub level_meter_control_selection: usize, // 0 = Mute, 1 = Solo, 2 = Dim
    /// Cached channel count to avoid rebuilding meter groups every frame
    pub level_meter_last_channel_count: usize,
    /// Cached speaker config to avoid rebuilding meter groups every frame
    pub level_meter_last_speaker_config: Option<String>,
    /// Peak hold values per channel (linear scale, 0.0 to 1.0+)
    pub level_meter_peak_hold: Vec<f64>,
    /// Last update time for peak hold decay
    pub level_meter_peak_hold_last_update: Option<std::time::Instant>,

    // Spectrum analyzer
    pub spectrum_visible: bool,

    // Flags
    pub needs_rescan: bool,
    pub is_loading_initial_data: bool,

    // Scan progress modal (for library, bliss, waveform, replaygain scans)
    pub scan_progress_modal: Option<crate::app::types::ScanProgressModal>,

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

    // ReplayGain settings
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: ReplayGainMode,
    pub replay_gain_preamp: f32,

    // Plugin UI states
    pub upmixer_config_open: bool,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,

    // Rack panel collapse states
    pub rack_detail_collapsed: bool, // Horizontal divider between rack and detail
    pub input_meter_collapsed: bool, // Left meter panel
    pub output_meter_collapsed: bool, // Right meter panel

    // Rack panel widths (for resizing)
    pub input_meter_width: f32,  // Width of input meter panel
    pub output_meter_width: f32, // Width of output meter panel

    // Divider drag state
    pub dragging_divider: Option<DividerDragState>,

    // Composed state structs (for better separation of concerns)
    /// Playback-related state
    pub playback: PlaybackState,
    /// Library-related state
    pub library_state: LibraryState,
    /// Plugin-related state
    pub plugin_state: PluginState,
    /// UI-related state
    pub ui_state: UIState,
    /// Input-related state (text fields, autocomplete)
    pub input_state: InputState,
    /// Audio device state (input/output devices, playback source)
    pub audio_device_state: super::AudioDeviceState,
    /// Measurement and EQ workflow state
    pub measurement_state: super::MeasurementState,

    /// Shared state across managers
    pub shared_state: Arc<super::SharedState>,

    /// Debug state history tracker
    pub state_history: StateHistory,

    /// Event sourcing for playback state
    pub playback_events: super::playback_events::PlaybackEventStore,
}

/// GPUI-compatible state wrapper

/// GPUI-compatible state wrapper
pub struct AppState {
    pub app: App,
    pub player: Arc<parking_lot::Mutex<Player>>,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            library_stats: LibraryStats::default(),
            library_scanner: None,
            queue: Vec::new(),
            expanded_queue_items: Vec::new(),

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
            level_meter_groups: Vec::new(),
            selected_level_meter_group: 0,
            level_meter_control_selection: 0,
            level_meter_last_channel_count: 0,
            level_meter_last_speaker_config: None,
            level_meter_peak_hold: Vec::new(),
            level_meter_peak_hold_last_update: None,
            spectrum_visible: false,
            needs_rescan: false,
            is_loading_initial_data: true,
            scan_progress_modal: None,
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
            replay_gain_enabled: true,
            replay_gain_mode: ReplayGainMode::Track,
            replay_gain_preamp: 0.0,
            upmixer_config_open: false,
            spectrum_tilt_select_open: false,
            spectrum_reference_select_open: false,
            rack_detail_collapsed: false,
            input_meter_collapsed: false,
            output_meter_collapsed: false,
            input_meter_width: 80.0,   // Default width for input meter panel
            output_meter_width: 140.0, // Default width for output meter panel
            dragging_divider: None,
            // Initialize composed state structs
            playback: PlaybackState::new(),
            library_state: LibraryState::new(),
            plugin_state: PluginState::new(),
            ui_state: UIState::new(),
            input_state: InputState::new(),
            audio_device_state: super::AudioDeviceState::new(),
            measurement_state: super::MeasurementState::new(),
            shared_state: Arc::new(super::SharedState::new()),
            state_history: StateHistory::new(),
            playback_events: super::playback_events::PlaybackEventStore::new(),
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    /// Dispatch a message to the appropriate manager
    pub fn dispatch(&mut self, msg: AppMessage) -> Result<(), ManagerError> {
        match msg {
            AppMessage::Library(event) => self.library_state.handle_event(event),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // State transition methods with debug logging
    // ─────────────────────────────────────────────────────────────────────────

    /// Set input mode with debug logging and state history capture
    pub fn set_input_mode(&mut self, mode: InputMode, trigger: &str) {
        let old_mode = self.ui_state.input_mode;
        if old_mode != mode {
            crate::app::debug::log_input_mode_transition(old_mode, mode, trigger);
            self.state_history.capture(
                self.ui_state.current_screen,
                mode,
                self.audio_device_state.current_output_device_name.clone(),
                format!("input_mode: {}", trigger),
            );
            self.ui_state.input_mode = mode;
        }
    }

    /// Set current screen with debug logging and state history capture
    pub fn set_screen(&mut self, screen: crate::app::Screen, trigger: &str) {
        let old_screen = self.ui_state.current_screen;
        if old_screen != screen {
            crate::app::debug::log_screen_transition(old_screen, screen, trigger);
            self.ui_state.last_screen = old_screen;
            self.state_history.capture(
                screen,
                self.ui_state.input_mode,
                self.audio_device_state.current_output_device_name.clone(),
                format!("screen: {}", trigger),
            );
            self.ui_state.current_screen = screen;
        }
    }

    /// Set output device with debug logging
    pub fn set_output_device(&mut self, device_name: Option<String>, trigger: &str) {
        let old_device = self.audio_device_state.current_output_device_name.as_deref();
        let new_device = device_name.as_deref();
        if old_device != new_device {
            crate::app::debug::log_device_change(old_device, new_device, trigger);
            self.state_history.capture(
                self.ui_state.current_screen,
                self.ui_state.input_mode,
                device_name.clone(),
                format!("device: {}", trigger),
            );
            self.audio_device_state.current_output_device_name = device_name;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Playback event recording methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Record playback started event
    pub fn record_playback_started(&mut self, queue_index: usize, track_path: Option<std::path::PathBuf>) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Started { queue_index, track_path });
    }

    /// Record playback paused event
    pub fn record_playback_paused(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Paused);
    }

    /// Record playback resumed event
    pub fn record_playback_resumed(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Resumed);
    }

    /// Record playback stopped event
    pub fn record_playback_stopped(&mut self) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Stopped);
    }

    /// Record track change event
    pub fn record_track_changed(
        &mut self,
        from_index: Option<usize>,
        to_index: usize,
        trigger: super::playback_events::TrackChangeTrigger,
    ) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::TrackChanged {
            from_index,
            to_index,
            trigger,
        });
    }

    /// Record volume change event
    pub fn record_volume_changed(&mut self, from: f32, to: f32) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::VolumeChanged { from, to });
    }

    /// Record mute change event
    pub fn record_mute_changed(&mut self, muted: bool) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::MuteChanged { muted });
    }

    /// Record seek event
    pub fn record_seek(&mut self, from_position: f64, to_position: f64) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Seeked { from_position, to_position });
    }

    /// Record track ended event
    pub fn record_track_ended(&mut self, queue_index: usize) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::TrackEnded { queue_index });
    }

    /// Record playback error event
    pub fn record_playback_error(&mut self, message: impl Into<String>) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Error { message: message.into() });
    }

    /// Get playback event summary for debugging
    pub fn playback_event_summary(&self) -> super::playback_events::EventStoreSummary {
        self.playback_events.summary()
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library_state.library.load_from_database()?;
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
        if self.ui_state.startup_db_check_done {
            return;
        }
        self.ui_state.startup_db_check_done = true;

        // Try to load from database
        if let Err(e) = self.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
        }

        // Check if library is empty
        if self.library_state.library.albums.is_empty() {
            // Show modal prompting to scan for music
            self.ui_state.input_mode = InputMode::EmptyLibraryPrompt;
        }

        self.is_loading_initial_data = false;
    }

    /// Update directory scan times from database
    fn update_directory_scan_times(&mut self) {
        self.library_state.library.update_directory_scan_times();
    }

    /// Select the best default sample rate from available rates.
    /// Prefers 48000, then 44100, then device default, then first available, then 48000 as fallback.
    fn select_default_sample_rate(
        available_rates: &[u32],
        device_default_rate: Option<u32>,
    ) -> u32 {
        if available_rates.contains(&48000) {
            48000
        } else if available_rates.contains(&44100) {
            44100
        } else {
            device_default_rate.unwrap_or_else(|| available_rates.first().copied().unwrap_or(48000))
        }
    }

    pub fn load_audio_devices(&mut self) {
        // Load available devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                self.audio_device_state.output_devices = output_devices.clone();
                // Find the default device
                if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                    self.audio_device_state.selected_output_device_index = default_idx;
                    // Initialize recording state playback device if not already set
                    if self.measurement_state.recording_state.playback_config.device_name.is_empty() {
                        let device = &output_devices[default_idx];
                        self.measurement_state.recording_state.playback_config.device_name = device.name.clone();
                        self.measurement_state.recording_state.playback_config.device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state.recording_state.playback_config.num_channels =
                                config.channels as usize;
                        }
                        self.measurement_state.recording_state.playback_config.available_sample_rates =
                            device.available_sample_rates.clone();
                        self.measurement_state.recording_state.playback_config.sample_rate =
                            Self::select_default_sample_rate(
                                &device.available_sample_rates,
                                device.default_config.as_ref().map(|c| c.sample_rate),
                            );
                    }
                }
            }
            if let Some(input_devices) = devices_map.get("input") {
                self.audio_device_state.input_devices = input_devices.clone();
                // Find the default device
                if let Some(default_idx) = input_devices.iter().position(|d| d.is_default) {
                    self.audio_device_state.selected_input_device_index = default_idx;
                    // Initialize recording state recording device if not already set
                    if self.measurement_state.recording_state.recording_config.device_name.is_empty() {
                        let device = &input_devices[default_idx];
                        self.measurement_state.recording_state.recording_config.device_name = device.name.clone();
                        self.measurement_state.recording_state.recording_config.device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state.recording_state.recording_config.num_channels =
                                config.channels as usize;
                        }
                        self.measurement_state.recording_state.recording_config.available_sample_rates =
                            device.available_sample_rates.clone();
                        self.measurement_state.recording_state.recording_config.sample_rate =
                            Self::select_default_sample_rate(
                                &device.available_sample_rates,
                                device.default_config.as_ref().map(|c| c.sample_rate),
                            );
                    }
                }
            }
        }
    }

    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::Config;
        let config = Config::load()?;

        // Restore directories
        self.library_state.library.directories = config.directories;

        // Restore theme
        self.ui_state.theme_id = config.theme;
        self.ui_state.theme = Theme::from_id(config.theme);

        // Restore language
        self.ui_state.language = config.language;
        self.ui_state.translations = Translations::for_language(config.language);

        // Restore keymap preset
        self.ui_state.keymap_preset = config.keymap_preset;

        // Restore panel layout
        self.queue_panel_ratio = config.panel_layout.queue_ratio;
        self.meters_panel_ratio = config.panel_layout.meters_ratio;
        self.queue_list_ratio = config.panel_layout.queue_list_ratio;
        self.lufs_panel_ratio = config.panel_layout.lufs_ratio;

        // Restore volume and muted state
        // self.playback.volume = config.volume; // Always start at default (10%) per requirement
        self.playback.muted = config.muted;

        // Restore recording config
        if !config.recording_config.playback.device_name.is_empty() {
            self.measurement_state.recording_state.playback_config = config.recording_config.playback;
        }
        if !config.recording_config.recording.device_name.is_empty() {
            self.measurement_state.recording_state.recording_config = config.recording_config.recording;
        }

        self.measurement_state.recording_state.signal_type = config.recording_config.signal_type;
        self.measurement_state.recording_state.signal_duration_secs = config.recording_config.signal_duration_secs;
        self.measurement_state.recording_state.signal_level_db = config.recording_config.signal_level_db;
        self.measurement_state.recording_state.mic_calibration_path = config.recording_config.mic_calibration_path;
        self.measurement_state.recording_state.recording_directory = config.recording_config.recording_directory;
        self.measurement_state.recording_state.recording_base_directory =
            config.recording_config.recording_base_directory;

        // Reload calibration data if path exists
        if let Some(ref path) = self.measurement_state.recording_state.mic_calibration_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.measurement_state.recording_state.mic_calibration_data =
                    crate::app::types::CalibrationData::parse(&content);
            }
        }

        // Restore plugin presets path if we had a last loaded preset
        if let Some(preset_name) = config.last_loaded_plugin_preset {
            self.plugin_state.last_loaded_preset = Some(preset_name.clone());
            // Load the preset file
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                log::warn!("Could not find presets directory, skipping preset restore");
                return Ok(());
            };
            match self.plugin_state.plugin_chain.load_from_file(&presets_dir, &preset_name) {
                Ok(_) => {
                    self.plugin_state.pending_plugin_update =
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
            directories: self.library_state.library.directories.clone(),
            last_loaded_plugin_preset: self.plugin_state.last_loaded_preset.clone(),
            theme: self.ui_state.theme_id,
            language: self.ui_state.language,
            keymap_preset: self.ui_state.keymap_preset,
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
            volume: self.playback.volume,
            muted: self.playback.muted,
            recording_config: crate::app::config::RecordingConfigState {
                playback: self.measurement_state.recording_state.playback_config.clone(),
                recording: self.measurement_state.recording_state.recording_config.clone(),
                signal_type: self.measurement_state.recording_state.signal_type,
                signal_duration_secs: self.measurement_state.recording_state.signal_duration_secs,
                signal_level_db: self.measurement_state.recording_state.signal_level_db,
                mic_calibration_path: self.measurement_state.recording_state.mic_calibration_path.clone(),
                recording_directory: self.measurement_state.recording_state.recording_directory.clone(),
                recording_base_directory: self.measurement_state.recording_state.recording_base_directory.clone(),
            },
        };
        config.save()?;
        Ok(())
    }

    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.audio_device_state.output_devices
            .get(self.audio_device_state.selected_output_device_index)
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    /// Check and dismiss expired toast messages
    pub fn update_toast(&mut self) {
        if let Some(ref toast) = self.ui_state.toast_message {
            if toast.should_dismiss() {
                self.ui_state.toast_message = None;
            }
        }
    }

    /// Dismiss the current toast message manually
    pub fn dismiss_toast(&mut self) {
        self.ui_state.toast_message = None;
    }

    /// Cycle to the next theme
    pub fn next_theme(&mut self) {
        self.ui_state.theme_id = self.ui_state.theme_id.next();
        self.ui_state.theme = Theme::from_id(self.ui_state.theme_id);
    }

    /// Cycle to the next language
    pub fn next_language(&mut self) {
        self.ui_state.language = self.ui_state.language.next();
        self.ui_state.translations = Translations::for_language(self.ui_state.language);
    }

    /// Set a specific theme
    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.ui_state.theme_id = theme_id;
        self.ui_state.theme = Theme::from_id(theme_id);
    }

    /// Set a specific language
    pub fn set_language(&mut self, language: Language) {
        self.ui_state.language = language;
        self.ui_state.translations = Translations::for_language(language);
    }

    /// Cycle to the next keymap preset
    pub fn next_keymap_preset(&mut self) {
        self.ui_state.keymap_preset = self.ui_state.keymap_preset.next();
    }

    /// Set a specific keymap preset
    pub fn set_keymap_preset(&mut self, preset: KeymapPreset) {
        self.ui_state.keymap_preset = preset;
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

        for album in &self.library_state.library.albums {
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

    /// Helper to calculate max characters based on panel width and text properties.
    /// Formula: ((panel_width - fixed_offset).max(min_width) / char_width).clamp(min_chars, max_chars)
    fn calculate_max_chars(
        panel_width: f32,
        fixed_offset: f32,
        min_width: f32,
        char_width: f32,
        min_chars: usize,
        max_chars: usize,
    ) -> usize {
        let available_width = (panel_width - fixed_offset).max(min_width);
        ((available_width / char_width) as usize).clamp(min_chars, max_chars)
    }

    /// Calculate the width of the queue list panel.
    fn queue_list_width(&self) -> f32 {
        self.ui_state.window_width * self.queue_list_ratio
    }

    /// Calculate the width of the center panel (remaining after queue list and meters).
    fn center_panel_width(&self) -> f32 {
        let center_ratio = 1.0 - self.queue_list_ratio - self.meters_panel_ratio;
        self.ui_state.window_width * center_ratio
    }

    /// Maximum characters for queue list album title (text_sm ~7px).
    pub fn max_chars_queue_list_title(&self) -> usize {
        Self::calculate_max_chars(self.queue_list_width(), 56.0, 50.0, 7.0, 15, 100)
    }

    /// Maximum characters for queue list artist (text_xs ~6px).
    pub fn max_chars_queue_list_artist(&self) -> usize {
        Self::calculate_max_chars(self.queue_list_width(), 56.0, 50.0, 6.0, 15, 120)
    }

    /// Maximum characters for Now Playing album title (text_lg ~9px).
    pub fn max_chars_now_playing_title(&self) -> usize {
        Self::calculate_max_chars(self.center_panel_width(), 192.0, 100.0, 9.0, 20, 150)
    }

    /// Maximum characters for Now Playing artist (text_sm ~7px).
    pub fn max_chars_now_playing_artist(&self) -> usize {
        Self::calculate_max_chars(self.center_panel_width(), 192.0, 100.0, 7.0, 20, 180)
    }

    /// Maximum characters for track titles (text_sm ~7px).
    pub fn max_chars_track_title(&self) -> usize {
        Self::calculate_max_chars(self.center_panel_width(), 120.0, 100.0, 7.0, 20, 200)
    }
}
