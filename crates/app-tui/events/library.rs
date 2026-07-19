use super::PlayerCommand;
use crate::app::{App, InputMode, Screen};
use crate::ui::keybinding_catalog::{LibraryCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_library_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match resolve_command(TuiKeyContext::Library, key) {
        Some(TuiCommand::Library(command)) => handle_documented_command(app, key, command),
        Some(command) => unreachable!("non-library command in Library context: {command:?}"),
        None => handle_undocumented_command(app, key),
    }
}

fn handle_documented_command(
    app: &mut App,
    key: KeyEvent,
    command: LibraryCommand,
) -> Option<PlayerCommand> {
    use crate::app::{ChannelFilter, LibrarySortOrder, LibraryViewMode};

    const PAGE_SIZE: usize = 20;

    match command {
        LibraryCommand::Navigate => {
            let previous = matches!(key.code, KeyCode::Up | KeyCode::Char('k'));
            match (app.library_view.mode, previous) {
                (LibraryViewMode::Flat, true) => app.select_previous_album(),
                (LibraryViewMode::Flat, false) => app.select_next_album(),
                (LibraryViewMode::TreeView, true) => app.select_previous_tree_item(),
                (LibraryViewMode::TreeView, false) => app.select_next_tree_item(),
            }
            None
        }
        LibraryCommand::Page => {
            let previous = key.code == KeyCode::PageUp;
            match (app.library_view.mode, previous) {
                (LibraryViewMode::Flat, true) => app.page_up_albums(PAGE_SIZE),
                (LibraryViewMode::Flat, false) => app.page_down_albums(PAGE_SIZE),
                (LibraryViewMode::TreeView, true) => app.page_up_tree(PAGE_SIZE),
                (LibraryViewMode::TreeView, false) => app.page_down_tree(PAGE_SIZE),
            }
            None
        }
        LibraryCommand::Search => {
            app.input_mode = InputMode::Search;
            None
        }
        LibraryCommand::ToggleTree => {
            app.toggle_library_view_mode();
            None
        }
        LibraryCommand::ToggleArtist => {
            if app.library_view.mode == LibraryViewMode::TreeView {
                app.toggle_artist_expansion();
            }
            None
        }
        LibraryCommand::Sort => {
            let order = match key.code {
                KeyCode::Char('s') => match app.library_view.sort_order {
                    LibrarySortOrder::Year => LibrarySortOrder::Genre,
                    LibrarySortOrder::Genre => LibrarySortOrder::Artist,
                    LibrarySortOrder::Artist => LibrarySortOrder::Album,
                    LibrarySortOrder::Album => LibrarySortOrder::Tracks,
                    LibrarySortOrder::Tracks => LibrarySortOrder::Composer,
                    LibrarySortOrder::Composer => LibrarySortOrder::Popularity,
                    LibrarySortOrder::Popularity => LibrarySortOrder::Year,
                },
                KeyCode::Char('1') => LibrarySortOrder::Year,
                KeyCode::Char('2') => LibrarySortOrder::Genre,
                KeyCode::Char('3') => LibrarySortOrder::Artist,
                KeyCode::Char('4') => LibrarySortOrder::Album,
                _ => unreachable!("non-sort chord resolved as LibrarySort: {key:?}"),
            };
            app.set_library_sort_order(order);
            None
        }
        LibraryCommand::Filter => {
            match key.code {
                KeyCode::Char('c') => app.cycle_channel_filter(),
                KeyCode::Char('5') => app.set_channel_filter(ChannelFilter::All),
                KeyCode::Char('6') => app.set_channel_filter(ChannelFilter::Mono),
                KeyCode::Char('7') => app.set_channel_filter(ChannelFilter::Stereo),
                KeyCode::Char('8') => app.set_channel_filter(ChannelFilter::Surround),
                KeyCode::Char('9') => app.set_channel_filter(ChannelFilter::Mixed),
                _ => unreachable!("non-filter chord resolved as LibraryFilter: {key:?}"),
            }
            None
        }
        LibraryCommand::AddToQueue => {
            let result = match app.library_view.mode {
                LibraryViewMode::Flat => app.add_album_to_queue(),
                LibraryViewMode::TreeView => Ok(app.add_tree_selection_to_queue()),
            };
            match result {
                Ok(Some(source)) => Some(PlayerCommand::Play(source)),
                Err(error) => {
                    app.ui.error_message = Some(error);
                    app.enter_overlay_mode(InputMode::ShowError);
                    None
                }
                _ => None,
            }
        }
        LibraryCommand::OpenQueue => {
            app.current_screen = Screen::Queue;
            None
        }
    }
}

fn handle_undocumented_command(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Char('X') => {
            app.library_view.search_query.clear();
            app.library_view.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        KeyCode::Char('f') => {
            app.toggle_selected_album_favorite();
            None
        }
        KeyCode::Char('F') => {
            app.toggle_favorites_filter();
            None
        }
        KeyCode::Char('m') => {
            if let Some(album) = app
                .library_view
                .cached_filtered_albums
                .get(app.library_view.selected_album_index)
            {
                match crate::app::MetadataEditorState::for_album(album) {
                    Ok(editor) => {
                        app.modal.metadata_editor = Some(editor);
                        app.input_mode = InputMode::MetadataEditor;
                    }
                    Err(error) => app.ui.status_message = Some(format!("Metadata: {error}")),
                }
            }
            None
        }
        KeyCode::Char('A') => {
            if let Some(active_id) = app.playlists.controller.active_playlist_id() {
                let index = app.library_view.selected_album_index;
                if let Some(album) = app.library_view.cached_filtered_albums.get(index) {
                    let album = album.clone();
                    if let Some(database) = app.library.get_database()
                        && let Some(playlist_index) = app
                            .playlists
                            .controller
                            .playlists()
                            .iter()
                            .position(|playlist| playlist.id == Some(active_id))
                    {
                        match app.playlists.controller.add_album_to_playlist(
                            database,
                            playlist_index,
                            &album,
                        ) {
                            Ok(()) => {
                                app.ui.status_message =
                                    Some(format!("Added '{}' to playlist", album.title))
                            }
                            Err(error) => app.ui.status_message = Some(format!("Error: {error}")),
                        }
                    }
                }
            } else {
                app.ui.status_message = Some("Open a playlist first (Y screen)".to_string());
            }
            None
        }
        _ => None,
    }
}
