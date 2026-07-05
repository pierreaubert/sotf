//! E2E tests for Library Workflow Integration.

use crate::driver::AppDriver;
use crate::factories::{album, stereo_track, surround_track};
use crate::pages::library::LibraryPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::app::{Screen, state::library::ChannelFilter};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;
use std::path::{Path, PathBuf};

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

struct LibrarySearchFilterButtonScenario;

struct LibraryAlbumDoubleClickScenario;

fn real_file_album(id: i64, title: &str, artist: &str, path: &Path) -> Album {
    Album {
        id: Some(id),
        title: title.to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: PathBuf::from(path),
            title: Some(format!("{} Track", title)),
            artist: Some(artist.to_string()),
            channels: Some(2),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            ..Default::default()
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    }
}

impl TestScenario for LibrarySearchFilterButtonScenario {
    fn name(&self) -> &'static str {
        "Library Search Filter Button"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        driver.update_app(|app, _| {
            let stereo_album = album("Kind of Blue Stereo")
                .with_year(1959)
                .add_track(stereo_track("So What", "Miles Davis"))
                .build();

            let surround_album = album("Kind of Blue Immersive")
                .with_year(1959)
                .add_track(surround_track("So What Atmos", "Miles Davis", 6))
                .build();

            let unrelated_surround_album = album("Blue Train Immersive")
                .with_year(1958)
                .add_track(surround_track("Moment's Notice", "John Coltrane", 6))
                .build();

            app.library_state.library.albums =
                vec![stereo_album, surround_album, unrelated_surround_album];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
            app.queue_state.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });

        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        let mut page = LibraryPage::new(&mut driver);
        page.click_library_search_tab()?;
        page.click_search_input()?;
        page.type_search_query_one_char_at_a_time_asserting("kind")?;

        let search_count = page.get_app_filtered_albums_count();
        if search_count != 2 {
            return Err(format!("Expected search to match 2 albums, got {}", search_count).into());
        }

        page.click_library_filter_tab()?;
        page.click_channel_filter_button("filter-btn-5.x Surround (1)")?;

        let selected_filter = page.get_channel_filter();
        if selected_filter != ChannelFilter::Surround {
            return Err(format!("Expected Surround filter, got {:?}", selected_filter).into());
        }

        let titles = page.get_app_filtered_album_titles();
        if titles != vec!["Kind of Blue Immersive".to_string()] {
            return Err(format!(
                "Expected only the searched surround album after filter button click, got {:?}",
                titles
            )
            .into());
        }

        let rendered_count = page.rendered_album_wrapper_count(3);
        if rendered_count != 1 {
            return Err(format!(
                "Expected rendered album grid to show 1 album after filter button click, got {} wrappers",
                rendered_count
            )
            .into());
        }

        Ok(())
    }
}

impl TestScenario for LibraryAlbumDoubleClickScenario {
    fn name(&self) -> &'static str {
        "Library Album Double Click"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let tmp_dir = tempfile::tempdir()?;
        let track_path = tmp_dir.path().join("double-click.flac");
        std::fs::write(&track_path, b"fake audio")?;

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![real_file_album(
                10,
                "Double Click Suite",
                "Interaction Tester",
                &track_path,
            )];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
            app.queue_state.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });

        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        let mut page = LibraryPage::new(&mut driver);
        page.double_click_album(0)?;
        assert_single_album_playing(&mut page, "Double Click Suite", "main library")?;

        page.clear_queue_and_stop_playback();
        page.click_sidebar_search()?;
        page.click_search_input()?;
        page.type_search_query_one_char_at_a_time_asserting("double")?;

        let search_count = page.get_app_filtered_albums_count();
        if search_count != 1 {
            return Err(format!("Expected search to match 1 album, got {}", search_count).into());
        }
        if !page.is_search_focused() {
            return Err("Expected Search mode to still be active before album double-click".into());
        }

        page.double_click_album(0)?;
        assert_single_album_playing(&mut page, "Double Click Suite", "search library")?;

        Ok(())
    }
}

fn assert_single_album_playing(
    page: &mut LibraryPage<'_, '_>,
    expected_title: &str,
    context: &str,
) -> Result<(), Box<dyn Error>> {
    let titles = page.get_queue_album_titles();
    if titles != vec![expected_title.to_string()] {
        return Err(format!(
            "Expected {} double-click to queue only '{}', got {:?}",
            context, expected_title, titles
        )
        .into());
    }

    let current_index = page.current_queue_index();
    if current_index != Some(0) {
        return Err(format!(
            "Expected {} double-click to select queue index Some(0), got {:?}",
            context, current_index
        )
        .into());
    }

    if !page.is_playing() {
        return Err(format!("Expected {} double-click to start playback", context).into());
    }

    Ok(())
}

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
        let queue_len = driver.read_app(|app| app.queue_state.len());
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
async fn test_library_search_filter_button_click(cx: &mut TestAppContext) {
    let scenario = LibrarySearchFilterButtonScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Library search filter button click test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_library_album_double_click_queues_and_plays(cx: &mut TestAppContext) {
    let scenario = LibraryAlbumDoubleClickScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Library album double click test failed: {:?}",
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
