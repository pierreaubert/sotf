use crate::driver::AppDriver;
use gpui::{Modifiers, MouseButton};
use sotf_audio_player_gpui::app::InputMode;
use sotf_audio_player_gpui::app::actions::ToggleSearch;
use sotf_audio_player_gpui::app::state::library::{ChannelFilter, LibrarySortOrder};
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

    pub fn click_library_search_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-search", "library search tab")
    }

    pub fn click_library_filter_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-filter", "library filter tab")
    }

    pub fn click_library_year_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-year", "library year tab")
    }

    pub fn click_library_genre_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-genre", "library genre tab")
    }

    pub fn click_library_artist_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-artist", "library artist tab")
    }

    pub fn click_library_album_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-album", "library album tab")
    }

    pub fn click_library_tracks_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-tracks", "library tracks tab")
    }

    pub fn click_library_composer_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.click_debug_element("tab-composer", "library composer tab")
    }

    pub fn click_channel_filter_button(
        &mut self,
        selector: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        self.click_debug_element(selector, "channel filter button")
    }

    fn click_debug_element(
        &mut self,
        selector: &'static str,
        description: &str,
    ) -> Result<(), Box<dyn Error>> {
        let bounds = self
            .driver
            .cx
            .debug_bounds(selector)
            .ok_or_else(|| format!("{} bounds should be available", description))?;
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

    pub fn double_click_album(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        let selector = match index {
            0 => "library-album-wrapper-0",
            _ => return Err(format!("album wrapper {} selector is not registered", index).into()),
        };
        let bounds = self
            .driver
            .cx
            .debug_bounds(selector)
            .ok_or_else(|| format!("album wrapper {} bounds should be available", index))?;
        let center = bounds.center();
        for _ in 0..2 {
            self.driver
                .cx
                .simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
            self.driver
                .cx
                .simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
            self.driver.run_until_parked();
        }
        Ok(())
    }

    pub fn click_search_input(&mut self) -> Result<(), Box<dyn Error>> {
        let bounds = self
            .driver
            .cx
            .debug_bounds("search-input")
            .ok_or("search input bounds should be available")?;
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

    pub fn click_search_bar_chrome(&mut self) -> Result<(), Box<dyn Error>> {
        let bounds = self
            .driver
            .cx
            .debug_bounds("library-search-bar")
            .ok_or("library search bar bounds should be available")?;
        let position = gpui::point(bounds.right() - gpui::px(8.0), bounds.center().y);
        self.driver
            .cx
            .simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
        self.driver
            .cx
            .simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
        self.driver.run_until_parked();
        Ok(())
    }

    pub fn is_input_editing(&mut self) -> bool {
        gpui_ui_kit::is_input_editing()
    }

    pub fn is_search_focused(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.input_mode == InputMode::Search)
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

    pub fn type_search_query_one_char_at_a_time_asserting(
        &mut self,
        query: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut expected = self.get_search_query();
        for ch in query.chars() {
            self.driver.simulate_keystrokes(&ch.to_string());
            self.driver.run_until_parked();
            expected.push(ch);

            let actual = self.get_search_query();
            if actual != expected {
                return Err(format!(
                    "Search query lost typed characters after '{}'. Expected '{}', got '{}'",
                    ch, expected, actual
                )
                .into());
            }
        }
        Ok(())
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

    pub fn get_app_filtered_album_titles(&mut self) -> Vec<String> {
        self.driver.read_app(|app| {
            app.filtered_albums()
                .into_iter()
                .map(|album| album.title.clone())
                .collect()
        })
    }

    pub fn get_app_filtered_albums_count(&mut self) -> usize {
        self.driver.read_app(|app| app.filtered_albums().len())
    }

    pub fn rendered_album_wrapper_count(&mut self, max_probe: usize) -> usize {
        (0..max_probe)
            .filter(|index| {
                let selector = match index {
                    0 => "library-album-wrapper-0",
                    1 => "library-album-wrapper-1",
                    2 => "library-album-wrapper-2",
                    3 => "library-album-wrapper-3",
                    4 => "library-album-wrapper-4",
                    _ => return false,
                };
                self.driver.cx.debug_bounds(selector).is_some()
            })
            .count()
    }

    pub fn get_channel_filter(&mut self) -> ChannelFilter {
        self.driver.read_app(|app| app.library_state.filter)
    }

    pub fn is_filter_menu_open(&mut self) -> bool {
        self.driver.read_app(|app| app.ui_state.filter_menu_open)
    }

    pub fn get_sort_order(&mut self) -> LibrarySortOrder {
        self.driver.read_app(|app| app.library_state.sort_order)
    }

    pub fn get_queue_album_titles(&mut self) -> Vec<String> {
        self.driver.read_app(|app| {
            app.queue_state
                .iter()
                .map(|item| item.album.title.clone())
                .collect()
        })
    }

    pub fn is_playing(&mut self) -> bool {
        self.driver.read_app(|app| app.playback.is_playing)
    }

    pub fn current_queue_index(&mut self) -> Option<usize> {
        self.driver.read_app(|app| app.playback.current_queue_index)
    }

    pub fn clear_queue_and_stop_playback(&mut self) {
        self.driver.update_app(|app, _| {
            app.queue_state.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });
        self.driver.run_until_parked();
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
