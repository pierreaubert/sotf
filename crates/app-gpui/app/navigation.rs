//! Navigation and selection methods.
//!
//! Contains methods for navigating and selecting items in various lists.

use super::state::App;

impl App {
    pub fn select_next_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            self.library_state.selected_index =
                (self.library_state.selected_index + 1) % albums.len();
        }
    }

    pub fn select_previous_album(&mut self) {
        let albums = self.filtered_albums();
        if !albums.is_empty() {
            if self.library_state.selected_index == 0 {
                self.library_state.selected_index = albums.len() - 1;
            } else {
                self.library_state.selected_index -= 1;
            }
        }
    }

    pub fn page_down_albums(&mut self, page_size: usize) {
        let current_page_albums = self.get_paginated_albums();
        if current_page_albums.is_empty() {
            return;
        }

        // Move selection down by page size
        let next_index = self.library_state.selected_index + page_size;
        if next_index < current_page_albums.len() {
            self.library_state.selected_index = next_index;
        } else {
            // Move to last item
            self.library_state.selected_index = current_page_albums.len() - 1;
            // Trigger load more if at end
            self.load_more_albums();
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let current_page_albums = self.get_paginated_albums();
        if current_page_albums.is_empty() {
            return;
        }

        // Move selection up by page size
        if self.library_state.selected_index >= page_size {
            self.library_state.selected_index -= page_size;
        } else {
            // Move to first item
            self.library_state.selected_index = 0;
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

    /// Navigate grid left
    pub fn select_grid_left(&mut self) {
        let albums = self.get_paginated_albums();
        if albums.is_empty() {
            return;
        }

        if self.library_state.selected_index > 0 {
            self.library_state.selected_index -= 1;
        }
    }

    /// Navigate grid right
    pub fn select_grid_right(&mut self) {
        let albums = self.get_paginated_albums();
        if albums.is_empty() {
            return;
        }

        if self.library_state.selected_index < albums.len() - 1 {
            self.library_state.selected_index += 1;
        } else {
            // Trigger load more
            self.load_more_albums();
        }
    }

    /// Navigate grid up
    pub fn select_grid_up(&mut self) {
        let grid_columns = self.library_state.library_columns.max(1);

        if self.library_state.selected_index >= grid_columns {
            self.library_state.selected_index -= grid_columns;
        }
    }

    /// Navigate grid down
    pub fn select_grid_down(&mut self) {
        let albums = self.get_paginated_albums();
        if albums.is_empty() {
            return;
        }

        let grid_columns = self.library_state.library_columns.max(1);
        let next_row_index = self.library_state.selected_index + grid_columns;
        let max_index = albums.len() - 1;

        if next_row_index <= max_index {
            self.library_state.selected_index = next_row_index;
        } else {
            // Trigger load more
            self.load_more_albums();
            // If we loaded more, try to move down again
            let albums = self.get_paginated_albums();
            if self.library_state.selected_index + grid_columns < albums.len() {
                self.library_state.selected_index += grid_columns;
            } else {
                self.library_state.selected_index = albums.len() - 1;
            }
        }
    }
}
