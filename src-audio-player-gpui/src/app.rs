use sotf_audio_player::{Album, LoudnessInfo, MusicLibrary, Player, PluginChain, PluginType, SpectrumInfo, Track};
use sotf_audio::devices::AudioDevice;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Spectrum,
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
    Help,
}

/// Toast message type for color coding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

/// Toast message with type and timing
#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub auto_dismiss_ms: Option<u64>, // None = no auto-dismiss
}

impl ToastMessage {
    pub fn new(message: String, toast_type: ToastType) -> Self {
        Self {
            message,
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: Some(5000), // Default 5 seconds
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Success)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Error)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Info)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Warning)
    }

    pub fn persistent(message: impl Into<String>, toast_type: ToastType) -> Self {
        Self {
            message: message.into(),
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: None, // No auto-dismiss
        }
    }

    pub fn should_dismiss(&self) -> bool {
        if let Some(dismiss_ms) = self.auto_dismiss_ms {
            self.created_at.elapsed() > Duration::from_millis(dismiss_ms)
        } else {
            false
        }
    }
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
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
    pub toast_message: Option<ToastMessage>, // Enhanced toast notifications

    // Autocomplete state
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_index: usize,

    // Plugin preset selection
    pub available_plugin_presets: Vec<String>, // List of preset filenames
    pub selected_preset_index: usize,

    // Library tree view
    pub library_view_mode: LibraryViewMode,
    pub artist_tree: Vec<ArtistNode>,
    pub selected_tree_index: usize, // Index in flattened tree (artists + visible albums)

    // Plugin system
    pub plugin_chain: PluginChain,
    pub needs_plugin_update: bool,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize, // Which parameter is selected in edit mode

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub position_secs: f64,

    // Loudness monitoring
    pub loudness_info: Option<LoudnessInfo>,

    // Spectrum analyzer
    pub spectrum_visible: bool,
    pub spectrum_info: Option<SpectrumInfo>,

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

        Self {
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
            library_view_mode: LibraryViewMode::Flat,
            artist_tree: Vec::new(),
            plugin_chain: PluginChain::new(),
            needs_plugin_update: false,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            loudness_info: None,
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

    pub fn filtered_albums(&self) -> Vec<&Album> {
        use ChannelFilter::*;
        use LibrarySortOrder::*;

        // First filter by search query
        let mut albums: Vec<&Album> = if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        };

        // Then filter by channel count
        albums.retain(|album| match self.channel_filter {
            All => true,
            Mono => album.uniform_channel_count() == Some(1),
            Stereo => album.uniform_channel_count() == Some(2),
            Multichannel => {
                if let Some(count) = album.uniform_channel_count() {
                    count > 2
                } else {
                    false
                }
            }
            Mixed => album.uniform_channel_count().is_none(),
            Specific(n) => album.uniform_channel_count() == Some(n),
        });

        // Finally, sort
        match self.library_sort_order {
            Artist => {
                albums.sort_by(|a, b| {
                    a.artist
                        .cmp(&b.artist)
                        .then_with(|| a.year.cmp(&b.year).reverse())
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            Album => {
                albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            Title => {
                albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            Year => {
                albums.sort_by(|a, b| {
                    b.year
                        .cmp(&a.year)
                        .then_with(|| a.artist.cmp(&b.artist))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
        }

        albums
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

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        self.channel_filter = match self.channel_filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Multichannel,
            ChannelFilter::Multichannel => ChannelFilter::Mixed,
            ChannelFilter::Mixed => ChannelFilter::All,
            ChannelFilter::Specific(_) => ChannelFilter::All,
        };
        // Reset selection and rebuild tree
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
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

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        if self.queue.is_empty() {
            return None;
        }

        // Set current index to 0
        self.current_queue_index = Some(0);
        self.is_playing = true;

        // Get the first track of the first album
        self.queue
            .first()
            .and_then(|item| item.current_track())
            .map(|track| track.path.clone())
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        let current_idx = self.current_queue_index?;
        let item = self.queue.get_mut(current_idx)?;

        // Try to advance to next track in current album
        if let Some(track) = item.next_track() {
            return Some(track.path.clone());
        }

        // No more tracks in current album, try next album
        if current_idx + 1 < self.queue.len() {
            self.current_queue_index = Some(current_idx + 1);
            self.queue[current_idx + 1].current_track_index = 0;
            return self.queue[current_idx + 1]
                .current_track()
                .map(|t| t.path.clone());
        }

        // No more albums
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
                        return prev_item.current_track().map(|t| t.path.clone());
                    }
                }
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

    /// Add a directory to the library (interactive version with UI feedback)
    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.needs_rescan = true;
                    self.toast_message = Some(ToastMessage::success("Directory added. Press 's' to scan."));
                } else {
                    self.toast_message = Some(ToastMessage::warning("Directory already exists."));
                }
            }
            Err(msg) => {
                self.toast_message = Some(ToastMessage::error(msg));
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library.add_directory(path);
    }

    /// Remove the selected directory from the library
    pub fn remove_selected_directory(&mut self) {
        // We need to map from tree index to actual directory index
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            // Only allow removing level 0 directories (main directories, not subdirectories)
            if *level == 0 {
                // Find the actual index in the directories vector
                if let Some(dir_index) = self
                    .library
                    .directories
                    .iter()
                    .position(|d| d.path == *path)
                {
                    if self.library.remove_directory(dir_index).is_some() {
                        // Adjust selected_directory_index if needed
                        let tree_items = self.get_directory_tree_items();
                        if self.selected_directory_index >= tree_items.len()
                            && self.selected_directory_index > 0
                        {
                            self.selected_directory_index = tree_items.len() - 1;
                        }
                        self.needs_rescan = true;
                        self.toast_message = Some(ToastMessage::success("Directory removed."));
                    }
                }
            } else {
                self.toast_message = Some(ToastMessage::error("Cannot remove subdirectory."));
            }
        }
    }

    /// Start library scan (sets up progress tracking flags)
    pub fn start_library_scan(&mut self) {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.toast_message = Some(ToastMessage::info("Starting library scan..."));
    }

    /// Scan the library with progress tracking
    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use parking_lot::Mutex;
        use std::sync::Arc;

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.toast_message = Some(ToastMessage::persistent("Scanning library...", ToastType::Info));

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
                self.toast_message = Some(ToastMessage::success(format!(
                    "Scan complete: {} tracks in {} albums",
                    track_count, album_count
                )));
                log::info!(
                    "Scan complete: {} tracks in {} albums",
                    track_count,
                    album_count
                );
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!("Scan failed: {}", e)));
                log::error!("Scan failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        result
    }

    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::Config;
        let config = Config::load()?;

        // Restore directories
        self.library.directories = config.directories;

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
        use crate::config::Config;
        let config = Config {
            directories: self.library.directories.clone(),
            last_loaded_plugin_preset: self.last_loaded_preset.clone(),
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

    pub fn page_down_queue(&mut self, page_size: usize) {
        if !self.queue.is_empty() {
            self.selected_queue_index =
                (self.selected_queue_index + page_size).min(self.queue.len() - 1);
        }
    }

    pub fn page_up_queue(&mut self, page_size: usize) {
        if !self.queue.is_empty() {
            self.selected_queue_index = self.selected_queue_index.saturating_sub(page_size);
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

    pub fn toggle_queue_item_expansion(&mut self) {
        if self.selected_queue_index < self.expanded_queue_items.len() {
            self.expanded_queue_items[self.selected_queue_index] =
                !self.expanded_queue_items[self.selected_queue_index];
        }
    }

    pub fn toggle_directory_expansion(&mut self) {
        // Find which directory in the tree we're selecting
        let tree_items = self.get_directory_tree_items();
        if let Some((path, level, _)) = tree_items.get(self.selected_directory_index) {
            // Only toggle if we're on a main directory (level 0)
            if *level == 0 {
                // Find the directory in our list and toggle it
                if let Some(dir_info) = self
                    .library
                    .directories
                    .iter_mut()
                    .find(|d| d.path == *path)
                {
                    dir_info.expanded = !dir_info.expanded;
                }
            }
            // If we're on a subdirectory (level 1), do nothing - it's already part of the tree
            // Don't add it as a new main directory or trigger a rescan
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        let mut items = Vec::new();
        for dir_info in &self.library.directories {
            // Add the main directory (level 0)
            items.push((dir_info.path.clone(), 0, dir_info.expanded));

            // Add subdirectories if expanded (level 1)
            if dir_info.expanded {
                for subdir in &dir_info.subdirectories {
                    items.push((subdir.path.clone(), 1, false));
                }
            }
        }
        items
    }

    // Plugin management methods
    pub fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType) {
        let new_index = self.plugin_chain.add_plugin(plugin_type);
        self.selected_plugin_index = new_index;
        self.plugin_chain.update_binaural_decoder_channels();
        self.needs_plugin_update = true;
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_chain.toggle_plugin(index);
        // Update BinauralDecoder input channels after toggle
        self.plugin_chain.update_binaural_decoder_channels();
        self.needs_plugin_update = true;
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if index > 0 {
            self.plugin_chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if index < self.plugin_chain.len() - 1 {
            self.plugin_chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
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

    pub fn remove_plugin(&mut self, index: usize) {
        if index < self.plugin_chain.len() {
            self.plugin_chain.remove_plugin(index);
            // Update BinauralDecoder input channels after removal
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
            // Adjust selection
            if self.selected_plugin_index >= self.plugin_chain.len()
                && self.selected_plugin_index > 0
            {
                self.selected_plugin_index = self.plugin_chain.len() - 1;
            }
        }
    }

    // Plugin editing methods
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
                                .unwrap_or(0);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % configs.len()
                            } else {
                                if current_idx == 0 {
                                    configs.len() - 1
                                } else {
                                    current_idx - 1
                                }
                            };
                            *speaker_config = configs[new_idx].to_string();
                            channel_count_changed = true;
                            true
                        }
                        1 => {
                            *gain_front_direct = (*gain_front_direct + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        2 => {
                            *gain_front_ambient = (*gain_front_ambient + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        3 => {
                            *gain_rear_ambient = (*gain_rear_ambient + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        4 => {
                            *lfe_cutoff_hz = (*lfe_cutoff_hz + delta as f32 * 10.0).max(20.0).min(200.0);
                            true
                        }
                        5 => {
                            *stereo_width = (*stereo_width + delta as f32 * 0.1).max(0.0).min(2.0);
                            true
                        }
                        6 => {
                            *bandpass_hz = (*bandpass_hz + delta as f32 * 50.0).max(100.0).min(1000.0);
                            true
                        }
                        7 => {
                            *height_gain = (*height_gain + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        8 => {
                            *lfe_gain = (*lfe_gain + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        9 => {
                            *enable_subharmonic_synth = !*enable_subharmonic_synth;
                            true
                        }
                        10 => {
                            *subharmonic_gain = (*subharmonic_gain + delta as f32).max(-30.0).min(30.0);
                            true
                        }
                        11 => {
                            *enable_hr_direct = !*enable_hr_direct;
                            true
                        }
                        12 => {
                            *hr_sharpen = (*hr_sharpen + delta as f32 * 0.1).max(0.0).min(2.0);
                            true
                        }
                        13 => {
                            *safety_cap_db = (*safety_cap_db + delta as f32).max(-30.0).min(0.0);
                            true
                        }
                        _ => false,
                    }
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
                        0 => {
                            *threshold_db = (*threshold_db + delta as f32).max(-60.0).min(0.0);
                            true
                        }
                        1 => {
                            *ratio = (*ratio + delta as f32 * 0.1).max(1.0).min(20.0);
                            true
                        }
                        2 => {
                            *attack_ms = (*attack_ms + delta as f32 * 0.1).max(0.1).min(100.0);
                            true
                        }
                        3 => {
                            *release_ms = (*release_ms + delta as f32).max(1.0).min(1000.0);
                            true
                        }
                        4 => {
                            *knee_db = (*knee_db + delta as f32 * 0.1).max(0.0).min(12.0);
                            true
                        }
                        5 => {
                            *makeup_gain_db = (*makeup_gain_db + delta as f32 * 0.1).max(-20.0).min(20.0);
                            true
                        }
                        6 => {
                            *mix = (*mix + delta as f32 * 0.01).max(0.0).min(1.0);
                            true
                        }
                        7 => {
                            *auto_makeup = !*auto_makeup;
                            true
                        }
                        8 => {
                            *link_channels = !*link_channels;
                            true
                        }
                        9 => {
                            *sidechain_hpf_hz = (*sidechain_hpf_hz + delta as f32).max(20.0).min(500.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Limiter {
                    threshold_db,
                    release_ms,
                    mix,
                } => {
                    match param_idx {
                        0 => {
                            *threshold_db = (*threshold_db + delta as f32 * 0.1).max(-20.0).min(0.0);
                            true
                        }
                        1 => {
                            *release_ms = (*release_ms + delta as f32).max(1.0).min(500.0);
                            true
                        }
                        2 => {
                            *mix = (*mix + delta as f32 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        _ => false,
                    }
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
                        0 => {
                            *threshold_db = (*threshold_db + delta as f32).max(-80.0).min(0.0);
                            true
                        }
                        1 => {
                            *ratio = (*ratio + delta as f32 * 0.1).max(1.0).min(100.0);
                            true
                        }
                        2 => {
                            *attack_ms = (*attack_ms + delta as f32 * 0.1).max(0.1).min(100.0);
                            true
                        }
                        3 => {
                            *release_ms = (*release_ms + delta as f32).max(1.0).min(1000.0);
                            true
                        }
                        4 => {
                            *mix = (*mix + delta as f32 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        5 => {
                            *link_channels = !*link_channels;
                            true
                        }
                        6 => {
                            *sidechain_hpf_hz = (*sidechain_hpf_hz + delta as f32 * 5.0).max(0.0).min(200.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::LoudnessCompensation {
                    target_lufs,
                    min_gain_db,
                    max_gain_db,
                } => {
                    match param_idx {
                        0 => {
                            *target_lufs = (*target_lufs + delta as f32).max(-40.0).min(0.0);
                            true
                        }
                        1 => {
                            *min_gain_db = (*min_gain_db + delta as f32).max(-20.0).min(0.0);
                            true
                        }
                        2 => {
                            *max_gain_db = (*max_gain_db + delta as f32).max(0.0).min(20.0);
                            true
                        }
                        _ => false,
                    }
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
                                // Frequency
                                filter.frequency = (filter.frequency + delta * 10.0).max(20.0).min(20_000.0);
                                true
                            }
                            1 => {
                                // Q
                                filter.q = (filter.q + delta * 0.1).max(0.1).min(10.0);
                                true
                            }
                            2 => {
                                // Gain
                                filter.gain_db = (filter.gain_db + delta * 0.5).max(-24.0).min(24.0);
                                true
                            }
                            3 => {
                                // Filter type
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
                                    if current_idx == 0 {
                                        types.len() - 1
                                    } else {
                                        current_idx - 1
                                    }
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
                    // sofa_file (param 0) is set via file browser - not adjustable here
                    match param_idx {
                        1 => {
                            *input_channels = ((*input_channels as i64) + delta as i64).max(2).min(16) as usize;
                            true
                        }
                        2 => {
                            *enable_optimization = !*enable_optimization;
                            true
                        }
                        3 => {
                            *externalization = (*externalization + delta as f32 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        4 => {
                            *near_field_strength = (*near_field_strength + delta as f32 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Gain { gain_db } => {
                    if param_idx == 0 {
                        *gain_db = (*gain_db + delta as f32 * 0.1).max(-60.0).min(20.0);
                        true
                    } else {
                        false
                    }
                }
                PluginSettings::Resampler { target_sample_rate } => {
                    if param_idx == 0 {
                        let rates = [44100, 48000, 88200, 96000, 176400, 192000];
                        let current_idx = rates
                            .iter()
                            .position(|&r| r == *target_sample_rate)
                            .unwrap_or(1);
                        let new_idx = if delta > 0.0 {
                            (current_idx + 1) % rates.len()
                        } else {
                            if current_idx == 0 {
                                rates.len() - 1
                            } else {
                                current_idx - 1
                            }
                        };
                        *target_sample_rate = rates[new_idx];
                        true
                    } else {
                        false
                    }
                }
                PluginSettings::Matrix { .. } => {
                    // Matrix has no adjustable parameters for now
                    false
                }
            }
        } else {
            false
        };

        if result && channel_count_changed {
            self.plugin_chain.update_binaural_decoder_channels();
        }

        if result {
            self.needs_plugin_update = true;
        }

        result
    }

    /// Load EQ filters from APO file
    pub fn load_apo_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::{EQFilter, PluginSettings};
        use std::path::Path;

        let path = Path::new(&self.apo_file_input);

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently editing plugin if it's an EQ
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                plugin.settings = PluginSettings::EQ { filters };
                self.needs_plugin_update = true;
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Update SOFA file path for the currently editing binaural decoder plugin
    pub fn load_sofa_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::PluginSettings;

        // Update the currently editing plugin if it's a binaural decoder
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = Some(std::path::PathBuf::from(&self.sofa_file_input));
                self.needs_plugin_update = true;
                Ok(())
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    // Directory autocomplete methods

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
}

// Helper function to get parameter count for a plugin
fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    use sotf_audio_player::PluginSettings;
    match settings {
        PluginSettings::Upmixer { .. } => 14,
        PluginSettings::EQ { filters } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Gain { .. } => 1,
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Gate { .. } => 7, // threshold, ratio, attack, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 3, // threshold, release, mix
        PluginSettings::LoudnessCompensation { .. } => 3, // target_lufs, min_gain, max_gain
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Resampler { .. } => 1, // target_sample_rate
        PluginSettings::Matrix { .. } => 0, // No adjustable params for now
    }
}
