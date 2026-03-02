use crate::theme::Theme;
use sotf_audio::LoudnessData;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{Album, ChannelConflict, MusicLibrary, PluginChain, Track};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// Import types from sibling modules
use super::parameters::TuiEditablePlugin;
use super::types::{
    ArtistNode, ChannelFilter, ChannelGroup, FilePickerMode, FilePickerOrigin, InputMode,
    LibrarySortOrder, LibraryViewMode, MatrixEditMode, PendingParameterUpdate, QueueEntry, Screen,
};

pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueEntry>,
    pub current_screen: Screen,
    pub input_mode: InputMode,
    pub saved_input_mode: InputMode, // Saved mode for overlay modals (ShowHelp, ShowError, ChannelConflict)

    // Theme
    pub theme: Theme,

    // UI state
    pub search_query: String,
    pub directory_input: String,
    pub editing_directory: bool,
    pub plugin_file_input: String, // For save/load plugin chain
    pub apo_file_input: String,    // For loading APO EQ files
    pub sofa_file_input: String,   // For loading SOFA HRTF files
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub selected_queue_track_index: Option<usize>, // None = album header, Some(i) = track i
    pub selected_plugin_index: usize,
    pub add_plugin_selected_index: usize, // For plugin add dialog
    pub album_list_offset: usize,
    pub status_message: Option<String>, // For displaying save/load status
    pub error_message: Option<String>,  // For displaying decode/playback errors in a modal

    // Channel conflict dialog state
    pub channel_conflict_path: Option<PathBuf>, // File pending playback
    pub channel_conflict_selection: usize,      // Currently highlighted option (0-2)
    pub channel_conflict_track_channels: usize, // File's channel count
    pub channel_conflicts: Vec<ChannelConflict>, // All incompatible plugins

    // Cached filtered results
    pub cached_filtered_albums: Vec<Album>,
    pub needs_filter_update: bool,

    // Autocomplete state
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_index: usize,

    // Plugin preset selection
    pub available_plugin_presets: Vec<String>, // List of preset filenames
    pub selected_preset_index: usize,

    // Library tree view
    pub library_view_mode: LibraryViewMode,
    pub library_sort_order: LibrarySortOrder,
    pub channel_filter: ChannelFilter,
    pub artist_tree: Vec<ArtistNode>,
    pub selected_tree_index: usize, // Index in flattened tree (artists + visible albums)

    // Plugin system
    pub plugin_chain: PluginChain,
    pub needs_plugin_update: bool,
    pub pending_param_update: Option<PendingParameterUpdate>,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize, // Which parameter is selected in edit mode
    pub plugin_update_last_attempt: Option<std::time::Instant>,
    pub plugin_update_retry_count: u32,
    pub plugin_update_in_progress: bool,

    // Matrix editor state
    pub matrix_edit_mode: MatrixEditMode, // Header (channels/preset) or Grid (cells)
    pub matrix_grid_row: usize,           // Selected output row in grid
    pub matrix_grid_col: usize,           // Selected input column in grid
    pub matrix_header_selection: usize,   // 0 = Input Channels, 1 = Output Channels, 2 = Preset

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub position_secs: f64,
    pub current_sample_rate: Option<u32>, // Actual playback rate from engine

    // Play tracking for statistics (30s threshold)
    pub current_track_path: Option<PathBuf>,
    pub current_track_start_time: Option<std::time::Instant>,
    pub current_track_already_recorded: bool,

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

    // Audio devices
    pub output_devices: Vec<AudioDevice>,
    pub selected_output_device_index: usize,
    pub current_output_device_name: Option<String>,

    // Loading screen animation
    pub loading_tick: u16,

    // Flags
    pub read_only: bool, // Second instance: no DB writes, no scans
    pub should_quit: bool,
    pub needs_rescan: bool,
    pub needs_redraw: bool,

    // Scan progress
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,

    // Maintenance progress
    pub maintenance_in_progress: bool,
    pub maintenance_progress_checked: usize,
    pub maintenance_progress_total: usize,

    // Shared pause flag: true while playing, scanners sleep-loop on it
    pub scanner_pause_flag: Arc<AtomicBool>,
    // When true, don't auto-pause scanners even during playback
    pub scanner_pause_override: bool,

    // ReplayGain scanner manager
    pub replay_gain_manager: sotf_audio_player::ReplayGainScanManager,

    // ReplayGain playback settings
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: super::types::ReplayGainMode,
    pub replay_gain_preamp: f32,

    // Waveform scanner manager
    pub waveform_manager: sotf_audio_player::WaveformScanManager,

    // Bliss audio analysis scanner manager
    pub bliss_manager: sotf_audio_player::BlissScanManager,

    // Last loaded plugin preset name (for config persistence)
    pub last_loaded_preset: Option<String>,

    // File explorer state
    pub file_explorer_items: Vec<PathBuf>,
    pub file_explorer_selected: usize,
    pub file_explorer_dir: PathBuf,
    pub file_explorer_filter: Option<String>,
    pub file_explorer_show_hidden: bool,
    pub file_picker_mode: FilePickerMode,
    pub file_picker_origin: FilePickerOrigin,
    pub file_picker_title: String,

    // Album cover image display
    pub album_images: Vec<PathBuf>, // List of image files in current album directory
    pub selected_image_index: usize, // Current image being displayed
    pub image_picker: Option<ratatui_image::picker::Picker>, // Image protocol picker
    pub image_protocol: Option<ratatui_image::protocol::StatefulProtocol>, // Cached protocol for current image
    pub image_protocol_path: Option<PathBuf>, // Path the cached protocol was created from

    // Configure section sub-screen
    pub configure_sub_screen: super::types::ConfigureSubScreen,

    // Spinorama EQ wizard state
    pub spinorama_eq: super::types::SpinoramaEqTuiState,
    // Headphone EQ wizard state
    pub headphone_eq: super::types::HeadphoneEqTuiState,
    // Room EQ wizard state
    pub room_eq: super::types::RoomEqTuiState,
    // Recording wizard state
    pub recording: super::types::RecordingTuiState,
}

impl App {
    pub fn new(theme: Theme, read_only: bool) -> Self {
        // Try to create library with database, fallback to simple library
        let db_result = if read_only {
            MusicLibrary::with_database_secondary()
        } else {
            MusicLibrary::with_database()
        };
        let library = db_result.unwrap_or_else(|e| {
            log::warn!(
                "Failed to initialize database, using in-memory library: {}",
                e
            );
            MusicLibrary::new()
        });

        // Shared pause flag for all background scanners
        let scanner_pause_flag = Arc::new(AtomicBool::new(false));

        Self {
            library,
            queue: Vec::new(),
            current_screen: Screen::Loading,
            input_mode: InputMode::Normal,
            saved_input_mode: InputMode::Normal,
            theme,
            search_query: String::new(),
            directory_input: String::new(),
            editing_directory: false,
            plugin_file_input: String::new(),
            apo_file_input: String::new(),
            sofa_file_input: String::new(),
            selected_album_index: 0,
            selected_directory_index: 0,
            selected_queue_index: 0,
            selected_queue_track_index: None,
            selected_plugin_index: 0,
            add_plugin_selected_index: 0,
            album_list_offset: 0,
            status_message: None,
            error_message: None,
            channel_conflict_path: None,
            channel_conflict_selection: 0,
            channel_conflict_track_channels: 2,
            channel_conflicts: Vec::new(),
            cached_filtered_albums: Vec::new(),
            needs_filter_update: true,
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            library_view_mode: LibraryViewMode::Flat,
            library_sort_order: LibrarySortOrder::Artist,
            channel_filter: ChannelFilter::All,
            artist_tree: Vec::new(),
            selected_tree_index: 0,
            plugin_chain: PluginChain::with_default_rack(),
            needs_plugin_update: false,
            pending_param_update: None,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            plugin_update_last_attempt: None,
            plugin_update_retry_count: 0,
            plugin_update_in_progress: false,
            matrix_edit_mode: MatrixEditMode::Header,
            matrix_grid_row: 0,
            matrix_grid_col: 0,
            matrix_header_selection: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            current_sample_rate: None,
            current_track_path: None,
            current_track_start_time: None,
            current_track_already_recorded: false,
            loudness_info: None,
            level_meter_groups: Vec::new(),
            selected_level_meter_group: 0,
            level_meter_control_selection: 0,
            level_meter_last_channel_count: 0,
            level_meter_last_speaker_config: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            loading_tick: 0,
            read_only,
            should_quit: false,
            needs_rescan: false,
            needs_redraw: true,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            library_scanner: None,
            maintenance_in_progress: false,
            maintenance_progress_checked: 0,
            maintenance_progress_total: 0,
            scanner_pause_flag: Arc::clone(&scanner_pause_flag),
            scanner_pause_override: false,
            replay_gain_manager: sotf_audio_player::ReplayGainScanManager::with_pause_flag(
                Arc::clone(&scanner_pause_flag),
            ),
            replay_gain_enabled: true,
            replay_gain_mode: super::types::ReplayGainMode::Track,
            replay_gain_preamp: 0.0,
            waveform_manager: sotf_audio_player::WaveformScanManager::with_pause_flag(Arc::clone(
                &scanner_pause_flag,
            )),
            bliss_manager: sotf_audio_player::BlissScanManager::with_pause_flag(Arc::clone(
                &scanner_pause_flag,
            )),
            last_loaded_preset: None,
            file_explorer_items: Vec::new(),
            file_explorer_selected: 0,
            file_explorer_dir: directories::UserDirs::new()
                .map(|u| u.home_dir().to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/")),
            file_explorer_filter: None,
            file_explorer_show_hidden: false,
            file_picker_mode: FilePickerMode::File,
            file_picker_origin: FilePickerOrigin::SofaFile,
            file_picker_title: String::new(),
            album_images: Vec::new(),
            selected_image_index: 0,
            image_picker: None,
            image_protocol: None,
            image_protocol_path: None,
            configure_sub_screen: super::types::ConfigureSubScreen::Directories,
            spinorama_eq: super::types::SpinoramaEqTuiState::default(),
            headphone_eq: super::types::HeadphoneEqTuiState::default(),
            room_eq: super::types::RoomEqTuiState::default(),
            recording: super::types::RecordingTuiState::default(),
        }
    }

    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
            .map(|track| track.path.clone())
    }

    /// Get the currently playing track info
    pub fn current_track(&self) -> Option<&Track> {
        self.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
    }

    /// Compute the effective replay gain for the current track, considering mode + preamp + enabled.
    /// Returns `None` if disabled or no gain value is available.
    pub fn get_replay_gain_for_current_track(&self) -> Option<f64> {
        use super::types::ReplayGainMode;

        if !self.replay_gain_enabled {
            return None;
        }
        let track = self.current_track()?;
        let gain = match self.replay_gain_mode {
            ReplayGainMode::Track => track.replay_gain,
            ReplayGainMode::Album => track.album_gain.or(track.replay_gain),
        };
        gain.map(|g| g + self.replay_gain_preamp as f64)
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index {
            if let Some(entry) = self.queue.get_mut(idx) {
                if let Some(track) = entry.item.next_track() {
                    return Some(track.path.clone());
                }
            }

            // Album finished (or entry missing), remove it and move to next
            self.remove_from_queue(idx);
            return self.current_track_path();
        }
        None
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index
            && let Some(entry) = self.queue.get_mut(idx)
        {
            if let Some(track) = entry.item.previous_track() {
                return Some(track.path.clone());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.current_queue_index = Some(idx - 1);
                    // Go to last track of previous album
                    if let Some(prev_entry) = self.queue.get_mut(idx - 1) {
                        prev_entry.item.current_track_index =
                            prev_entry.item.album.tracks.len().saturating_sub(1);
                    }
                    return self.current_track_path();
                }
            }
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        if !self.queue.is_empty() {
            self.current_queue_index = Some(0);
            self.queue[0].item.current_track_index = 0;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    /// Jump to the selected album/track in queue and start playing
    pub fn jump_to_selected_album(&mut self) -> Option<PathBuf> {
        if self.selected_queue_index < self.queue.len() {
            self.current_queue_index = Some(self.selected_queue_index);
            let track_idx = self.selected_queue_track_index.unwrap_or(0);
            self.queue[self.selected_queue_index]
                .item
                .current_track_index = track_idx;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    /// Load APO file and update the currently selected EQ plugin
    pub fn load_apo_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::{EQFilter, PluginSettings};
        use std::path::Path;

        let path = Path::new(&self.apo_file_input);

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently selected plugin if it's an EQ
        if let Some(plugin) = self.plugin_chain.get_plugin_mut(self.selected_plugin_index) {
            if let PluginSettings::EQ { channels, .. } = &plugin.settings {
                let channels = *channels;
                let filter_count = filters.len();
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters: None,
                    per_channel_mode: false,
                    max_filters: filter_count.clamp(1, 20),
                };
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin selected".to_string())
        }
    }

    /// Update SOFA file path for the currently selected binaural decoder plugin
    pub fn load_sofa_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::PluginSettings;

        // Update the currently selected plugin if it's a binaural decoder
        if let Some(plugin) = self.plugin_chain.get_plugin_mut(self.selected_plugin_index) {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = self.sofa_file_input.clone();
                Ok(())
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin selected".to_string())
        }
    }

    /// Returns `(slot_index, filter_count)` for the last non-permanent EQ plugin, or `None`.
    pub fn find_last_eq_info(&self) -> Option<(usize, usize)> {
        use sotf_audio_player::PluginSettings;
        (0..self.plugin_chain.len()).rev().find_map(|i| {
            if let Some(p) = self.plugin_chain.get_plugin(i) {
                if !p.is_permanent() {
                    if let PluginSettings::EQ { filters, .. } = &p.settings {
                        return Some((i, filters.len()));
                    }
                }
            }
            None
        })
    }

    /// Generic: convert biquad filter tuples to EQFilters and apply to the plugin chain.
    /// `filters` are (filter_type_str, freq, q, db_gain) tuples.
    /// Returns a success message or an error string.
    pub fn apply_eq_filters_to_chain(
        &mut self,
        filters: &[(String, f64, f64, f64)],
        label: &str,
    ) -> Result<String, String> {
        use math_audio_iir_fir::BiquadFilterType;
        use sotf_audio_player::{EQFilter, PluginSettings, PluginType};

        if filters.is_empty() {
            return Err("No optimization results to apply".to_string());
        }

        let eq_filters: Vec<EQFilter> = filters
            .iter()
            .map(|(ft_str, freq, q, db_gain)| {
                let ft = match ft_str.as_str() {
                    "Peak" => BiquadFilterType::Peak,
                    "Lowshelf" => BiquadFilterType::Lowshelf,
                    "Highshelf" => BiquadFilterType::Highshelf,
                    "Lowpass" => BiquadFilterType::Lowpass,
                    "Highpass" => BiquadFilterType::Highpass,
                    "Bandpass" => BiquadFilterType::Bandpass,
                    "Notch" => BiquadFilterType::Notch,
                    "AllPass" => BiquadFilterType::AllPass,
                    _ => BiquadFilterType::Peak,
                };
                EQFilter::new(ft, *freq, *q, *db_gain)
            })
            .collect();

        let n = eq_filters.len();

        // Find the last non-permanent EQ plugin
        let eq_idx = (0..self.plugin_chain.len()).rev().find(|&i| {
            if let Some(p) = self.plugin_chain.get_plugin(i) {
                !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. })
            } else {
                false
            }
        });

        let target_idx = if let Some(idx) = eq_idx {
            idx
        } else {
            // No EQ plugin found — insert one at the user-plugin slot
            let insert_at = self.plugin_chain.user_plugin_insert_index();
            self.plugin_chain.insert_plugin(insert_at, &PluginType::EQ);
            insert_at
        };

        // Update the plugin settings
        if let Some(plugin) = self.plugin_chain.get_plugin_mut(target_idx) {
            let channels = match &plugin.settings {
                PluginSettings::EQ { channels, .. } => *channels,
                _ => 2,
            };
            plugin.settings = PluginSettings::EQ {
                channels,
                filters: eq_filters,
                channel_filters: None,
                per_channel_mode: false,
                max_filters: n.clamp(1, 20),
            };
            plugin.enabled = true;
        }

        self.plugin_chain.update_channel_dependent_plugins();
        self.request_plugin_update();

        Ok(format!(
            "Applied {} EQ filters for '{}' to plugin slot {}",
            n, label, target_idx
        ))
    }

    /// Apply Spinorama EQ results to the plugin chain.
    pub fn apply_spinorama_to_plugin_chain(&mut self) -> Result<String, String> {
        let filters: Vec<_> = self
            .spinorama_eq
            .filters
            .iter()
            .map(|f| (f.filter_type.clone(), f.freq, f.q, f.db_gain))
            .collect();
        let label = self
            .spinorama_eq
            .selected_speaker
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        self.apply_eq_filters_to_chain(&filters, &label)
    }

    /// Apply Headphone EQ results to the plugin chain.
    pub fn apply_headphone_to_plugin_chain(&mut self) -> Result<String, String> {
        let filters: Vec<_> = self
            .headphone_eq
            .filters
            .iter()
            .map(|f| (f.filter_type.clone(), f.freq, f.q, f.db_gain))
            .collect();
        let label = std::path::Path::new(&self.headphone_eq.measurement_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "headphone".to_string());
        self.apply_eq_filters_to_chain(&filters, &label)
    }

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: PathBuf) {
        self.current_track_path = Some(track_path);
        self.current_track_start_time = Some(std::time::Instant::now());
        self.current_track_already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.read_only || self.current_track_already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.current_track_path, self.current_track_start_time)
        {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= 30 {
                // Record the play in the database
                if let Some(db) = self.library.get_database() {
                    let duration = self.position_secs as u64;
                    if let Err(e) = db.record_play(path, duration) {
                        log::error!("Failed to record play: {}", e);
                    } else {
                        log::info!("Recorded play for {:?} ({}s)", path, duration);
                        self.current_track_already_recorded = true;
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

    // ========================================================================
    // Favorites Methods
    // ========================================================================

    /// Toggle favorite on the currently selected album in library view
    pub fn toggle_selected_album_favorite(&mut self) {
        // Copy the index first to avoid borrow conflicts with filtered_albums()
        let idx = self.selected_album_index;
        let album_id = self.cached_filtered_albums.get(idx).and_then(|a| a.id);
        if let Some(album_id) = album_id {
            if let Some(db) = self.library.get_database() {
                match db.toggle_album_favorite(album_id) {
                    Ok(new_state) => {
                        // Update in-memory state
                        for a in &mut self.library.albums {
                            if a.id == Some(album_id) {
                                a.is_favorite = new_state;
                            }
                        }
                        // Invalidate filter cache since library data changed
                        self.request_filter_update();
                        log::info!(
                            "Toggled album favorite: id={} is_favorite={}",
                            album_id,
                            new_state
                        );
                    }
                    Err(e) => log::error!("Failed to toggle album favorite: {}", e),
                }
            }
        }
    }

    /// Toggle favorite on the current queue album
    pub fn toggle_current_queue_album_favorite(&mut self) {
        let album_id = self
            .current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.album.id);
        if let Some(album_id) = album_id {
            if let Some(db) = self.library.get_database() {
                match db.toggle_album_favorite(album_id) {
                    Ok(new_state) => {
                        // Update in queue
                        for qi in &mut self.queue {
                            if qi.item.album.id == Some(album_id) {
                                qi.item.album.is_favorite = new_state;
                            }
                        }
                        // Update in library
                        for a in &mut self.library.albums {
                            if a.id == Some(album_id) {
                                a.is_favorite = new_state;
                            }
                        }
                        log::info!(
                            "Toggled queue album favorite: id={} is_favorite={}",
                            album_id,
                            new_state
                        );
                    }
                    Err(e) => log::error!("Failed to toggle album favorite: {}", e),
                }
            }
        }
    }

    // ========================================================================
    // File Explorer Methods
    // ========================================================================

    /// Open the file explorer modal for the given origin context.
    pub fn open_file_explorer(
        &mut self,
        origin: FilePickerOrigin,
        mode: FilePickerMode,
        title: &str,
        start_dir: Option<&str>,
        extension_filter: Option<&str>,
    ) {
        self.file_picker_origin = origin;
        self.file_picker_mode = mode;
        self.file_picker_title = title.to_string();
        self.file_explorer_filter = extension_filter.map(|s| s.to_lowercase());
        self.file_explorer_show_hidden = false;

        // Smart start directory: use provided path's parent if it exists, else home
        let dir = start_dir
            .and_then(|s| {
                let p = std::path::Path::new(s);
                if p.is_dir() {
                    Some(p.to_path_buf())
                } else {
                    p.parent()
                        .filter(|pp| pp.is_dir())
                        .map(|pp| pp.to_path_buf())
                }
            })
            .unwrap_or_else(|| {
                directories::UserDirs::new()
                    .map(|u| u.home_dir().to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/"))
            });

        self.file_explorer_dir = dir;
        self.refresh_file_explorer();
        self.input_mode = InputMode::FileExplorer;
    }

    /// Close the file explorer and restore the appropriate input mode.
    pub fn close_file_explorer(&mut self) {
        self.input_mode =
            match self.file_picker_origin {
                FilePickerOrigin::SofaFile
                | FilePickerOrigin::IrFile
                | FilePickerOrigin::ApoFile => InputMode::EditPlugin,
                FilePickerOrigin::AddDirectory => InputMode::ConfigureDirectories,
                FilePickerOrigin::RecordingOutputDir
                | FilePickerOrigin::RecordingMicCalibration => InputMode::ConfigureRecording,
                FilePickerOrigin::RoomEqFilePath | FilePickerOrigin::RoomEqExportPath => {
                    InputMode::ConfigureRoomEq
                }
                FilePickerOrigin::HeadphoneMeasurement
                | FilePickerOrigin::HeadphoneCustomTarget => InputMode::ConfigureHeadphoneEq,
            };
    }

    /// Save the current input mode and switch to an overlay modal.
    pub fn enter_overlay_mode(&mut self, mode: InputMode) {
        self.saved_input_mode = self.input_mode;
        self.input_mode = mode;
    }

    /// Restore the input mode that was active before the overlay modal.
    pub fn exit_overlay_mode(&mut self) {
        self.input_mode = self.saved_input_mode;
        self.saved_input_mode = InputMode::Normal;
    }

    pub fn refresh_file_explorer(&mut self) {
        self.file_explorer_items.clear();
        self.file_explorer_selected = 0;

        if let Ok(entries) = std::fs::read_dir(&self.file_explorer_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Skip hidden files unless toggled on
                if !self.file_explorer_show_hidden && name.starts_with('.') {
                    continue;
                }

                if path.is_dir() {
                    dirs.push(path);
                } else if path.is_file() {
                    if let Some(ext) = &self.file_explorer_filter {
                        if path
                            .extension()
                            .is_some_and(|e| e.to_string_lossy().to_lowercase() == *ext)
                        {
                            files.push(path);
                        }
                    } else {
                        files.push(path);
                    }
                }
            }

            dirs.sort();
            files.sort();

            self.file_explorer_items.extend(dirs);
            self.file_explorer_items.extend(files);
        }
    }

    pub fn file_explorer_enter_dir(&mut self, path: PathBuf) {
        self.file_explorer_dir = path;
        self.refresh_file_explorer();
    }

    pub fn file_explorer_go_parent(&mut self) {
        if let Some(parent) = self.file_explorer_dir.parent() {
            let parent = parent.to_path_buf();
            self.file_explorer_dir = parent;
            self.refresh_file_explorer();
        }
    }

    pub fn file_explorer_select_next(&mut self) {
        if !self.file_explorer_items.is_empty() {
            self.file_explorer_selected =
                (self.file_explorer_selected + 1) % self.file_explorer_items.len();
        }
    }

    pub fn file_explorer_select_prev(&mut self) {
        if !self.file_explorer_items.is_empty() {
            if self.file_explorer_selected == 0 {
                self.file_explorer_selected = self.file_explorer_items.len() - 1;
            } else {
                self.file_explorer_selected -= 1;
            }
        }
    }

    pub fn file_explorer_toggle_hidden(&mut self) {
        self.file_explorer_show_hidden = !self.file_explorer_show_hidden;
        self.refresh_file_explorer();
    }

    /// Returns the currently selected path, if any.
    pub fn file_explorer_current(&self) -> Option<&PathBuf> {
        self.file_explorer_items.get(self.file_explorer_selected)
    }
}

// Helper function to get parameter count for a plugin
pub(super) fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    settings.get_descriptors().len()
}

impl Default for App {
    fn default() -> Self {
        Self::new(Theme::default(), false)
    }
}
