use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::InputMode;
use sotf_audio_player_gpui::app::actions::ToggleSearch;
use std::error::Error;

pub struct LibraryPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> LibraryPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn open_search(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver.cx.dispatch_action(ToggleSearch);
        self.driver.run_until_parked();
        Ok(())
    }

    pub fn is_search_focused(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.input_mode == InputMode::Search)
    }

    pub fn type_search_query(&mut self, query: &str) {
        // We type one char at a time to ensure events are processed
        self.driver.simulate_keystrokes(query);
        self.driver.run_until_parked();
    }

    pub fn get_search_query(&mut self) -> String {
        self.driver
            .read_app(|app| app.library_state.search_query.clone())
    }

    pub fn verify_filtered_results_contain(&mut self, text: &str) -> Result<(), Box<dyn Error>> {
        let text = text.to_lowercase();
        self.driver.read_app(|app| {
            let albums = app.library_state.filtered_albums();
            // Verify at least one album matches or list is empty if none match (but user expects results)
            // User said "show only vivaldi related album".
            // So we check that ALL displayed albums match "vivaldi" (in artist or title).
            if albums.is_empty() {
                return Err(format!("No albums found matching query '{}'", text).into());
            }

            for album in albums {
                let matches = album.title.to_lowercase().contains(&text)
                    || album.artist().to_lowercase().contains(&text);
                if !matches {
                    return Err(format!(
                        "Album '{}' by '{}' does not match query '{}'",
                        album.title,
                        album.artist(),
                        text
                    )
                    .into());
                }
            }
            Ok(())
        })
    }

    pub fn get_filtered_albums_count(&mut self) -> usize {
        self.driver
            .read_app(|app| app.library_state.filtered_albums().len())
    }
}
