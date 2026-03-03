use anyhow::anyhow;
use clap::Parser;
use gpui::*;
use mimalloc::MiMalloc;
use rust_embed::RustEmbed;
use sotf_audio_player::{Player, ReleaseChannel};
use sotf_audio_player_gpui::app::actions::*;
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::{
    App, AppState, Screen,
    i18n::{Language, Translations},
};
use sotf_audio_player_gpui::config::Config;
use sotf_audio_player_gpui::keybindings::{KeymapPreset, get_keybindings};
use sotf_audio_player_gpui::ui;
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser, Debug)]
#[command(name = "SotF")]
#[command(version, about = "SOTF GPUI Music Player", long_about = None)]
struct Args {}

actions!(sotf_player, [Quit, NextScreen, PrevScreen]);

/// Embedded assets including Lucide SVG icons and brand images
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "brands/*.jpg"]
#[include = "brands/*.jpeg"]
#[include = "brands/*.png"]
#[include = "brands/*.webp"]
#[include = "sotf.jpg"]
struct Assets;

fn main() {
    // Parse command line arguments (handles --version and --help)
    let _args = Args::parse();
    // Initialize logging to file
    if let Some(log_path) = sotf_audio_player::config::get_gpui_log_path() {
        // Initialize logging to file with restricted permissions (owner only)
        #[cfg(unix)]
        let log_result = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600) // Owner read/write only
                .open(&log_path)
        };
        #[cfg(not(unix))]
        let log_result = OpenOptions::new().create(true).append(true).open(&log_path);

        if let Ok(log_file) = log_result {
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

    gpui::Application::with_platform(std::rc::Rc::new(gpui_macos::MacPlatform::new(false)))
        .with_assets(Assets)
        .run(move |cx| {
            // Load custom fonts
            let fonts = vec![
                "fonts/B612-Regular.ttf",
                "fonts/B612-Italic.ttf",
                "fonts/B612-Bold.ttf",
                "fonts/B612-BoldItalic.ttf",
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

            // Load configuration to get language, keymap preset, and window geometry
            let config = Config::load().ok();
            let (language, keymap_preset, release_channel) = config
                .as_ref()
                .map(|c| (c.language, c.keymap_preset, c.release_channel))
                .unwrap_or_else(|| {
                    (
                        Language::default(),
                        KeymapPreset::default(),
                        ReleaseChannel::default(),
                    )
                });

            let translations = Translations::for_language(language);

            // Register keyboard shortcuts
            cx.bind_keys(get_keybindings(keymap_preset));

            // Build View menu items, filtering by release channel
            let mut view_menu_items = vec![
                MenuItem::action(translations.screen_library, SwitchToLibrary),
                MenuItem::action(translations.screen_studio, SwitchToStudio),
                MenuItem::action(translations.screen_studio_full, SwitchToPluginGraph),
                MenuItem::action(translations.screen_recording, SwitchToRecording),
            ];
            if release_channel.allows(Screen::RoomEq.maturity()) {
                view_menu_items.push(MenuItem::action(
                    translations.screen_room_eq,
                    SwitchToRoomEQ,
                ));
            }
            view_menu_items.push(MenuItem::action(
                translations.screen_headphone_eq,
                SwitchToHeadphoneEQ,
            ));
            view_menu_items.push(MenuItem::action(
                translations.screen_spinorama,
                SwitchToSpinorama,
            ));

            cx.set_menus(vec![
                Menu {
                    name: format!("SotF-v{}", env!("CARGO_PKG_VERSION")).into(),
                    items: vec![
                        MenuItem::action(translations.menu_about, About),
                        MenuItem::separator(),
                        MenuItem::action(translations.menu_open_config, OpenConfig),
                        MenuItem::separator(),
                        MenuItem::os_submenu("Services", SystemMenuType::Services),
                        MenuItem::separator(),
                        MenuItem::action(translations.menu_quit, QuitApp),
                    ],
                },
                Menu {
                    name: translations.menu_view.into(),
                    items: view_menu_items,
                },
                Menu {
                    name: translations.menu_help.into(),
                    items: vec![MenuItem::action(
                        translations.menu_keyboard_shortcuts,
                        ToggleHelp,
                    )],
                },
            ]);

            // Use window geometry from already loaded config
            let window_geometry = config
                .as_ref()
                .map(|c| c.window_geometry.clone())
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
                    // Load configuration (directories, theme, etc.) before creating entities
                    let mut temp_app = App::new();
                    let layout_state = match temp_app.load_config() {
                        Ok(l) => l,
                        Err(e) => {
                            log::warn!("Could not load saved configuration: {}", e);
                            LayoutState::default()
                        }
                    };

                    let player = Player::new();
                    // Apply loaded volume to player
                    if let Err(e) = player.set_volume(temp_app.playback.volume) {
                        log::warn!("Failed to set initial volume: {}", e);
                    }

                    let layout = cx.new(|_| layout_state);
                    #[allow(clippy::arc_with_non_send_sync)]
                    let player_arc = Arc::new(parking_lot::Mutex::new(player));

                    // Create application state
                    // Note: Database loading is deferred to after UI renders via check_library_on_startup()
                    let app_state = cx.new(|_cx| {
                        let mut app = temp_app;
                        // Load output devices
                        app.load_audio_devices();

                        AppState {
                            app,
                            layout,
                            player: player_arc,
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
