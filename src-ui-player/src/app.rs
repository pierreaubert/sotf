use crate::library::{Album, MusicLibrary, Track};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
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
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub album_list_offset: usize,

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub position_secs: f64,

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
            selected_album_index: 0,
            selected_directory_index: 0,
            selected_queue_index: 0,
            album_list_offset: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 1.0,
            position_secs: 0.0,
            should_quit: false,
            needs_rescan: false,
        }
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
        Ok(())
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
