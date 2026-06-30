use crate::driver::AppDriver;
use crate::pages::library::LibraryPage;
use crate::runner::E2ERunner;
use crate::runner::TestScenario;
use gpui::{VisualTestContext, WindowHandle};
use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;
use std::path::PathBuf;

pub struct SearchScenario;

impl TestScenario for SearchScenario {
    fn name(&self) -> &'static str {
        "Search Library"
    }

    fn setup(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // 1. Inject Mock Data
        driver.update_app(|app, _| {
            // Create Vivaldi Album
            let vivaldi_tracks = vec![Track {
                title: Some("Spring".to_string()),
                artist: Some("Vivaldi".to_string()),
                ..create_empty_track()
            }];
            let vivaldi_album = Album {
                title: "The Four Seasons".to_string(),
                tracks: vivaldi_tracks,
                ..create_empty_album()
            };

            // Create Other Album
            let other_tracks = vec![Track {
                title: Some("Song".to_string()),
                artist: Some("Other Artist".to_string()),
                ..create_empty_track()
            }];
            let other_album = Album {
                title: "Other Album".to_string(),
                tracks: other_tracks,
                ..create_empty_album()
            };

            app.library_state.library.albums = vec![vivaldi_album, other_album];
            // Since we updated albums directly, we might need to update filtering or stats
            app.invalidate_library_stats();
            // Also need to trigger filtering update if it's cached.
            // Usually setting search query later will re-trigger it.
            // But we need to ensure initial view is correct (Validation step if needed, but not strictly required by user).
        });

        // 2. Navigate to Library
        driver.navigate_to(Screen::Library);

        let mut page = LibraryPage::new(&mut driver);

        // 3. Verify Search box not focused initially
        if page.is_search_focused() {
            return Err("Search should not be focused initially".into());
        }

        // 4. Click Search (Toggle Search)
        page.open_search()?;

        // 5. Verify Focused
        if !page.is_search_focused() {
            return Err("Search box should be focused after clicking/toggling search".into());
        }

        // 6. Type "vivaldi"
        page.type_search_query("vivaldi");

        // 7. Verify Query
        let query = page.get_search_query();
        if query != "vivaldi" {
            return Err(
                format!("Search query mismatch. Expected 'vivaldi', got '{}'", query).into(),
            );
        }

        // 8. Verify Filters
        // Wait for update (simulate_keystrokes waits for parked, so filtering should be done)

        // Count should be 1
        let count = page.get_filtered_albums_count();
        if count != 1 {
            return Err(format!("Expected 1 album after filter, got {}", count).into());
        }

        // Verify content
        page.verify_filtered_results_contain("vivaldi")?;

        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

fn create_empty_track() -> Track {
    Track {
        path: PathBuf::from("mock"),
        title: None,
        artist: None,
        track_number: None,
        duration_secs: None,
        channels: None,
        sample_rate: None,
        bit_depth: None,
        replay_gain: None,
        replay_peak: None,
        album_gain: None,
        album_peak: None,
        waveform: None,
        genre: None,
        composer: None,
        disc_number: None,
        conductor: None,
        performer: None,
        isrc: None,
        album_artist: None,
        ensemble: None,
        edition: None,
        is_favorite: false,
        play_count: 0,
        source: None,
        uuid: None,
    }
}

fn create_empty_album() -> Album {
    Album {
        id: None,
        title: "".to_string(),
        year: None,
        tracks: vec![],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    }
}

#[gpui::test]
async fn test_search_flow(cx: &mut gpui::TestAppContext) {
    let scenario = SearchScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}

pub struct SearchInputKeepsFocusScenario;

impl TestScenario for SearchInputKeepsFocusScenario {
    fn name(&self) -> &'static str {
        "Search Library Input Keeps Focus"
    }

    fn setup(&mut self, cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        cx.update(|cx| {
            cx.bind_keys(sotf_audio_player_gpui::app::keybindings::get_keybindings(
                sotf_audio_player_gpui::app::KeymapPreset::Default,
            ));
        });
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);
        driver.navigate_to(Screen::Home);

        let mut page = LibraryPage::new(&mut driver);
        page.click_sidebar_search()?;

        page.type_search_query_one_char_at_a_time_asserting("s")?;

        // Regression guard for the live symptom: after the first search update,
        // focus can end up back on the player root while Search mode remains
        // active. The remaining keystrokes must still extend the search query.
        page.focus_player_root();
        page.type_search_query_one_char_at_a_time_asserting("earch")?;

        let query = page.get_search_query();
        if query != "search" {
            return Err(format!(
                "Search query should preserve every typed character. Expected 'search', got '{}'",
                query
            )
            .into());
        }

        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[gpui::test]
async fn test_search_input_keeps_focus_between_keystrokes(cx: &mut gpui::TestAppContext) {
    let scenario = SearchInputKeepsFocusScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}

pub struct SearchInputAccumulatesFromLibraryTabScenario;

impl TestScenario for SearchInputAccumulatesFromLibraryTabScenario {
    fn name(&self) -> &'static str {
        "Search Library Input Accumulates From Library Tab"
    }

    fn setup(&mut self, cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        cx.update(|cx| {
            cx.bind_keys(sotf_audio_player_gpui::app::keybindings::get_keybindings(
                sotf_audio_player_gpui::app::KeymapPreset::Default,
            ));
        });
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);
        driver.navigate_to(Screen::Library);

        let mut page = LibraryPage::new(&mut driver);
        page.click_library_search_tab()?;
        if !page.is_search_focused() {
            return Err("Search input should be focused after opening from the library tab".into());
        }

        page.type_search_query_one_char_at_a_time_asserting("v")?;
        page.type_search_query_without_key_char_asserting("ivaldi")?;

        let query = page.get_search_query();
        if query != "vivaldi" {
            return Err(format!(
                "Search query should preserve every typed character. Expected 'vivaldi', got '{}'",
                query
            )
            .into());
        }

        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[gpui::test]
async fn test_search_input_accumulates_from_library_tab(cx: &mut gpui::TestAppContext) {
    let scenario = SearchInputAccumulatesFromLibraryTabScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}
