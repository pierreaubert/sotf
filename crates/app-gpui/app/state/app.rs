//! Application state definitions.
//!
//! Contains the main App struct and AppState wrapper.

use super::ui::LayoutState;
use super::{InputState, LibraryState, PlaybackState, PluginState, UIState};
use crate::app::constants::recording::DEFAULT_SIGNAL_LEVEL_DB;
use crate::app::debug::StateHistory;
use crate::app::manager::{Manager, ManagerError};
use crate::app::types::{
    InputMode, LayoutOrientation, LibraryStats, RackDisplayMode, ToastMessage,
};
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::i18n::{Language, Translations};
use crate::keybindings::KeymapPreset;
use crate::theme::{CommunityThemeId, Theme, ThemeAccentPreference, ThemeId};
use gpui_themes::{
    AccessibilityPalette, CommunityThemeBundle, ThemeAppearance, ThemeModePreference,
    ThemeSchedule, ThemeTransition,
};
use sotf_audio_player::QueuePlaybackEffect;
pub use sotf_audio_player::federation_scan::FederationScanResult;
use std::sync::Arc;

mod consts;
mod error;
mod federation_state;
mod misc;
mod queue_state;
mod remote_album_cache;
mod remote_refresh_requests;
mod remote_server_probe_status;
mod remote_state;
mod speaker_opt_state;
mod stream_ui_state;
mod types;

pub use consts::*;
pub use error::*;
pub use federation_state::*;
pub use queue_state::*;
pub use remote_album_cache::*;
pub use remote_refresh_requests::*;
pub use remote_server_probe_status::*;
pub use remote_state::*;
pub use speaker_opt_state::*;
pub use stream_ui_state::*;
pub use types::*;

use misc::current_minutes_after_midnight;
use misc::stream_queue_album;

#[derive(Debug)]
pub struct LibraryViewState {
    pub selected_directory_index: usize,
    pub album_list_offset: usize,
    pub stats: LibraryStats,
    pub stats_computing: bool,
    pub pending_stats: Arc<parking_lot::Mutex<Option<LibraryStats>>>,
    pub loading_initial_data: bool,
}

pub struct ScanState {
    pub needs_rescan: bool,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,
    pub status_hidden: bool,
    pub total_files: usize,
    pub started_at: Option<std::time::Instant>,
    pub progress_elapsed_secs: u64,
    pub progress_eta_secs: Option<u64>,
    pub progress_tracks_per_sec: f32,
    pub progress_phase: String,
    pub ctrl: sotf_audio_player::ScanController,
}

pub struct ModalState {
    pub metadata_editor: Option<crate::app::MetadataEditorState>,
    pub channel_conflict_path: Option<sotf_audio::decoder::AudioSource>,
    pub channel_conflicts: Vec<sotf_audio_player::ChannelConflict>,
    pub channel_conflict_track_channels: usize,
}

pub struct WorkspaceLayoutState {
    pub divider_click_start: Option<std::time::Instant>,
    pub orientation: LayoutOrientation,
    pub rack_display_mode: RackDisplayMode,
    pub hide_queue_meters_for_rack: bool,
    pub dragging_divider: Option<DividerDragState>,
    pub rack_detail_collapsed: bool,
    pub input_meter_collapsed: bool,
    pub output_meter_collapsed: bool,
    pub rack_strip_height: f32,
    pub input_meter_width: f32,
    pub output_meter_width: f32,
    pub spectrum_visible: bool,
}

pub struct DragState {
    pub volume_drag: Option<VolumeDragState>,
    pub knob_drag: Option<KnobDragState>,
}

pub struct PluginUiState {
    pub upmixer_config_open: bool,
    pub upmixer_tab: usize,
    pub spatial_spider: crate::components::plugins::spatial_spider::SpatialSpiderUiState,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub show_add_plugin_menu: bool,
    pub plugin_auto_tab: std::collections::HashMap<usize, usize>,
    pub plugin_auto_overflow_open: std::collections::HashMap<usize, bool>,
}

pub struct PlaylistState {
    pub controller: sotf_audio_player::PlaylistController,
}

pub struct SettingsState {
    pub expanded_sections: Vec<String>,
}

pub struct TutorialState {
    pub completed: bool,
    pub seen_hints: Vec<String>,
    pub current_hint: Option<crate::components::dialogs::tutorial::ContextualHint>,
}

pub struct GeometryState {
    pub last_saved_geometry: Option<crate::config::WindowGeometry>,
}

pub struct App {
    // Library view state
    pub library_view: LibraryViewState,
    // Queue state
    pub queue_state: QueueState,
    // Saved HTTP/SOTF streams
    pub stream_state: StreamUiState,
    // Speaker Optimization State
    pub speaker_opt: SpeakerOptState,
    // Level meters
    pub level_meters: LevelMeterState,
    // Scan progress / control
    pub scan: ScanState,
    // Shared album/track metadata editor modal state + channel conflict dialog
    pub modal: ModalState,
    // 3-Panel layout, rack display, divider drag / sizes
    pub layout: WorkspaceLayoutState,
    // Active drag operations
    pub drag: DragState,
    // Plugin-specific UI overlays
    pub plugin_ui: PluginUiState,
    // Playlists
    pub playlist: PlaylistState,
    // Settings accordion expanded sections
    pub settings: SettingsState,
    // Tutorial / hint state
    pub tutorial: TutorialState,
    // Cached window geometry
    pub geometry: GeometryState,
    // Play tracking for statistics (30s threshold)
    pub track_tracking: TrackTrackingState,

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

    // Federation & Server configuration
    pub federation: FederationState,

    // Native SOTF remote-control server picker state.
    pub remote: RemoteState,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            library_view: LibraryViewState {
                selected_directory_index: 0,
                album_list_offset: 0,
                stats: LibraryStats::default(),
                stats_computing: false,
                pending_stats: Arc::new(parking_lot::Mutex::new(None)),
                loading_initial_data: true,
            },
            queue_state: QueueState::new(),
            stream_state: StreamUiState::default(),

            speaker_opt: SpeakerOptState::default(),

            level_meters: LevelMeterState::default(),
            scan: ScanState {
                needs_rescan: false,
                library_scanner: None,
                status_hidden: false,
                total_files: 0,
                started_at: None,
                progress_elapsed_secs: 0,
                progress_eta_secs: None,
                progress_tracks_per_sec: 0.0,
                progress_phase: String::new(),
                ctrl: sotf_audio_player::ScanController::new(),
            },
            modal: ModalState {
                metadata_editor: None,
                channel_conflict_path: None,
                channel_conflicts: Vec::new(),
                channel_conflict_track_channels: 2,
            },
            layout: WorkspaceLayoutState {
                divider_click_start: None,
                orientation: LayoutOrientation::default(),
                rack_display_mode: RackDisplayMode::default(),
                hide_queue_meters_for_rack: false,
                dragging_divider: None,
                rack_detail_collapsed: false,
                input_meter_collapsed: false,
                output_meter_collapsed: false,
                rack_strip_height: RACK_STRIP_DEFAULT_HEIGHT,
                input_meter_width: 80.0, // Default width for input meter panel
                output_meter_width: 140.0, // Default width for output meter panel
                spectrum_visible: false,
            },
            drag: DragState {
                volume_drag: None,
                knob_drag: None,
            },
            plugin_ui: PluginUiState {
                upmixer_config_open: false,
                // Upmixer configuration tabs use the 10..=13 namespace.
                upmixer_tab: 10,
                spatial_spider:
                    crate::components::plugins::spatial_spider::SpatialSpiderUiState::default(),
                spectrum_tilt_select_open: false,
                spectrum_reference_select_open: false,
                show_add_plugin_menu: false,
                plugin_auto_tab: std::collections::HashMap::new(),
                plugin_auto_overflow_open: std::collections::HashMap::new(),
            },
            playlist: PlaylistState {
                controller: sotf_audio_player::PlaylistController::new(),
            },
            settings: SettingsState {
                expanded_sections: vec!["library".to_string()],
            },
            tutorial: TutorialState {
                completed: false,
                seen_hints: Vec::new(),
                current_hint: None,
            },
            geometry: GeometryState {
                last_saved_geometry: None,
            },
            track_tracking: TrackTrackingState::default(),

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

            federation: FederationState::default(),
            remote: RemoteState::default(),
        };

        // Initialize default stereo meter layout so meters are visible before audio starts
        app.update_level_meter_groups();

        app
    }

    pub fn save_stream_from_inputs(&mut self) -> Result<(), String> {
        let stream = sotf_audio_player::SavedStream::new(
            self.stream_state.name_input.clone(),
            self.stream_state.url_input.clone(),
            self.stream_state.format_hint(),
            self.stream_state.seekable_input,
        )
        .map_err(|err| err.to_string())?;
        self.stream_state.store.upsert(stream);
        sotf_audio_player::save_saved_streams(&self.stream_state.store)
            .map_err(|err| err.to_string())?;
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some("Stream saved".to_string());
        Ok(())
    }

    pub fn add_stream_to_queue(
        &mut self,
        stream: sotf_audio_player::SavedStream,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let was_empty = self.queue_state.is_empty();
        let was_not_playing = !self.playback.is_playing;
        let album = stream_queue_album(&stream);
        self.queue_state.add_album(album)?;
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some(format!("Added {}", stream.name));
        if was_empty || was_not_playing {
            return Ok(self.start_queue());
        }
        Ok(None)
    }

    pub fn play_stream_now(
        &mut self,
        stream: sotf_audio_player::SavedStream,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let effect = self
            .queue_state
            .play_album_now(stream_queue_album(&stream))?;
        self.playback.current_queue_index = self.queue_state.current_index();
        self.stream_state.last_error = None;
        self.stream_state.last_status = Some(format!("Playing {}", stream.name));
        if let QueuePlaybackEffect::Play(source) = effect {
            self.playback.is_playing = true;
            return Ok(Some(source));
        }
        Ok(None)
    }

    pub fn remove_stream_at(&mut self, index: usize) -> Result<(), String> {
        let Some(stream) = self.stream_state.store.streams.get(index).cloned() else {
            return Err("No stream selected".to_string());
        };
        if self.stream_state.store.remove_by_url(&stream.url) {
            sotf_audio_player::save_saved_streams(&self.stream_state.store)
                .map_err(|err| err.to_string())?;
            if self.stream_state.selected_index >= self.stream_state.store.streams.len()
                && self.stream_state.selected_index > 0
            {
                self.stream_state.selected_index -= 1;
            }
            self.stream_state.last_error = None;
            self.stream_state.last_status = Some(format!("Removed {}", stream.name));
        }
        Ok(())
    }

    pub fn set_stream_inputs_from_selected(&mut self, index: usize) {
        if let Some(stream) = self.stream_state.store.streams.get(index) {
            self.stream_state.selected_index = index;
            self.stream_state.name_input = stream.name.clone();
            self.stream_state.url_input = stream.url.clone();
            self.stream_state.format_hint_input = stream.format_hint.clone().unwrap_or_default();
            self.stream_state.seekable_input = stream.seekable;
            self.stream_state.last_error = None;
        }
    }

    pub fn save_remote_media_stream(
        &mut self,
        name: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<sotf_audio_player::SavedStream, String> {
        let stream = sotf_audio_player::SavedStream::new(name, url, None, true)
            .map_err(|err| err.to_string())?;
        self.stream_state.store.upsert(stream.clone());
        sotf_audio_player::save_saved_streams(&self.stream_state.store)
            .map_err(|err| err.to_string())?;
        Ok(stream)
    }

    pub fn record_stream_error(&mut self, error: impl Into<String>) {
        self.stream_state.last_error = Some(error.into());
        self.stream_state.last_status = None;
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
        if !self.tutorial.seen_hints.iter().any(|s| s == id_str)
            && self.tutorial.current_hint.is_none()
        {
            self.tutorial.current_hint =
                Some(crate::components::dialogs::tutorial::ContextualHint { hint_id });
        }
    }

    /// Dismiss the current hint and mark it as seen.
    pub fn dismiss_hint(&mut self) {
        if let Some(hint) = self.tutorial.current_hint.take() {
            let id_str = hint.hint_id.as_str().to_string();
            if !self.tutorial.seen_hints.contains(&id_str) {
                self.tutorial.seen_hints.push(id_str);
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
        match action_id {
            "rescan-library" => {
                if let Err(err) = self.scan_library() {
                    self.ui_state.toast_message =
                        Some(ToastMessage::error(format!("Scan failed: {err}")));
                }
            }
            _ => {
                log::warn!("Unhandled toast action: {}", action_id);
            }
        }
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
        self.track_tracking.path = Some(track_path);
        self.track_tracking.start_time = Some(std::time::Instant::now());
        self.track_tracking.already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.track_tracking.already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.track_tracking.path, self.track_tracking.start_time)
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
                    self.track_tracking.already_recorded = true;

                    // Update in-memory play_count so UI reflects immediately
                    let path = path.clone();
                    for item in self.queue_state.iter_mut() {
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
        self.track_tracking.path = None;
        self.track_tracking.start_time = None;
        self.track_tracking.already_recorded = false;
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
        for item in self.queue_state.iter_mut() {
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
        for item in self.queue_state.iter_mut() {
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
        if !self.tutorial.completed {
            self.ui_state.input_mode = InputMode::Tutorial;
            self.ui_state.tutorial_screen = 0;
            self.library_view.loading_initial_data = false;
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

        if self.should_prompt_for_empty_local_library() {
            // Show modal prompting to scan for music
            self.ui_state.input_mode = InputMode::EmptyLibraryPrompt;
        }

        self.library_view.loading_initial_data = false;
    }

    /// Whether startup should block Home/Search with the local-empty-library prompt.
    pub fn should_prompt_for_empty_local_library(&self) -> bool {
        self.library_state.library.albums.is_empty()
            && self.remote.server_store.selected_server_id.is_none()
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
        // Cache window geometry so save_config doesn't need to re-read from disk
        self.geometry.last_saved_geometry = Some(config.window_geometry.clone());

        // Restore directories
        self.library_state.library.directories = config.directories;
        self.install_external_plugin_runtime_sandbox();

        // Restore theme
        self.ui_state.theme_id = config.theme;
        self.ui_state.theme_mode_preference = config.theme_mode_preference;
        self.ui_state.accessibility_palette = config.accessibility_palette;
        self.ui_state.theme_accent_preference = config.theme_accent_preference;
        self.ui_state.community_theme_id = config.community_theme_id;
        if let Some(theme_id) = self.ui_state.community_theme_id {
            self.set_community_theme(theme_id);
        } else {
            self.apply_theme(config.theme);
        }
        self.ui_state.reduce_motion = config.reduce_motion;
        self.ui_state.density_mode = config.density_mode;

        // Restore language
        self.ui_state.language = config.language;
        self.ui_state.translations = Translations::for_language(config.language);

        // Restore keymap preset
        self.ui_state.keymap_preset = config.keymap_preset;

        // Restore font scale
        self.ui_state.font_scale = config.font_scale;

        // Restore font size bounds
        self.ui_state.min_font_size_px = config.min_font_size_px;
        self.ui_state.max_font_size_px = config.max_font_size_px;

        // Restore release channel
        self.ui_state.release_channel = config.release_channel;

        // Restore design language
        self.ui_state.design_language = config.design_language;

        // Restore plugin chassis theme state (rack default + overrides)
        self.plugin_state.rack_theme_state = config.rack_theme_state;

        // Build LayoutState from config
        let layout_state = LayoutState {
            queue_panel_ratio: config.panel_layout.queue_ratio,
            meters_panel_ratio: config.panel_layout.meters_ratio,
            queue_list_ratio: config.panel_layout.queue_list_ratio,
            lufs_panel_ratio: config.panel_layout.lufs_ratio,
            rack_detail_ratio: config.panel_layout.rack_detail_ratio,
            library_h_ratio: config.panel_layout.library_h_ratio,
            queue_h_ratio: config.panel_layout.queue_h_ratio,
            rack_h_ratio: config.panel_layout.rack_h_ratio,
            library_v_ratio: config.panel_layout.library_v_ratio,
            queue_v_ratio: config.panel_layout.queue_v_ratio,
            rack_v_ratio: config.panel_layout.rack_v_ratio,
            ..Default::default()
        };
        self.layout.rack_detail_collapsed = config.panel_layout.rack_detail_ratio <= 0.05;

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
            if (config.recording_config.signal_level_db - -20.0).abs() < f32::EPSILON {
                DEFAULT_SIGNAL_LEVEL_DB
            } else {
                config.recording_config.signal_level_db
            };
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
                .graph
                .load_from_file(&presets_dir, &preset_name)
            {
                Ok(warnings) => {
                    self.plugin_state.update_state.pending_plugin_update =
                        Some(crate::app::types::PluginUpdateType::Structural);
                    self.sync_spectrum_visible();
                    if warnings.is_empty() {
                        log::info!("Restored plugin preset: {}", preset_name);
                    } else {
                        log::warn!(
                            "Restored plugin preset '{}' with {} skipped plugin(s)",
                            preset_name,
                            warnings.len()
                        );
                        for w in &warnings {
                            log::warn!("  {}", w);
                        }
                        self.ui_state.toast_message =
                            Some(crate::app::ToastMessage::warning(format!(
                                "Preset '{}': {} plugin(s) skipped",
                                preset_name,
                                warnings.len()
                            )));
                    }
                }
                Err(e) => {
                    log::warn!("Could not restore preset '{}': {}", preset_name, e);
                }
            }
        }

        // Restore tutorial completed state
        self.tutorial.completed = config.tutorial_completed;
        self.tutorial.seen_hints = config.seen_hints.clone();

        // Restore scanner thread count
        self.ui_state.scanner_threads = config.scanner_threads;
        if let Some(threads) = config.scanner_threads {
            self.scan.ctrl.set_num_threads(Some(threads as usize));
        }

        // Restore max CPU cores
        self.ui_state.max_cpu_cores = config.max_cpu_cores;

        // Restore the remote library identity associated with any local
        // database/cache content.
        self.remote.local_library_identity = config.remote_library_identity;

        // Restore non-secret native remote server records. Bearer tokens live
        // in platform credential stores keyed by each server's token key.
        match sotf_audio_player::config::load_remote_server_store() {
            Ok(store) => {
                self.remote.server_store = store;
                self.load_persisted_remote_server_tokens();
            }
            Err(e) => {
                log::warn!("Could not load remote server store: {e}");
            }
        }

        Ok(layout_state)
    }

    pub fn save_config(&mut self, layout: &LayoutState) -> Result<(), Box<dyn std::error::Error>> {
        self.save_config_with_geometry(layout, None)
    }

    /// Save config with optional window geometry
    pub fn save_config_with_geometry(
        &mut self,
        layout: &LayoutState,
        window_geometry: Option<crate::config::WindowGeometry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::{Config, PanelLayout};
        let geometry = window_geometry.unwrap_or_else(|| {
            self.geometry
                .last_saved_geometry
                .clone()
                .unwrap_or_default()
        });
        // Update cache so future saves without geometry don't need disk I/O
        self.geometry.last_saved_geometry = Some(geometry.clone());
        let config = Config {
            directories: self.library_state.library.directories.clone(),
            last_loaded_plugin_preset: self.plugin_state.last_loaded_preset.clone(),
            theme: self.ui_state.theme_id,
            theme_mode_preference: self.ui_state.theme_mode_preference.clone(),
            accessibility_palette: self.ui_state.accessibility_palette,
            theme_accent_preference: self.ui_state.theme_accent_preference,
            community_theme_id: self.ui_state.community_theme_id,
            reduce_motion: self.ui_state.reduce_motion,
            density_mode: self.ui_state.density_mode,
            language: self.ui_state.language,
            keymap_preset: self.ui_state.keymap_preset,
            panel_layout: PanelLayout {
                queue_ratio: layout.queue_panel_ratio,
                meters_ratio: layout.meters_panel_ratio,
                queue_list_ratio: layout.queue_list_ratio,
                lufs_ratio: layout.lufs_panel_ratio,
                rack_detail_ratio: layout.rack_detail_ratio,
                // 3-Panel Layout ratios
                library_h_ratio: layout.library_h_ratio,
                queue_h_ratio: layout.queue_h_ratio,
                rack_h_ratio: layout.rack_h_ratio,
                library_v_ratio: layout.library_v_ratio,
                queue_v_ratio: layout.queue_v_ratio,
                rack_v_ratio: layout.rack_v_ratio,
            },
            window_geometry: geometry,
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
            min_font_size_px: self.ui_state.min_font_size_px,
            max_font_size_px: self.ui_state.max_font_size_px,
            tutorial_completed: self.tutorial.completed,
            seen_hints: self.tutorial.seen_hints.clone(),
            design_language: self.ui_state.design_language.clone(),
            rack_theme_state: self.plugin_state.rack_theme_state.clone(),
            remote_library_identity: self.remote.local_library_identity.clone(),
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
        let theme_id = self.ui_state.theme_id.next();
        self.set_theme(theme_id);
    }

    /// Cycle to the next language
    pub fn next_language(&mut self) {
        self.ui_state.language = self.ui_state.language.next();
        self.ui_state.translations = Translations::for_language(self.ui_state.language);
    }

    /// Set a specific theme
    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.ui_state.community_theme_id = None;
        self.apply_theme(theme_id);
        self.ui_state.theme_mode_preference = theme_id.mode_preference();
        self.ui_state.accessibility_palette = theme_id.accessibility_palette();
    }

    pub fn set_community_theme(&mut self, theme_id: CommunityThemeId) {
        let manifest = theme_id.manifest();
        self.ui_state.community_theme_id = Some(theme_id);
        self.ui_state.theme_mode_preference = manifest.preferred_mode;
        self.ui_state.accessibility_palette = manifest.accessibility;
        self.apply_community_theme(theme_id);
    }

    pub fn set_community_theme_from_json(&mut self, json: &str) -> Result<(), String> {
        let bundle = CommunityThemeBundle::from_json(json)
            .map_err(|error| format!("failed to parse community theme JSON: {error}"))?;
        let theme = Theme::from_community_bundle(&bundle)?
            .with_accent_preference(self.ui_state.theme_accent_preference);

        self.ui_state.community_theme_id = CommunityThemeId::from_id(&bundle.manifest.id);
        self.ui_state.theme_id = ThemeId::for_appearance(theme.appearance());
        self.ui_state.theme = theme;
        self.ui_state.theme_mode_preference = bundle.manifest.preferred_mode;
        self.ui_state.accessibility_palette = bundle.manifest.accessibility;
        Ok(())
    }

    pub fn set_community_theme_json_draft(&mut self, json: impl Into<String>) {
        self.ui_state.community_theme_json_draft = json.into();
    }

    pub fn apply_community_theme_json_draft(&mut self) -> Result<(), String> {
        let json = self.ui_state.community_theme_json_draft.clone();
        let trimmed = json.trim();
        if trimmed.is_empty() {
            return Err("community theme JSON is empty".to_string());
        }
        self.set_community_theme_from_json(trimmed)
    }

    /// Set the light/dark mode policy and apply its current effective theme.
    pub fn set_theme_mode_preference(&mut self, preference: ThemeModePreference) {
        self.set_theme_mode_preference_with_system(preference, ThemeAppearance::Dark);
    }

    /// Set the light/dark mode policy using the supplied system appearance.
    pub fn set_theme_mode_preference_with_system(
        &mut self,
        preference: ThemeModePreference,
        system_appearance: ThemeAppearance,
    ) {
        self.set_theme_mode_preference_at_minutes(
            preference,
            system_appearance,
            current_minutes_after_midnight(),
        );
    }

    pub fn set_theme_mode_preference_at_minutes(
        &mut self,
        preference: ThemeModePreference,
        system_appearance: ThemeAppearance,
        minutes_after_midnight: u16,
    ) {
        let appearance = preference.resolve(system_appearance, minutes_after_midnight);
        self.ui_state.theme_mode_preference = preference;
        self.ui_state.community_theme_id = None;
        self.apply_theme(ThemeId::for_accessibility_palette(
            self.ui_state.accessibility_palette,
            appearance,
        ));
    }

    pub fn theme_schedule(&self) -> ThemeSchedule {
        match &self.ui_state.theme_mode_preference {
            ThemeModePreference::Scheduled { schedule } => *schedule,
            _ => ThemeSchedule::default(),
        }
    }

    pub fn set_theme_schedule(&mut self, schedule: ThemeSchedule) {
        self.set_theme_schedule_with_system(schedule, ThemeAppearance::Dark);
    }

    pub fn set_theme_schedule_with_system(
        &mut self,
        schedule: ThemeSchedule,
        system_appearance: ThemeAppearance,
    ) {
        self.set_theme_mode_preference_with_system(
            ThemeModePreference::Scheduled { schedule },
            system_appearance,
        );
    }

    pub fn set_theme_schedule_at_minutes(
        &mut self,
        schedule: ThemeSchedule,
        system_appearance: ThemeAppearance,
        minutes_after_midnight: u16,
    ) {
        self.set_theme_mode_preference_at_minutes(
            ThemeModePreference::Scheduled { schedule },
            system_appearance,
            minutes_after_midnight,
        );
    }

    pub fn refresh_scheduled_theme(&mut self) -> bool {
        self.refresh_scheduled_theme_at_minutes(current_minutes_after_midnight())
    }

    pub fn refresh_scheduled_theme_at_minutes(&mut self, minutes_after_midnight: u16) -> bool {
        let ThemeModePreference::Scheduled { schedule } = &self.ui_state.theme_mode_preference
        else {
            return false;
        };
        let appearance = schedule.resolve_at_minutes(minutes_after_midnight);
        let theme_id =
            ThemeId::for_accessibility_palette(self.ui_state.accessibility_palette, appearance);

        if self.ui_state.community_theme_id.is_none() && self.ui_state.theme_id == theme_id {
            false
        } else {
            self.ui_state.community_theme_id = None;
            self.apply_theme(theme_id);
            true
        }
    }

    /// Set an accessibility palette and apply the matching theme.
    pub fn set_accessibility_palette(&mut self, palette: AccessibilityPalette) {
        self.set_accessibility_palette_with_system(palette, ThemeAppearance::Dark);
    }

    /// Set an accessibility palette using the supplied system appearance.
    pub fn set_accessibility_palette_with_system(
        &mut self,
        palette: AccessibilityPalette,
        system_appearance: ThemeAppearance,
    ) {
        let appearance = self.resolved_theme_appearance_with_system(system_appearance);
        self.ui_state.accessibility_palette = palette;
        self.ui_state.community_theme_id = None;
        self.apply_theme(ThemeId::for_accessibility_palette(palette, appearance));
    }

    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.ui_state.reduce_motion = reduce_motion;
    }

    pub fn set_theme_accent_preference(&mut self, preference: ThemeAccentPreference) {
        self.ui_state.theme_accent_preference = preference;
        self.apply_selected_theme();
    }

    pub fn resolved_theme_appearance(&self) -> ThemeAppearance {
        self.resolved_theme_appearance_with_system(ThemeAppearance::Dark)
    }

    pub fn resolved_theme_appearance_with_system(
        &self,
        system_appearance: ThemeAppearance,
    ) -> ThemeAppearance {
        self.ui_state
            .theme_mode_preference
            .resolve(system_appearance, current_minutes_after_midnight())
    }

    pub fn theme_transition_duration_ms(&self) -> u16 {
        ThemeTransition::default().effective_duration_ms(self.ui_state.reduce_motion)
    }

    fn apply_theme(&mut self, theme_id: ThemeId) {
        self.ui_state.theme_id = theme_id;
        self.ui_state.theme =
            Theme::from_id(theme_id).with_accent_preference(self.ui_state.theme_accent_preference);
    }

    fn apply_community_theme(&mut self, theme_id: CommunityThemeId) {
        let theme = theme_id
            .theme()
            .with_accent_preference(self.ui_state.theme_accent_preference);
        self.ui_state.theme_id = ThemeId::for_appearance(theme.appearance());
        self.ui_state.theme = theme;
    }

    fn apply_selected_theme(&mut self) {
        if let Some(theme_id) = self.ui_state.community_theme_id {
            self.apply_community_theme(theme_id);
        } else {
            self.apply_theme(self.ui_state.theme_id);
        }
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
        self.library_view.stats.valid = false;
    }

    /// Compute library statistics from scratch.
    /// This is expensive - O(n) over all albums and tracks.
    pub fn compute_library_stats_static(albums: &[sotf_audio_player::Album]) -> LibraryStats {
        LibraryStats::compute(albums)
    }

    /// Get library statistics, returning currently cached values.
    /// If stats are invalid, they should be updated via compute_library_stats_async.
    pub fn get_library_stats(&self) -> &LibraryStats {
        &self.library_view.stats
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
    pub fn center_panel_width(&self, layout: &LayoutState) -> f32 {
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
        Self::calculate_max_chars(self.center_panel_width(layout), 60.0, 0.0, 9.0, 6, 150)
    }

    /// Maximum characters for Now Playing artist (text_sm ~7px).
    pub fn max_chars_now_playing_artist(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 60.0, 0.0, 7.0, 6, 180)
    }

    /// Maximum characters for track titles (text_sm ~7px).
    pub fn max_chars_track_title(&self, layout: &LayoutState) -> usize {
        Self::calculate_max_chars(self.center_panel_width(layout), 120.0, 100.0, 7.0, 20, 200)
    }
}
