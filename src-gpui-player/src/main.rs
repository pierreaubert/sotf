mod app;
mod config;
mod database;
mod library;
mod player;
mod plugins;
mod ui;

use app::{App, AppState};
use gpui::*;
use player::Player;
use std::path::PathBuf;
use std::sync::Arc;

actions!(sotf_player, [Quit, NextScreen, PrevScreen]);

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF GPUI Player starting...");

    gpui::App::new().run(|cx: &mut AppContext| {
        // Load assets
        cx.set_global(Assets);

        // Create window with app state
        cx.open_window(
            WindowOptions {
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
            },
            |cx| {
                // Create application state
                let app_state = cx.new_model(|cx| {
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

                    AppState {
                        app,
                        player: Arc::new(Player::new()),
                    }
                });

                // Enable loudness monitoring
                if let Ok(state) = app_state.read(cx).player.enable_loudness_monitoring() {
                    log::info!("Loudness monitoring enabled");
                }

                // Set up keyboard actions
                cx.on_action(|_: &Quit, cx| {
                    cx.quit();
                });

                // Build the root view
                cx.new_view(|cx| ui::PlayerView::new(app_state.clone(), cx))
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
