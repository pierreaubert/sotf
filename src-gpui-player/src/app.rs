use crate::library::{Album, MusicLibrary, Track};
use crate::plugins::{PluginChain, PluginType};
use crate::player::Player;
use sotf_audio::devices::AudioDevice;
use sotf_audio::plugins::{LoudnessInfo, SpectrumInfo};
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
    pub player: Arc<parking_lot::Mutex<crate::player::Player>>,
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
        if self.search_query.is_empty() {
            self.library.albums.iter().collect()
        } else {
            self.library.search_albums(&self.search_query)
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

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.expanded_queue_items.clear();
        self.current_queue_index = None;
        self.selected_queue_index = 0;
        self.is_playing = false;
    }

    // Simplified methods for GPUI version
    pub fn rebuild_artist_tree(&mut self) {
        // TODO: Implement tree building logic if needed
    }

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_in_progress = true;
        self.library.scan_directories()?;
        self.library.save_to_database()?;
        self.rebuild_artist_tree();
        self.scan_in_progress = false;
        Ok(())
    }

    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        if !self.library.directories.contains(&path) {
            self.library.directories.push(path);
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
            // TODO: Load the preset file
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
}
