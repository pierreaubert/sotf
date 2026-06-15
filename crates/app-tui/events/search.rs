use super::PlayerCommand;
use crate::app::{App, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_search_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::LibraryViewMode;

    const PAGE_SIZE: usize = 20;

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            // Don't clear query, just exit mode to persist search
            None
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.library_view.selected_album_index = 0;
            None
        }
        // Allow navigation while searching
        KeyCode::Up => {
            match app.library_view.mode {
                LibraryViewMode::Flat => app.select_previous_album(),
                LibraryViewMode::TreeView => app.select_previous_tree_item(),
            }
            None
        }
        KeyCode::Down => {
            match app.library_view.mode {
                LibraryViewMode::Flat => app.select_next_album(),
                LibraryViewMode::TreeView => app.select_next_tree_item(),
            }
            None
        }
        KeyCode::PageUp => {
            match app.library_view.mode {
                LibraryViewMode::Flat => app.page_up_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_up_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::PageDown => {
            match app.library_view.mode {
                LibraryViewMode::Flat => app.page_down_albums(PAGE_SIZE),
                LibraryViewMode::TreeView => app.page_down_tree(PAGE_SIZE),
            }
            None
        }
        KeyCode::Char(c) => {
            app.library_view.search_query.push(c);
            app.library_view.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        KeyCode::Backspace => {
            app.library_view.search_query.pop();
            app.library_view.selected_album_index = 0;
            app.request_filter_update();
            None
        }
        _ => None,
    }
}
