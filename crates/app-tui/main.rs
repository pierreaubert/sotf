use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use sotf_audio_player::Player;
use sotf_audio_player_tui::app::{App, Screen};
use sotf_audio_player_tui::media_controls::TuiMediaControls;
use sotf_audio_player_tui::ui;
use std::fs::OpenOptions;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

#[path = "main/misc.rs"]
mod misc;
#[path = "main/try_.rs"]
mod try_;
#[path = "main/types.rs"]
mod types;

use misc::print_sotf_api_connection_qr;
#[cfg(not(any(unix, windows)))]
use try_::try_acquire_lock;
#[cfg(unix)]
use try_::try_acquire_lock;
#[cfg(windows)]
use try_::try_acquire_lock;
use types::Args;
#[cfg(not(feature = "dev-api"))]
use types::DevApiRx;
use types::run_app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI args before touching the terminal. Server mode and clap's
    // help/error output must run in a normal terminal, not raw alt-screen mode.
    let args: Args = clap::Parser::parse();

    // Apply QA directory override before any config dir access, including logs.
    if let Some(qa_dir) = args.qa.clone() {
        sotf_audio_player::config::set_config_dir_override(qa_dir);
    }

    // Setup logging to file (stderr is invisible in a TUI).
    let tui_log_path = sotf_audio_player::config::get_tui_log_path();
    if let Some(log_path) = &tui_log_path {
        let log_result = OpenOptions::new().create(true).append(true).open(log_path);

        if let Ok(log_file) = log_result {
            env_logger::Builder::from_default_env()
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .filter_level(log::LevelFilter::Info)
                .filter_module("symphonia_core", log::LevelFilter::Warn)
                .init();
        } else {
            env_logger::Builder::from_default_env()
                .filter_level(log::LevelFilter::Info)
                .filter_module("symphonia_core", log::LevelFilter::Warn)
                .init();
        }
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .filter_module("symphonia_core", log::LevelFilter::Warn)
            .init();
    }

    if args.qr {
        print_sotf_api_connection_qr()?;
        std::process::exit(0);
    }

    // Headless server mode — skip UI entirely
    if args.server {
        if let Some(log_path) = tui_log_path {
            eprintln!("TUI log file: {}", log_path.display());
        }
        match sotf_audio_player::server::run_server_mode() {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Install panic hook BEFORE entering alt screen so panics restore the terminal
    // and the backtrace remains visible after the app exits.
    let original_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_panic_hook(info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    // Enable Kitty keyboard protocol so modifiers (Shift, Ctrl, etc.) are
    // reported with arrow keys and other special keys.
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    );
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize app state
    let t_startup = std::time::Instant::now();
    let theme = sotf_audio_player_tui::theme::Theme::default();

    // Try to acquire exclusive lock — second instance becomes read-only
    let config_dir = sotf_audio_player::config::get_app_config_dir()
        .expect("Could not determine config directory");
    let (_lock_file, lock_acquired) = try_acquire_lock(&config_dir);
    let read_only = !lock_acquired;
    if read_only {
        log::info!("Another instance is running — starting in read-only mode");
    }

    let t0 = std::time::Instant::now();
    let mut app = App::new(theme, read_only);
    log::info!(
        "[startup] App::new: {:.1}ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );

    // Apply scanner thread count from CLI
    if let Some(threads) = args.scanner_threads {
        app.set_scanner_threads(Some(threads as usize));
    }

    // Initialize audio player
    let mut player = Player::new();

    // Load music library (asynchronously if requested)
    use sotf_audio_player::MusicLibrary;
    let db_is_empty = if !args.scan {
        // Take the library (with its DB connection) for background loading
        let mut library = std::mem::replace(&mut app.library, MusicLibrary::new());
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = library.load_from_database().map_err(|e| e.to_string());
            let _ = tx.send((library, result));
        });

        // Animate loading screen while waiting for the library to load
        let mut db_empty = true;
        loop {
            app.ui.loading_tick = app.ui.loading_tick.wrapping_add(1);
            terminal.draw(|f| ui::draw(f, &mut app))?;

            match rx.try_recv() {
                Ok((loaded_library, result)) => {
                    app.library = loaded_library;
                    if let Err(e) = &result {
                        log::warn!("Failed to load library from database: {}", e);
                        db_empty = true;
                    } else {
                        let album_count = app.library.albums.len();
                        log::info!("Loaded library from database: {} albums", album_count);
                        app.rebuild_artist_tree();
                        app.update_directory_scan_times();
                        app.request_filter_update();

                        if !read_only {
                            // Start waveform scan first; bliss scan will start
                            // automatically when waveform completes to avoid
                            // excessive concurrent memory usage from parallel
                            // full-file decodings.
                            if let Err(e) = app.start_waveform_scan() {
                                log::warn!("Failed to start waveform scan: {}", e);
                            }
                        }
                        db_empty = album_count == 0;
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    log::error!("Library loading thread panicked");
                    // Restore a fresh library with database
                    let fallback = if read_only {
                        MusicLibrary::with_database_secondary()
                    } else {
                        MusicLibrary::with_database()
                    };
                    app.library = fallback.unwrap_or_else(|_| MusicLibrary::new());
                    break;
                }
            }

            std::thread::sleep(Duration::from_millis(40));
        }
        db_empty
    } else {
        false // Explicit scan requested
    };

    // Load saved configuration
    let t0 = std::time::Instant::now();
    if let Err(e) = app.load_config() {
        log::warn!("Could not load saved configuration: {}", e);
    }
    log::info!(
        "[startup] load_config: {:.1}ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );

    // Load available audio devices (single enumeration for both output + recording)
    app.load_all_audio_devices();

    // Auto-scan if:
    // 1. Explicit --scan flag provided, OR
    // 2. Database is empty and we have directories to scan
    // Never scan in read-only mode
    let will_scan = !read_only && (args.scan || db_is_empty) && !app.library.directories.is_empty();
    if will_scan {
        log::info!(
            "Starting library scan (scan={}, db_empty={}, dirs={})",
            args.scan,
            db_is_empty,
            app.library.directories.len()
        );
        // Use non-blocking scan - progress will be checked in the main loop
        app.start_library_scan();
    }

    // Transition from loading screen to the appropriate initial screen
    if !app.queue.is_empty() {
        app.current_screen = Screen::Queue;
        log::info!(
            "Starting with Queue view (queue has {} items)",
            app.queue.len()
        );
    } else {
        app.current_screen = Screen::Library;
        log::info!("Starting with Library view (queue is empty)");
    }

    // Initialize media controls (best-effort: works without them on headless servers)
    let t0 = std::time::Instant::now();
    let mut media_controls = match TuiMediaControls::new() {
        Ok(mc) => {
            log::info!("Media controls initialized (Now Playing + media keys)");
            Some(mc)
        }
        Err(e) => {
            log::warn!(
                "Media controls unavailable: {}. Continuing without them.",
                e
            );
            None
        }
    };
    log::info!(
        "[startup] media_controls: {:.1}ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );
    log::info!(
        "[startup] TUI total startup: {:.1}ms",
        t_startup.elapsed().as_secs_f64() * 1000.0
    );

    // Start dev-api server if requested (QA/debug builds only)
    #[cfg(feature = "dev-api")]
    let dev_api_rx = std::env::var("SOTF_DEV_API_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .map(sotf_audio_player_tui::dev_api::start);

    #[cfg(not(feature = "dev-api"))]
    let dev_api_rx: Option<DevApiRx> = None;

    // Main loop
    let result = run_app(
        &mut terminal,
        &mut app,
        &mut player,
        &mut media_controls,
        dev_api_rx,
    );

    // Save configuration before exit (skip in read-only mode)
    if !app.read_only
        && let Err(e) = app.save_config()
    {
        log::error!("Failed to save configuration: {}", e);
    }

    // Restore terminal
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    log::info!("SOTF UI Player exiting...");

    // Surface any error from the main loop now that the alt screen is gone — otherwise
    // it gets eaten when the terminal switches back and the user only sees a blank exit.
    if let Err(e) = &result {
        eprintln!("sotf-tui: error: {e}");
        let mut src = e.source();
        while let Some(s) = src {
            eprintln!("  caused by: {s}");
            src = s.source();
        }
    }

    // Force exit — audio engine threads (decoder, processing, playback) use blocking
    // channel operations that can deadlock during sequential shutdown. The OS will
    // clean up all threads. Config is already saved and terminal is restored above.
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}
