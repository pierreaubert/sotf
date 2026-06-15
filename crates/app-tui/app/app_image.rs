#[cfg(not(target_os = "windows"))]
mod inner {
    use crate::app::app_impl::App;
    use std::path::PathBuf;

    impl App {
        /// Find and load all image files in the currently playing album's directory
        pub fn load_album_images(&mut self) {
            self.library_view.album_images.clear();
            self.library_view.selected_image_index = 0;
            self.library_view.image_protocol = None;
            self.library_view.image_protocol_path = None;

            // Initialize image picker if not already done.
            // macOS Terminal.app doesn't support graphics protocols (Kitty/iTerm2)
            // and the stdio query leaks escape sequences onto the screen.
            // Use halfblocks directly for terminals that don't support graphics.
            if self.library_view.image_picker.is_none() {
                let use_halfblocks = std::env::var("TERM_PROGRAM")
                    .map(|tp| tp == "Apple_Terminal")
                    .unwrap_or(false);

                if use_halfblocks {
                    log::info!("Terminal.app detected, using halfblocks for album art");
                    self.library_view.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
                } else {
                    match ratatui_image::picker::Picker::from_query_stdio() {
                        Ok(picker) => {
                            self.library_view.image_picker = Some(picker);
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to query terminal for font size: {}, using halfblocks fallback",
                                e
                            );
                            self.library_view.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
                        }
                    }
                }
            }

            // Get the currently playing album
            if let Some(queue_index) = self.playback.current_queue_index
                && let Some(entry) = self.queue.get(queue_index)
                && let Some(first_track) = entry.item.album.tracks.first()
                && let Some(parent_dir) = first_track.path.parent()
            {
                // Find all image files in the directory
                if let Ok(entries) = std::fs::read_dir(parent_dir) {
                    for entry in entries.flatten() {
                        if let Ok(path) = entry.path().canonicalize()
                            && let Some(ext) = path.extension()
                        {
                            let ext_lower = ext.to_string_lossy().to_lowercase();
                            if matches!(
                                ext_lower.as_str(),
                                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                            ) {
                                self.library_view.album_images.push(path);
                            }
                        }
                    }
                }
                // Sort images for consistent order
                self.library_view.album_images.sort();
            }
        }

        /// Cycle to the next image in the album directory
        pub fn next_album_image(&mut self) {
            if !self.library_view.album_images.is_empty() {
                self.library_view.selected_image_index =
                    (self.library_view.selected_image_index + 1) % self.library_view.album_images.len();
                self.library_view.image_protocol = None;
                self.library_view.image_protocol_path = None;
            }
        }

        /// Cycle to the previous image in the album directory
        pub fn prev_album_image(&mut self) {
            if !self.library_view.album_images.is_empty() {
                if self.library_view.selected_image_index == 0 {
                    self.library_view.selected_image_index = self.library_view.album_images.len() - 1;
                } else {
                    self.library_view.selected_image_index -= 1;
                }
                self.library_view.image_protocol = None;
                self.library_view.image_protocol_path = None;
            }
        }

        /// Get the currently selected album image path
        pub fn get_current_album_image(&self) -> Option<&PathBuf> {
            self.library_view.album_images.get(self.library_view.selected_image_index)
        }
    }
}
