use crate::library::{Album, MusicLibrary, Track};
use crate::plugins::{PluginChain, PluginType};
use sotf_audio::devices::AudioDevice;
use sotf_audio::plugins::LoudnessInfo;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,      // Original list view
    TreeView,  // Hierarchical artist → albums
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
}

#[derive(Debug)]
pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueItem>,
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

    // Audio devices
    pub output_devices: Vec<AudioDevice>,
    pub selected_output_device_index: usize,

    // Flags
    pub should_quit: bool,
    pub needs_rescan: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            library: MusicLibrary::new(),
            queue: Vec::new(),
            current_screen: Screen::Library,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            directory_input: String::new(),
            plugin_file_input: String::new(),
            selected_album_index: 0,
            selected_directory_index: 0,
            selected_queue_index: 0,
            selected_plugin_index: 0,
            album_list_offset: 0,
            status_message: None,
            library_view_mode: LibraryViewMode::Flat,
            artist_tree: Vec::new(),
            selected_tree_index: 0,
            plugin_chain: PluginChain::new(),
            needs_plugin_update: false,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            loudness_info: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            should_quit: false,
            needs_rescan: false,
        }
    }

    pub fn load_output_devices(&mut self) {
        // Load available output devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices() {
            if let Some(output_devices) = devices_map.get("output") {
                self.output_devices = output_devices.clone();
                // Find the default device
                if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                    self.selected_output_device_index = default_idx;
                }
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

    pub fn filtered_albums(&self) -> Vec<&Album> {
        if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
        }
    }

    pub fn add_album_to_queue(&mut self) {
        let albums = self.filtered_albums();
        if let Some(album) = albums.get(self.selected_album_index) {
            self.queue.push(QueueItem::new((*album).clone()));
        }
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
            // Adjust current queue index if needed
            if let Some(current_idx) = self.current_queue_index {
                if current_idx == index {
                    self.current_queue_index = None;
                    self.is_playing = false;
                } else if current_idx > index {
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
        self.current_queue_index = None;
        self.selected_queue_index = 0;
        self.is_playing = false;
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

    pub fn select_next_directory(&mut self) {
        if !self.library.directories.is_empty() {
            self.selected_directory_index =
                (self.selected_directory_index + 1) % self.library.directories.len();
        }
    }

    pub fn select_previous_directory(&mut self) {
        if !self.library.directories.is_empty() {
            if self.selected_directory_index == 0 {
                self.selected_directory_index = self.library.directories.len() - 1;
            } else {
                self.selected_directory_index -= 1;
            }
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
        self.library.add_directory(path);
        self.needs_rescan = true;
    }

    pub fn remove_selected_directory(&mut self) {
        if self.library.remove_directory(self.selected_directory_index).is_some() {
            if self.selected_directory_index >= self.library.directories.len()
                && self.selected_directory_index > 0
            {
                self.selected_directory_index = self.library.directories.len() - 1;
            }
            self.needs_rescan = true;
        }
    }

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.scan()?;
        self.needs_rescan = false;
        self.selected_album_index = 0;
        self.album_list_offset = 0;
        self.rebuild_artist_tree();
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
    pub fn add_tree_selection_to_queue(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

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
                                }
                            }
                            return;
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
                    }
                }
            }
        }
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
                } else {
                    // Move to next album in queue
                    if idx + 1 < self.queue.len() {
                        self.current_queue_index = Some(idx + 1);
                        return self.current_track_path();
                    }
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

    pub fn increase_volume(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    pub fn decrease_volume(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }

    // Plugin management
    pub fn add_plugin(&mut self, plugin_type: &PluginType) {
        self.plugin_chain.add_plugin(plugin_type);
        self.needs_plugin_update = true;
    }

    pub fn remove_plugin(&mut self, index: usize) {
        self.plugin_chain.remove_plugin(index);
        if self.selected_plugin_index >= self.plugin_chain.len() && self.selected_plugin_index > 0 {
            self.selected_plugin_index = self.plugin_chain.len() - 1;
        }
        self.needs_plugin_update = true;
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_chain.toggle_plugin(index);
        self.needs_plugin_update = true;
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if index > 0 {
            self.plugin_chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            self.needs_plugin_update = true;
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if index < self.plugin_chain.len() - 1 {
            self.plugin_chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            self.needs_plugin_update = true;
        }
    }

    pub fn select_next_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            self.selected_plugin_index =
                (self.selected_plugin_index + 1) % self.plugin_chain.len();
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

    pub fn get_editing_plugin(&self) -> Option<&crate::plugins::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin(idx))
    }

    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut crate::plugins::Plugin> {
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
        use crate::plugins::PluginSettings;

        let param_idx = self.plugin_param_selection;

        if let Some(plugin) = self.get_editing_plugin_mut() {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    center_level_db,
                    lfe_level_db,
                    surround_delay_ms,
                } => {
                    match param_idx {
                        0 => *center_level_db = (*center_level_db + delta).clamp(-20.0, 20.0),
                        1 => *lfe_level_db = (*lfe_level_db + delta).clamp(-20.0, 20.0),
                        2 => *surround_delay_ms = (*surround_delay_ms + delta).clamp(0.0, 100.0),
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
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                        1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                        2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                        3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                        4 => *knee_db = (*knee_db + delta * 0.1).clamp(0.0, 12.0),
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Limiter {
                    threshold_db,
                    release_ms,
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta * 0.1).clamp(-20.0, 0.0),
                        1 => *release_ms = (*release_ms + delta).clamp(1.0, 500.0),
                        _ => return false,
                    }
                    true
                }
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                } => {
                    match param_idx {
                        0 => *threshold_db = (*threshold_db + delta).clamp(-80.0, 0.0),
                        1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 100.0),
                        2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                        3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
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
                PluginSettings::EQ { .. } => {
                    // EQ is more complex - we'll implement basic support for now
                    // TODO: Implement full EQ editing with filter type selection
                    false
                }
            }
        } else {
            false
        }
    }

    /// Save plugin chain to file
    pub fn save_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        let path = PathBuf::from(&self.plugin_file_input);
        match self.plugin_chain.save_to_file(&path) {
            Ok(_) => {
                self.status_message = Some(format!("Saved plugin chain to {}", path.display()));
                log::info!("Saved plugin chain to {}", path.display());
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

        let path = PathBuf::from(&self.plugin_file_input);
        match self.plugin_chain.load_from_file(&path) {
            Ok(_) => {
                self.status_message = Some(format!("Loaded plugin chain from {}", path.display()));
                self.needs_plugin_update = true;
                log::info!("Loaded plugin chain from {}", path.display());
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading: {}", e));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }
}

// Helper function to get parameter count for a plugin
fn get_param_count(settings: &crate::plugins::PluginSettings) -> usize {
    use crate::plugins::PluginSettings;
    match settings {
        PluginSettings::EQ { filters } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Upmixer { .. } => 3, // center_level, lfe_level, surround_delay
        PluginSettings::Compressor { .. } => 5, // threshold, ratio, attack, release, knee
        PluginSettings::Limiter { .. } => 2, // threshold, release
        PluginSettings::Gate { .. } => 4, // threshold, ratio, attack, release
        PluginSettings::LoudnessCompensation { .. } => 3, // target_lufs, min_gain, max_gain
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
