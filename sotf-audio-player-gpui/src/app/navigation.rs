//! Navigation and selection methods.
//!
//! Contains methods for navigating and selecting items in various lists.

use super::state::App;

impl App {
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
        let current_page_albums = self.get_paginated_albums();
        if current_page_albums.is_empty() {
            return;
        }

        // Try to move selection within current page
        let next_index = self.selected_album_index + page_size;
        if next_index < current_page_albums.len() {
            // Selection stays on current page
            self.selected_album_index = next_index;
        } else {
            // Need to move to next page
            let total_pages = self.get_total_pages();
            if self.library_page + 1 < total_pages {
                self.library_page += 1;
                // Wrap to first item of next page
                self.selected_album_index = 0;
            } else if self.selected_album_index < current_page_albums.len() - 1 {
                // Stay on last page, move to last item
                self.selected_album_index = current_page_albums.len() - 1;
            }
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let current_page_albums = self.get_paginated_albums();
        if current_page_albums.is_empty() {
            return;
        }

        // Try to move selection within current page
        if self.selected_album_index >= page_size {
            // Selection stays on current page
            self.selected_album_index -= page_size;
        } else if self.library_page > 0 {
            // Need to move to previous page
            self.library_page -= 1;
            // Move to last item of previous page
            let new_page_albums = self.get_paginated_albums();
            if !new_page_albums.is_empty() {
                self.selected_album_index = new_page_albums.len() - 1;
            }
        } else if self.selected_album_index > 0 {
            // Stay on first page, move to first item
            self.selected_album_index = 0;
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

        // Calculate grid dimensions (150px cards + 16px gap = 166px per item)
        // Assuming standard window width of ~1200px gives us ~7 columns
        let grid_columns = 7;

        if self.selected_album_index % grid_columns > 0 {
            self.selected_album_index -= 1;
        } else if self.library_page > 0 {
            // Move to end of previous page
            self.library_page -= 1;
            self.selected_album_index = (self.library_items_per_page / grid_columns) * grid_columns
                - grid_columns
                + (grid_columns - 1);
        }
    }

    /// Navigate grid right
    pub fn select_grid_right(&mut self) {
        let albums = self.get_paginated_albums();
        if albums.is_empty() {
            return;
        }

        let grid_columns = 7;
        let max_index = albums.len() - 1;

        if self.selected_album_index % grid_columns < grid_columns - 1
            && self.selected_album_index < max_index
        {
            self.selected_album_index += 1;
        } else if self.selected_album_index < max_index {
            // Move to start of next page
            self.library_page += 1;
            self.selected_album_index = 0;
        }
    }

    /// Navigate grid up
    pub fn select_grid_up(&mut self) {
        let grid_columns = 7;

        if self.selected_album_index >= grid_columns {
            self.selected_album_index -= grid_columns;
        } else if self.library_page > 0 {
            // Move to end of previous page
            self.library_page -= 1;
            let albums = self.get_paginated_albums();
            if !albums.is_empty() {
                let last_row_start = ((albums.len() - 1) / grid_columns) * grid_columns;
                self.selected_album_index = (self.selected_album_index % grid_columns)
                    .min(albums.len() - 1 - last_row_start)
                    + last_row_start;
            }
        }
    }

    /// Navigate grid down
    pub fn select_grid_down(&mut self) {
        let albums = self.get_paginated_albums();
        if albums.is_empty() {
            return;
        }

        let grid_columns = 7;
        let next_row_index = self.selected_album_index + grid_columns;
        let max_index = albums.len() - 1;

        if next_row_index <= max_index {
            self.selected_album_index = next_row_index;
        } else if self.selected_album_index < max_index {
            // Stay on current position if not at end of page
            self.selected_album_index = max_index;
        } else {
            // Move to next page, same column
            let total_pages = self.get_total_pages();
            if self.library_page + 1 < total_pages {
                self.library_page += 1;
                let col = self.selected_album_index % grid_columns;
                self.selected_album_index = col.min(self.get_paginated_albums().len() - 1);
            }
        }
    }
}
