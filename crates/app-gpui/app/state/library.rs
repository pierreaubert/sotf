//! Library state management.
//!
//! Thin wrapper around `LibraryController` from sotf-player, adding GPUI-specific
//! Manager trait and the `library_columns` UI field.

use std::ops::{Deref, DerefMut};

use crate::app::manager::{Manager, ManagerError};
use sotf_audio_player::{LibraryController, MusicLibrary};

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
        }
    }

    pub fn with_library(library: MusicLibrary) -> Self {
        Self {
            ctrl: LibraryController::with_library(library),
            library_columns: 4,
        }
    }

    pub fn new_for_test() -> Self {
        Self {
            ctrl: LibraryController::new_for_test(),
            library_columns: 4,
        }
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
            LibraryQuery::SelectedAlbum => {
                LibraryResponse::Album(self.selected_album().cloned())
            }
            LibraryQuery::FilteredAlbums => LibraryResponse::None,
        }
    }

    fn state(&self) -> &Self::State {
        self
    }
}
