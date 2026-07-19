//! Library state management.
//!
//! Thin wrapper around `LibraryController` from sotf-player, adding GPUI-specific
//! Manager trait and the `library_columns` UI field.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use crate::app::manager::{Manager, ManagerError};
use sotf_audio_player::{
    LibraryController, MetadataEditPreview, MetadataError, MetadataImportCandidate, MetadataPatch,
    MetadataTarget, MusicLibrary,
};

pub use sotf_audio_player::{ChannelFilter, LibrarySortOrder};

/// Library management events (GPUI event system)
#[derive(Debug, Clone)]
pub enum LibraryEvent {
    SetSortOrder(LibrarySortOrder),
    SetFilter(ChannelFilter),
    SetSearchQuery(String),
    ClearSearch,
    CycleFilter,
    NextPage,
    PrevPage,
    SelectNext,
    SelectPrev,
    SelectGridRight(usize),
    SelectGridLeft(usize),
    SelectGridDown(usize),
    SelectGridUp(usize),
    PageDown(usize),
    PageUp(usize),
    Scan,
}

/// Library queries (GPUI event system)
#[derive(Debug, Clone)]
pub enum LibraryQuery {
    ItemCount,
    SelectedAlbum,
    FilteredAlbums,
}

/// Library query responses (GPUI event system)
#[derive(Debug)]
pub enum LibraryResponse {
    Count(usize),
    Album(Option<sotf_audio_player::Album>),
    None,
}

/// Visible keyboard selection within the Home screen's album shelves.
///
/// The shelf identifier keeps repeated albums in separate discovery rows from
/// producing multiple simultaneous selections, while the index remains stable
/// for the currently rendered collapsed or expanded shelf.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomeAlbumSelection {
    pub shelf_id: Option<String>,
    pub album_index: usize,
}

/// Library state — wraps `LibraryController` with GPUI-specific additions.
///
/// Deref/DerefMut to `LibraryController` so all existing field access
/// (`self.library_state.library`, `.sort_order`, `.selected_index`, etc.)
/// continues to work unchanged.
#[derive(Debug)]
pub struct LibraryState {
    ctrl: LibraryController,

    /// Number of columns in grid layout (UI-specific, not in controller)
    pub library_columns: usize,
    /// Keyboard selection for the local or remote Home discovery shelves.
    pub home_album_selection: HomeAlbumSelection,
    /// Bumped when the underlying local album/track data changes.
    content_generation: u64,
}

impl Deref for LibraryState {
    type Target = LibraryController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for LibraryState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl Default for LibraryState {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            ctrl: LibraryController::new(),
            library_columns: 4,
            home_album_selection: HomeAlbumSelection::default(),
            content_generation: 0,
        }
    }

    pub fn with_library(library: MusicLibrary) -> Self {
        Self {
            ctrl: LibraryController::with_library(library),
            library_columns: 4,
            home_album_selection: HomeAlbumSelection::default(),
            content_generation: 0,
        }
    }

    pub fn new_for_test() -> Self {
        Self {
            ctrl: LibraryController::new_for_test(),
            library_columns: 4,
            home_album_selection: HomeAlbumSelection::default(),
            content_generation: 0,
        }
    }

    pub fn content_generation(&self) -> u64 {
        self.content_generation
    }

    pub fn invalidate_cache(&mut self) {
        self.ctrl.invalidate_cache();
        self.bump_content_generation();
    }

    pub fn preview_metadata_edit(
        &self,
        target: MetadataTarget,
        patch: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        self.ctrl.preview_metadata_edit(target, patch)
    }

    pub fn apply_metadata_edit(
        &mut self,
        target: MetadataTarget,
        patch: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let result = self.ctrl.apply_metadata_edit(target, patch);
        if result.is_ok() {
            self.bump_content_generation();
        }
        result
    }

    pub fn import_metadata_candidate(
        &mut self,
        target: MetadataTarget,
        candidate: MetadataImportCandidate,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let result = self.ctrl.import_metadata_candidate(target, candidate);
        if result.is_ok() {
            self.bump_content_generation();
        }
        result
    }

    pub fn scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.ctrl.scan();
        if result.is_ok() {
            self.bump_content_generation();
        }
        result
    }

    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.ctrl.load_from_database();
        if result.is_ok() {
            self.bump_content_generation();
        }
        result
    }

    pub fn clean_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let result = self.ctrl.clean_database();
        if result.as_ref().is_ok_and(|removed| *removed > 0) {
            self.bump_content_generation();
        }
        result
    }

    pub fn clear_library_content(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let result = self.ctrl.clear_library_content();
        if result.is_ok() {
            self.bump_content_generation();
        }
        result
    }

    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        let removed = self.ctrl.remove_directory(index);
        if removed.is_some() {
            self.bump_content_generation();
        }
        removed
    }

    fn bump_content_generation(&mut self) {
        self.content_generation = self.content_generation.wrapping_add(1);
    }
}

impl Manager for LibraryState {
    type State = Self;
    type Event = LibraryEvent;
    type Query = LibraryQuery;
    type Response = LibraryResponse;

    fn handle_event(&mut self, event: Self::Event) -> Result<(), ManagerError> {
        match event {
            LibraryEvent::SetSortOrder(order) => self.set_sort_order(order),
            LibraryEvent::SetFilter(filter) => self.set_filter(filter),
            LibraryEvent::SetSearchQuery(q) => self.set_search_query(q),
            LibraryEvent::ClearSearch => self.clear_search(),
            LibraryEvent::CycleFilter => self.cycle_filter(),
            LibraryEvent::NextPage => self.next_page(),
            LibraryEvent::PrevPage => self.prev_page(),
            LibraryEvent::SelectNext => self.select_next(),
            LibraryEvent::SelectPrev => self.select_prev(),
            LibraryEvent::SelectGridRight(cols) => self.select_grid_right(cols),
            LibraryEvent::SelectGridLeft(cols) => self.select_grid_left(cols),
            LibraryEvent::SelectGridDown(cols) => self.select_grid_down(cols),
            LibraryEvent::SelectGridUp(cols) => self.select_grid_up(cols),
            LibraryEvent::PageDown(size) => self.page_down(size),
            LibraryEvent::PageUp(size) => self.page_up(size),
            LibraryEvent::Scan => self.scan().map_err(|e| ManagerError::from(e.to_string()))?,
        }
        Ok(())
    }

    fn query(&self, query: Self::Query) -> Self::Response {
        match query {
            LibraryQuery::ItemCount => LibraryResponse::Count(self.item_count()),
            LibraryQuery::SelectedAlbum => LibraryResponse::Album(self.selected_album().cloned()),
            LibraryQuery::FilteredAlbums => LibraryResponse::None,
        }
    }

    fn state(&self) -> &Self::State {
        self
    }
}
