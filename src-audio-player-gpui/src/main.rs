mod app;
mod config;
mod ui;

use app::{App, AppState};
use gpui::AppContext;
use gpui::*;
use sotf_audio_player::Player;
use std::path::PathBuf;
use std::sync::Arc;

actions!(sotf_player, [Quit, NextScreen, PrevScreen]);

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF GPUI Player starting...");

    gpui::Application::new().run(move |cx| {
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
                    // Enable loudness monitoring
                    if let Err(e) = player.enable_loudness_monitoring() {
                        log::warn!("Failed to enable loudness monitoring: {}", e);
                    }

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
