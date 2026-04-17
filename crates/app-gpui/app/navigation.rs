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
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                (self.queue_state.selected_index + 1) % self.queue_state.len();
        }
    }

    pub fn select_previous_queue_item(&mut self) {
        if !self.queue_state.is_empty() {
            if self.queue_state.selected_index == 0 {
                self.queue_state.selected_index = self.queue_state.len() - 1;
            } else {
                self.queue_state.selected_index -= 1;
            }
        }
    }

    pub fn page_down_queue(&mut self, page_size: usize) {
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                (self.queue_state.selected_index + page_size).min(self.queue_state.len() - 1);
        }
    }

    pub fn page_up_queue(&mut self, page_size: usize) {
        if !self.queue_state.is_empty() {
            self.queue_state.selected_index =
                self.queue_state.selected_index.saturating_sub(page_size);
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
}
