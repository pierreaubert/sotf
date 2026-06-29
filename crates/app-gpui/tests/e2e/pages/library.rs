use crate::driver::AppDriver;
use gpui::{Modifiers, MouseButton};
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

    pub fn click_sidebar_search(&mut self) -> Result<(), Box<dyn Error>> {
        let bounds = self
            .driver
            .cx
            .debug_bounds("nav-search")
            .ok_or("sidebar search bounds should be available")?;
        let center = bounds.center();
        self.driver
            .cx
            .simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        self.driver
            .cx
            .simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        self.driver.run_until_parked();
        Ok(())
    }

    pub fn is_search_focused(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.input_mode == InputMode::Search)
    }

    pub fn focus_player_root(&mut self) {
        self.driver
            .view
            .update(self.driver.cx, |view, window, cx| {
                view.focus_handle.focus(window, cx);
            })
            .unwrap();
        self.driver.run_until_parked();
    }

    pub fn type_search_query(&mut self, query: &str) {
        self.driver.simulate_keystrokes(query);
        self.driver.run_until_parked();
    }

    pub fn type_search_query_one_char_at_a_time(&mut self, query: &str) {
        for ch in query.chars() {
            self.driver.simulate_keystrokes(&ch.to_string());
            self.driver.run_until_parked();
        }
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

    pub fn select_album(&mut self, index: usize) {
        self.driver.update_app(|app, _| {
            app.library_state.selected_index = index;
        });
        self.driver.run_until_parked();
    }

    pub fn get_selected_index(&mut self) -> usize {
        self.driver.read_app(|app| app.library_state.selected_index)
    }
}
