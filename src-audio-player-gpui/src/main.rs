mod app;
mod config;
mod ui;

use app::{App, AppState};
use gpui::*;
use sotf_audio_player::Player;
use std::sync::Arc;

actions!(sotf_player, [Quit, NextScreen, PrevScreen]);

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF GPUI Player starting...");

    gpui::Application::new().run(move |cx| {
        // Register keyboard shortcuts
        cx.bind_keys([
            // Playback controls
            KeyBinding::new("space", ui::PlayPause, None),
            KeyBinding::new("n", ui::NextTrack, None),
            KeyBinding::new(">", ui::NextTrack, None),
            KeyBinding::new("b", ui::PrevTrack, None),
            KeyBinding::new("<", ui::PrevTrack, None),
            KeyBinding::new("+", ui::VolumeUp, None),
            KeyBinding::new("=", ui::VolumeUp, None),
            KeyBinding::new("-", ui::VolumeDown, None),
            KeyBinding::new("_", ui::VolumeDown, None),
            // Screen navigation
            KeyBinding::new("shift-l", ui::SwitchToLibrary, None),
            KeyBinding::new("shift-q", ui::SwitchToQueue, None),
            KeyBinding::new("shift-p", ui::SwitchToPlugins, None),
            KeyBinding::new("shift-o", ui::SwitchToDevices, None),
            KeyBinding::new("shift-d", ui::SwitchToDirectoryManager, None),
            KeyBinding::new("L", ui::SwitchToLibrary, None),
            KeyBinding::new("Q", ui::SwitchToQueue, None),
            KeyBinding::new("P", ui::SwitchToPlugins, None),
            KeyBinding::new("O", ui::SwitchToDevices, None),
            KeyBinding::new("D", ui::SwitchToDirectoryManager, None),
            // General actions
            KeyBinding::new("/", ui::ToggleSearch, None),
            KeyBinding::new("escape", ui::Cancel, None),
            KeyBinding::new("t", ui::ToggleLibraryView, None),
            KeyBinding::new("?", ui::ToggleHelp, None),
            // Sort controls
            KeyBinding::new("s", ui::CycleSortOrder, None),
            KeyBinding::new("1", ui::SetSortArtist, None),
            KeyBinding::new("2", ui::SetSortAlbum, None),
            KeyBinding::new("3", ui::SetSortTitle, None),
            KeyBinding::new("4", ui::SetSortYear, None),
            // Filter controls
            KeyBinding::new("c", ui::CycleChannelFilter, None),
            KeyBinding::new("5", ui::SetFilterAll, None),
            KeyBinding::new("6", ui::SetFilterMono, None),
            KeyBinding::new("7", ui::SetFilterStereo, None),
            KeyBinding::new("8", ui::SetFilterMultichannel, None),
            KeyBinding::new("9", ui::SetFilterMixed, None),
            // Navigation
            KeyBinding::new("up", ui::SelectPrev, None),
            KeyBinding::new("k", ui::SelectPrev, None),
            KeyBinding::new("down", ui::SelectNext, None),
            KeyBinding::new("j", ui::SelectNext, None),
            KeyBinding::new("pageup", ui::SelectPrevPage, None),
            KeyBinding::new("pagedown", ui::SelectNextPage, None),
            // Library pagination (Ctrl/Cmd + arrows)
            KeyBinding::new("ctrl-left", ui::PrevPage, None),
            KeyBinding::new("ctrl-right", ui::NextPage, None),
            KeyBinding::new("cmd-left", ui::PrevPage, None),
            KeyBinding::new("cmd-right", ui::NextPage, None),
            // Expand/collapse
            KeyBinding::new("left", ui::ToggleExpand, None),
            KeyBinding::new("h", ui::ToggleExpand, None),
            KeyBinding::new("right", ui::ToggleExpand, None),
            KeyBinding::new("l", ui::ToggleExpand, None),
            // Enter action - add album to queue
            KeyBinding::new("enter", ui::Enter, None),
            KeyBinding::new("a", ui::Enter, None),
            // Remove/delete
            KeyBinding::new("d", ui::RemoveItem, None),
            KeyBinding::new("delete", ui::RemoveItem, None),
            // Plugin controls
            KeyBinding::new("u", ui::MovePluginUp, None),
            KeyBinding::new("shift-n", ui::MovePluginDown, None),
            KeyBinding::new("shift-t", ui::TogglePlugin, None),
            KeyBinding::new("e", ui::EditPlugin, None),
            // Directory management
            KeyBinding::new("shift-a", ui::AddDirectory, None),
            KeyBinding::new("shift-s", ui::ScanLibrary, None),
            // Quick add plugins (Shift + number keys)
            KeyBinding::new("!", ui::QuickAddEQ, None),
            KeyBinding::new("@", ui::QuickAddUpmixer, None),
            KeyBinding::new("#", ui::QuickAddCompressor, None),
            KeyBinding::new("$", ui::QuickAddGate, None),
            KeyBinding::new("%", ui::QuickAddLimiter, None),
            KeyBinding::new("^", ui::QuickAddLoudness, None),
            KeyBinding::new("&", ui::QuickAddBinaural, None),
        ]);

        // Create window with app state
        cx.open_window(
            WindowOptions {
                app_id: Some("com.spinorama.sotf-player".into()),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::new(px(100.0), px(100.0)),
                    size: Size {
                        width: px(1200.0),
                        height: px(800.0),
                    },
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("SOTF Audio Player".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                display_id: None,
                is_minimizable: true,
                is_resizable: true,
                tabbing_identifier: None,
                window_decorations: None,
                window_min_size: None,
            },
            |_, cx| {
                // Create application state
                let app_state = cx.new(|_cx| {
                    let mut app = App::new();

                    // Load from database
                    if let Err(e) = app.load_library_from_database() {
                        log::warn!("Failed to load library from database: {}", e);
                    }

                    // Load output devices
                    app.load_output_devices();

                    // Load configuration
                    if let Err(e) = app.load_config() {
                        log::warn!("Could not load saved configuration: {}", e);
                    }

                    let mut player = Player::new();

                    AppState {
                        app,
                        player: Arc::new(parking_lot::Mutex::new(player)),
                    }
                });

                // Set up keyboard actions
                cx.on_action(|_: &Quit, cx| {
                    cx.quit();
                });

                // Build the root view
                cx.new(|cx| ui::PlayerView::new(app_state.clone(), cx))
            },
        );
    });
}

struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        // For now, return None - no custom assets needed
        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
