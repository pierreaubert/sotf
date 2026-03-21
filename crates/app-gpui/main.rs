// Suppress the console window on Windows (GUI app, not a console app)
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
use std::path::PathBuf;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser, Debug)]
#[command(name = "SotF")]
#[command(version, about = "SOTF GPUI Music Player", long_about = None)]
struct Args {
    /// Use a custom data directory (for QA testing)
    #[arg(long)]
    qa: Option<PathBuf>,
}

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
#[include = "sotf.png"]
#[include = "presets/*.json"]
struct Assets;

fn main() {
    // Parse command line arguments (handles --version and --help)
    let args = Args::parse();

    // Apply QA directory override before any config dir access
    if let Some(qa_dir) = args.qa {
        sotf_audio_player::config::set_config_dir_override(qa_dir);
    }

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

    // Install desktop integration on Linux (first-launch .desktop + icon)
    #[cfg(target_os = "linux")]
    {
        let icon_png = Assets::get("sotf.png");
        sotf_audio_player_gpui::desktop_integration::ensure_desktop_integration(
            icon_png.as_ref().map(|f| f.data.as_ref()),
        );
    }

    // Install default presets (only copies files that don't already exist)
    install_default_presets();

    // Install signal handlers for clean shutdown on Ctrl-C (SIGINT) and SIGTERM
    #[cfg(unix)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};

        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&shutdown_flag);

        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&shutdown_flag))
            .expect("Failed to register SIGINT handler");
        signal_hook::flag::register(signal_hook::consts::SIGTERM, flag_clone)
            .expect("Failed to register SIGTERM handler");

        std::thread::spawn(move || {
            while !shutdown_flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            log::info!("Signal received (SIGINT/SIGTERM), shutting down...");
            // Give a brief moment for any in-flight operations
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::process::exit(0);
        });
    }

    #[cfg(target_os = "macos")]
    let platform = std::rc::Rc::new(gpui_macos::MacPlatform::new(false));
    #[cfg(target_os = "linux")]
    let platform = gpui_linux::current_platform(false);
    #[cfg(target_os = "windows")]
    let platform = std::rc::Rc::new(gpui_windows::WindowsPlatform::new(false).expect("failed to create Windows platform"));

    gpui::Application::with_platform(platform)
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

            let mut app_menu_items = vec![
                MenuItem::action(translations.menu_about, About),
                MenuItem::separator(),
                MenuItem::action(translations.menu_open_config, OpenConfig),
                MenuItem::separator(),
            ];
            // Services submenu is macOS-only (no equivalent on Windows/Linux)
            #[cfg(target_os = "macos")]
            {
                app_menu_items.push(MenuItem::os_submenu("Services", SystemMenuType::Services));
                app_menu_items.push(MenuItem::separator());
            }
            app_menu_items.push(MenuItem::action(translations.menu_quit, QuitApp));

            cx.set_menus(vec![
                Menu {
                    name: format!("SotF-v{}", env!("CARGO_PKG_VERSION")).into(),
                    items: app_menu_items,
                },
                Menu {
                    name: translations.menu_view.into(),
                    items: view_menu_items,
                },
                Menu {
                    name: translations.menu_help.into(),
                    items: vec![
                        MenuItem::action("Screen Guide", ToggleScreenGuide),
                        MenuItem::action(
                            translations.menu_keyboard_shortcuts,
                            ToggleHelp,
                        ),
                    ],
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

            let _ = window;
        });
}

/// Copy bundled preset files to the user's plugin_presets directory.
/// Skips any file that already exists (never overwrites user presets).
fn install_default_presets() {
    let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
        log::warn!("Could not determine plugin presets directory, skipping default preset installation");
        return;
    };

    for path in Assets::iter() {
        if let Some(filename) = path.strip_prefix("presets/") {
            let dest = presets_dir.join(filename);
            if dest.exists() {
                log::debug!("Default preset already exists, skipping: {}", filename);
                continue;
            }
            if let Some(file) = Assets::get(&path) {
                match std::fs::write(&dest, &file.data) {
                    Ok(()) => log::info!("Installed default preset: {}", filename),
                    Err(e) => log::warn!("Failed to install default preset {}: {}", filename, e),
                }
            }
        }
    }
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
