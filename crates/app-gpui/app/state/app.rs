//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::Entity;
use gpui_ui_kit::workflow::NodeId;
use sotf_audio_player::Player;

use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{Theme, ThemeId};

use crate::app::debug::StateHistory;
use crate::app::types::{
    ChannelGroup, InputMode, LayoutOrientation, LibraryStats, MeterDisplayMode,
    OptimizationUiState, RackDisplayMode, ToastMessage,
};

use super::ui::LayoutState;
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
    pub queue: sotf_audio_player::QueueController,
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
    pub meter_display_mode: MeterDisplayMode, // Which meter to show (LUFS or Levels)

    // Spectrum analyzer
    pub spectrum_visible: bool,

    // Flags
    pub needs_rescan: bool,
    pub is_loading_initial_data: bool,
    pub library_stats_computing: bool,
    pub pending_library_stats: Arc<parking_lot::Mutex<Option<LibraryStats>>>,

    // Scan progress modal (for library, bliss, waveform, replaygain scans)
    pub scan_progress_modal: Option<crate::app::types::ScanProgressModal>,

    // Layout configuration is now managed via AppState.layout entity
    pub divider_click_start: Option<std::time::Instant>,

    // 3-Panel Layout (Library | Queue | Rack)
    pub layout_orientation: LayoutOrientation,
    pub rack_display_mode: RackDisplayMode,
    // Hide queue meters when rack is visible in 3-panel layout
    pub hide_queue_meters_for_rack: bool,

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

    // Scan managers (ReplayGain, Waveform, Bliss)
    pub scan_ctrl: sotf_audio_player::ScanController,

    // Plugin UI states
    pub upmixer_config_open: bool,
    pub upmixer_tab: usize,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub show_add_plugin_menu: bool,
    /// Active secondary tab index for auto-layout plugins (per-plugin, keyed by plugin_idx)
    pub plugin_auto_tab: std::collections::HashMap<usize, usize>,

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

    // Play tracking for statistics (30s threshold)
    pub current_track_path: Option<std::path::PathBuf>,
    pub current_track_start_time: Option<std::time::Instant>,
    pub current_track_already_recorded: bool,

    // Channel conflict dialog state
    pub channel_conflict_path: Option<sotf_audio::decoder::AudioSource>,
    pub channel_conflicts: Vec<sotf_audio_player::ChannelConflict>,
    pub channel_conflict_track_channels: usize,

    /// Whether the tutorial has been completed/dismissed (persisted to config)
    pub tutorial_completed: bool,

    /// Hint IDs that have been shown and dismissed (persisted to config).
    /// Uses string IDs so new hints can be added without migration.
    pub seen_hints: Vec<String>,
    /// Currently displayed contextual hint (None if no hint active).
    pub current_hint: Option<crate::components::dialogs::tutorial::ContextualHint>,
}

/// GPUI-compatible state wrapper
pub struct AppState {
    pub app: App,
    pub layout: Entity<LayoutState>,
    pub player: Arc<parking_lot::Mutex<Player>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            library_stats: LibraryStats::default(),
            library_scanner: None,
            queue: sotf_audio_player::QueueController::new(),
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
            meter_display_mode: MeterDisplayMode::default(),
            spectrum_visible: false,
            needs_rescan: false,
            is_loading_initial_data: true,
            library_stats_computing: false,
            pending_library_stats: Arc::new(parking_lot::Mutex::new(None)),
            scan_progress_modal: None,
            divider_click_start: None,
            // 3-Panel Layout defaults
            layout_orientation: LayoutOrientation::default(),
            rack_display_mode: RackDisplayMode::default(),
            hide_queue_meters_for_rack: false,
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
            scan_ctrl: sotf_audio_player::ScanController::new(),
            upmixer_config_open: false,
            upmixer_tab: 1,
            spectrum_tilt_select_open: false,
            spectrum_reference_select_open: false,
            show_add_plugin_menu: false,
            plugin_auto_tab: std::collections::HashMap::new(),
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
            current_track_path: None,
            current_track_start_time: None,
            current_track_already_recorded: false,
            channel_conflict_path: None,
            channel_conflicts: Vec::new(),
            channel_conflict_track_channels: 2,
            tutorial_completed: false,
            seen_hints: Vec::new(),
            current_hint: None,
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    pub fn rollback_failed_plugin_update(
        &mut self,
        plugin_state_snapshot: PluginState,
        error: impl Into<String>,
    ) {
        let error = error.into();
        self.plugin_state = plugin_state_snapshot;
        self.sync_spectrum_visible();
        self.update_level_meter_groups();
        self.ui_state.toast_message = Some(ToastMessage::error(format!(
            "Plugin update failed: {}",
            error
        )));
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

    /// Set current screen with debug logging and state history capture.
    /// If the screen's maturity exceeds the current release channel, redirects to Library.
    pub fn set_screen(&mut self, screen: crate::app::Screen, trigger: &str) {
        let target = if self.ui_state.release_channel.allows(screen.maturity()) {
            screen
        } else {
            log::info!(
                "Screen {:?} requires {:?}, but release channel is {:?} — redirecting to Library",
                screen,
                screen.maturity(),
                self.ui_state.release_channel
            );
            self.ui_state.toast_message = Some(crate::app::ToastMessage::info(format!(
                "{:?} is not available on the {:?} release channel",
                screen, self.ui_state.release_channel
            )));
            crate::app::Screen::Library
        };
        let old_screen = self.ui_state.current_screen;
        if old_screen != target {
            crate::app::debug::log_screen_transition(old_screen, target, trigger);
            self.ui_state.last_screen = old_screen;
            self.state_history.capture(
                target,
                self.ui_state.input_mode,
                self.audio_device_state.current_output_device_name.clone(),
                format!("screen: {}", trigger),
            );
            self.ui_state.current_screen = target;
            self.plugin_state.clear_confirmations();

            // Trigger contextual hints for first-time screen visits
            use crate::components::dialogs::tutorial::HintId;
            match target {
                crate::app::Screen::Studio => self.try_show_hint(HintId::StudioFirstVisit),
                crate::app::Screen::RoomEq => self.try_show_hint(HintId::RoomEqFirstVisit),
                _ => {}
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Contextual hints
    // ─────────────────────────────────────────────────────────────────────────

    /// Try to show a contextual hint. Only shows if the hint hasn't been seen before.
    pub fn try_show_hint(&mut self, hint_id: crate::components::dialogs::tutorial::HintId) {
        let id_str = hint_id.as_str();
        if !self.seen_hints.iter().any(|s| s == id_str) && self.current_hint.is_none() {
            self.current_hint =
                Some(crate::components::dialogs::tutorial::ContextualHint { hint_id });
        }
    }

    /// Dismiss the current hint and mark it as seen.
    pub fn dismiss_hint(&mut self) {
        if let Some(hint) = self.current_hint.take() {
            let id_str = hint.hint_id.as_str().to_string();
            if !self.seen_hints.contains(&id_str) {
                self.seen_hints.push(id_str);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Toast action handling
    // ─────────────────────────────────────────────────────────────────────────

    /// Handle a toast action button click.
    /// Override this to add custom action_id handlers for different toast actions.
    pub fn handle_toast_action(&mut self, action_id: &str) {
        log::info!("Toast action triggered: {}", action_id);
        // Add domain-specific action handlers here as needed, e.g.:
        // "retry-plugin-update" => self.retry_last_plugin_update(),
        log::warn!("Unhandled toast action: {}", action_id);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Release channel helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Current feature release channel.
    pub fn release_channel(&self) -> sotf_audio_player::ReleaseChannel {
        self.ui_state.release_channel
    }

    /// Whether the given screen is accessible at the current release channel.
    pub fn is_screen_available(&self, screen: &crate::app::Screen) -> bool {
        self.ui_state.release_channel.allows(screen.maturity())
    }

    /// Whether the given plugin type is accessible at the current release channel.
    pub fn is_plugin_available(&self, plugin_type: &sotf_audio_player::PluginType) -> bool {
        self.ui_state.release_channel.allows(plugin_type.maturity())
    }

    /// Maximum number of RoomEQ channels at the current release channel.
    pub fn max_room_eq_channels(&self) -> usize {
        match self.ui_state.release_channel {
            sotf_audio_player::ReleaseChannel::Prod => 0,
            sotf_audio_player::ReleaseChannel::Beta => 3,
            sotf_audio_player::ReleaseChannel::Alpha => 128,
        }
    }

    /// Set the release channel and redirect if current screen is no longer available.
    pub fn set_release_channel(&mut self, channel: sotf_audio_player::ReleaseChannel) {
        self.ui_state.release_channel = channel;
        // If current screen is no longer available, redirect to Library
        if !self.is_screen_available(&self.ui_state.current_screen) {
            self.ui_state.current_screen = crate::app::Screen::Library;
        }
    }

    /// Set output device with debug logging
    pub fn set_output_device(&mut self, device_name: Option<String>, trigger: &str) {
        let old_device = self
            .audio_device_state
            .current_output_device_name
            .as_deref();
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
    pub fn record_playback_started(
        &mut self,
        queue_index: usize,
        track_path: Option<std::path::PathBuf>,
    ) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Started {
            queue_index,
            track_path,
        });
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
        self.playback_events
            .record_event(PlaybackEvent::TrackChanged {
                from_index,
                to_index,
                trigger,
            });
    }

    /// Record volume change event
    pub fn record_volume_changed(&mut self, from: f32, to: f32) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::VolumeChanged { from, to });
    }

    /// Record mute change event
    pub fn record_mute_changed(&mut self, muted: bool) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::MuteChanged { muted });
    }

    /// Record seek event
    pub fn record_seek(&mut self, from_position: f64, to_position: f64) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Seeked {
            from_position,
            to_position,
        });
    }

    /// Record track ended event
    pub fn record_track_ended(&mut self, queue_index: usize) {
        use super::playback_events::PlaybackEvent;
        self.playback_events
            .record_event(PlaybackEvent::TrackEnded { queue_index });
    }

    /// Record playback error event
    pub fn record_playback_error(&mut self, message: impl Into<String>) {
        use super::playback_events::PlaybackEvent;
        self.playback_events.record_event(PlaybackEvent::Error {
            message: message.into(),
        });
    }

    /// Get playback event summary for debugging
    pub fn playback_event_summary(&self) -> super::playback_events::EventStoreSummary {
        self.playback_events.summary()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Play tracking for statistics (30s threshold)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: std::path::PathBuf) {
        self.current_track_path = Some(track_path);
        self.current_track_start_time = Some(std::time::Instant::now());
        self.current_track_already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.current_track_already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.current_track_path, self.current_track_start_time)
        {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= 30
                && let Some(db) = self.library_state.library.get_database()
            {
                let duration = self.playback.position_secs as u64;
                if let Err(e) = db.record_play(path, duration) {
                    log::error!("Failed to record play: {}", e);
                } else {
                    log::info!("Recorded play for {:?} ({}s)", path, duration);
                    self.current_track_already_recorded = true;

                    // Update in-memory play_count so UI reflects immediately
                    let path = path.clone();
                    for item in &mut self.queue {
                        for track in &mut item.album.tracks {
                            if track.path == path {
                                track.play_count += 1;
                            }
                        }
                    }
                    for album in &mut self.library_state.library.albums {
                        for track in &mut album.tracks {
                            if track.path == path {
                                track.play_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Stop tracking the current track (called when track changes or stops)
    pub fn stop_track_tracking(&mut self) {
        self.current_track_path = None;
        self.current_track_start_time = None;
        self.current_track_already_recorded = false;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Favorites
    // ─────────────────────────────────────────────────────────────────────────

    /// Toggle favorite status for a track and update in-memory state
    pub fn toggle_track_favorite(&mut self, track_path: &std::path::Path) {
        let new_state = if let Some(db) = self.library_state.library.get_database() {
            match db.toggle_track_favorite(track_path) {
                Ok(state) => state,
                Err(e) => {
                    log::error!("Failed to toggle track favorite: {}", e);
                    return;
                }
            }
        } else {
            return;
        };

        // Update in queue
        for item in &mut self.queue {
            for track in &mut item.album.tracks {
                if track.path == track_path {
                    track.is_favorite = new_state;
                }
            }
        }

        // Update in library cache
        for album in &mut self.library_state.library.albums {
            for track in &mut album.tracks {
                if track.path == track_path {
                    track.is_favorite = new_state;
                }
            }
        }
    }

    /// Toggle favorite status for an album and update in-memory state
    pub fn toggle_album_favorite(&mut self, album_id: i64) {
        let new_state = if let Some(db) = self.library_state.library.get_database() {
            match db.toggle_album_favorite(album_id) {
                Ok(state) => state,
                Err(e) => {
                    log::error!("Failed to toggle album favorite: {}", e);
                    return;
                }
            }
        } else {
            return;
        };

        // Update in queue
        for item in &mut self.queue {
            if item.album.id == Some(album_id) {
                item.album.is_favorite = new_state;
            }
        }

        // Update in library cache
        for album in &mut self.library_state.library.albums {
            if album.id == Some(album_id) {
                album.is_favorite = new_state;
            }
        }

        // Invalidate library cache so sorted views update
        self.library_state.invalidate_cache();
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library_state.load_from_database()?;
        self.library_state.ensure_cache_valid();
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

        // Show tutorial on first launch
        if !self.tutorial_completed {
            self.ui_state.input_mode = InputMode::Tutorial;
            self.ui_state.tutorial_screen = 0;
            self.is_loading_initial_data = false;
            return;
        }

        // Try to load from database
        let t0 = std::time::Instant::now();
        if let Err(e) = self.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
        }
        log::info!(
            "[startup] check_library_on_startup: {:.1}ms ({} albums)",
            t0.elapsed().as_secs_f64() * 1000.0,
            self.library_state.library.albums.len(),
        );

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
        use crate::app::state::audio_device::AudioDeviceState;

        // Load available devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                // Use smart default selection that avoids virtual devices like HAL/BlackHole
                self.audio_device_state
                    .set_output_devices_with_smart_default(output_devices.clone());

                let selected_idx = self.audio_device_state.selected_output_device_index;

                // Initialize recording state playback device if not already set
                if self
                    .measurement_state
                    .recording_state
                    .playback_config
                    .device_name
                    .is_empty()
                {
                    // For recording config, prefer a device that's not virtual
                    let recording_device_idx = output_devices
                        .iter()
                        .position(|d| d.is_default && !AudioDeviceState::is_virtual_device(&d.name))
                        .or_else(|| {
                            output_devices
                                .iter()
                                .position(|d| !AudioDeviceState::is_virtual_device(&d.name))
                        })
                        .unwrap_or(selected_idx);

                    if let Some(device) = output_devices.get(recording_device_idx) {
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .device_name = device.name.clone();
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state
                                .recording_state
                                .playback_config
                                .num_channels = config.channels as usize;
                        }
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .available_sample_rates = device.available_sample_rates.clone();
                        self.measurement_state
                            .recording_state
                            .playback_config
                            .sample_rate = Self::select_default_sample_rate(
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
                    if self
                        .measurement_state
                        .recording_state
                        .recording_config
                        .device_name
                        .is_empty()
                    {
                        let device = &input_devices[default_idx];
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .device_name = device.name.clone();
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .device_id = device.name.clone();
                        if let Some(ref config) = device.default_config {
                            self.measurement_state
                                .recording_state
                                .recording_config
                                .num_channels = config.channels as usize;
                        }
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .available_sample_rates = device.available_sample_rates.clone();
                        self.measurement_state
                            .recording_state
                            .recording_config
                            .sample_rate = Self::select_default_sample_rate(
                            &device.available_sample_rates,
                            device.default_config.as_ref().map(|c| c.sample_rate),
                        );
                    }
                }
                // Clamp recording channel count to selected device's max channels
                let rec_config = &self.measurement_state.recording_state.recording_config;
                if let Some(device) = input_devices
                    .iter()
                    .find(|d| d.name == rec_config.device_name)
                    && let Some(max_ch) =
                        device.default_config.as_ref().map(|c| c.channels as usize)
                    && self
                        .measurement_state
                        .recording_state
                        .recording_config
                        .num_channels
                        > max_ch
                {
                    self.measurement_state
                        .recording_state
                        .recording_config
                        .num_channels = max_ch;
                }
            }
        }
    }

    pub fn load_config(&mut self) -> Result<LayoutState, Box<dyn std::error::Error>> {
        use crate::config::Config;
        let config = Config::load()?;
        self.load_config_from(config)
    }

    /// Load configuration from an already-loaded Config (avoids duplicate disk read).
    pub fn load_config_from(
        &mut self,
        config: crate::config::Config,
    ) -> Result<LayoutState, Box<dyn std::error::Error>> {

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

        // Restore font scale
        self.ui_state.font_scale = config.font_scale;

        // Restore release channel
        self.ui_state.release_channel = config.release_channel;

        // Build LayoutState from config
        let layout_state = LayoutState {
            queue_panel_ratio: config.panel_layout.queue_ratio,
            meters_panel_ratio: config.panel_layout.meters_ratio,
            queue_list_ratio: config.panel_layout.queue_list_ratio,
            lufs_panel_ratio: config.panel_layout.lufs_ratio,
            library_h_ratio: config.panel_layout.library_h_ratio,
            queue_h_ratio: config.panel_layout.queue_h_ratio,
            rack_h_ratio: config.panel_layout.rack_h_ratio,
            library_v_ratio: config.panel_layout.library_v_ratio,
            queue_v_ratio: config.panel_layout.queue_v_ratio,
            rack_v_ratio: config.panel_layout.rack_v_ratio,
            ..Default::default()
        };

        // Restore volume and muted state
        // self.playback.volume = config.volume; // Always start at default (10%) per requirement
        self.playback.muted = config.muted;

        // Restore recording config
        if !config.recording_config.playback.device_name.is_empty() {
            self.measurement_state.recording_state.playback_config =
                config.recording_config.playback;
        }
        if !config.recording_config.recording.device_name.is_empty() {
            self.measurement_state.recording_state.recording_config =
                config.recording_config.recording;
        }

        self.measurement_state.recording_state.signal_type = config.recording_config.signal_type;
        self.measurement_state.recording_state.signal_duration_secs =
            config.recording_config.signal_duration_secs;
        self.measurement_state.recording_state.signal_level_db =
            config.recording_config.signal_level_db;
        self.measurement_state.recording_state.mic_calibration_path =
            config.recording_config.mic_calibration_path.clone();
        // Migrate per-channel calibration paths
        let mut mic_cal_paths = config.recording_config.mic_calibration_paths;
        if mic_cal_paths.is_empty()
            && let Some(ref path) = config.recording_config.mic_calibration_path
        {
            mic_cal_paths = vec![Some(path.clone())];
        }
        self.measurement_state.recording_state.mic_calibration_paths = mic_cal_paths;
        self.measurement_state.recording_state.recording_directory =
            config.recording_config.recording_directory;
        self.measurement_state
            .recording_state
            .recording_base_directory = config.recording_config.recording_base_directory;

        // Reload calibration data if path exists (global)
        if let Some(ref path) = self.measurement_state.recording_state.mic_calibration_path
            && let Ok(content) = std::fs::read_to_string(path)
        {
            self.measurement_state.recording_state.mic_calibration_data =
                crate::app::types::CalibrationData::parse(&content);
        }

        // Reload per-channel calibration data
        self.measurement_state
            .recording_state
            .mic_calibration_data_per_channel = self
            .measurement_state
            .recording_state
            .mic_calibration_paths
            .iter()
            .map(|opt_path| {
                opt_path.as_ref().and_then(|path| {
                    std::fs::read_to_string(path)
                        .ok()
                        .and_then(|content| crate::app::types::CalibrationData::parse(&content))
                })
            })
            .collect();

        // Restore plugin presets path if we had a last loaded preset
        if let Some(preset_name) = config.last_loaded_plugin_preset {
            self.plugin_state.last_loaded_preset = Some(preset_name.clone());
            // Load the preset file
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                log::warn!("Could not find presets directory, skipping preset restore");
                return Ok(layout_state);
            };
            match self
                .plugin_state
                .chain
                .load_from_file(&presets_dir, &preset_name)
            {
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

        // Restore tutorial completed state
        self.tutorial_completed = config.tutorial_completed;
        self.seen_hints = config.seen_hints.clone();

        // Restore scanner thread count
        self.ui_state.scanner_threads = config.scanner_threads;
        if let Some(threads) = config.scanner_threads {
            self.scan_ctrl.set_num_threads(Some(threads as usize));
        }

        // Restore max CPU cores
        self.ui_state.max_cpu_cores = config.max_cpu_cores;

        Ok(layout_state)
    }

    pub fn save_config(&self, layout: &LayoutState) -> Result<(), Box<dyn std::error::Error>> {
        self.save_config_with_geometry(layout, None)
    }

    /// Save config with optional window geometry
    pub fn save_config_with_geometry(
        &self,
        layout: &LayoutState,
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
                queue_ratio: layout.queue_panel_ratio,
                meters_ratio: layout.meters_panel_ratio,
                queue_list_ratio: layout.queue_list_ratio,
                lufs_ratio: layout.lufs_panel_ratio,
                // 3-Panel Layout ratios
                library_h_ratio: layout.library_h_ratio,
                queue_h_ratio: layout.queue_h_ratio,
                rack_h_ratio: layout.rack_h_ratio,
                library_v_ratio: layout.library_v_ratio,
                queue_v_ratio: layout.queue_v_ratio,
                rack_v_ratio: layout.rack_v_ratio,
            },
            window_geometry: window_geometry.unwrap_or_else(|| {
                // If no geometry provided, use current saved value or default
                Config::load()
                    .ok()
                    .map(|c| c.window_geometry)
                    .unwrap_or_default()
            }),
            volume: self.playback.volume,
            muted: self.playback.muted,
            recording_config: crate::app::config::RecordingConfigState {
                playback: self
                    .measurement_state
                    .recording_state
                    .playback_config
                    .clone(),
                recording: self
                    .measurement_state
                    .recording_state
                    .recording_config
                    .clone(),
                signal_type: self.measurement_state.recording_state.signal_type,
                signal_duration_secs: self.measurement_state.recording_state.signal_duration_secs,
                signal_level_db: self.measurement_state.recording_state.signal_level_db,
                mic_calibration_path: self
                    .measurement_state
                    .recording_state
                    .mic_calibration_path
                    .clone(),
                mic_calibration_paths: self
                    .measurement_state
                    .recording_state
                    .mic_calibration_paths
                    .clone(),
                recording_directory: self
                    .measurement_state
                    .recording_state
                    .recording_directory
                    .clone(),
                recording_base_directory: self
                    .measurement_state
                    .recording_state
                    .recording_base_directory
                    .clone(),
            },
            font_scale: self.ui_state.font_scale,
            release_channel: self.ui_state.release_channel,
            scanner_threads: self.ui_state.scanner_threads,
            max_cpu_cores: self.ui_state.max_cpu_cores,
            tutorial_completed: self.tutorial_completed,
            seen_hints: self.seen_hints.clone(),
        };
        config.save()?;
        Ok(())
    }

    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.audio_device_state
            .output_devices
            .get(self.audio_device_state.selected_output_device_index)
            .and_then(|device| {
                device
                    .supported_configs
                    .iter()
                    .map(|c| c.channels as usize)
                    .max()
            })
    }

    /// Check and dismiss expired toast messages
    pub fn update_toast(&mut self) {
        if let Some(ref toast) = self.ui_state.toast_message
            && toast.should_dismiss()
        {
            self.ui_state.toast_message = None;
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

    /// Compute library statistics from scratch.
    /// This is expensive - O(n) over all albums and tracks.
    pub fn compute_library_stats_static(albums: &[sotf_audio_player::Album]) -> LibraryStats {
        LibraryStats::compute(albums)
    }

    /// Get library statistics, returning currently cached values.
    /// If stats are invalid, they should be updated via compute_library_stats_async.
    pub fn get_library_stats(&self) -> &LibraryStats {
        &self.library_stats
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
    fn queue_list_width(&self, layout: &LayoutState) -> f32 {
        self.ui_state.window_width * layout.queue_list_ratio
    }

    /// Calculate the width of the center panel (remaining after queue list and meters).
    fn center_panel_width(&self, layout: &LayoutState) -> f32 {
        let center_ratio = 1.0 - layout.queue_list_ratio - layout.meters_panel_ratio;
        self.ui_state.window_width * center_ratio
    }

    /// Maximum characters for queue list album title (text_sm ~7px).
    pub fn max_chars_queue_list_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.queue_list_width(layout), 56.0, 50.0, 7.0, 15, 100)
    }

    /// Maximum characters for queue list artist (text_xs ~6px).
    pub fn max_chars_queue_list_artist(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.queue_list_width(layout), 56.0, 50.0, 6.0, 15, 120)
    }

    /// Maximum characters for Now Playing album title (text_lg ~9px).
    pub fn max_chars_now_playing_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 192.0, 100.0, 9.0, 20, 150)
    }

    /// Maximum characters for Now Playing artist (text_sm ~7px).
    pub fn max_chars_now_playing_artist(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 192.0, 100.0, 7.0, 20, 180)
    }

    /// Maximum characters for track titles (text_sm ~7px).
    pub fn max_chars_track_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 120.0, 100.0, 7.0, 20, 200)
    }
}
