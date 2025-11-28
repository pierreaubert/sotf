use crate::theme::Theme;
use sotf_audio::LoudnessData;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{Album, MusicLibrary, PluginChain, PluginType, Track};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Plugins,
    Devices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    ShowHelp,
    ShowError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Main,   // Main content area (library, queue, etc.)
    Meters, // Right column with level meters
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
}

/// Library sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySortOrder {
    Artist,
    Album,
    Title,
    Year,
    Popularity,
}

/// Channel filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelFilter {
    All,           // Show all albums
    Mono,          // Only 1-channel albums
    Stereo,        // Only 2-channel albums
    Multichannel,  // Only albums with > 2 channels
    Mixed,         // Only albums with mixed channel counts
    Specific(u32), // Only albums with specific channel count
}

/// Artist node in tree view
#[derive(Debug, Clone)]
pub struct ArtistNode {
    pub artist: String,
    pub album_indices: Vec<usize>, // Indices into library.albums
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Artist { name: String, expanded: bool },
    Album { index: usize },
}

/// Channel group for level meter display
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
}

/// Individual channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub index: usize,              // Index in loudness.channel_peaks
    pub name: String,              // e.g., "FL", "FR", "C"
    pub display_name: Vec<String>, // Multi-line display: ["F", "L"] or ["T", "B", "R"]
}

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueItem>,
    pub expanded_queue_items: Vec<bool>, // Track which queue items are expanded
    pub current_screen: Screen,
    pub input_mode: InputMode,
    pub focused_pane: FocusedPane, // Which pane (Main or Meters) has focus

    // Theme
    pub theme: Theme,

    // UI state
    pub search_query: String,
    pub directory_input: String,
    pub plugin_file_input: String, // For save/load plugin chain
    pub apo_file_input: String,    // For loading APO EQ files
    pub sofa_file_input: String,   // For loading SOFA HRTF files
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub selected_plugin_index: usize,
    pub album_list_offset: usize,
    pub status_message: Option<String>, // For displaying save/load status
    pub error_message: Option<String>,  // For displaying decode/playback errors in a modal

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

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub position_secs: f64,

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
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,

    // Maintenance progress
    pub maintenance_in_progress: bool,
    pub maintenance_progress_checked: usize,
    pub maintenance_progress_total: usize,

    // ReplayGain scanner progress
    pub replay_gain_scanner: Option<Arc<sotf_audio_player::ReplayGainScanner>>,
    pub replay_gain_in_progress: bool,
    pub replay_gain_total: usize,
    pub replay_gain_processed: usize,
    pub replay_gain_succeeded: usize,
    pub replay_gain_failed: usize,

    // Waveform scanner progress
    pub waveform_scanner: Option<Arc<sotf_audio_player::WaveformScanner>>,
    pub waveform_in_progress: bool,
    pub waveform_total: usize,
    pub waveform_processed: usize,
    pub waveform_succeeded: usize,
    pub waveform_failed: usize,

    // Last loaded plugin preset name (for config persistence)
    pub last_loaded_preset: Option<String>,

    // Album cover image display
    pub album_images: Vec<PathBuf>, // List of image files in current album directory
    pub selected_image_index: usize, // Current image being displayed
    pub image_picker: Option<ratatui_image::picker::Picker>, // Image protocol picker
}

impl App {
    pub fn new(theme: Theme) -> Self {
        // Try to create library with database, fallback to simple library
        let library = MusicLibrary::with_database().unwrap_or_else(|e| {
            log::warn!(
                "Failed to initialize database, using in-memory library: {}",
                e
            );
            MusicLibrary::new()
        });

        Self {
            library,
            queue: Vec::new(),
            expanded_queue_items: Vec::new(),
            current_screen: Screen::Library,
            input_mode: InputMode::Normal,
            focused_pane: FocusedPane::Main,
            theme,
            search_query: String::new(),
            directory_input: String::new(),
            plugin_file_input: String::new(),
            apo_file_input: String::new(),
            sofa_file_input: String::new(),
            selected_album_index: 0,
            selected_directory_index: 0,
            selected_queue_index: 0,
            selected_plugin_index: 0,
            album_list_offset: 0,
            status_message: None,
            error_message: None,
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            library_view_mode: LibraryViewMode::Flat,
            library_sort_order: LibrarySortOrder::Artist,
            channel_filter: ChannelFilter::All,
            artist_tree: Vec::new(),
            selected_tree_index: 0,
            plugin_chain: {
                let mut chain = PluginChain::new();
                // Add default analyzer plugins for LUFS and level meters
                chain.add_plugin(&PluginType::LoudnessMonitor);
                // Add ChannelMuteSolo plugin as last plugin (disabled by default)
                chain.add_plugin(&PluginType::ChannelMuteSolo);
                chain
            },
            needs_plugin_update: false,
            pending_param_update: None,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            plugin_update_last_attempt: None,
            plugin_update_retry_count: 0,
            plugin_update_in_progress: false,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            current_track_path: None,
            current_track_start_time: None,
            current_track_already_recorded: false,
            loudness_info: None,
            level_meter_groups: Vec::new(),
            selected_level_meter_group: 0,
            level_meter_control_selection: 0,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            should_quit: false,
            needs_rescan: false,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            library_scanner: None,
            maintenance_in_progress: false,
            maintenance_progress_checked: 0,
            maintenance_progress_total: 0,
            replay_gain_scanner: None,
            replay_gain_in_progress: false,
            replay_gain_total: 0,
            replay_gain_processed: 0,
            replay_gain_succeeded: 0,
            replay_gain_failed: 0,
            waveform_scanner: None,
            waveform_in_progress: false,
            waveform_total: 0,
            waveform_processed: 0,
            waveform_succeeded: 0,
            waveform_failed: 0,
            last_loaded_preset: None,
            album_images: Vec::new(),
            selected_image_index: 0,
            image_picker: None,
        }
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.rebuild_artist_tree();
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

    pub fn select_next_output_device(&mut self) {
        if !self.output_devices.is_empty() {
            self.selected_output_device_index =
                (self.selected_output_device_index + 1) % self.output_devices.len();
        }
    }

    pub fn select_previous_output_device(&mut self) {
        if !self.output_devices.is_empty() {
            if self.selected_output_device_index == 0 {
                self.selected_output_device_index = self.output_devices.len() - 1;
            } else {
                self.selected_output_device_index -= 1;
            }
        }
    }

    pub fn get_selected_output_device(&self) -> Option<&AudioDevice> {
        self.output_devices.get(self.selected_output_device_index)
    }

    /// Get the maximum output channels supported by the selected device
    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.get_selected_output_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    pub fn filtered_albums(&self) -> Vec<&Album> {
        use sotf_audio_player::AlbumChannelType;

        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Apply channel filter
        albums.retain(|album| match self.channel_filter {
            ChannelFilter::All => true,
            ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
            ChannelFilter::Stereo => {
                matches!(album.channel_type(), Some(AlbumChannelType::Stereo))
            }
            ChannelFilter::Multichannel => {
                matches!(
                    album.channel_type(),
                    Some(AlbumChannelType::Multichannel(_))
                )
            }
            ChannelFilter::Mixed => {
                matches!(album.channel_type(), Some(AlbumChannelType::Mixed))
            }
            ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
        });

        // Sort albums based on current sort order
        match self.library_sort_order {
            LibrarySortOrder::Artist => {
                albums.sort_by(|a, b| {
                    a.artist
                        .cmp(&b.artist)
                        .then(a.title.cmp(&b.title))
                        .then(a.year.cmp(&b.year))
                });
            }
            LibrarySortOrder::Album => {
                albums.sort_by(|a, b| {
                    a.title
                        .cmp(&b.title)
                        .then(a.artist.cmp(&b.artist))
                        .then(a.year.cmp(&b.year))
                });
            }
            LibrarySortOrder::Title => {
                // Same as Album - sort by album title
                albums.sort_by(|a, b| {
                    a.title
                        .cmp(&b.title)
                        .then(a.artist.cmp(&b.artist))
                        .then(a.year.cmp(&b.year))
                });
            }
            LibrarySortOrder::Year => {
                albums.sort_by(|a, b| {
                    // Sort by year descending (newest first), then artist, then title
                    b.year
                        .cmp(&a.year)
                        .then(a.artist.cmp(&b.artist))
                        .then(a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Popularity => {
                albums.sort_by(|a, b| {
                    // Sort by play count descending (most played first), then artist, then title
                    b.play_count
                        .cmp(&a.play_count)
                        .then(a.artist.cmp(&b.artist))
                        .then(a.title.cmp(&b.title))
                });
            }
        }

        albums
    }

    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;
        let albums = self.filtered_albums();

        if let Some(album) = albums.get(self.selected_album_index) {
            self.queue.push(QueueItem::new((*album).clone()));
            self.expanded_queue_items.push(false);

            // Auto-play if queue was empty OR if nothing was playing
            if was_empty || was_not_playing {
                return self.start_queue();
            }
        }
        None
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);

            // Safely remove from expanded_queue_items, handling potential sync issues
            if index < self.expanded_queue_items.len() {
                self.expanded_queue_items.remove(index);
            } else {
                // If vectors are out of sync, resync them
                log::warn!(
                    "Queue sync issue detected: queue.len()={}, expanded.len()={}",
                    self.queue.len(),
                    self.expanded_queue_items.len()
                );
                // Resize expanded_queue_items to match queue
                self.expanded_queue_items.resize(self.queue.len(), false);
            }

            // Adjust current queue index if needed
            if let Some(current_idx) = self.current_queue_index {
                if current_idx == index {
                    // We deleted the currently playing album
                    if self.queue.is_empty() {
                        // Queue is now empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    } else if index < self.queue.len() {
                        // There are albums after the deleted one, stay at same index
                        // (items have shifted down, so index now points to the next album)
                        self.current_queue_index = Some(index);
                        // Reset to first track of the new album at this position
                        if let Some(item) = self.queue.get_mut(index) {
                            item.current_track_index = 0;
                        }
                    } else if index > 0 {
                        // Deleted last album, move to previous album
                        self.current_queue_index = Some(index - 1);
                        // Stay on whatever track was playing in that album
                    } else {
                        // Queue is empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    }
                } else if current_idx > index {
                    // Deleted an album before the current one, adjust index
                    self.current_queue_index = Some(current_idx - 1);
                }
            }
            if self.selected_queue_index >= self.queue.len() && self.selected_queue_index > 0 {
                self.selected_queue_index = self.queue.len() - 1;
            }
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.expanded_queue_items.clear();
        self.current_queue_index = None;
        self.selected_queue_index = 0;
        self.is_playing = false;
    }

    pub fn toggle_queue_item_expansion(&mut self) {
        if self.selected_queue_index < self.expanded_queue_items.len() {
            self.expanded_queue_items[self.selected_queue_index] =
                !self.expanded_queue_items[self.selected_queue_index];
        }
    }

    pub fn select_next_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            self.selected_album_index = (self.selected_album_index + 1) % albums.len();
        }
    }

    pub fn select_previous_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            if self.selected_album_index == 0 {
                self.selected_album_index = albums.len() - 1;
            } else {
                self.selected_album_index -= 1;
            }
        }
    }

    pub fn page_down_albums(&mut self, page_size: usize) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            self.selected_album_index =
                (self.selected_album_index + page_size).min(albums.len() - 1);
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            self.selected_album_index = self.selected_album_index.saturating_sub(page_size);
        }
    }

    pub fn page_down_tree(&mut self, page_size: usize) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index =
                (self.selected_tree_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_tree(&mut self, page_size: usize) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index = self.selected_tree_index.saturating_sub(page_size);
        }
    }

    pub fn select_next_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index = (self.selected_directory_index + 1) % tree_items.len();
        }
    }

    pub fn select_previous_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            if self.selected_directory_index == 0 {
                self.selected_directory_index = tree_items.len() - 1;
            } else {
                self.selected_directory_index -= 1;
            }
        }
    }

    pub fn page_down_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index =
                (self.selected_directory_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index = self.selected_directory_index.saturating_sub(page_size);
        }
    }

    pub fn select_next_queue_item(&mut self) {
        if !self.queue.is_empty() {
            self.selected_queue_index = (self.selected_queue_index + 1) % self.queue.len();
        }
    }

    pub fn select_previous_queue_item(&mut self) {
        if !self.queue.is_empty() {
            if self.selected_queue_index == 0 {
                self.selected_queue_index = self.queue.len() - 1;
            } else {
                self.selected_queue_index -= 1;
            }
        }
    }

    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.needs_rescan = true;
                    self.status_message = Some("Directory added. Press 's' to scan.".to_string());
                } else {
                    self.status_message = Some("Directory already exists.".to_string());
                }
            }
            Err(msg) => {
                self.status_message = Some(msg);
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library.add_directory(path);
    }

    pub fn remove_selected_directory(&mut self) {
        if self
            .library
            .remove_directory(self.selected_directory_index)
            .is_some()
        {
            if self.selected_directory_index >= self.library.directories.len()
                && self.selected_directory_index > 0
            {
                self.selected_directory_index = self.library.directories.len() - 1;
            }
            self.needs_rescan = true;
        }
    }

    pub fn toggle_directory_expansion(&mut self) {
        // Find which directory in the tree we're selecting
        let tree_items = self.get_directory_tree_items();
        if let Some((path, _, _)) = tree_items.get(self.selected_directory_index) {
            // Helper to find and toggle directory recursively
            fn toggle_recursive(
                directories: &mut [sotf_audio_player::DirectoryInfo],
                target_path: &std::path::Path,
            ) -> bool {
                for dir in directories {
                    if dir.path == target_path {
                        dir.expanded = !dir.expanded;
                        return true;
                    }
                    if dir.expanded {
                        if toggle_recursive(&mut dir.subdirectories, target_path) {
                            return true;
                        }
                    }
                }
                false
            }

            toggle_recursive(&mut self.library.directories, path);
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        let mut items = Vec::new();

        fn add_recursive(
            items: &mut Vec<(PathBuf, usize, bool)>,
            dir_info: &sotf_audio_player::DirectoryInfo,
            level: usize,
        ) {
            items.push((dir_info.path.clone(), level, dir_info.expanded));

            if dir_info.expanded {
                for subdir in &dir_info.subdirectories {
                    add_recursive(items, subdir, level + 1);
                }
            }
        }

        for dir_info in &self.library.directories {
            add_recursive(&mut items, dir_info, 0);
        }
        items
    }

    /// Start library scan (non-blocking background scan)
    pub fn start_library_scan(&mut self) {
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner
        let scanner = sotf_audio_player::LibraryScanner::start(directories);
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Starting library scan...".to_string());
        log::info!("Started background library scan");
    }

    /// Start force library scan (non-blocking background scan, rescans ALL files)
    ///
    /// Unlike `start_library_scan()`, this rescans all files regardless of modification time.
    /// ReplayGain values are preserved (not overwritten).
    pub fn start_force_library_scan(&mut self) {
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner with force=true
        let scanner = sotf_audio_player::LibraryScanner::start_force(directories);
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Starting FORCE library scan (all files)...".to_string());
        log::info!("Started FORCE background library scan");
    }

    /// Check progress of background library scan
    pub fn check_library_scan_progress(&mut self) {
        if !self.scan_in_progress {
            return;
        }

        // Collect messages first to avoid borrow issues
        let messages: Vec<_> = {
            let scanner = match &self.library_scanner {
                Some(s) => s,
                None => return,
            };
            let mut msgs = Vec::new();
            while let Some(msg) = scanner.try_recv() {
                msgs.push(msg);
            }
            msgs
        };

        // Process collected messages
        for msg in messages {
            use sotf_audio_player::LibraryScanMessage;

            match msg {
                LibraryScanMessage::Progress { tracks, albums } => {
                    self.scan_progress_tracks = tracks;
                    self.scan_progress_albums = albums;
                    self.status_message = Some(format!(
                        "Scanning: {} tracks, {} albums found...",
                        tracks, albums
                    ));
                }
                LibraryScanMessage::Complete { tracks, albums } => {
                    self.scan_in_progress = false;
                    self.library_scanner = None;
                    self.needs_rescan = false;
                    self.status_message = Some(format!(
                        "Scan complete: {} tracks in {} albums",
                        tracks, albums
                    ));
                    log::info!("Library scan complete: {} tracks in {} albums", tracks, albums);

                    // Reload library from database to get the new data
                    if let Err(e) = self.library.load_from_database() {
                        log::error!("Failed to reload library after scan: {}", e);
                    }
                    self.rebuild_artist_tree();

                    // Start background waveform scan for new tracks
                    if let Err(e) = self.start_waveform_scan() {
                        log::warn!("Failed to start waveform scan: {}", e);
                    }
                }
                LibraryScanMessage::Error { message } => {
                    self.scan_in_progress = false;
                    self.library_scanner = None;
                    self.status_message = Some(format!("Scan failed: {}", message));
                    log::error!("Library scan failed: {}", message);
                }
            }
        }
    }

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Scanning library...".to_string());

        // Create shared progress state
        let progress_tracks = Arc::new(Mutex::new(0usize));
        let progress_albums = Arc::new(Mutex::new(0usize));
        let last_update_tracks = Arc::new(Mutex::new(0usize));

        let progress_tracks_clone = Arc::clone(&progress_tracks);
        let progress_albums_clone = Arc::clone(&progress_albums);
        let last_update_clone = Arc::clone(&last_update_tracks);

        // Use progress callback to update shared progress
        let result = self.library.scan_with_progress(move |tracks, albums| {
            let should_update = if let Ok(last) = last_update_clone.lock() {
                tracks - *last >= 1000 || tracks == 0
            } else {
                false
            };

            if should_update {
                if let Ok(mut pt) = progress_tracks_clone.lock() {
                    *pt = tracks;
                }
                if let Ok(mut pa) = progress_albums_clone.lock() {
                    *pa = albums;
                }
                if let Ok(mut last) = last_update_clone.lock() {
                    *last = tracks;
                }
                log::info!("Scan progress: {} tracks, {} albums found", tracks, albums);
            }
        });

        // Update app state with final progress
        if let Ok(pt) = progress_tracks.lock() {
            self.scan_progress_tracks = *pt;
        }
        if let Ok(pa) = progress_albums.lock() {
            self.scan_progress_albums = *pa;
        }

        self.scan_in_progress = false;
        self.needs_rescan = false;
        self.selected_album_index = 0;
        self.album_list_offset = 0;

        match &result {
            Ok(_) => {
                let album_count = self.library.albums.len();
                let track_count: usize = self.library.albums.iter().map(|a| a.tracks.len()).sum();
                self.status_message = Some(format!(
                    "Scan complete: {} tracks in {} albums",
                    track_count, album_count
                ));
                log::info!(
                    "Scan complete: {} tracks in {} albums",
                    track_count,
                    album_count
                );
            }
            Err(e) => {
                self.status_message = Some(format!("Scan failed: {}", e));
                log::error!("Scan failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        // Start background waveform scan for new tracks
        if result.is_ok() {
            if let Err(e) = self.start_waveform_scan() {
                log::warn!("Failed to start waveform scan: {}", e);
            }
        }

        result
    }

    pub fn clean_library_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        self.maintenance_in_progress = true;
        self.maintenance_progress_checked = 0;
        self.maintenance_progress_total = 0;
        self.status_message = Some("Starting database maintenance...".to_string());

        // Create shared progress state
        let progress_checked = Arc::new(Mutex::new(0usize));
        let progress_total = Arc::new(Mutex::new(0usize));

        let progress_checked_clone = Arc::clone(&progress_checked);
        let progress_total_clone = Arc::clone(&progress_total);

        // Use progress callback to update shared progress
        let result = self
            .library
            .clean_database_with_progress(move |checked, total| {
                if let Ok(mut pc) = progress_checked_clone.lock() {
                    *pc = checked;
                }
                if let Ok(mut pt) = progress_total_clone.lock() {
                    *pt = total;
                }
            });

        // Update app state with final progress
        if let Ok(pc) = progress_checked.lock() {
            self.maintenance_progress_checked = *pc;
        }
        if let Ok(pt) = progress_total.lock() {
            self.maintenance_progress_total = *pt;
        }

        self.maintenance_in_progress = false;

        match &result {
            Ok(removed) => {
                if *removed > 0 {
                    self.status_message =
                        Some(format!("Cleaned {} missing tracks from database", removed));
                    log::info!("Database maintenance: removed {} missing tracks", removed);
                } else {
                    self.status_message =
                        Some("Database is clean - no missing tracks found".to_string());
                    log::info!("Database maintenance: no missing tracks found");
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Database maintenance failed: {}", e));
                log::error!("Database maintenance failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        result
    }

    /// Start ReplayGain scanner for tracks without ReplayGain values
    pub fn start_replay_gain_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.replay_gain_in_progress {
            return Ok(()); // Already scanning
        }

        // Get database path
        let db_path = sotf_audio_player::MusicDatabase::default_path()
            .ok_or("Could not determine database path")?;

        // Get tracks that need ReplayGain analysis
        let db = sotf_audio_player::MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_replay_gain()?;

        if tracks.is_empty() {
            self.status_message = Some("All tracks already have ReplayGain values".to_string());
            return Ok(());
        }

        let total = tracks.len();
        log::info!("Starting ReplayGain scan for {} tracks", total);

        // Determine number of threads (use CPU count or max 4)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2);

        // Create scanner
        let scanner = Arc::new(sotf_audio_player::ReplayGainScanner::new(
            num_threads,
            db_path,
        ));

        // Queue all tracks
        scanner.scan_tracks(tracks);

        // Store scanner and initialize progress
        self.replay_gain_scanner = Some(scanner);
        self.replay_gain_in_progress = true;
        self.replay_gain_total = total;
        self.replay_gain_processed = 0;
        self.replay_gain_succeeded = 0;
        self.replay_gain_failed = 0;
        self.status_message = Some(format!("Analyzing {} tracks for ReplayGain...", total));

        Ok(())
    }

    /// Check for ReplayGain scanner progress updates
    pub fn check_replay_gain_progress(&mut self) {
        if !self.replay_gain_in_progress {
            return;
        }

        // Clone the Arc to avoid borrowing self
        let scanner = match &self.replay_gain_scanner {
            Some(s) => Arc::clone(s),
            None => return,
        };

        // Process all pending messages
        while let Some(msg) = scanner.try_recv() {
            use sotf_audio_player::ScanMessage;

            match msg {
                ScanMessage::Started { .. } => {
                    // Track started, no action needed
                }
                ScanMessage::Success { .. } => {
                    self.replay_gain_processed += 1;
                    self.replay_gain_succeeded += 1;
                }
                ScanMessage::Error { path, error } => {
                    self.replay_gain_processed += 1;
                    self.replay_gain_failed += 1;
                    log::error!("ReplayGain scan failed for {}: {}", path.display(), error);
                }
                ScanMessage::Complete {
                    total,
                    succeeded,
                    failed,
                } => {
                    self.replay_gain_in_progress = false;
                    self.replay_gain_scanner = None;
                    self.status_message = Some(format!(
                        "ReplayGain scan complete: {}/{} succeeded, {} failed",
                        succeeded, total, failed
                    ));
                    log::info!(
                        "ReplayGain scan complete: {}/{} succeeded, {} failed",
                        succeeded,
                        total,
                        failed
                    );
                }
            }
        }

        // Check if all tracks have been processed
        if self.replay_gain_in_progress && self.replay_gain_processed >= self.replay_gain_total {
            self.replay_gain_in_progress = false;
            self.replay_gain_scanner = None;
            self.status_message = Some(format!(
                "ReplayGain scan complete: {}/{} succeeded, {} failed",
                self.replay_gain_succeeded, self.replay_gain_total, self.replay_gain_failed
            ));
            log::info!(
                "ReplayGain scan complete: {}/{} succeeded, {} failed",
                self.replay_gain_succeeded,
                self.replay_gain_total,
                self.replay_gain_failed
            );
        }
    }

    /// Start background waveform scanning for tracks without waveform data
    pub fn start_waveform_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Skip if already in progress
        if self.waveform_in_progress {
            return Ok(());
        }

        // Get database path
        let db_path = sotf_audio_player::MusicDatabase::default_path()
            .ok_or("Could not determine database path")?;

        // Get tracks that need waveform analysis
        let db = sotf_audio_player::MusicDatabase::open(&db_path)?;
        let tracks = db.get_tracks_without_waveform()?;

        if tracks.is_empty() {
            log::debug!("All tracks already have waveform data");
            return Ok(());
        }

        let total = tracks.len();
        log::info!("Starting waveform scan for {} tracks", total);

        // Determine number of threads (use CPU count or max 4)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2);

        // Create scanner
        let scanner = Arc::new(sotf_audio_player::WaveformScanner::new(
            num_threads,
            db_path,
        ));

        // Queue all tracks
        scanner.scan_tracks(tracks);

        // Store scanner and initialize progress
        self.waveform_scanner = Some(scanner);
        self.waveform_in_progress = true;
        self.waveform_total = total;
        self.waveform_processed = 0;
        self.waveform_succeeded = 0;
        self.waveform_failed = 0;

        Ok(())
    }

    /// Check for waveform scanner progress updates
    pub fn check_waveform_progress(&mut self) {
        if !self.waveform_in_progress {
            return;
        }

        // Clone the Arc to avoid borrowing self
        let scanner = match &self.waveform_scanner {
            Some(s) => Arc::clone(s),
            None => return,
        };

        // Process all pending messages
        while let Some(msg) = scanner.try_recv() {
            use sotf_audio_player::WaveformScanMessage;

            match msg {
                WaveformScanMessage::Started { .. } => {
                    // Track started, no action needed
                }
                WaveformScanMessage::Success { .. } => {
                    self.waveform_processed += 1;
                    self.waveform_succeeded += 1;
                }
                WaveformScanMessage::Error { path, error } => {
                    self.waveform_processed += 1;
                    self.waveform_failed += 1;
                    log::error!("Waveform scan failed for {}: {}", path.display(), error);
                }
                WaveformScanMessage::Complete {
                    total,
                    succeeded,
                    failed,
                } => {
                    self.waveform_in_progress = false;
                    self.waveform_scanner = None;
                    log::info!(
                        "Waveform scan complete: {}/{} succeeded, {} failed",
                        succeeded,
                        total,
                        failed
                    );
                }
            }
        }

        // Check if all tracks have been processed
        if self.waveform_in_progress && self.waveform_processed >= self.waveform_total {
            self.waveform_in_progress = false;
            self.waveform_scanner = None;
            log::info!(
                "Waveform scan complete: {}/{} succeeded, {} failed",
                self.waveform_succeeded,
                self.waveform_total,
                self.waveform_failed
            );
        }
    }

    /// Save current app state to config file
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::AppConfig {
            version: 1,
            output_device: self.current_output_device_name.clone(),
            queue: self
                .queue
                .iter()
                .map(|item| (item.album.artist.clone(), item.album.title.clone()))
                .collect(),
            queue_index: self.current_queue_index,
            track_index: self
                .current_queue_index
                .and_then(|idx| self.queue.get(idx))
                .map(|item| item.current_track_index)
                .unwrap_or(0),
            plugin_preset: self.last_loaded_preset.clone(),
        };

        sotf_audio_player::config::save_app_config(&config)?;
        log::info!("Saved app configuration");
        Ok(())
    }

    /// Load app state from config file and restore it
    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::load_app_config()?;

        // Restore output device
        if let Some(device_name) = &config.output_device {
            self.current_output_device_name = Some(device_name.clone());
            // Find the device index
            if let Some(idx) = self
                .output_devices
                .iter()
                .position(|d| d.name == *device_name)
            {
                self.selected_output_device_index = idx;
            }
        }

        // Restore queue - need to find albums by artist/title
        for (artist, title) in config.queue {
            if let Some(album) = self
                .library
                .albums
                .iter()
                .find(|a| a.artist == artist && a.title == title)
                .cloned()
            {
                self.queue.push(QueueItem::new(album));
                self.expanded_queue_items.push(false);
            }
        }

        // Restore queue position
        if let Some(queue_idx) = config.queue_index
            && queue_idx < self.queue.len()
        {
            self.current_queue_index = Some(queue_idx);
            // Restore track position within album
            if let Some(item) = self.queue.get_mut(queue_idx)
                && config.track_index < item.album.tracks.len()
            {
                item.current_track_index = config.track_index;
            }
        }

        // Restore plugin preset
        if let Some(preset_name) = &config.plugin_preset {
            // Use the plugin chain's own load method (handles path construction and validation)
            match self.plugin_chain.load_from_file(preset_name) {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_chain.update_binaural_decoder_channels();

                    self.last_loaded_preset = Some(preset_name.clone());
                    self.request_plugin_update();
                    log::info!("Restored plugin preset: {}", preset_name);
                }
                Err(e) => {
                    log::warn!("Could not restore preset '{}': {}", preset_name, e);
                }
            }
        }

        log::info!(
            "Loaded app configuration: {} items in queue, device: {:?}, preset: {:?}",
            self.queue.len(),
            self.current_output_device_name,
            self.last_loaded_preset
        );
        Ok(())
    }

    /// Build the artist tree from the current album list
    pub fn rebuild_artist_tree(&mut self) {
        use std::collections::HashMap;

        let mut artist_map: HashMap<String, Vec<usize>> = HashMap::new();

        // Group albums by artist
        for (idx, album) in self.library.albums.iter().enumerate() {
            artist_map
                .entry(album.artist.clone())
                .or_default()
                .push(idx);
        }

        // Create artist nodes
        let mut artists: Vec<_> = artist_map.into_iter().collect();
        artists.sort_by(|a, b| a.0.cmp(&b.0));

        self.artist_tree = artists
            .into_iter()
            .map(|(artist, album_indices)| ArtistNode {
                artist,
                album_indices,
                expanded: false,
            })
            .collect();

        self.selected_tree_index = 0;
    }

    /// Toggle tree view mode
    pub fn toggle_library_view_mode(&mut self) {
        self.library_view_mode = match self.library_view_mode {
            LibraryViewMode::Flat => LibraryViewMode::TreeView,
            LibraryViewMode::TreeView => LibraryViewMode::Flat,
        };
        self.selected_tree_index = 0;
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_sort_order = order;
        // Reset selection to top when changing sort order
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Rebuild tree view if active (as sort order affects tree structure)
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.channel_filter = filter;
        // Reset selection to top when changing filter
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Rebuild tree view if active
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Get unique channel counts present in the library
    pub fn get_unique_channel_counts(&self) -> Vec<u32> {
        use std::collections::HashSet;

        let mut channel_counts = HashSet::new();

        for album in &self.library.albums {
            if let Some(count) = album.uniform_channel_count() {
                channel_counts.insert(count);
            }
        }

        let mut counts: Vec<u32> = channel_counts.into_iter().collect();
        counts.sort();
        counts
    }

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        // Get available channel counts in library (excluding mono and stereo since they have their own filters)
        let specific_counts: Vec<u32> = self
            .get_unique_channel_counts()
            .into_iter()
            .filter(|&count| count > 2)
            .collect();

        self.channel_filter = match self.channel_filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Multichannel,
            ChannelFilter::Multichannel => ChannelFilter::Mixed,
            ChannelFilter::Mixed => {
                // Cycle to first specific count if available, otherwise back to All
                if let Some(&first_count) = specific_counts.first() {
                    ChannelFilter::Specific(first_count)
                } else {
                    ChannelFilter::All
                }
            }
            ChannelFilter::Specific(current) => {
                // Find next specific count in the list
                if let Some(pos) = specific_counts.iter().position(|&c| c == current) {
                    if pos + 1 < specific_counts.len() {
                        ChannelFilter::Specific(specific_counts[pos + 1])
                    } else {
                        ChannelFilter::All
                    }
                } else {
                    ChannelFilter::All
                }
            }
        };
        // Reset selection
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Rebuild tree view if active
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Toggle expansion of the currently selected artist node
    pub fn toggle_artist_expansion(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        // Get the filtered tree items to find which artist we're on
        let tree_items = self.get_tree_items();
        if let Some(TreeItem::Artist { name, .. }) = tree_items.get(self.selected_tree_index) {
            // Find this artist in the tree and toggle expansion
            for artist_node in &mut self.artist_tree {
                if artist_node.artist == *name {
                    artist_node.expanded = !artist_node.expanded;
                    return;
                }
            }
        }
    }

    /// Get the set of album indices that pass the current search and channel filters
    fn filtered_album_indices(&self) -> std::collections::HashSet<usize> {
        use sotf_audio_player::AlbumChannelType;
        use std::collections::HashSet;

        let mut indices: HashSet<usize> = if self.search_query.is_empty() {
            (0..self.library.albums.len()).collect()
        } else {
            // Get filtered albums and find their indices in the library
            let filtered = self.library.search_albums(&self.search_query);
            self.library
                .albums
                .iter()
                .enumerate()
                .filter(|(_, album)| filtered.iter().any(|a| std::ptr::eq(*a, *album)))
                .map(|(idx, _)| idx)
                .collect()
        };

        // Apply channel filter
        indices.retain(|&idx| {
            if let Some(album) = self.library.albums.get(idx) {
                match self.channel_filter {
                    ChannelFilter::All => true,
                    ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
                    ChannelFilter::Stereo => {
                        matches!(album.channel_type(), Some(AlbumChannelType::Stereo))
                    }
                    ChannelFilter::Multichannel => {
                        matches!(
                            album.channel_type(),
                            Some(AlbumChannelType::Multichannel(_))
                        )
                    }
                    ChannelFilter::Mixed => {
                        matches!(album.channel_type(), Some(AlbumChannelType::Mixed))
                    }
                    ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
                }
            } else {
                false
            }
        });

        indices
    }

    /// Get the flattened tree items for rendering (returns artist names or album indices)
    /// Respects search query and channel filter
    pub fn get_tree_items(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();
        let filtered_indices = self.filtered_album_indices();

        for artist_node in &self.artist_tree {
            // Filter albums for this artist
            let visible_albums: Vec<usize> = artist_node
                .album_indices
                .iter()
                .copied()
                .filter(|idx| filtered_indices.contains(idx))
                .collect();

            // Skip artists with no visible albums
            if visible_albums.is_empty() {
                continue;
            }

            items.push(TreeItem::Artist {
                name: artist_node.artist.clone(),
                expanded: artist_node.expanded,
            });

            if artist_node.expanded {
                for album_idx in visible_albums {
                    items.push(TreeItem::Album { index: album_idx });
                }
            }
        }

        items
    }

    /// Select next item in tree view
    pub fn select_next_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index = (self.selected_tree_index + 1) % tree_items.len();
        }
    }

    /// Select previous item in tree view
    pub fn select_previous_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            if self.selected_tree_index == 0 {
                self.selected_tree_index = tree_items.len() - 1;
            } else {
                self.selected_tree_index -= 1;
            }
        }
    }

    /// Add the selected item (artist or album) to queue from tree view
    pub fn add_tree_selection_to_queue(&mut self) -> Option<PathBuf> {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return None;
        }

        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;
        let tree_items = self.get_tree_items();
        let filtered_indices = self.filtered_album_indices();

        if let Some(item) = tree_items.get(self.selected_tree_index) {
            match item {
                TreeItem::Artist { name, .. } => {
                    // Find this artist in the tree and add their filtered albums
                    for artist_node in &self.artist_tree {
                        if artist_node.artist == *name {
                            // Add only albums that pass the current filter
                            for &album_idx in &artist_node.album_indices {
                                if filtered_indices.contains(&album_idx) {
                                    if let Some(album) = self.library.albums.get(album_idx) {
                                        self.queue.push(QueueItem::new(album.clone()));
                                        self.expanded_queue_items.push(false);
                                    }
                                }
                            }
                            // Auto-play if queue was empty OR if nothing was playing
                            if was_empty || was_not_playing {
                                return self.start_queue();
                            }
                            return None;
                        }
                    }
                }
                TreeItem::Album { index } => {
                    // Add single album
                    if let Some(album) = self.library.albums.get(*index) {
                        self.queue.push(QueueItem::new(album.clone()));
                        self.expanded_queue_items.push(false);

                        // Auto-play if queue was empty OR if nothing was playing
                        if was_empty || was_not_playing {
                            return self.start_queue();
                        }
                    }
                }
            }
        }
        None
    }

    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|item| item.current_track())
            .map(|track| track.path.clone())
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index {
            if let Some(item) = self.queue.get_mut(idx) {
                if let Some(track) = item.next_track() {
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
            && let Some(item) = self.queue.get_mut(idx)
        {
            if let Some(track) = item.previous_track() {
                return Some(track.path.clone());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.current_queue_index = Some(idx - 1);
                    // Go to last track of previous album
                    if let Some(prev_item) = self.queue.get_mut(idx - 1) {
                        prev_item.current_track_index =
                            prev_item.album.tracks.len().saturating_sub(1);
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
            self.queue[0].current_track_index = 0;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    /// Jump to the selected album in queue and start playing its first track
    pub fn jump_to_selected_album(&mut self) -> Option<PathBuf> {
        if self.selected_queue_index < self.queue.len() {
            self.current_queue_index = Some(self.selected_queue_index);
            self.queue[self.selected_queue_index].current_track_index = 0;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    pub fn increase_volume(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    pub fn decrease_volume(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }

    // Plugin management

    /// Request a plugin update and reset retry state
    /// This should be called whenever the plugin chain is modified
    pub fn request_plugin_update(&mut self) {
        self.needs_plugin_update = true;
        self.plugin_update_retry_count = 0;
        self.plugin_update_in_progress = false;
    }

    pub fn add_plugin(&mut self, plugin_type: &PluginType) {
        self.plugin_chain.add_plugin(plugin_type);
        // Update BinauralDecoder input channels after adding
        self.plugin_chain.update_binaural_decoder_channels();
        self.request_plugin_update();
    }

    pub fn remove_plugin(&mut self, index: usize) {
        self.plugin_chain.remove_plugin(index);
        if self.selected_plugin_index >= self.plugin_chain.len() && self.selected_plugin_index > 0 {
            self.selected_plugin_index = self.plugin_chain.len() - 1;
        }
        // Update BinauralDecoder input channels after removal
        self.plugin_chain.update_binaural_decoder_channels();
        self.request_plugin_update();
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_chain.toggle_plugin(index);
        // Update BinauralDecoder input channels after toggle
        self.plugin_chain.update_binaural_decoder_channels();
        self.request_plugin_update();
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if index > 0 {
            self.plugin_chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.request_plugin_update();
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if index < self.plugin_chain.len() - 1 {
            self.plugin_chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.request_plugin_update();
        }
    }

    pub fn select_next_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.plugin_chain.len();
        }
    }

    pub fn select_previous_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.plugin_chain.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    // Plugin parameter editing
    pub fn enter_plugin_edit_mode(&mut self) {
        if self.selected_plugin_index < self.plugin_chain.len() {
            self.editing_plugin_index = Some(self.selected_plugin_index);
            self.plugin_param_selection = 0;
            self.input_mode = InputMode::EditPlugin;
        }
    }

    pub fn exit_plugin_edit_mode(&mut self) {
        self.editing_plugin_index = None;
        self.plugin_param_selection = 0;
        self.input_mode = InputMode::Normal;
    }

    pub fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin(idx))
    }

    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin_mut(idx))
    }

    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

    pub fn select_previous_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                if self.plugin_param_selection == 0 {
                    self.plugin_param_selection = param_count - 1;
                } else {
                    self.plugin_param_selection -= 1;
                }
            }
        }
    }

    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    pub fn adjust_selected_param(&mut self, delta: f64) -> bool {
        use sotf_audio_player::PluginSettings;

        let param_idx = self.plugin_param_selection;
        let mut channel_count_changed = false;

        let result = if let Some(plugin) = self.get_editing_plugin_mut() {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    speaker_config,
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    lfe_cutoff_hz,
                    stereo_width,
                    bandpass_hz,
                    height_gain,
                    lfe_gain,
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    enable_hr_direct,
                    hr_sharpen,
                    safety_cap_db,
                    decorrelation_mode,
                } => {
                    match param_idx {
                        0 => {
                            // speaker_config: cycle through available configs
                            let configs = [
                                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                "9.1.4", "9.1.6",
                            ];
                            let current_idx = configs
                                .iter()
                                .position(|&c| c == speaker_config.as_str())
                                .unwrap_or(2);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % configs.len()
                            } else {
                                (current_idx + configs.len() - 1) % configs.len()
                            };
                            *speaker_config = configs[new_idx].to_string();
                            channel_count_changed = true; // Upmixer changed channel count
                        }
                        1 => {
                            *gain_front_direct = (*gain_front_direct + delta * 0.1).clamp(0.0, 2.0)
                        }
                        2 => {
                            *gain_front_ambient =
                                (*gain_front_ambient + delta * 0.1).clamp(0.0, 2.0)
                        }
                        3 => {
                            *gain_rear_ambient = (*gain_rear_ambient + delta * 0.1).clamp(0.0, 2.0)
                        }
                        4 => *lfe_cutoff_hz = (*lfe_cutoff_hz + delta * 5.0).clamp(20.0, 200.0),
                        5 => *stereo_width = (*stereo_width + delta * 0.05).clamp(0.0, 1.0),
                        6 => *bandpass_hz = (*bandpass_hz + delta * 10.0).clamp(100.0, 500.0),
                        7 => *height_gain = (*height_gain + delta * 0.1).clamp(0.0, 2.0),
                        8 => *lfe_gain = (*lfe_gain + delta * 0.1).clamp(0.0, 2.0),
                        9 => {
                            // Toggle subharmonic synth on/off
                            *enable_subharmonic_synth = !*enable_subharmonic_synth;
                        }
                        10 => {
                            *subharmonic_gain = (*subharmonic_gain + delta * 0.05).clamp(0.0, 1.0)
                        }
                        11 => {
                            *enable_hr_direct = !*enable_hr_direct;
                        }
                        12 => *hr_sharpen = (*hr_sharpen + delta * 0.05).clamp(0.0, 1.0),
                        13 => *safety_cap_db = (*safety_cap_db + delta * 0.5).clamp(0.0, 12.0),
                        14 => {
                            // Toggle decorrelation mode (0 or 1)
                            if delta.abs() > 0.1 {
                                *decorrelation_mode = if *decorrelation_mode == 0 { 1 } else { 0 };
                            }
                        }
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Compressor {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    knee_db,
                    makeup_gain_db,
                    mix,
                    auto_makeup,
                    link_channels,
                    sidechain_hpf_hz,
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                        1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                        2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                        3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                        4 => *knee_db = (*knee_db + delta * 0.1).clamp(0.0, 12.0),
                        5 => *makeup_gain_db = (*makeup_gain_db + delta * 0.1).clamp(-20.0, 20.0),
                        6 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                        7 => {
                            if delta.abs() > 0.1 {
                                *auto_makeup = !*auto_makeup;
                            }
                        }
                        8 => {
                            if delta.abs() > 0.1 {
                                *link_channels = !*link_channels;
                            }
                        }
                        9 => *sidechain_hpf_hz = (*sidechain_hpf_hz + delta).clamp(20.0, 500.0),
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Limiter {
                    threshold_db,
                    release_ms,
                    mix,
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta * 0.1).clamp(-20.0, 0.0),
                        1 => *release_ms = (*release_ms + delta).clamp(1.0, 500.0),
                        2 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta).clamp(-80.0, 0.0),
                        1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 100.0),
                        2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                        3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                        4 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                        5 => {
                            // Toggle linked / unlinked sidechain detection
                            *link_channels = !*link_channels;
                        }
                        6 => {
                            // Adjust sidechain HPF cutoff in Hz
                            *sidechain_hpf_hz = (*sidechain_hpf_hz + delta * 5.0).clamp(0.0, 200.0);
                        }
                        _ => return false,
                    }
                    true
                }
                PluginSettings::LoudnessCompensation {
                    target_lufs,
                    min_gain_db,
                    max_gain_db,
                } => {
                    match param_idx {
                        0 => *target_lufs = (*target_lufs + delta).clamp(-40.0, 0.0),
                        1 => *min_gain_db = (*min_gain_db + delta).clamp(-20.0, 0.0),
                        2 => *max_gain_db = (*max_gain_db + delta).clamp(0.0, 20.0),
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Gain { gain_db } => match param_idx {
                    0 => {
                        *gain_db = (*gain_db + delta * 0.5).clamp(-40.0, 40.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::EQ { filters } => {
                    if filters.is_empty() {
                        return false;
                    }

                    let total_params = filters.len() * 4;
                    if param_idx >= total_params {
                        return false;
                    }

                    let filter_idx = param_idx / 4;
                    let field_idx = param_idx % 4;

                    if let Some(filter) = filters.get_mut(filter_idx) {
                        match field_idx {
                            0 => {
                                filter.frequency =
                                    (filter.frequency + delta * 10.0).clamp(20.0, 20_000.0);
                                true
                            }
                            1 => {
                                filter.q = (filter.q + delta * 0.1).clamp(0.1, 10.0);
                                true
                            }
                            2 => {
                                filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0);
                                true
                            }
                            3 => {
                                use sotf_audio_player::BiquadFilterType;

                                let types = [
                                    BiquadFilterType::Peak,
                                    BiquadFilterType::Lowshelf,
                                    BiquadFilterType::Highshelf,
                                    BiquadFilterType::Lowpass,
                                    BiquadFilterType::Highpass,
                                    BiquadFilterType::Bandpass,
                                    BiquadFilterType::Notch,
                                ];

                                let current_idx = types
                                    .iter()
                                    .position(|t| *t == filter.filter_type)
                                    .unwrap_or(0);
                                let new_idx = if delta > 0.0 {
                                    (current_idx + 1) % types.len()
                                } else {
                                    (current_idx + types.len() - 1) % types.len()
                                };
                                filter.filter_type = types[new_idx];
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                PluginSettings::BinauralDecoder {
                    input_channels,
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    ..
                } => {
                    // sofa_file (param 0) is set via 'f' key and cannot be adjusted here
                    match param_idx {
                        1 => {
                            *input_channels =
                                (*input_channels as i64 + delta as i64).clamp(2, 16) as usize;
                            true
                        }
                        2 => {
                            // Toggle optimization on/off
                            *enable_optimization = !*enable_optimization;
                            true
                        }
                        3 => {
                            *externalization = (*externalization + delta * 0.05).clamp(0.0, 1.0);
                            true
                        }
                        4 => {
                            *near_field_strength =
                                (*near_field_strength + delta * 0.05).clamp(0.0, 1.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Convolution {
                    ir_file: _,
                    mix,
                    gain_db,
                } => {
                    // ir_file (param 0) would need file browser - not adjustable here
                    match param_idx {
                        0 => false, // IR file
                        1 => {
                            *mix = (*mix + delta * 0.01).clamp(0.0, 1.0);
                            true
                        }
                        2 => {
                            *gain_db = (*gain_db + delta * 0.1).clamp(-20.0, 20.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::LoudnessMonitor => {
                    // Analyzer plugin - no parameters to adjust
                    false
                }
                PluginSettings::SpectrumAnalyzer {
                    num_bins,
                    min_freq,
                    max_freq,
                    smoothing,
                } => match param_idx {
                    0 => {
                        *num_bins = (*num_bins as i64 + delta as i64).clamp(10, 100) as usize;
                        true
                    }
                    1 => {
                        *min_freq = (*min_freq + delta as f32).clamp(10.0, 100.0);
                        true
                    }
                    2 => {
                        *max_freq = (*max_freq + delta as f32 * 100.0).clamp(1000.0, 24000.0);
                        true
                    }
                    3 => {
                        *smoothing = (*smoothing + delta as f32 * 0.01).clamp(0.0, 1.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::ChannelMuteSolo { .. } => {
                    // ChannelMuteSolo is automatically managed, not user-editable
                    false
                }
            }
        } else {
            false
        };

        // If speaker config changed, update downstream BinauralDecoder plugins
        if channel_count_changed {
            self.plugin_chain.update_binaural_decoder_channels();
        }

        result
    }

    /// Save plugin chain to file
    pub fn save_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Check if file exists and show warning if overwriting
        let filename_with_ext = if self.plugin_file_input.ends_with(".json") {
            self.plugin_file_input.clone()
        } else {
            format!("{}.json", self.plugin_file_input)
        };

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
            let full_path = presets_dir.join(&filename_with_ext);
            if full_path.exists() {
                self.status_message = Some(format!(
                    "Warning: Overwriting existing preset: {}",
                    filename_with_ext
                ));
                log::warn!("Overwriting existing preset: {}", filename_with_ext);
            }
        }

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        match self.plugin_chain.save_to_file(&self.plugin_file_input) {
            Ok(_) => {
                self.status_message = Some(format!("Saved preset: {}", filename_with_ext));
                self.last_loaded_preset = Some(filename_with_ext);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.status_message = Some(format!("Error saving: {}", e));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    /// Save plugin chain to selected preset file (overwrite confirmation shown in UI)
    pub fn save_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            // Pass filename as-is; save_to_file handles .json extension correctly
            // Save using the plugin chain's own save method
            match self.plugin_chain.save_to_file(&preset_filename) {
                Ok(_) => {
                    self.status_message = Some(format!("Overwritten preset: {}", preset_filename));
                    self.last_loaded_preset = Some(preset_filename);
                    // Refresh presets list
                    self.refresh_plugin_presets();
                }
                Err(e) => {
                    self.status_message = Some(format!("Error saving: {}", e));
                    log::error!("Failed to save plugin chain: {}", e);
                }
            }
        }
    }

    /// Load plugin chain from file
    pub fn load_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Load using the plugin chain's own load method (handles path, extension, etc.)
        match self.plugin_chain.load_from_file(&self.plugin_file_input) {
            Ok(_) => {
                // Update BinauralDecoder input channels after loading
                self.plugin_chain.update_binaural_decoder_channels();

                // Get the final filename (with .json appended if needed)
                let filename = if self.plugin_file_input.ends_with(".json") {
                    self.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.plugin_file_input)
                };

                self.status_message = Some(format!("Loaded preset: {}", filename));
                self.request_plugin_update();
                self.last_loaded_preset = Some(filename);
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading: {}", e));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    /// Refresh the list of available plugin presets from the config directory
    pub fn refresh_plugin_presets(&mut self) {
        self.available_plugin_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_plugin_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            // Sort presets alphabetically
            self.available_plugin_presets.sort();
        }

        log::info!(
            "Found {} plugin presets",
            self.available_plugin_presets.len()
        );
    }

    /// Select the next preset in the list
    pub fn select_next_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_plugin_presets.len();
        }
    }

    /// Select the previous preset in the list
    pub fn select_previous_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_plugin_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }

    /// Load the currently selected preset
    pub fn load_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            log::warn!("No presets available to load");
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            log::info!(
                "Loading preset: {} (index {})",
                preset_filename,
                self.selected_preset_index
            );
            // Use the plugin chain's own load method (handles path construction)
            match self.plugin_chain.load_from_file(&preset_filename) {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_chain.update_binaural_decoder_channels();

                    log::info!(
                        "Successfully loaded preset: {} ({} plugins)",
                        preset_filename,
                        self.plugin_chain.len()
                    );
                    self.status_message = Some(format!("Loaded preset: {}", preset_filename));
                    self.request_plugin_update();
                    self.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.status_message = Some(format!("Error loading preset: {}", e));
                    log::error!("Failed to load preset {}: {}", preset_filename, e);
                }
            }
        } else {
            log::error!(
                "Failed to get preset at index {}",
                self.selected_preset_index
            );
        }
    }

    /// Generate autocomplete suggestions for the current directory input
    pub fn generate_autocomplete_suggestions(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        let input = if self.directory_input.is_empty() {
            "./"
        } else {
            &self.directory_input
        };

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
            // User typed a complete directory with trailing slash
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            // User is typing a partial name
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            // Fallback to current directory
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

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
    }

    /// Apply the current autocomplete suggestion to the directory input
    pub fn apply_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.directory_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion
    pub fn next_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete();
        }
    }

    /// Clear autocomplete suggestions
    pub fn clear_autocomplete(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;
    }

    /// Generate autocomplete suggestions for saving presets (restricted to preset directory)
    /// This filters available presets by the current input and provides suggestions
    pub fn generate_autocomplete_suggestions_for_save_preset(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        // Get the current input (without .json extension if present)
        let input = self
            .plugin_file_input
            .trim_end_matches(".json")
            .to_lowercase();

        // Filter available presets by prefix match
        for preset in &self.available_plugin_presets {
            let preset_without_ext = preset.trim_end_matches(".json");
            if preset_without_ext.to_lowercase().starts_with(&input) {
                // Add suggestion without .json extension (save_to_file will add it)
                self.autocomplete_suggestions
                    .push(preset_without_ext.to_string());
            }
        }

        // Sort suggestions alphabetically
        self.autocomplete_suggestions.sort();
    }

    /// Generate autocomplete suggestions for plugin file input
    pub fn generate_autocomplete_suggestions_for_plugin_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.plugin_file_input.clone());
    }

    /// Apply autocomplete to plugin file input
    pub fn apply_autocomplete_to_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.plugin_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for plugin file input
    pub fn next_autocomplete_for_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_plugin_file();
        }
    }

    /// Generate autocomplete suggestions for APO file input
    pub fn generate_autocomplete_suggestions_for_apo_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.apo_file_input.clone());
    }

    /// Apply autocomplete to APO file input
    pub fn apply_autocomplete_to_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.apo_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for APO file input
    pub fn next_autocomplete_for_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_apo_file();
        }
    }

    /// Generate autocomplete suggestions for SOFA file input
    pub fn generate_autocomplete_suggestions_for_sofa_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.sofa_file_input.clone());
    }

    /// Apply autocomplete to SOFA file input
    pub fn apply_autocomplete_to_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.sofa_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for SOFA file input
    pub fn next_autocomplete_for_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_sofa_file();
        }
    }

    /// Generic autocomplete suggestions generator for any file input
    fn generate_autocomplete_suggestions_for_input(&mut self, input: &str) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

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
            // User typed a complete directory with trailing slash
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            // User is typing a partial name
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            // Fallback to current directory
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

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
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
            if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                plugin.settings = PluginSettings::EQ { filters };
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

    /// Find and load all image files in the currently playing album's directory
    pub fn load_album_images(&mut self) {
        self.album_images.clear();
        self.selected_image_index = 0;

        // Initialize image picker if not already done
        if self.image_picker.is_none() {
            self.image_picker = ratatui_image::picker::Picker::from_termios().ok();
            if self.image_picker.is_none() {
                self.image_picker = Some(ratatui_image::picker::Picker::new((8, 16)));
            }
        }

        // Get the currently playing album
        if let Some(queue_index) = self.current_queue_index {
            if let Some(queue_item) = self.queue.get(queue_index) {
                if let Some(first_track) = queue_item.album.tracks.first() {
                    if let Some(parent_dir) = first_track.path.parent() {
                        // Find all image files in the directory
                        if let Ok(entries) = std::fs::read_dir(parent_dir) {
                            for entry in entries.flatten() {
                                if let Ok(path) = entry.path().canonicalize() {
                                    if let Some(ext) = path.extension() {
                                        let ext_lower = ext.to_string_lossy().to_lowercase();
                                        if matches!(
                                            ext_lower.as_str(),
                                            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                                        ) {
                                            self.album_images.push(path);
                                        }
                                    }
                                }
                            }
                        }
                        // Sort images for consistent order
                        self.album_images.sort();
                    }
                }
            }
        }
    }

    /// Cycle to the next image in the album directory
    pub fn next_album_image(&mut self) {
        if !self.album_images.is_empty() {
            self.selected_image_index = (self.selected_image_index + 1) % self.album_images.len();
        }
    }

    /// Cycle to the previous image in the album directory
    pub fn prev_album_image(&mut self) {
        if !self.album_images.is_empty() {
            if self.selected_image_index == 0 {
                self.selected_image_index = self.album_images.len() - 1;
            } else {
                self.selected_image_index -= 1;
            }
        }
    }

    /// Get the currently selected album image path
    pub fn get_current_album_image(&self) -> Option<&PathBuf> {
        self.album_images.get(self.selected_image_index)
    }

    /// Build channel groups from current channel count
    pub fn update_level_meter_groups(&mut self) {
        self.level_meter_groups.clear();

        let num_channels = self
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        if num_channels == 0 {
            return;
        }

        // Standard channel layouts based on channel count
        match num_channels {
            1 => {
                // Mono
                self.level_meter_groups.push(ChannelGroup {
                    name: "Mono".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "M".to_string(),
                        display_name: vec!["M".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            2 => {
                // Stereo
                self.level_meter_groups.push(ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            4 => {
                // Quad (FL, FR, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 2,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 3,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            6 => {
                // 5.1 (FL, FR, FC, LFE, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            8 => {
                // 7.1 (FL, FR, FC, LFE, SL, SR, BL, BR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            10 => {
                // 5.1.4 (FL, FR, FC, LFE, SL, SR, TFL, TFR, TBL, TBR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 8,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            _ => {
                // Generic fallback - treat all channels as one group
                let mut channels = Vec::new();
                for i in 0..num_channels {
                    channels.push(ChannelInfo {
                        index: i,
                        name: format!("CH{}", i + 1),
                        display_name: vec![format!("CH{}", i + 1)],
                    });
                }
                self.level_meter_groups.push(ChannelGroup {
                    name: "All Channels".to_string(),
                    channels,
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        }

        // Update ChannelMuteSolo plugin to have correct number of channels
        self.update_channel_mute_solo_plugin();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_channel_mute_solo_plugin();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle solo for the selected level meter group
    pub fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meter_groups.iter_mut().enumerate() {
                if idx == self.selected_level_meter_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Update the ChannelMuteSolo plugin based on current level meter group states
    fn update_channel_mute_solo_plugin(&mut self) {
        use sotf_audio_player::PluginSettings;
        use sotf_plugins::ChannelState;

        // Calculate total channel count
        let num_channels = self
            .level_meter_groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meter_groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Determine if any channel is muted, soloed, or dimmed
        let enabled = channel_states
            .iter()
            .any(|s| s.muted || s.soloed || s.dimmed);

        // Find and update the ChannelMuteSolo plugin
        // We need to compute the engine index, which only counts enabled plugins
        let mut engine_index = 0;
        for i in 0..self.plugin_chain.len() {
            if let Some(plugin) = self.plugin_chain.get_plugin_mut(i) {
                if matches!(&plugin.settings, PluginSettings::ChannelMuteSolo { .. }) {
                    // Update settings in memory
                    plugin.settings = PluginSettings::ChannelMuteSolo {
                        enabled,
                        channel_states: channel_states.clone(),
                    };

                    // Queue zero-dropout parameter update
                    // Serialize channel states to JSON
                    // Use engine_index (not i) since the engine only has enabled plugins
                    if let Ok(json) = serde_json::to_string(&channel_states) {
                        self.pending_param_update = Some(PendingParameterUpdate {
                            plugin_index: engine_index,
                            param_id: "channel_states".to_string(),
                            value: json,
                        });
                    }
                    return;
                }
                // Only count enabled plugins toward the engine index
                if plugin.enabled {
                    engine_index += 1;
                }
            }
        }
    }

    /// Navigate to next level meter group
    pub fn select_next_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            self.selected_level_meter_group =
                (self.selected_level_meter_group + 1) % self.level_meter_groups.len();
        }
    }

    /// Navigate to previous level meter group
    pub fn select_previous_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            if self.selected_level_meter_group == 0 {
                self.selected_level_meter_group = self.level_meter_groups.len() - 1;
            } else {
                self.selected_level_meter_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    pub fn select_next_level_meter_control(&mut self) {
        self.level_meter_control_selection = (self.level_meter_control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    pub fn select_previous_level_meter_control(&mut self) {
        self.level_meter_control_selection = if self.level_meter_control_selection == 0 {
            2
        } else {
            self.level_meter_control_selection - 1
        };
    }

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: PathBuf) {
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
}

// Helper function to get parameter count for a plugin
fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    use sotf_audio_player::PluginSettings;
    match settings {
        PluginSettings::EQ { filters } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Gain { .. } => 1,                    // gain_db
        PluginSettings::Upmixer { .. } => 15, // speaker_config, gains (5), lfe_cutoff_hz, stereo_width, bandpass_hz, subharmonic (2), hr (2), safety_cap_db, decorrelation_mode
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 3,     // threshold, release, mix
        PluginSettings::Gate { .. } => 7, // threshold, ratio, attack, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::LoudnessCompensation { .. } => 3, // target_lufs, min_gain, max_gain
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Convolution { .. } => 3,     // ir_file, mix, gain_db
        PluginSettings::LoudnessMonitor => 0,        // No parameters
        PluginSettings::SpectrumAnalyzer { .. } => 4, // num_bins, min_freq, max_freq, smoothing
        PluginSettings::ChannelMuteSolo { .. } => 0, // Automatically managed, no user-editable parameters
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player::{Album, DirectoryInfo, Track};
    use sotf_audio_player::{PluginSettings, PluginType};
    use std::path::PathBuf;

    fn create_test_directory_info(path: &str) -> DirectoryInfo {
        DirectoryInfo {
            path: PathBuf::from(path),
            file_count: 10,
            album_count: 2,
            last_scanned: None,
            expanded: false,
            subdirectories: vec![],
        }
    }

    fn create_test_app_with_directories(num_dirs: usize) -> App {
        let mut app = App::new(Theme::default());
        for i in 0..num_dirs {
            app.library
                .directories
                .push(create_test_directory_info(&format!("/test/dir{}", i)));
        }
        app
    }

    #[test]
    fn test_select_next_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2);

        // Should wrap around to 0
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_select_previous_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        // Should wrap around to last item
        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 2);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_page_down_directories() {
        let mut app = create_test_app_with_directories(30);

        assert_eq!(app.selected_directory_index, 0);

        // Page down by 20
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 20);

        // Page down by 20 again - should stop at max (29)
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);

        // Should stay at max
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);
    }

    #[test]
    fn test_page_up_directories() {
        let mut app = create_test_app_with_directories(30);

        // Start at the end
        app.selected_directory_index = 29;

        // Page up by 20
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 9);

        // Page up by 20 again - should stop at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        // Should stay at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_empty_directories() {
        let mut app = App::new(Theme::default());
        assert_eq!(app.selected_directory_index, 0);

        // Should not crash with empty directories
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_expanded_directories() {
        let mut app = create_test_app_with_directories(2);

        // Add subdirectories to first directory
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];

        // Initially collapsed - tree has 2 items
        assert_eq!(app.get_directory_tree_items().len(), 2);
        assert_eq!(app.selected_directory_index, 0);

        // Expand first directory
        app.toggle_directory_expansion();

        // Now tree has 4 items: dir0, subdir1, subdir2, dir1
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 4);

        // Navigate through expanded tree
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1); // subdir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2); // subdir2

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 3); // dir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0); // wrap to dir0
    }

    #[test]
    fn test_get_directory_tree_items() {
        let mut app = create_test_app_with_directories(1);

        // Add subdirectories
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];

        // Collapsed - should only show root
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 1);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert_eq!(tree_items[0].2, false); // not expanded

        // Expand
        app.toggle_directory_expansion();

        // Should show root + 2 subdirectories
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 3);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert_eq!(tree_items[0].2, true); // expanded

        assert_eq!(tree_items[1].0, PathBuf::from("/test/dir0/subdir1"));
        assert_eq!(tree_items[1].1, 1); // level

        assert_eq!(tree_items[2].0, PathBuf::from("/test/dir0/subdir2"));
        assert_eq!(tree_items[2].1, 1); // level
    }

    fn create_test_album(artist: &str, title: &str, base_path: &str, track_count: usize) -> Album {
        let mut tracks = Vec::new();
        for i in 0..track_count {
            tracks.push(Track {
                path: PathBuf::from(format!("{}/track{}.flac", base_path, i)),
                title: None,
                track_number: Some(i as u32),
                duration_secs: None,
                channels: None,
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            });
        }
        Album {
            id: None,
            artist: artist.to_string(),
            title: title.to_string(),
            year: None,
            tracks,
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
        }
    }

    #[test]
    fn test_next_track_removes_finished_album_and_advances() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);

        app.queue = vec![QueueItem::new(album1), QueueItem::new(album2)];
        app.expanded_queue_items = vec![false, false];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        let first_path = app.current_track_path().unwrap();
        assert!(first_path.to_string_lossy().contains("track0.flac"));

        let second_path = app.next_track().unwrap();
        assert!(second_path.to_string_lossy().contains("track1.flac"));
        assert_eq!(app.queue.len(), 2);
        assert_eq!(app.current_queue_index, Some(0));

        let third_path = app.next_track().unwrap();
        assert!(third_path.to_string_lossy().contains("album2/track0.flac"));
        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.current_queue_index, Some(0));

        let fourth_path = app.next_track().unwrap();
        assert!(fourth_path.to_string_lossy().contains("album2/track1.flac"));

        let none = app.next_track();
        assert!(none.is_none());
        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_adjust_eq_parameters() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::EQ);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert!(!filters.is_empty());

        let orig_freq = filters[0].frequency;
        let orig_q = filters[0].q;
        let orig_gain = filters[0].gain_db;
        let orig_type = filters[0].filter_type;

        // Frequency
        app.plugin_param_selection = 0;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].frequency, orig_freq);

        // Q
        app.plugin_param_selection = 1;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].q, orig_q);

        // Gain
        app.plugin_param_selection = 2;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].gain_db, orig_gain);

        // Type
        app.plugin_param_selection = 3;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].filter_type, orig_type);
    }

    #[test]
    fn test_adjust_upmixer_parameters() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Upmixer);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (
            orig_speaker_config,
            orig_front_direct,
            orig_front_ambient,
            orig_rear_ambient,
            orig_lfe_cutoff,
            orig_stereo_width,
            orig_bandpass,
            orig_height_gain,
            orig_lfe_gain,
            orig_enable_subharm,
            orig_subharm_gain,
        ) = match &plugin.settings {
            PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                lfe_cutoff_hz,
                stereo_width,
                bandpass_hz,
                height_gain,
                lfe_gain,
                enable_subharmonic_synth,
                subharmonic_gain,
                ..
            } => (
                speaker_config.clone(),
                *gain_front_direct,
                *gain_front_ambient,
                *gain_rear_ambient,
                *lfe_cutoff_hz,
                *stereo_width,
                *bandpass_hz,
                *height_gain,
                *lfe_gain,
                *enable_subharmonic_synth,
                *subharmonic_gain,
            ),
            _ => panic!("Expected Upmixer plugin"),
        };

        for idx in 0..11 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            ..
        } = &plugin.settings
        {
            assert_ne!(*speaker_config, orig_speaker_config);
            assert_ne!(*gain_front_direct, orig_front_direct);
            assert_ne!(*gain_front_ambient, orig_front_ambient);
            assert_ne!(*gain_rear_ambient, orig_rear_ambient);
            assert_ne!(*lfe_cutoff_hz, orig_lfe_cutoff);
            assert_ne!(*stereo_width, orig_stereo_width);
            assert_ne!(*bandpass_hz, orig_bandpass);
            assert_ne!(*height_gain, orig_height_gain);
            assert_ne!(*lfe_gain, orig_lfe_gain);
            assert_ne!(*enable_subharmonic_synth, orig_enable_subharm);
            assert_ne!(*subharmonic_gain, orig_subharm_gain);
        } else {
            panic!("Expected Upmixer plugin");
        }
    }

    #[test]
    fn test_adjust_compressor_limiter_gate_loudness_parameters() {
        // Compressor
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Compressor);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (
            orig_thresh,
            orig_ratio,
            orig_attack,
            orig_release,
            orig_knee,
            orig_makeup,
            orig_mix,
            orig_auto_makeup,
            orig_link_channels,
            orig_sidechain_hpf,
        ) = match &plugin.settings {
            PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                *makeup_gain_db,
                *mix,
                *auto_makeup,
                *link_channels,
                *sidechain_hpf_hz,
            ),
            _ => panic!("Expected Compressor plugin"),
        };

        for idx in 0..10 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*ratio, orig_ratio);
            assert_ne!(*attack_ms, orig_attack);
            assert_ne!(*release_ms, orig_release);
            assert_ne!(*knee_db, orig_knee);
            assert_ne!(*makeup_gain_db, orig_makeup);
            assert_ne!(*mix, orig_mix);
            assert_ne!(*auto_makeup, orig_auto_makeup);
            assert_ne!(*link_channels, orig_link_channels);
            assert_ne!(*sidechain_hpf_hz, orig_sidechain_hpf);
        }

        // Limiter (use -1.0 since mix starts at 1.0 which is max)
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Limiter);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_thresh, orig_rel, orig_mix) = match &plugin.settings {
            PluginSettings::Limiter {
                threshold_db,
                release_ms,
                mix,
            } => (*threshold_db, *release_ms, *mix),
            _ => panic!("Expected Limiter plugin"),
        };
        for idx in 0..3 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(-1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Limiter {
            threshold_db,
            release_ms,
            mix,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*release_ms, orig_rel);
            assert_ne!(*mix, orig_mix);
        }

        // Gate - test parameters individually since mix starts at max (1.0) and hpf at min (0.0)
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Gate);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_thresh, orig_ratio, orig_attack, orig_release, orig_mix, orig_link, orig_hpf) =
            match &plugin.settings {
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => (
                    *threshold_db,
                    *ratio,
                    *attack_ms,
                    *release_ms,
                    *mix,
                    *link_channels,
                    *sidechain_hpf_hz,
                ),
                _ => panic!("Expected Gate plugin"),
            };
        // Adjust each parameter - mix (idx 4) decreases, hpf (idx 6) increases, others can go either way
        for idx in 0..7 {
            app.plugin_param_selection = idx;
            let delta = if idx == 4 { -1.0 } else { 1.0 }; // mix starts at max, decrease it
            assert!(app.adjust_selected_param(delta));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*ratio, orig_ratio);
            assert_ne!(*attack_ms, orig_attack);
            assert_ne!(*release_ms, orig_release);
            assert_ne!(*mix, orig_mix);
            assert_ne!(*link_channels, orig_link);
            assert_ne!(*sidechain_hpf_hz, orig_hpf);
        }

        // Loudness compensation
        let mut app = App::new(Theme::default());
        let plugin_idx = app
            .plugin_chain
            .add_plugin(&PluginType::LoudnessCompensation);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_target, orig_min, orig_max) = match &plugin.settings {
            PluginSettings::LoudnessCompensation {
                target_lufs,
                min_gain_db,
                max_gain_db,
            } => (*target_lufs, *min_gain_db, *max_gain_db),
            _ => panic!("Expected LoudnessCompensation plugin"),
        };
        for idx in 0..3 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::LoudnessCompensation {
            target_lufs,
            min_gain_db,
            max_gain_db,
        } = &plugin.settings
        {
            assert_ne!(*target_lufs, orig_target);
            assert_ne!(*min_gain_db, orig_min);
            assert_ne!(*max_gain_db, orig_max);
        }
    }

    #[test]
    fn test_adjust_binaural_decoder_parameters_and_set_sofa() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);
        app.editing_plugin_index = Some(plugin_idx);
        app.selected_plugin_index = plugin_idx;

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_sofa, orig_channels, orig_opt, orig_ext, orig_near) = match &plugin.settings {
            PluginSettings::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => (
                sofa_file.clone(),
                *input_channels,
                *enable_optimization,
                *externalization,
                *near_field_strength,
            ),
            _ => panic!("Expected BinauralDecoder plugin"),
        };

        // Adjust numeric / boolean parameters via adjust_selected_param
        for idx in 1..5 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } = &plugin.settings
        {
            assert_eq!(*sofa_file, orig_sofa); // unchanged by adjust_selected_param
            assert_ne!(*input_channels, orig_channels);
            assert_ne!(*enable_optimization, orig_opt);
            assert_ne!(*externalization, orig_ext);
            assert_ne!(*near_field_strength, orig_near);
        } else {
            panic!("Expected BinauralDecoder plugin");
        }

        // Now set SOFA file via load_sofa_file path
        app.sofa_file_input = "/tmp/test.sofa".to_string();
        app.load_sofa_file().unwrap();

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder { sofa_file, .. } = &plugin.settings {
            assert_eq!(sofa_file, "/tmp/test.sofa");
        } else {
            panic!("Expected BinauralDecoder plugin");
        }
    }
}
