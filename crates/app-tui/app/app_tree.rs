use super::app_impl::App;
use super::types::{LibraryViewMode, QueueEntry, QueueItem, TreeItem};
use std::path::PathBuf;

impl App {
    /// Get the flattened tree items for rendering (returns artist names or album indices)
    /// Respects search query and channel filter
    pub fn get_tree_items(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();
        let filtered_indices = self.filtered_album_indices();

        for artist_node in &self.artist_tree {
            // Filter albums for this artist
            let visible_albums: Vec<usize> = artist_node
                .album_indices
                .iter()
                .copied()
                .filter(|idx| filtered_indices.contains(idx))
                .collect();

            // Skip artists with no visible albums
            if visible_albums.is_empty() {
                continue;
            }

            // Single album/track: show directly without expand/collapse
            if visible_albums.len() == 1 {
                items.push(TreeItem::Album { index: visible_albums[0] });
                continue;
            }

            items.push(TreeItem::Artist {
                name: artist_node.artist.clone(),
                expanded: artist_node.expanded,
            });

            if artist_node.expanded {
                for album_idx in visible_albums {
                    items.push(TreeItem::Album { index: album_idx });
                }
            }
        }

        items
    }

    /// Select next item in tree view
    pub fn select_next_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index = (self.selected_tree_index + 1) % tree_items.len();
        }
    }

    /// Select previous item in tree view
    pub fn select_previous_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            if self.selected_tree_index == 0 {
                self.selected_tree_index = tree_items.len() - 1;
            } else {
                self.selected_tree_index -= 1;
            }
        }
    }

    /// Add the selected item (artist or album) to queue from tree view
    pub fn add_tree_selection_to_queue(&mut self) -> Option<PathBuf> {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return None;
        }

        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;
        let tree_items = self.get_tree_items();
        let filtered_indices = self.filtered_album_indices();

        if let Some(item) = tree_items.get(self.selected_tree_index) {
            match item {
                TreeItem::Artist { name, .. } => {
                    // Find this artist in the tree and add their filtered albums
                    for artist_node in &self.artist_tree {
                        if artist_node.artist == *name {
                            // Add only albums that pass the current filter
                            for &album_idx in &artist_node.album_indices {
                                if filtered_indices.contains(&album_idx) {
                                    if let Some(album) = self.library.albums.get(album_idx) {
                                        self.queue
                                            .push(QueueEntry::new(QueueItem::new(album.clone())));
                                    }
                                }
                            }
                            // Auto-play if queue was empty OR if nothing was playing
                            if was_empty || was_not_playing {
                                return self.start_queue();
                            }
                            return None;
                        }
                    }
                }
                TreeItem::Album { index } => {
                    // Add single album
                    if let Some(album) = self.library.albums.get(*index) {
                        self.queue
                            .push(QueueEntry::new(QueueItem::new(album.clone())));

                        // Auto-play if queue was empty OR if nothing was playing
                        if was_empty || was_not_playing {
                            return self.start_queue();
                        }
                    }
                }
            }
        }
        None
    }

}
