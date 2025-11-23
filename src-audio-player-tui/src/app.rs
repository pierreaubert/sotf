use crate::theme::Theme;
use sotf_audio::devices::AudioDevice;
use sotf_audio::LoudnessInfo;
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

    // Loudness monitoring
    pub loudness_info: Option<LoudnessInfo>,

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

    // Last loaded plugin preset name (for config persistence)
    pub last_loaded_preset: Option<String>,
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
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            library_view_mode: LibraryViewMode::Flat,
            library_sort_order: LibrarySortOrder::Artist,
            channel_filter: ChannelFilter::All,
            artist_tree: Vec::new(),
            selected_tree_index: 0,
            plugin_chain: PluginChain::new(),
            needs_plugin_update: false,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            plugin_update_last_attempt: None,
            plugin_update_retry_count: 0,
            plugin_update_in_progress: false,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            loudness_info: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            should_quit: false,
            needs_rescan: false,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            maintenance_in_progress: false,
            maintenance_progress_checked: 0,
            maintenance_progress_total: 0,
            replay_gain_scanner: None,
            replay_gain_in_progress: false,
            replay_gain_total: 0,
            replay_gain_processed: 0,
            replay_gain_succeeded: 0,
            replay_gain_failed: 0,
            last_loaded_preset: None,
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

    /// Start library scan (non-blocking, sets up progress tracking)
    pub fn start_library_scan(&mut self) {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Starting library scan...".to_string());
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

    /// Save current app state to config file
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::AppConfig {
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

        // Find which artist node we're on
        let mut current_row = 0;
        for artist_node in &mut self.artist_tree {
            if current_row == self.selected_tree_index {
                artist_node.expanded = !artist_node.expanded;
                return;
            }
            current_row += 1;
            if artist_node.expanded {
                current_row += artist_node.album_indices.len();
            }
        }
    }

    /// Get the flattened tree items for rendering (returns artist names or album indices)
    pub fn get_tree_items(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();

        for artist_node in &self.artist_tree {
            items.push(TreeItem::Artist {
                name: artist_node.artist.clone(),
                expanded: artist_node.expanded,
            });

            if artist_node.expanded {
                for &album_idx in &artist_node.album_indices {
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

        if let Some(item) = tree_items.get(self.selected_tree_index) {
            match item {
                TreeItem::Artist { .. } => {
                    // Add all albums from this artist
                    let mut current_row = 0;
                    for artist_node in &self.artist_tree {
                        if current_row == self.selected_tree_index {
                            for &album_idx in &artist_node.album_indices {
                                if let Some(album) = self.library.albums.get(album_idx) {
                                    self.queue.push(QueueItem::new(album.clone()));
                                    self.expanded_queue_items.push(false);
                                }
                            }
                            // Auto-play if queue was empty OR if nothing was playing
                            if was_empty || was_not_playing {
                                return self.start_queue();
                            }
                            return None;
                        }
                        current_row += 1;
                        if artist_node.expanded {
                            current_row += artist_node.album_indices.len();
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
                        12 => {
                            *hr_sharpen = (*hr_sharpen + delta * 0.05).clamp(0.0, 1.0)
                        }
                        13 => {
                            *safety_cap_db = (*safety_cap_db + delta * 0.5).clamp(0.0, 12.0)
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

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        match self.plugin_chain.save_to_file(&self.plugin_file_input) {
            Ok(_) => {
                // Get the final filename (with .json appended if needed)
                let filename = if self.plugin_file_input.ends_with(".json") {
                    self.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.plugin_file_input)
                };

                self.status_message = Some(format!("Saved preset: {}", filename));
                self.last_loaded_preset = Some(filename);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.status_message = Some(format!("Error saving: {}", e));
                log::error!("Failed to save plugin chain: {}", e);
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
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            // Use the plugin chain's own load method (handles path construction)
            match self.plugin_chain.load_from_file(&preset_filename) {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_chain.update_binaural_decoder_channels();

                    self.status_message = Some(format!("Loaded preset: {}", preset_filename));
                    self.request_plugin_update();
                    self.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.status_message = Some(format!("Error loading preset: {}", e));
                    log::error!("Failed to load preset: {}", e);
                }
            }
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
}

// Helper function to get parameter count for a plugin
fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    use sotf_audio_player::PluginSettings;
    match settings {
        PluginSettings::EQ { filters } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Upmixer { .. } => 14, // + enable_hr_direct, hr_sharpen, safety_cap_db
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 3,     // threshold, release, mix
        PluginSettings::Gate { .. } => 7, // threshold, ratio, attack, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::LoudnessCompensation { .. } => 3, // target_lufs, min_gain, max_gain
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Convolution { .. } => 3,     // ir_file, mix, gain_db
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
            });
        }
        Album {
            id: None,
            artist: artist.to_string(),
            title: title.to_string(),
            year: None,
            tracks,
            album_art_path: None,
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
        app.plugin_chain.add_plugin(&PluginType::EQ);
        app.editing_plugin_index = Some(0);

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].frequency, orig_freq);

        // Q
        app.plugin_param_selection = 1;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].q, orig_q);

        // Gain
        app.plugin_param_selection = 2;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].gain_db, orig_gain);

        // Type
        app.plugin_param_selection = 3;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].filter_type, orig_type);
    }

    #[test]
    fn test_adjust_upmixer_parameters() {
        let mut app = App::new(Theme::default());
        app.plugin_chain.add_plugin(&PluginType::Upmixer);
        app.editing_plugin_index = Some(0);

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        app.plugin_chain.add_plugin(&PluginType::Compressor);
        app.editing_plugin_index = Some(0);

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        // Limiter
        let mut app = App::new(Theme::default());
        app.plugin_chain.add_plugin(&PluginType::Limiter);
        app.editing_plugin_index = Some(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
            assert!(app.adjust_selected_param(1.0));
        }
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        // Gate
        let mut app = App::new(Theme::default());
        app.plugin_chain.add_plugin(&PluginType::Gate);
        app.editing_plugin_index = Some(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        for idx in 0..7 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        app.plugin_chain
            .add_plugin(&PluginType::LoudnessCompensation);
        app.editing_plugin_index = Some(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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
        app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);
        app.editing_plugin_index = Some(0);
        app.selected_plugin_index = 0;

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
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

        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        if let PluginSettings::BinauralDecoder { sofa_file, .. } = &plugin.settings {
            assert_eq!(sofa_file, "/tmp/test.sofa");
        } else {
            panic!("Expected BinauralDecoder plugin");
        }
    }
}
