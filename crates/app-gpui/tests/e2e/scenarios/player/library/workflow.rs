//! E2E tests for Library Workflow Integration.

use crate::driver::AppDriver;
use crate::factories::{album, stereo_track};
use crate::pages::library::LibraryPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct LibrarySearchFilterScenario;

impl TestScenario for LibrarySearchFilterScenario {
    fn name(&self) -> &'static str {
        "Library Search Filter"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Inject test albums
        driver.update_app(|app, _| {
            let beatles_album = album("Abbey Road")
                .with_year(1969)
                .add_track(stereo_track("Come Together", "The Beatles"))
                .add_track(stereo_track("Something", "The Beatles"))
                .build();

            let floyd_album = album("Dark Side of the Moon")
                .with_year(1973)
                .add_track(stereo_track("Time", "Pink Floyd"))
                .build();

            let tool_album = album("Lateralus")
                .with_year(2001)
                .add_track(stereo_track("Schism", "Tool"))
                .build();

            app.library_state.library.albums = vec![beatles_album, floyd_album, tool_album];
            app.invalidate_library_stats();
        });

        driver.navigate_to(Screen::Library);

        // Open search and filter
        let mut page = LibraryPage::new(&mut driver);
        page.open_search()?;
        page.type_search_query("beatles");

        // Verify filter results
        let count = page.get_filtered_albums_count();
        if count != 1 {
            return Err(format!("Expected 1 album matching 'beatles', got {}", count).into());
        }

        page.verify_filtered_results_contain("beatles")?;

        Ok(())
    }
}

struct LibraryEmptyStateScenario;

impl TestScenario for LibraryEmptyStateScenario {
    fn name(&self) -> &'static str {
        "Library Empty State"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Library should be empty by default
        let album_count = driver.read_app(|app| app.library_state.library.albums.len());
        if album_count != 0 {
            return Err(format!("Expected empty library, got {} albums", album_count).into());
        }

        // Queue should be empty
        let queue_len = driver.read_app(|app| app.queue.len());
        if queue_len != 0 {
            return Err(format!("Expected empty queue, got {} items", queue_len).into());
        }

        Ok(())
    }
}

struct LibraryAlbumInjectionScenario;

impl TestScenario for LibraryAlbumInjectionScenario {
    fn name(&self) -> &'static str {
        "Library Album Injection"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Inject albums and verify
        driver.update_app(|app, _| {
            let album1 = album("Test Album 1")
                .with_year(2020)
                .add_track(stereo_track("Track 1", "Artist A"))
                .build();

            let album2 = album("Test Album 2")
                .with_year(2021)
                .add_track(stereo_track("Track 2", "Artist B"))
                .add_track(stereo_track("Track 3", "Artist B"))
                .build();

            app.library_state.library.albums = vec![album1, album2];
            app.invalidate_library_stats();
        });

        let album_count = driver.read_app(|app| app.library_state.library.albums.len());
        if album_count != 2 {
            return Err(format!("Expected 2 albums, got {}", album_count).into());
        }

        // Verify second album has 2 tracks
        let track_count = driver.read_app(|app| app.library_state.library.albums[1].tracks.len());
        if track_count != 2 {
            return Err(format!("Expected 2 tracks in second album, got {}", track_count).into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_library_search_filter(cx: &mut TestAppContext) {
    let scenario = LibrarySearchFilterScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Library search filter test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_library_empty_state(cx: &mut TestAppContext) {
    let scenario = LibraryEmptyStateScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Library empty state test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_library_album_injection(cx: &mut TestAppContext) {
    let scenario = LibraryAlbumInjectionScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Library album injection test failed: {:?}",
        result.err()
    );
}
