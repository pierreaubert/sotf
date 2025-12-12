use anyhow::anyhow;
use gpui::*;
use rust_embed::RustEmbed;
use sotf_audio_player::Player;
use sotf_audio_player_gpui::actions::*;
use sotf_audio_player_gpui::app::{App, AppState};
use sotf_audio_player_gpui::config::Config;
use sotf_audio_player_gpui::keybindings::{KeymapPreset, get_keybindings};
use sotf_audio_player_gpui::ui;
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::sync::Arc;

actions!(sotf_player, [Quit, NextScreen, PrevScreen]);

/// Embedded assets including Lucide SVG icons and brand images
#[derive(RustEmbed)]
#[folder = "../assets"]
#[include = "icons/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "brands/*.jpg"]
#[include = "brands/*.jpeg"]
#[include = "brands/*.png"]
#[include = "brands/*.webp"]
#[include = "sotf.jpg"]
struct Assets;

fn main() {
    // Initialize logging to file
    if let Some(log_path) = sotf_audio_player::config::get_gpui_log_path() {
        if let Ok(log_file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            env_logger::Builder::from_default_env()
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .filter_level(log::LevelFilter::Debug)
                .filter_module("symphonia_core", log::LevelFilter::Debug)
                .init();
        } else {
            // Fallback to stderr if file cannot be opened
            env_logger::Builder::from_default_env()
                .filter_level(log::LevelFilter::Debug)
                .filter_module("symphonia_core", log::LevelFilter::Debug)
                .init();
        }
    } else {
        // Fallback to stderr if path cannot be determined
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .filter_module("symphonia_core", log::LevelFilter::Debug)
            .init();
    }

    log::info!("SOTF GPUI Player starting...");

    gpui::Application::new().with_assets(Assets).run(move |cx| {
        // Load custom fonts
        let fonts = vec![
            "fonts/DMSerifDisplay-Regular.ttf",
            "fonts/DMSerifDisplay-Italic.ttf",
        ];

        let mut font_data = Vec::new();
        for path in fonts {
            if let Some(file) = Assets::get(path) {
                font_data.push(file.data);
            } else {
                log::warn!("Failed to load font: {}", path);
            }
        }

        if !font_data.is_empty() {
            cx.text_system().add_fonts(font_data).unwrap();
        }

        cx.set_menus(vec![
            Menu {
                name: "SotF Player".into(),
                items: vec![
                    MenuItem::action("About SotF Player", About),
                    MenuItem::separator(),
                    MenuItem::action("Settings...", OpenConfig),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Quit SotF Player", QuitApp),
                ],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Library", SwitchToLibrary),
                    MenuItem::action("Queue", SwitchToQueue),
                    MenuItem::action("Settings", SwitchToSettings),
                    MenuItem::action("Devices", SwitchToDevices),
                    MenuItem::action("Plugins", SwitchToPlugins),
                    MenuItem::action("HeadphoneEQ", SwitchToHeadphoneEQ),
                    MenuItem::action("RoomEQ", SwitchToRoomEQ),
                ],
            },
            Menu {
                name: "Help".into(),
                items: vec![MenuItem::action("Keyboard Shortcuts", ToggleHelp)],
            },
        ]);

        // Register keyboard shortcuts from the keybindings module
        // Default preset is used at startup; can be changed via settings
        let keymap_preset = KeymapPreset::Default;
        cx.bind_keys(get_keybindings(keymap_preset));

        // Load window geometry from config
        let window_geometry = Config::load()
            .ok()
            .map(|c| c.window_geometry)
            .unwrap_or_default();

        // Create window with app state
        let window = cx.open_window(
            WindowOptions {
                app_id: Some("org.spinorama.sotf".into()),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::new(px(window_geometry.x), px(window_geometry.y)),
                    size: Size {
                        width: px(window_geometry.width),
                        height: px(window_geometry.height),
                    },
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("SotF".into()),
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
                    app.load_audio_devices();

                    // Load configuration
                    if let Err(e) = app.load_config() {
                        log::warn!("Could not load saved configuration: {}", e);
                    }

                    let player = Player::new();

                    // Apply loaded volume to player
                    if let Err(e) = player.set_volume(app.volume) {
                        log::warn!("Failed to set initial volume: {}", e);
                    }

                    AppState {
                        app,
                        player: Arc::new(parking_lot::Mutex::new(player)),
                    }
                });

                // Note: Window close and quit handling is done in PlayerView::quit_app
                // which saves window geometry before quitting

                // Build the root view
                cx.new(|cx| ui::PlayerView::new(app_state.clone(), cx))
            },
        );

        // Note: Window activation is handled in PlayerView::render on first frame
        // to ensure macOS menu bar is properly active
        let _ = window;
    });
}

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|file| Some(file.data))
            .ok_or_else(|| anyhow!("Could not find asset at path \"{}\"", path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| {
                if p.starts_with(path) {
                    Some(SharedString::from(p.to_string()))
                } else {
                    None
                }
            })
            .collect())
    }
}
