//! E2E tests for Album Context Menu
//!
//! Tests album right-click menu functionality:
//! - Context menu appears with "Add to Queue" and "Play Now" options
//! - Both options add album to queue
//! - "Play Now" also starts playback
//! - Keyboard shortcuts 'a' and Enter work

use crate::driver::AppDriver;
use crate::pages::library::LibraryPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player::{Album, Track};
use sotf_audio_player_gpui::InputMode;
use sotf_audio_player_gpui::app::{ContextMenuState, ContextMenuType, Screen};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct AlbumContextMenuScenario;

impl TestScenario for AlbumContextMenuScenario {
    fn name(&self) -> &'static str {
        "Album Context Menu"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // Inject test albums
        let album = Album {
            id: Some(1),
            title: "Test Album".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: std::path::PathBuf::from("/test/track.flac"),
                title: Some("Test Track".to_string()),
                ..Default::default()
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
            uuid: None,
        };

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![album];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
        });

        // Navigate to Library
        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        // Verify initial state: no context menu, empty queue
        let has_menu = driver.read_app(|app| app.ui_state.context_menu.is_some());
        if has_menu {
            return Err("Context menu should not be visible initially".into());
        }

        // ===== Test 1: Select album and open context menu =====
        driver.update_app(|app, _| {
            app.library_state.selected_index = 0;
        });
        driver.run_until_parked();

        // Simulate right-click by directly setting context menu state
        driver.update_app(|app, _| {
            app.ui_state.context_menu = Some(ContextMenuState {
                menu_type: ContextMenuType::Album,
                position_x: 100.0,
                position_y: 100.0,
                item_index: 0,
            });
        });
        driver.run_until_parked();

        // Verify context menu is visible
        let has_menu = driver.read_app(|app| app.ui_state.context_menu.is_some());
        if !has_menu {
            return Err("Context menu should be visible after right-click".into());
        }

        // Verify menu type is Album
        let menu_type = driver.read_app(|app| {
            app.ui_state
                .context_menu
                .as_ref()
                .map(|m| m.menu_type.clone())
        });
        if !matches!(menu_type, Some(ContextMenuType::Album)) {
            return Err("Menu type should be Album".into());
        }

        // ===== Test 2: Click "Add to Queue" from context menu =====
        // Instead of directly calling the function, simulate what happens when
        // user clicks the "Add to Queue" menu item in the context menu
        // This is what the UI does when menu item is selected
        driver.update_app(|app, _| {
            // Simulate clicking "Add to Queue" - same as on_select handler
            app.ui_state.context_menu = None;
            app.ui_state.input_mode = InputMode::Normal;
            if let Ok(Some(_path)) = app.add_album_to_queue() {
                // In real UI, this would call PlayerView::play_track(state, path)
                // But we just need to verify queue was added
            }
        });
        driver.run_until_parked();

        // Verify album was added to queue
        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Album should be added to queue via context menu".into());
        }
        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if is_playing {
            return Err("Add to Queue should enqueue without starting playback".into());
        }

        // ===== Test 3: Verify queue has album =====
        let queue_tracks = driver.read_app(|app| {
            app.queue_state
                .iter()
                .map(|item| item.album.title.clone())
                .collect::<Vec<_>>()
        });
        if queue_tracks.is_empty() {
            return Err("Queue should have tracks".into());
        }
        println!("Queue has albums: {:?}", queue_tracks);

        // ===== Test 4: Clear queue and test Play Now via context menu flow =====
        driver.update_app(|app, _| {
            app.queue_state.clear();
            app.queue_state.expanded.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });
        driver.run_until_parked();

        // Open context menu first
        driver.update_app(|app, _| {
            app.ui_state.context_menu = Some(ContextMenuState {
                menu_type: ContextMenuType::Album,
                position_x: 100.0,
                position_y: 100.0,
                item_index: 0,
            });
            app.ui_state.input_mode = InputMode::ContextMenu;
        });
        driver.run_until_parked();

        // Simulate clicking "Play Now" menu item (same as on_select handler)
        driver.update_app(|app, _| {
            app.ui_state.context_menu = None;
            app.ui_state.input_mode = InputMode::Normal;
            if let Ok(Some(_path)) = app.play_album_now() {
                // In real UI, this would call PlayerView::play_track(state, path)
            }
        });
        driver.run_until_parked();

        // Verify album was added
        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Album should be added to queue after Play Now".into());
        }

        // Verify playback started
        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if !is_playing {
            return Err("Playback should start after Play Now".into());
        }

        // ===== Test 6: Close context menu with Escape =====
        // Re-open context menu
        driver.update_app(|app, _| {
            app.ui_state.context_menu = Some(ContextMenuState {
                menu_type: ContextMenuType::Album,
                position_x: 100.0,
                position_y: 100.0,
                item_index: 0,
            });
        });
        driver.run_until_parked();

        // Simulate Escape key to close menu
        driver.simulate_keystrokes("escape");
        driver.run_until_parked();

        // Verify menu closed
        let has_menu = driver.read_app(|app| app.ui_state.context_menu.is_some());
        if has_menu {
            return Err("Context menu should close after Escape".into());
        }

        println!("All album context menu tests passed!");
        Ok(())
    }
}

/// Test that adding an album that's already in the queue doesn't create duplicates
pub struct AlbumNoDuplicateScenario;

impl TestScenario for AlbumNoDuplicateScenario {
    fn name(&self) -> &'static str {
        "Album No Duplicate in Queue"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // Inject test album — use a real temp file so validate_album_has_files passes
        let tmp_dir = tempfile::tempdir().unwrap();
        let track_path = tmp_dir.path().join("track.flac");
        std::fs::write(&track_path, b"fake").unwrap();

        let album = Album {
            id: Some(1),
            title: "Test Album".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: track_path,
                title: Some("Test Track".to_string()),
                ..Default::default()
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
            uuid: None,
        };

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![album];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
        });

        // Navigate to Library
        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        // Add album to queue first time
        driver.update_app(|app, _| {
            app.library_state.selected_index = 0;
            let _ = app.add_album_to_queue();
        });
        driver.run_until_parked();

        // Verify album was added
        let queue_len = driver.read_app(|app| app.queue_state.len());
        println!("Queue length after first add: {}", queue_len);
        assert_eq!(queue_len, 1, "Expected 1 album in queue, got {}", queue_len);

        // Try to add the same album again via add_album_to_queue
        driver.update_app(|app, _| {
            app.library_state.selected_index = 0;
            let _ = app.add_album_to_queue();
        });
        driver.run_until_parked();

        // BUG: Album is added again - this should fail until we fix it
        let queue_len_after = driver.read_app(|app| app.queue_state.len());
        println!("Queue length after second add: {}", queue_len_after);
        assert_eq!(
            queue_len_after, 1,
            "Album was added twice! Expected 1 album in queue, got {}. Duplicates should be prevented.",
            queue_len_after
        );

        println!("No duplicate album test passed!");
        Ok(())
    }
}

/// Clicks the actual rendered context-menu items and verifies they execute.
pub struct AlbumContextMenuClickScenario;

impl TestScenario for AlbumContextMenuClickScenario {
    fn name(&self) -> &'static str {
        "Album Context Menu Click"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // Inject a test album backed by a real temp file so add_album_to_queue succeeds.
        let tmp_dir = tempfile::tempdir().unwrap();
        let track_path = tmp_dir.path().join("track.flac");
        std::fs::write(&track_path, b"fake").unwrap();

        let album = Album {
            id: Some(1),
            title: "Test Album".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: track_path,
                title: Some("Test Track".to_string()),
                ..Default::default()
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
            uuid: None,
        };

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![album];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
        });

        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        driver.update_app(|app, _| {
            app.library_state.selected_index = 0;
        });
        driver.run_until_parked();

        // Enter search mode, focus the search input, and type a query that matches.
        // This reproduces the post-search-fix state in which the menu regressed.
        {
            let mut page = LibraryPage::new(&mut driver);
            page.click_library_search_tab()?;
            page.click_search_bar_chrome()?;
            page.type_search_query_one_char_at_a_time("test");
        }

        // Open the context menu by right-clicking the first album card.
        let wrapper_bounds = driver
            .cx
            .debug_bounds("library-album-wrapper-0")
            .ok_or("Album wrapper should be rendered")?;
        let wrapper_center = wrapper_bounds.center();
        driver.cx.simulate_mouse_down(
            wrapper_center,
            MouseButton::Right,
            Modifiers::default(),
        );
        driver
            .cx
            .simulate_mouse_up(wrapper_center, MouseButton::Right, Modifiers::default());
        driver.run_until_parked();

        let menu_visible = driver.read_app(|app| app.ui_state.context_menu.is_some());
        if !menu_visible {
            return Err("Context menu should appear after right-clicking an album".into());
        }

        // Click the rendered "Add to Queue" menu item.
        let bounds = driver
            .cx
            .debug_bounds("menu-item-add-to-queue")
            .ok_or("Add to Queue menu item should be rendered")?;
        let center = bounds.center();
        driver
            .cx
            .simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        driver
            .cx
            .simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        driver.run_until_parked();

        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Add to Queue menu item should add the album to the queue".into());
        }

        let menu_closed = driver.read_app(|app| app.ui_state.context_menu.is_none());
        if !menu_closed {
            return Err("Context menu should close after Add to Queue".into());
        }

        let menu_item_gone = driver.cx.debug_bounds("menu-item-add-to-queue").is_none();
        if !menu_item_gone {
            return Err("Rendered context menu should disappear after Add to Queue".into());
        }

        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if is_playing {
            return Err("Add to Queue should not start playback".into());
        }

        // Clear queue and reopen menu to test Play Now.
        driver.update_app(|app, _| {
            app.queue_state.clear();
            app.queue_state.expanded.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });
        driver.run_until_parked();

        // Right-click the album again to reopen the context menu.
        driver.cx.simulate_mouse_down(
            wrapper_center,
            MouseButton::Right,
            Modifiers::default(),
        );
        driver
            .cx
            .simulate_mouse_up(wrapper_center, MouseButton::Right, Modifiers::default());
        driver.run_until_parked();

        let menu_visible = driver.read_app(|app| app.ui_state.context_menu.is_some());
        if !menu_visible {
            return Err("Context menu should reappear for Play Now test".into());
        }

        let bounds = driver
            .cx
            .debug_bounds("menu-item-play-now")
            .ok_or("Play Now menu item should be rendered")?;
        let center = bounds.center();
        driver
            .cx
            .simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        driver
            .cx
            .simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        driver.run_until_parked();

        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Play Now menu item should add the album to the queue".into());
        }

        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if !is_playing {
            return Err("Play Now menu item should start playback".into());
        }

        println!("Album context menu click test passed!");
        Ok(())
    }
}

/// Activates the context menu via keyboard after using the search box.
/// This reproduces the regression where the search Input keeps focus and
/// intercepts the 'a' / Enter shortcuts that should trigger menu actions.
pub struct AlbumContextMenuKeyboardAfterSearchScenario;

impl TestScenario for AlbumContextMenuKeyboardAfterSearchScenario {
    fn name(&self) -> &'static str {
        "Album Context Menu Keyboard After Search"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // Inject a test album backed by a real temp file so add_album_to_queue succeeds.
        let tmp_dir = tempfile::tempdir().unwrap();
        let track_path = tmp_dir.path().join("track.flac");
        std::fs::write(&track_path, b"fake").unwrap();

        let album = Album {
            id: Some(1),
            title: "Test Album".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: track_path.clone(),
                title: Some("Test Track".to_string()),
                ..Default::default()
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
            uuid: None,
        };

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![album];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
        });

        driver.navigate_to(Screen::Library);
        driver.run_until_parked();

        driver.update_app(|app, _| {
            app.library_state.selected_index = 0;
        });
        driver.run_until_parked();

        // Enter search mode, focus the search input, and type a query that matches.
        {
            let mut page = LibraryPage::new(&mut driver);
            page.click_library_search_tab()?;
            page.click_search_bar_chrome()?;
            page.type_search_query_one_char_at_a_time("test");
        }

        // Open the context menu by right-clicking the first album card.
        let wrapper_bounds = driver
            .cx
            .debug_bounds("library-album-wrapper-0")
            .ok_or("Album wrapper should be rendered")?;
        let wrapper_center = wrapper_bounds.center();
        driver.cx.simulate_mouse_down(
            wrapper_center,
            MouseButton::Right,
            Modifiers::default(),
        );
        driver
            .cx
            .simulate_mouse_up(wrapper_center, MouseButton::Right, Modifiers::default());
        driver.run_until_parked();

        let input_mode = driver.read_app(|app| app.ui_state.input_mode);
        if !matches!(input_mode, InputMode::ContextMenu) {
            return Err("Input mode should be ContextMenu after right-click".into());
        }

        // After opening the context menu, the search input must no longer be
        // in editing mode so that menu keyboard shortcuts ('a', Enter) are
        // routed to the menu instead of the search input.
        if gpui_ui_kit::is_input_editing() {
            return Err(
                "Search input is still editing after context menu opened; keyboard shortcuts will not work".into(),
            );
        }

        // Press 'a' to add the album to the queue.
        driver.simulate_keystrokes("a");
        driver.run_until_parked();

        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Pressing 'a' in album context menu should add the album to the queue".into());
        }

        let menu_closed = driver.read_app(|app| app.ui_state.context_menu.is_none());
        if !menu_closed {
            return Err("Context menu should close after pressing 'a'".into());
        }

        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if is_playing {
            return Err("Add to Queue should not start playback".into());
        }

        // Clear queue and reopen menu to test Play Now via Enter.
        driver.update_app(|app, _| {
            app.queue_state.clear();
            app.queue_state.expanded.clear();
            app.playback.is_playing = false;
            app.playback.current_queue_index = None;
        });
        driver.run_until_parked();

        driver.cx.simulate_mouse_down(
            wrapper_center,
            MouseButton::Right,
            Modifiers::default(),
        );
        driver
            .cx
            .simulate_mouse_up(wrapper_center, MouseButton::Right, Modifiers::default());
        driver.run_until_parked();

        let input_mode = driver.read_app(|app| app.ui_state.input_mode);
        if !matches!(input_mode, InputMode::ContextMenu) {
            return Err("Input mode should be ContextMenu for Play Now test".into());
        }

        // Press Enter to play the album now.
        driver.simulate_keystrokes("enter");
        driver.run_until_parked();

        let queue_len = driver.read_app(|app| app.queue_state.len());
        if queue_len == 0 {
            return Err("Pressing Enter in album context menu should add the album to the queue".into());
        }

        let is_playing = driver.read_app(|app| app.playback.is_playing);
        if !is_playing {
            return Err("Pressing Enter in album context menu should start playback".into());
        }

        println!("Album context menu keyboard-after-search test passed!");
        Ok(())
    }
}

#[gpui::test]
async fn test_album_context_menu(cx: &mut TestAppContext) {
    let scenario = AlbumContextMenuScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok(), "Album context menu test failed");
}

#[gpui::test]
async fn test_album_no_duplicate_in_queue(cx: &mut TestAppContext) {
    let scenario = AlbumNoDuplicateScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(
        result.is_ok(),
        "Album no duplicate test failed: should not add album twice"
    );
}

#[gpui::test]
async fn test_album_context_menu_click(cx: &mut TestAppContext) {
    let scenario = AlbumContextMenuClickScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(
        result.is_ok(),
        "Album context menu click test failed: menu item click should enqueue or play album"
    );
}

#[gpui::test]
async fn test_album_context_menu_keyboard_after_search(cx: &mut TestAppContext) {
    let scenario = AlbumContextMenuKeyboardAfterSearchScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(
        result.is_ok(),
        "Album context menu keyboard-after-search test failed: keyboard shortcuts should work after searching"
    );
}
