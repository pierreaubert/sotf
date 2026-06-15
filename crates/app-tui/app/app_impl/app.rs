use super::super::types::{
    ArtistNode, CastDeviceInfo, ChannelFilter, ChannelGroup, FilePickerMode, FilePickerOrigin,
    InputMode, LibrarySortOrder, LibraryViewMode, MatrixEditMode, PendingParameterUpdate,
    QueueEntry, Screen,
};
use super::load::load_server_tui_state;
use super::types::FederationScanResult;
use crate::theme::Theme;
use sotf_audio::LoudnessData;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::plugin_graph::PluginGraph;
use sotf_audio_player::{Album, ChannelConflict, MusicLibrary, Track};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct UiState {
    pub loading_tick: u16,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub needs_redraw: bool,
}

pub struct ModalState {
    pub metadata_editor: Option<super::super::types::MetadataEditorState>,
    pub channel_conflict_path: Option<sotf_audio::decoder::AudioSource>,
    pub channel_conflict_selection: usize,
    pub channel_conflict_track_channels: usize,
    pub channel_conflicts: Vec<ChannelConflict>,
}

pub struct LibraryViewState {
    pub search_query: String,
    pub directory_input: String,
    pub editing_directory: bool,
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub album_list_offset: usize,
    pub cached_filtered_albums: Vec<Album>,
    pub needs_filter_update: bool,
    pub mode: LibraryViewMode,
    pub sort_order: LibrarySortOrder,
    pub channel_filter: ChannelFilter,
    pub show_favorites_only: bool,
    pub artist_tree: Vec<ArtistNode>,
    pub selected_tree_index: usize,
    #[cfg(not(target_os = "windows"))]
    pub album_images: Vec<PathBuf>,
    #[cfg(not(target_os = "windows"))]
    pub selected_image_index: usize,
    #[cfg(not(target_os = "windows"))]
    pub image_picker: Option<ratatui_image::picker::Picker>,
    #[cfg(not(target_os = "windows"))]
    pub image_protocol: Option<ratatui_image::protocol::StatefulProtocol>,
    #[cfg(not(target_os = "windows"))]
    pub image_protocol_path: Option<PathBuf>,
}

pub struct QueueViewState {
    pub selected_index: usize,
    pub selected_track_index: Option<usize>,
}

pub struct PlaybackState {
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub muted: bool,
    pub position_secs: f64,
    pub current_sample_rate: Option<u32>,
    pub current_track_path: Option<PathBuf>,
    pub current_track_start_time: Option<std::time::Instant>,
    pub current_track_already_recorded: bool,
    pub loudness_info: Option<LoudnessData>,
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: super::super::types::ReplayGainMode,
    pub replay_gain_preamp: f32,
}

pub struct PluginRackState {
    pub graph: PluginGraph,
    pub needs_update: bool,
    pub pending_param_update: Option<PendingParameterUpdate>,
    pub editing_index: Option<usize>,
    pub param_selection: usize,
    pub update_last_attempt: Option<std::time::Instant>,
    pub update_retry_count: u32,
    pub update_in_progress: bool,
    pub selected_index: usize,
    pub add_selected_index: usize,
    pub available_presets: Vec<String>,
    pub selected_preset_index: usize,
    pub file_input: String,
    pub apo_input: String,
    pub sofa_input: String,
    pub last_loaded_preset: Option<String>,
}

pub struct PlaylistState {
    pub controller: sotf_audio_player::PlaylistController,
    pub mode: super::super::types::PlaylistMode,
    pub name_input: String,
}

pub struct MatrixEditState {
    pub edit_mode: MatrixEditMode,
    pub grid_row: usize,
    pub grid_col: usize,
    pub header_selection: usize,
}

pub struct LevelMeterState {
    pub groups: Vec<ChannelGroup>,
    pub selected_group: usize,
    pub control_selection: usize,
    pub last_channel_count: usize,
    pub last_speaker_config: Option<String>,
}

pub struct AudioDeviceState {
    pub outputs: Vec<AudioDevice>,
    pub selected_output_index: usize,
    pub current_output_name: Option<String>,
    pub cast: Vec<CastDeviceInfo>,
    pub cast_discovery_running: bool,
    pub cast_discovery_receiver: Option<std::sync::mpsc::Receiver<Vec<CastDeviceInfo>>>,
}

pub struct MediaControlState {
    pub last_queue_index: Option<usize>,
    pub last_title: Option<String>,
    pub last_artist: Option<String>,
    pub last_album: Option<String>,
    pub last_cover_url: Option<String>,
    pub last_duration_secs: Option<u64>,
    pub last_position_secs: f64,
    pub last_is_playing_state: bool,
    pub last_loudness_signature: u64,
}

pub struct ScanState {
    pub in_progress: bool,
    pub progress_tracks: usize,
    pub progress_albums: usize,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,
    pub maintenance_in_progress: bool,
    pub maintenance_progress_checked: usize,
    pub maintenance_progress_total: usize,
    pub pause_flag: Arc<AtomicBool>,
    pub pause_override: bool,
    pub replay_gain_manager: sotf_audio_player::ReplayGainScanManager,
    pub waveform_manager: sotf_audio_player::WaveformScanManager,
    pub bliss_manager: sotf_audio_player::BlissScanManager,
    pub threads: Option<usize>,
    pub needs_rescan: bool,
}

pub struct FileExplorerState {
    pub items: Vec<PathBuf>,
    pub selected: usize,
    pub dir: PathBuf,
    pub filter: Option<String>,
    pub show_hidden: bool,
    pub picker_mode: FilePickerMode,
    pub picker_origin: FilePickerOrigin,
    pub picker_title: String,
}

pub struct AutocompleteState {
    pub suggestions: Vec<String>,
    pub index: usize,
    pub menu_active: bool,
}

pub struct FederationReceivers {
    pub scan: Option<std::sync::mpsc::Receiver<FederationScanResult>>,
    pub test: Option<
        std::sync::mpsc::Receiver<(
            String,
            sotf_audio_player::federation_config::ConnectionStatus,
        )>,
    >,
}

pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueEntry>,
    pub queue_view: QueueViewState,
    pub current_screen: Screen,
    pub input_mode: InputMode,
    pub saved_input_mode: InputMode,
    pub theme: Theme,
    pub read_only: bool,
    pub should_quit: bool,
    pub ui: UiState,
    pub modal: ModalState,
    pub library_view: LibraryViewState,
    pub playback: PlaybackState,
    pub plugin_rack: PluginRackState,
    pub playlists: PlaylistState,
    pub matrix: MatrixEditState,
    pub level_meters: LevelMeterState,
    pub audio_devices: AudioDeviceState,
    pub media_control: MediaControlState,
    pub scan: ScanState,
    pub file_explorer: FileExplorerState,
    pub autocomplete: AutocompleteState,
    pub configure_sub_screen: super::super::types::ConfigureSubScreen,
    pub spinorama_eq: super::super::types::SpinoramaEqTuiState,
    pub headphone_eq: super::super::types::HeadphoneEqTuiState,
    pub room_eq: super::super::types::RoomEqTuiState,
    pub recording: super::super::types::RecordingTuiState,
    pub federation_state: super::super::types::FederationTuiState,
    pub server_state: super::super::types::ServersTuiState,
    pub federation_receivers: FederationReceivers,
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

        let mut app = Self {
            library,
            queue: Vec::new(),
            queue_view: QueueViewState {
                selected_index: 0,
                selected_track_index: None,
            },
            current_screen: Screen::Loading,
            input_mode: InputMode::Normal,
            saved_input_mode: InputMode::Normal,
            theme,
            read_only,
            should_quit: false,
            ui: UiState {
                loading_tick: 0,
                status_message: None,
                error_message: None,
                needs_redraw: true,
            },
            modal: ModalState {
                metadata_editor: None,
                channel_conflict_path: None,
                channel_conflict_selection: 0,
                channel_conflict_track_channels: 2,
                channel_conflicts: Vec::new(),
            },
            library_view: LibraryViewState {
                search_query: String::new(),
                directory_input: String::new(),
                editing_directory: false,
                selected_album_index: 0,
                selected_directory_index: 0,
                album_list_offset: 0,
                cached_filtered_albums: Vec::new(),
                needs_filter_update: true,
                mode: LibraryViewMode::Flat,
                sort_order: LibrarySortOrder::Artist,
                channel_filter: ChannelFilter::All,
                show_favorites_only: false,
                artist_tree: Vec::new(),
                selected_tree_index: 0,
                #[cfg(not(target_os = "windows"))]
                album_images: Vec::new(),
                #[cfg(not(target_os = "windows"))]
                selected_image_index: 0,
                #[cfg(not(target_os = "windows"))]
                image_picker: None,
                #[cfg(not(target_os = "windows"))]
                image_protocol: None,
                #[cfg(not(target_os = "windows"))]
                image_protocol_path: None,
            },
            playback: PlaybackState {
                is_playing: false,
                current_queue_index: None,
                volume: 0.1,
                muted: false,
                position_secs: 0.0,
                current_sample_rate: None,
                current_track_path: None,
                current_track_start_time: None,
                current_track_already_recorded: false,
                loudness_info: None,
                replay_gain_enabled: true,
                replay_gain_mode: super::super::types::ReplayGainMode::Track,
                replay_gain_preamp: 0.0,
            },
            plugin_rack: PluginRackState {
                graph: PluginGraph::with_default_rack(),
                needs_update: false,
                pending_param_update: None,
                editing_index: None,
                param_selection: 0,
                update_last_attempt: None,
                update_retry_count: 0,
                update_in_progress: false,
                selected_index: 0,
                add_selected_index: 0,
                available_presets: Vec::new(),
                selected_preset_index: 0,
                file_input: String::new(),
                apo_input: String::new(),
                sofa_input: String::new(),
                last_loaded_preset: None,
            },
            playlists: PlaylistState {
                controller: sotf_audio_player::PlaylistController::new(),
                mode: super::super::types::PlaylistMode::List,
                name_input: String::new(),
            },
            matrix: MatrixEditState {
                edit_mode: MatrixEditMode::Header,
                grid_row: 0,
                grid_col: 0,
                header_selection: 0,
            },
            level_meters: LevelMeterState {
                groups: Vec::new(),
                selected_group: 0,
                control_selection: 0,
                last_channel_count: 0,
                last_speaker_config: None,
            },
            audio_devices: AudioDeviceState {
                outputs: Vec::new(),
                selected_output_index: 0,
                current_output_name: None,
                cast: Vec::new(),
                cast_discovery_running: false,
                cast_discovery_receiver: None,
            },
            media_control: MediaControlState {
                last_queue_index: None,
                last_title: None,
                last_artist: None,
                last_album: None,
                last_cover_url: None,
                last_duration_secs: None,
                last_position_secs: f64::NAN,
                last_is_playing_state: false,
                last_loudness_signature: 0,
            },
            scan: ScanState {
                in_progress: false,
                progress_tracks: 0,
                progress_albums: 0,
                library_scanner: None,
                maintenance_in_progress: false,
                maintenance_progress_checked: 0,
                maintenance_progress_total: 0,
                pause_flag: Arc::clone(&scanner_pause_flag),
                pause_override: false,
                replay_gain_manager: sotf_audio_player::ReplayGainScanManager::with_pause_flag(
                    Arc::clone(&scanner_pause_flag),
                ),
                waveform_manager: sotf_audio_player::WaveformScanManager::with_pause_flag(
                    Arc::clone(&scanner_pause_flag),
                ),
                bliss_manager: sotf_audio_player::BlissScanManager::with_pause_flag(
                    Arc::clone(&scanner_pause_flag),
                ),
                threads: None,
                needs_rescan: false,
            },
            file_explorer: FileExplorerState {
                items: Vec::new(),
                selected: 0,
                dir: directories::UserDirs::new()
                    .map(|u| u.home_dir().to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/")),
                filter: None,
                show_hidden: false,
                picker_mode: FilePickerMode::File,
                picker_origin: FilePickerOrigin::SofaFile,
                picker_title: String::new(),
            },
            autocomplete: AutocompleteState {
                suggestions: Vec::new(),
                index: 0,
                menu_active: false,
            },
            configure_sub_screen: super::super::types::ConfigureSubScreen::Directories,
            spinorama_eq: super::super::types::SpinoramaEqTuiState::default(),
            headphone_eq: super::super::types::HeadphoneEqTuiState::default(),
            room_eq: super::super::types::RoomEqTuiState::default(),
            recording: super::super::types::RecordingTuiState::default(),
            federation_state: super::super::types::FederationTuiState::default(),
            server_state: load_server_tui_state(),
            federation_receivers: FederationReceivers { scan: None, test: None },
        };

        // Load playlists from database
        if let Some(db) = app.library.get_database()
            && let Err(e) = app.playlists.controller.load_playlists(db)
        {
            log::warn!("Failed to load playlists: {}", e);
        }

        app
    }

    /// Set the number of scanner threads for all background scanners.
    /// If None, each scanner will auto-detect (capped at 4).
    pub fn set_scanner_threads(&mut self, threads: Option<usize>) {
        self.scan.threads = threads;
        self.scan.replay_gain_manager.set_num_threads(threads);
        self.scan.waveform_manager.set_num_threads(threads);
        self.scan.bliss_manager.set_num_threads(threads);
        if let Some(t) = threads {
            log::info!("Scanner thread count set to {}", t);
        }
    }

    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.current_track_source()
            .and_then(|s| s.as_path().map(|p| p.to_path_buf()))
    }

    pub fn current_track_source(&self) -> Option<sotf_audio::decoder::AudioSource> {
        self.playback.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
            .map(|track| track.audio_source())
    }

    /// Get the currently playing track info
    pub fn current_track(&self) -> Option<&Track> {
        self.playback.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
    }

    /// Peek at the next track without mutating state (for gapless pre-queuing).
    pub fn peek_next_track(&self) -> Option<&Track> {
        let idx = self.playback.current_queue_index?;
        let entry = self.queue.get(idx)?;

        // Try next track in current album
        if let Some(track) = entry.item.peek_next_track() {
            return Some(track);
        }

        // Try first track of next album
        self.queue.get(idx + 1)?.item.album.tracks.first()
    }

    /// Compute the effective replay gain for the current track, considering mode + preamp + enabled.
    /// Returns `None` if disabled or no gain value is available.
    pub fn get_replay_gain_for_current_track(&self) -> Option<f64> {
        use super::super::types::ReplayGainMode;

        if !self.playback.replay_gain_enabled {
            return None;
        }
        let track = self.current_track()?;
        let gain = match self.playback.replay_gain_mode {
            ReplayGainMode::Track => track.replay_gain,
            ReplayGainMode::Album => track.album_gain,
        };
        gain.map(|g| g + self.playback.replay_gain_preamp as f64)
    }

    pub fn next_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        if let Some(idx) = self.playback.current_queue_index {
            if let Some(entry) = self.queue.get_mut(idx)
                && let Some(track) = entry.item.next_track()
            {
                return Some(track.audio_source());
            }

            // Album finished (or entry missing), remove it and move to next
            self.remove_from_queue(idx);
            return self.current_track_source();
        }
        None
    }

    pub fn previous_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        if let Some(idx) = self.playback.current_queue_index
            && let Some(entry) = self.queue.get_mut(idx)
        {
            if let Some(track) = entry.item.previous_track() {
                return Some(track.audio_source());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.playback.current_queue_index = Some(idx - 1);
                    // Go to last track of previous album
                    if let Some(prev_entry) = self.queue.get_mut(idx - 1) {
                        prev_entry.item.current_track_index =
                            prev_entry.item.album.tracks.len().saturating_sub(1);
                    }
                    return self.current_track_source();
                }
            }
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        if !self.queue.is_empty() {
            self.playback.current_queue_index = Some(0);
            self.queue[0].item.current_track_index = 0;
            self.playback.is_playing = true;
            self.current_track_source()
        } else {
            None
        }
    }

    /// Jump to the selected album/track in queue and start playing
    pub fn jump_to_selected_album(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        if self.queue_view.selected_index < self.queue.len() {
            self.playback.current_queue_index = Some(self.queue_view.selected_index);
            let track_idx = self.queue_view.selected_track_index.unwrap_or(0);
            self.queue[self.queue_view.selected_index]
                .item
                .current_track_index = track_idx;
            self.playback.is_playing = true;
            self.current_track_source()
        } else {
            None
        }
    }

    /// Load APO file and update the currently selected EQ plugin
    pub fn load_apo_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::{EQFilter, PluginSettings};
        use std::path::Path;

        let path = Path::new(&self.plugin_rack.apo_input);

        // Validate path before reading
        sotf_audio_player::security::validate_plugin_file_path(path).map_err(|e| e.to_string())?;

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently selected plugin if it's an EQ
        if let Some(plugin) = self.plugin_rack.graph.get_plugin_mut(self.plugin_rack.selected_index) {
            if let PluginSettings::EQ { channels, .. } = &plugin.settings {
                let channels = *channels;
                let filter_count = filters.len();
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters: None,
                    per_channel_mode: false,
                    max_filters: filter_count.clamp(1, 20),
                    tdf2: false,
                    topology: 0.0,
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
        if let Some(plugin) = self.plugin_rack.graph.get_plugin_mut(self.plugin_rack.selected_index) {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = self.plugin_rack.sofa_input.clone();
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
        (0..self.plugin_rack.graph.len()).rev().find_map(|i| {
            if let Some(p) = self.plugin_rack.graph.get_plugin(i)
                && !p.is_permanent()
                && let PluginSettings::EQ { filters, .. } = &p.settings
            {
                return Some((i, filters.len()));
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
        let eq_idx = (0..self.plugin_rack.graph.len()).rev().find(|&i| {
            if let Some(p) = self.plugin_rack.graph.get_plugin(i) {
                !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. })
            } else {
                false
            }
        });

        let target_idx = if let Some(idx) = eq_idx {
            idx
        } else {
            // No EQ plugin found — insert one at the user-plugin slot
            let insert_at = self.plugin_rack.graph.user_plugin_insert_index();
            self.plugin_rack.graph
                .insert_plugin(insert_at, &PluginType::EQ)
                .ok();
            insert_at
        };

        // Update the plugin settings
        if let Some(plugin) = self.plugin_rack.graph.get_plugin_mut(target_idx) {
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
                tdf2: false,
                topology: 0.0,
            };
            plugin.enabled = true;
        }

        self.plugin_rack.graph.update_channel_dependent_plugins();
        self.request_plugin_update();

        Ok(format!(
            "Applied {} EQ filters for '{}' to plugin slot {}",
            n, label, target_idx
        ))
    }

    /// Apply Spinorama EQ results to the plugin chain.
    pub fn apply_spinorama_to_plugins(&mut self) -> Result<String, String> {
        let filters: Vec<_> = self
            .spinorama_eq
            .model
            .filters
            .iter()
            .map(|f| (f.filter_type.clone(), f.freq, f.q, f.db_gain))
            .collect();
        let label = self
            .spinorama_eq
            .model
            .selected_speaker
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        self.apply_eq_filters_to_chain(&filters, &label)
    }

    /// Apply Headphone EQ results to the plugin chain.
    pub fn apply_headphone_to_plugins(&mut self) -> Result<String, String> {
        let filters: Vec<_> = self
            .headphone_eq
            .model
            .filters
            .iter()
            .map(|f| (f.filter_type.clone(), f.freq, f.q, f.db_gain))
            .collect();
        let label = std::path::Path::new(&self.headphone_eq.model.measurement_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "headphone".to_string());
        self.apply_eq_filters_to_chain(&filters, &label)
    }

    /// Apply the captured Room EQ optimization output to the live plugin
    /// chain. Auto-detects whether the result fits the linear rack or
    /// requires graph routing (multi-driver crossovers, bass management)
    /// and dispatches to the matching `sotf-player::autoeq::apply` entry
    /// — the same algorithm the GPUI "Apply to Rack"/"Apply as Graph"
    /// buttons use.
    ///
    /// On success, schedules a structural plugin update; the main loop's
    /// flush picks `update_plugins` (linear) or `update_plugin_graph`
    /// (non-linear) based on the resulting topology.
    pub fn apply_room_eq_to_chain(&mut self) -> Result<String, String> {
        use sotf_audio_player::autoeq::{self, RoomEqApplyOutcome};

        let Some(dsp_output) = self.room_eq.model.dsp_output.clone() else {
            return Err("No optimization results to apply. Run the optimizer first.".to_string());
        };

        let channel_names: Vec<String> = self
            .room_eq
            .model
            .channel_results
            .iter()
            .map(|r| r.channel_name.clone())
            .collect();

        let sample_rate = self
            .playback.current_sample_rate
            .map(|r| r as f64)
            .unwrap_or_else(|| self.get_current_sample_rate());

        let outcome = autoeq::apply_room_eq_to_chain(
            &mut self.plugin_rack.graph,
            &dsp_output,
            sample_rate,
            &channel_names,
        )?;

        self.plugin_rack.graph.update_channel_dependent_plugins();
        self.request_plugin_update();

        match outcome {
            RoomEqApplyOutcome::Rack(o) => {
                if o.total_filters == 0 && o.total_broadband == 0 {
                    return Err("No EQ filters found in optimization results".to_string());
                }
                Ok(format!(
                    "Applied Room EQ to rack: {} channels, {} main filters, {} broadband",
                    o.num_channels, o.total_filters, o.total_broadband
                ))
            }
            RoomEqApplyOutcome::Graph(o) => Ok(format!(
                "Applied Room EQ as graph: {} nodes, {} edges",
                o.num_nodes, o.num_edges
            )),
        }
    }

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: PathBuf) {
        self.playback.current_track_path = Some(track_path);
        self.playback.current_track_start_time = Some(std::time::Instant::now());
        self.playback.current_track_already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.read_only || self.playback.current_track_already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.playback.current_track_path, self.playback.current_track_start_time)
        {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= 30 {
                // Record the play in the database
                if let Some(db) = self.library.get_database() {
                    let duration = self.playback.position_secs as u64;
                    if let Err(e) = db.record_play(path, duration) {
                        log::error!("Failed to record play: {}", e);
                    } else {
                        log::info!("Recorded play for {:?} ({}s)", path, duration);
                        self.playback.current_track_already_recorded = true;
                    }
                }
            }
        }
    }

    /// Stop tracking the current track (called when track changes or stops)
    pub fn stop_track_tracking(&mut self) {
        self.playback.current_track_path = None;
        self.playback.current_track_start_time = None;
        self.playback.current_track_already_recorded = false;
    }

    // ========================================================================
    // Favorites Methods
    // ========================================================================

    /// Toggle favorite on the currently selected album in library view
    pub fn toggle_selected_album_favorite(&mut self) {
        // Copy the index first to avoid borrow conflicts with filtered_albums()
        let idx = self.library_view.selected_album_index;
        let album_id = self.library_view.cached_filtered_albums.get(idx).and_then(|a| a.id);
        if let Some(album_id) = album_id
            && let Some(db) = self.library.get_database()
        {
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

    /// Toggle favorite on the current queue album
    pub fn toggle_current_queue_album_favorite(&mut self) {
        let album_id = self
            .playback.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.album.id);
        if let Some(album_id) = album_id
            && let Some(db) = self.library.get_database()
        {
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

    /// Toggle the favorites-only filter in the library view
    pub fn toggle_favorites_filter(&mut self) {
        self.library_view.show_favorites_only = !self.library_view.show_favorites_only;
        self.request_filter_update();
        self.library_view.selected_album_index = 0;
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
        self.file_explorer.picker_origin = origin;
        self.file_explorer.picker_mode = mode;
        self.file_explorer.picker_title = title.to_string();
        self.file_explorer.filter = extension_filter.map(|s| s.to_lowercase());
        self.file_explorer.show_hidden = false;

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

        self.file_explorer.dir = dir;
        self.refresh_file_explorer();
        self.input_mode = InputMode::FileExplorer;
    }

    /// Close the file explorer and restore the appropriate input mode.
    pub fn close_file_explorer(&mut self) {
        self.input_mode =
            match self.file_explorer.picker_origin {
                FilePickerOrigin::SofaFile
                | FilePickerOrigin::IrFile
                | FilePickerOrigin::ApoFile
                | FilePickerOrigin::ABConfigA
                | FilePickerOrigin::ABConfigB => InputMode::EditPlugin,
                FilePickerOrigin::AddDirectory => InputMode::ConfigureDirectories,
                FilePickerOrigin::RecordingOutputDir
                | FilePickerOrigin::RecordingMicCalibration => InputMode::ConfigureRecording,
                FilePickerOrigin::RoomEqFilePath | FilePickerOrigin::RoomEqExportPath => {
                    InputMode::ConfigureRoomEq
                }
                FilePickerOrigin::HeadphoneMeasurement
                | FilePickerOrigin::HeadphoneCustomTarget => InputMode::ConfigureHeadphoneEq,
                FilePickerOrigin::PlaylistImport | FilePickerOrigin::PlaylistExport => {
                    InputMode::Normal
                }
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
        self.file_explorer.items.clear();
        self.file_explorer.selected = 0;

        if let Ok(entries) = std::fs::read_dir(&self.file_explorer.dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Skip hidden files unless toggled on
                if !self.file_explorer.show_hidden && name.starts_with('.') {
                    continue;
                }

                if path.is_dir() {
                    dirs.push(path);
                } else if path.is_file() {
                    if let Some(ext) = &self.file_explorer.filter {
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

            self.file_explorer.items.extend(dirs);
            self.file_explorer.items.extend(files);
        }
    }

    pub fn file_explorer_enter_dir(&mut self, path: PathBuf) {
        self.file_explorer.dir = path;
        self.refresh_file_explorer();
    }

    pub fn file_explorer_go_parent(&mut self) {
        if let Some(parent) = self.file_explorer.dir.parent() {
            let parent = parent.to_path_buf();
            self.file_explorer.dir = parent;
            self.refresh_file_explorer();
        }
    }

    pub fn file_explorer_select_next(&mut self) {
        if !self.file_explorer.items.is_empty() {
            self.file_explorer.selected =
                (self.file_explorer.selected + 1) % self.file_explorer.items.len();
        }
    }

    pub fn file_explorer_select_prev(&mut self) {
        if !self.file_explorer.items.is_empty() {
            if self.file_explorer.selected == 0 {
                self.file_explorer.selected = self.file_explorer.items.len() - 1;
            } else {
                self.file_explorer.selected -= 1;
            }
        }
    }

    pub fn file_explorer_toggle_hidden(&mut self) {
        self.file_explorer.show_hidden = !self.file_explorer.show_hidden;
        self.refresh_file_explorer();
    }

    /// Returns the currently selected path, if any.
    pub fn file_explorer_current(&self) -> Option<&PathBuf> {
        self.file_explorer.items.get(self.file_explorer.selected)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(Theme::default(), false)
    }
}
