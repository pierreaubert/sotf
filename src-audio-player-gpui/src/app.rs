use sotf_audio_player::{Album, LoudnessInfo, MusicLibrary, Player, PluginChain, PluginType, SpectrumInfo, Track};
use sotf_audio::devices::AudioDevice;
use std::path::PathBuf;
use std::sync::Arc;

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
    Help,
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
            selected_directory_index: 0,
            selected_queue_index: 0,
            album_list_offset: 0,
            status_message: None,
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

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_in_progress = true;
        self.library.scan()?;
        self.rebuild_artist_tree();
        self.scan_in_progress = false;
        Ok(())
    }

    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        if !self.library.directories.iter().any(|d| d.path == path) {
            self.library.add_directory(path);
        }
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
}
