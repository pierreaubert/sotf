use super::PlayerCommand;
use crate::app::{App, InputMode, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_library_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{ChannelFilter, LibrarySortOrder, LibraryViewMode};

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            None
        }
        KeyCode::Char('X') => {
            // Explicitly clear search query
            app.search_query.clear();
            app.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        KeyCode::Char('t') => {
            // Toggle between flat and tree view
            app.toggle_library_view_mode();
            None
        }
        KeyCode::Char('s') => {
            // Cycle through sort orders
            let next_order = match app.library_sort_order {
                LibrarySortOrder::Year => LibrarySortOrder::Genre,
                LibrarySortOrder::Genre => LibrarySortOrder::Artist,
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Tracks,
                LibrarySortOrder::Tracks => LibrarySortOrder::Composer,
                LibrarySortOrder::Composer => LibrarySortOrder::Popularity,
                LibrarySortOrder::Popularity => LibrarySortOrder::Year,
            };
            app.set_library_sort_order(next_order);
            None
        }
        KeyCode::Char('c') => {
            // Cycle through channel filters
            app.cycle_channel_filter();
            None
        }
        KeyCode::Char('1') => {
            // Sort by year
            app.set_library_sort_order(LibrarySortOrder::Year);
            None
        }
        KeyCode::Char('2') => {
            // Sort by genre
            app.set_library_sort_order(LibrarySortOrder::Genre);
            None
        }
        KeyCode::Char('3') => {
            // Sort by artist
            app.set_library_sort_order(LibrarySortOrder::Artist);
            None
        }
        KeyCode::Char('4') => {
            // Sort by album
            app.set_library_sort_order(LibrarySortOrder::Album);
            None
        }
        KeyCode::Char('5') => {
            // Filter: Show all
            app.set_channel_filter(ChannelFilter::All);
            None
        }
        KeyCode::Char('6') => {
            // Filter: Mono only
            app.set_channel_filter(ChannelFilter::Mono);
            None
        }
        KeyCode::Char('7') => {
            // Filter: Stereo only
            app.set_channel_filter(ChannelFilter::Stereo);
            None
        }
        KeyCode::Char('8') => {
            // Filter: Surround only (5.0/5.1)
            app.set_channel_filter(ChannelFilter::Surround);
            None
        }
        KeyCode::Char('9') => {
            // Filter: Mixed channels only
            app.set_channel_filter(ChannelFilter::Mixed);
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_previous_album(),
                LibraryViewMode::TreeView => app.select_previous_tree_item(),
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.select_next_album(),
                LibraryViewMode::TreeView => app.select_next_tree_item(),
            }
            None
        }
        KeyCode::PageUp => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_up_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_up_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::PageDown => {
            match app.library_view_mode {
                LibraryViewMode::Flat => app.page_down_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_down_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Expand artist in tree view
            if app.library_view_mode == LibraryViewMode::TreeView {
                app.toggle_artist_expansion();
            }
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse artist in tree view
            if app.library_view_mode == LibraryViewMode::TreeView {
                app.toggle_artist_expansion();
            }
            None
        }
        KeyCode::Char('a') | KeyCode::Enter => {
            let path = match app.library_view_mode {
                LibraryViewMode::Flat => app.add_album_to_queue(),
                LibraryViewMode::TreeView => app.add_tree_selection_to_queue(),
            };
            path.map(PlayerCommand::Play)
        }
        KeyCode::Char('f') => {
            // Toggle favorite on selected album
            app.toggle_selected_album_favorite();
            None
        }
        KeyCode::Char('F') => {
            // Toggle favorites-only filter
            app.toggle_favorites_filter();
            None
        }
        KeyCode::Char('A') => {
            // Add selected album to the active playlist
            if let Some(active_id) = app.playlist_controller.active_playlist_id() {
                let idx = app.selected_album_index;
                if let Some(album) = app.cached_filtered_albums.get(idx) {
                    let album_clone = album.clone();
                    if let Some(db) = app.library.get_database() {
                        // Find the playlist index for the active playlist's ID
                        let pl_idx = app
                            .playlist_controller
                            .playlists()
                            .iter()
                            .position(|p| p.id == Some(active_id));
                        if let Some(pl_idx) = pl_idx {
                            match app.playlist_controller.add_album_to_playlist(
                                db,
                                pl_idx,
                                &album_clone,
                            ) {
                                Ok(()) => {
                                    app.status_message =
                                        Some(format!("Added '{}' to playlist", album_clone.title))
                                }
                                Err(e) => app.status_message = Some(format!("Error: {}", e)),
                            }
                        }
                    }
                }
            } else {
                app.status_message = Some("Open a playlist first (Y screen)".to_string());
            }
            None
        }
        KeyCode::Char('q') => {
            app.current_screen = Screen::Queue;
            None
        }
        _ => None,
    }
}
