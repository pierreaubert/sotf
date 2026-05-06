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
use sotf_audio_player_tui::app::{App, InputMode, Screen};
use sotf_audio_player_tui::events::{
    AppEvent, PlayerCommand, handle_events, handle_key_event, handle_media_control_event,
    poll_delay_detection, poll_federation_scan, poll_federation_test, poll_headphone_download,
    poll_headphone_eq_optimization, poll_headphone_list_load, poll_probe_capture, poll_recording,
    poll_room_eq_optimization, poll_spinorama_optimization, poll_spinorama_speaker_load,
};
use sotf_audio_player_tui::media_controls::{self, TuiMediaControls};
use sotf_audio_player_tui::ui;
use sotf_media_controls::{MediaPlayback, MediaPosition};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Try to acquire an exclusive advisory lock on `sotf.lock` in the config dir.
/// Returns the open `File` (must be held for process lifetime) and whether the
/// exclusive lock was obtained. If not, a second instance is already running.
#[cfg(unix)]
fn try_acquire_lock(config_dir: &Path) -> (File, bool) {
    use std::os::unix::io::AsRawFd;

    let lock_path = config_dir.join("sotf.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("Failed to open lock file");

    let exclusive = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 };
    (file, exclusive)
}

#[cfg(windows)]
fn try_acquire_lock(config_dir: &Path) -> (File, bool) {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(
            hFile: *mut core::ffi::c_void,
            dwFlags: u32,
            dwReserved: u32,
            nNumberOfBytesToLockLow: u32,
            nNumberOfBytesToLockHigh: u32,
            lpOverlapped: *mut Overlapped,
        ) -> i32;
    }

    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x00000002;
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x00000001;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut core::ffi::c_void,
    }

    let lock_path = config_dir.join("sotf.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("Failed to open lock file");

    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: core::ptr::null_mut(),
    };

    // SAFETY: LockFileEx is a well-defined Win32 API. We pass a valid file handle
    // and a zeroed OVERLAPPED struct for a synchronous non-blocking lock attempt.
    let exclusive = unsafe {
        LockFileEx(
            file.as_raw_handle() as *mut core::ffi::c_void,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        ) != 0
    };
    (file, exclusive)
}

#[cfg(not(any(unix, windows)))]
fn try_acquire_lock(_config_dir: &Path) -> (File, bool) {
    let file = tempfile::tempfile().expect("Failed to create temp lock file");
    (file, true)
}

#[derive(clap::Parser)]
struct Args {
    /// Force a full library rescan on startup
    #[arg(long)]
    scan: bool,

    /// Use a custom data directory (for QA testing)
    #[arg(long)]
    qa: Option<PathBuf>,

    /// Number of scanner threads for waveform/bliss/replaygain analysis (1-8, default: auto)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=8))]
    scanner_threads: Option<u8>,

    /// Run in headless server mode (MPD/DLNA) without UI
    #[arg(long)]
    server: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging to file (stderr is invisible in a TUI)
    if let Some(log_path) = sotf_audio_player::config::get_tui_log_path() {
        let log_result = OpenOptions::new().create(true).append(true).open(&log_path);

        if let Ok(log_file) = log_result {
            env_logger::Builder::from_default_env()
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .filter_level(log::LevelFilter::Info)
                .filter_module("symphonia_core", log::LevelFilter::Warn)
                .init();
        } else {
            env_logger::init();
        }
    } else {
        env_logger::init();
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
    let args: Args = clap::Parser::parse();

    // Apply QA directory override before any config dir access
    if let Some(qa_dir) = args.qa {
        sotf_audio_player::config::set_config_dir_override(qa_dir);
    }

    // Headless server mode — skip UI entirely
    if args.server {
        match sotf_audio_player::server::run_server_mode() {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
    }

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
            app.loading_tick = app.loading_tick.wrapping_add(1);
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

    // Main loop
    let result = run_app(&mut terminal, &mut app, &mut player, &mut media_controls);

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

fn run_app<B: ratatui::backend::Backend<Error: 'static>>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    player: &mut Player,
    media_controls: &mut Option<TuiMediaControls>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initial media control update (important for macOS to see the app as a media player)
    update_media_controls(app, player, media_controls);

    loop {
        // Pump macOS event loop so media key callbacks are delivered BEFORE we poll for events.
        media_controls::pump_macos_event_loop();

        // Draw UI only if needed
        if app.needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            app.needs_redraw = false;
        }

        // Handle events
        if let Some(event) = handle_events(Duration::from_millis(100), media_controls.as_ref())? {
            app.needs_redraw = true;
            match event {
                AppEvent::Key(key) => {
                    if let Some(cmd) = handle_key_event(app, key) {
                        if let Err(e) = handle_player_command(player, app, cmd) {
                            log::error!("[TUI] Player command error: {}", e);
                            app.error_message = Some(e.to_string());
                            app.enter_overlay_mode(InputMode::ShowError);
                            app.is_playing = false;
                        }
                        update_media_controls(app, player, media_controls);
                    }
                }
                AppEvent::MediaControl(event) => {
                    if let Some(cmd) = handle_media_control_event(app, event) {
                        if let Err(e) = handle_player_command(player, app, cmd) {
                            log::error!("[TUI] Media control command error: {}", e);
                            app.error_message = Some(e.to_string());
                            app.enter_overlay_mode(InputMode::ShowError);
                            app.is_playing = false;
                        }
                        update_media_controls(app, player, media_controls);
                    }
                }
                AppEvent::Tick => {
                    // Poll optimizer progress (non-blocking, no-op when not running)
                    if poll_spinorama_optimization(app) {
                        app.needs_redraw = true;
                    }
                    if poll_headphone_eq_optimization(app) {
                        app.needs_redraw = true;
                    }
                    if poll_headphone_list_load(app) {
                        app.needs_redraw = true;
                    }
                    if poll_headphone_download(app) {
                        app.needs_redraw = true;
                    }
                    if poll_room_eq_optimization(app) {
                        app.needs_redraw = true;
                    }
                    if poll_delay_detection(app) {
                        app.needs_redraw = true;
                    }
                    if poll_recording(app) {
                        app.needs_redraw = true;
                    }
                    if poll_probe_capture(app) {
                        app.needs_redraw = true;
                    }
                    // Poll speaker-load result (non-blocking, no-op when not loading)
                    if poll_spinorama_speaker_load(app) {
                        app.needs_redraw = true;
                    }
                    if poll_federation_scan(app) {
                        app.needs_redraw = true;
                    }
                    if poll_federation_test(app) {
                        app.needs_redraw = true;
                    }

                    let state = player.get_playback_state();

                    // Pause background scanners while playing to avoid CPU starvation,
                    // unless the user explicitly started a scan.
                    app.scanner_pause_flag.store(
                        app.is_playing && state.is_playing && !app.scanner_pause_override,
                        std::sync::atomic::Ordering::Relaxed,
                    );

                    // Update app state
                    app.position_secs = state.position_secs;
                    // Read loudness from cache using plugin chain's engine index
                    app.loudness_info = app
                        .plugin_graph
                        .output_monitor_engine_index()
                        .and_then(|idx| player.get_cached_plugin_data(idx))
                        .and_then(|d| d.downcast_ref::<sotf_audio_player::LoudnessData>().cloned());
                    app.current_sample_rate = state.sample_rate;
                    app.needs_redraw = true; // Always redraw on tick to update meters/position

                    // Redraw while scanning or processing
                    if app.scan_in_progress
                        || app.maintenance_in_progress
                        || app.replay_gain_manager.in_progress
                        || app.waveform_manager.in_progress
                        || app.bliss_manager.in_progress
                    {
                        app.needs_redraw = true;
                    }

                    // Check if we should record a play (30s threshold)
                    if app.is_playing && state.is_playing {
                        app.check_and_record_play();
                    }

                    // Engine crash handling (priority order: fatal > error > restarted > auto-advance)
                    if state.engine_fatal {
                        log::error!("[TUI] Engine crashed fatally, cannot auto-restart");
                        app.error_message = Some(
                            "Audio engine crashed. Please play a new track to restart.".to_string(),
                        );
                        app.enter_overlay_mode(InputMode::ShowError);
                        app.is_playing = false;
                    } else if let Some(err) = state.last_error {
                        log::error!("[TUI] Playback error: {}", err);
                        app.error_message = Some(err);
                        app.enter_overlay_mode(InputMode::ShowError);
                        app.is_playing = false;
                    } else if state.engine_restarted {
                        log::info!("[TUI] Engine auto-restarted after crash, resuming playback");
                    } else if let Some(_transition_source) = state.gapless_transition {
                        // Gapless transition — engine already playing the new file,
                        // just advance the queue UI to match.
                        log::info!("[TUI] Gapless transition detected");
                        app.stop_track_tracking();
                        let _ = app.next_track();
                        if let Some(path) = app.current_track_path() {
                            app.start_track_tracking(path);
                        }
                        update_media_controls(app, player, media_controls);
                    } else if (state.track_ended || (app.is_playing && !state.is_playing))
                        && app.current_queue_index.is_some()
                    {
                        log::info!("[TUI] Track ended, attempting auto-advance...");
                        // Track ended cleanly, stop tracking the previous track
                        app.stop_track_tracking();

                        // Advance to next
                        if let Some(path) = app.next_track() {
                            log::info!("[TUI] Auto-advancing to: {:?}", path);

                            let track_channels =
                                app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;

                            // Clear suspensions from previous track and check for conflicts
                            app.plugin_graph.clear_suspensions();
                            app.plugin_graph.update_channel_dependent_plugins();

                            let conflicts = app.plugin_graph.find_channel_conflicts(track_channels);
                            if !conflicts.is_empty() {
                                log::info!(
                                    "[TUI] Auto-advance channel conflict: {}ch file with {} incompatible plugin(s)",
                                    track_channels,
                                    conflicts.len()
                                );
                                // Auto-suspend without modal (user already consented by continuing playback)
                                let indices: Vec<usize> =
                                    conflicts.iter().map(|c| c.index).collect();
                                app.plugin_graph.suspend_plugins(&indices);
                                app.plugin_graph.update_channel_dependent_plugins();
                            }

                            if let Err(e) = start_playback(player, app, path, track_channels) {
                                log::error!("[TUI] Failed to auto-advance: {}", e);
                                app.error_message = Some(format!("Auto-advance failed: {}", e));
                                app.enter_overlay_mode(InputMode::ShowError);
                                app.is_playing = false;
                            } else {
                                log::info!("[TUI] Auto-advance successful");
                            }
                        } else {
                            log::info!("[TUI] No more tracks in queue, stopping playback");
                            app.is_playing = false;
                        }
                        update_media_controls(app, player, media_controls);
                    }

                    // Gapless pre-queuing: when near end of track, queue the next file
                    if state.is_playing && app.current_queue_index.is_some() {
                        let position = state.position_secs;
                        let duration = app
                            .current_track()
                            .and_then(|t| t.duration_secs)
                            .unwrap_or(0) as f64;
                        let near_end =
                            duration > 0.0 && position > 0.0 && (duration - position) < 10.0;

                        if near_end && let Some(next_track) = app.peek_next_track() {
                            let next_ch = next_track.channels.unwrap_or(2) as usize;
                            let current_ch =
                                app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;

                            // Only gapless when channel count matches (engine constraint)
                            if next_ch == current_ch {
                                let _ = player.queue_next(next_track.path.clone());
                            }
                        }
                    }

                    // Apply pending plugin updates with debouncing and retry logic
                    if app.needs_plugin_update && !app.plugin_update_in_progress {
                        const MAX_RETRIES: u32 = 3;
                        const DEBOUNCE_MS: u64 = 500;

                        // Check if enough time has passed since last attempt
                        let should_attempt = match app.plugin_update_last_attempt {
                            None => true,
                            Some(last) => last.elapsed().as_millis() >= DEBOUNCE_MS as u128,
                        };

                        if should_attempt {
                            // Check retry limit
                            if app.plugin_update_retry_count >= MAX_RETRIES {
                                log::error!(
                                    "[TUI] Plugin update failed after {} retries, giving up",
                                    MAX_RETRIES
                                );
                                app.status_message = Some(format!(
                                    "Plugin update failed after {} retries. Check logs for details.",
                                    MAX_RETRIES
                                ));
                                app.needs_plugin_update = false;
                                app.plugin_update_retry_count = 0;
                                app.plugin_update_in_progress = false;
                            } else {
                                // Mark update as in progress and clear the trigger flag immediately
                                app.plugin_update_in_progress = true;
                                app.needs_plugin_update = false;
                                app.plugin_update_last_attempt = Some(std::time::Instant::now());

                                log::debug!(
                                    "[TUI] Attempting plugin update (attempt {}/{})",
                                    app.plugin_update_retry_count + 1,
                                    MAX_RETRIES
                                );

                                // Recompute replay gain before building plugin configs
                                let rg_gain = app.get_replay_gain_for_current_track();
                                app.plugin_graph.set_replay_gain(rg_gain);

                                let sample_rate = app
                                    .current_sample_rate
                                    .map(|r| r as f64)
                                    .unwrap_or_else(|| app.get_current_sample_rate());

                                // Branch on graph topology — flattening a
                                // non-linear graph through `to_plugin_configs`
                                // silently drops parallel branches and
                                // routed bass management. Same fix as the
                                // GPUI app's structural-flush path.
                                let result = if app.plugin_graph.is_linear() {
                                    let plugins = app.plugin_graph.to_plugin_configs(sample_rate);
                                    player.update_plugins(plugins)
                                } else {
                                    let config =
                                        app.plugin_graph.to_plugin_graph_config(sample_rate);
                                    log::info!(
                                        "[TUI] Plugin update (graph): {} nodes, {} edges",
                                        config.nodes.len(),
                                        config.edges.len()
                                    );
                                    player.update_plugin_graph(config)
                                };

                                match result {
                                    Ok(()) => {
                                        log::info!("[TUI] Plugin update successful");
                                        app.status_message =
                                            Some("Plugin chain updated".to_string());
                                        app.plugin_update_retry_count = 0;
                                        app.plugin_update_in_progress = false;
                                    }
                                    Err(e) => {
                                        app.plugin_update_retry_count += 1;
                                        app.plugin_update_in_progress = false;

                                        log::warn!(
                                            "[TUI] Plugin update failed (attempt {}/{}): {}",
                                            app.plugin_update_retry_count,
                                            MAX_RETRIES,
                                            e
                                        );

                                        if app.plugin_update_retry_count < MAX_RETRIES {
                                            // Retry on next tick (after debounce delay)
                                            app.needs_plugin_update = true;
                                            app.status_message = Some(format!(
                                                "Plugin update failed, retrying... ({}/{})",
                                                app.plugin_update_retry_count, MAX_RETRIES
                                            ));
                                        } else {
                                            // Max retries reached
                                            app.status_message =
                                                Some(format!("Plugin update failed: {}", e));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Apply pending parameter updates (zero-dropout updates)
                    if let Some(param_update) = app.pending_param_update.take() {
                        log::debug!(
                            "[TUI] Applying parameter update: plugin {} param {} = {}",
                            param_update.plugin_index,
                            param_update.param_id,
                            param_update.value
                        );

                        match player.set_plugin_parameter(
                            param_update.plugin_index,
                            param_update.param_id,
                            param_update.value,
                        ) {
                            Ok(()) => {
                                log::debug!("[TUI] Parameter updated successfully");
                            }
                            Err(e) => {
                                log::warn!("[TUI] Failed to update parameter: {}", e);
                            }
                        }
                    }

                    // Start library scan if needed (non-blocking)
                    if app.needs_rescan && !app.scan_in_progress {
                        // Switch to directory view so user can see scan progress
                        app.current_screen = Screen::Configure;
                        app.start_library_scan();
                    }

                    // Check library scan progress
                    app.check_library_scan_progress();

                    // Check ReplayGain scan progress
                    app.check_replay_gain_progress();

                    // Check waveform scan progress
                    app.check_waveform_progress();

                    // Check bliss scan progress
                    app.check_bliss_progress();

                    // Regularly sync media controls (position, state)
                    update_media_controls(app, player, media_controls);
                }
                AppEvent::Resize => {
                    // Terminal resized, will redraw on next iteration
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Start playback for an audio source, handling matrix adaptation and channel clamping.
fn start_playback(
    player: &mut Player,
    app: &mut App,
    source: sotf_audio::decoder::AudioSource,
    track_channels: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let track_sample_rate = app
        .current_track()
        .and_then(|t| t.sample_rate)
        .unwrap_or(48000);
    let sample_rate = app.get_target_sample_rate(track_sample_rate);

    log::info!(
        "[TUI] Starting playback: track={}Hz, target={}Hz, device_default={}Hz",
        track_sample_rate,
        sample_rate,
        app.get_current_sample_rate()
    );

    app.plugin_graph.adapt_matrix_to_input(track_channels);

    // Apply ReplayGain correction to the permanent Gain plugin
    let rg_gain = app.get_replay_gain_for_current_track();
    app.plugin_graph.set_replay_gain(rg_gain);

    let plugins = app.plugin_graph.to_plugin_configs(sample_rate);
    let mut output_channels = app.plugin_graph.output_channels_for_input(track_channels);

    let device_max = app.get_device_max_channels();
    log::info!(
        "[TUI] Plugin chain wants {} output channels, device max = {:?}",
        output_channels,
        device_max,
    );

    // Clamp output channels to device max — the playback thread will
    // downmix automatically when the processing chain outputs more
    // channels than the hardware supports.
    if let Some(max_channels) = device_max
        && output_channels > max_channels
    {
        log::info!(
            "[TUI] Clamping output from {} to {} channels (device limit)",
            output_channels,
            max_channels
        );
        output_channels = max_channels;
    }

    // Sync volume to the engine before playback starts
    player.set_volume(app.volume)?;

    let source_path = source.as_path().map(|p| p.to_path_buf());
    player.load_and_play_source(
        source,
        plugins,
        output_channels,
        app.current_output_device_name.clone(),
    )?;

    if let Some(path) = source_path {
        app.start_track_tracking(path);
    }
    Ok(())
}

fn handle_player_command(
    player: &mut Player,
    app: &mut App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Cancel any pending gapless queue before manual play
            let _ = player.cancel_next();
            // Stop tracking previous track if any
            app.stop_track_tracking();

            // Load album images when starting playback
            #[cfg(not(target_os = "windows"))]
            app.load_album_images();

            let track_channels = app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;

            // Clear suspensions from previous track
            app.plugin_graph.clear_suspensions();
            app.plugin_graph.update_channel_dependent_plugins();

            // Check for channel conflicts with all fixed-channel plugins
            let conflicts = app.plugin_graph.find_channel_conflicts(track_channels);
            if !conflicts.is_empty() {
                log::info!(
                    "[TUI] Channel conflict: {}ch file with {} incompatible plugin(s)",
                    track_channels,
                    conflicts.len()
                );
                app.channel_conflicts = conflicts;
                app.channel_conflict_path = Some(path);
                app.channel_conflict_selection = 0;
                app.channel_conflict_track_channels = track_channels;
                app.enter_overlay_mode(InputMode::ChannelConflict);
                return Ok(());
            }

            start_playback(player, app, path, track_channels)?;
        }
        PlayerCommand::PlayResolved(path) => {
            // Play after channel conflict was resolved — skip clearing suspensions
            // and conflict re-check since the user already handled it.
            app.stop_track_tracking();
            #[cfg(not(target_os = "windows"))]
            app.load_album_images();
            let track_channels = app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;
            start_playback(player, app, path, track_channels)?;
        }
        PlayerCommand::Pause => {
            player.pause()?;
        }
        PlayerCommand::Resume => {
            player.resume()?;
        }
        PlayerCommand::Stop => {
            player.stop()?;
            // Stop tracking when playback stops
            app.stop_track_tracking();
        }
        PlayerCommand::SetVolume(volume) => {
            player.set_volume(volume)?;
        }
        PlayerCommand::SetOutputDevice(device_name) => {
            // Store the device name for future playback
            app.current_output_device_name = Some(device_name.clone());
            player.set_output_device(device_name.clone())?;
            app.status_message = Some(format!(
                "Output device set to '{}'; will be used for next playback",
                device_name
            ));
            log::info!("Output device changed");
        }
        PlayerCommand::Seek(position) => {
            player.seek(position)?;
            log::info!("Seeked to {} seconds", position);
        }
        PlayerCommand::SeekRelative(offset) => {
            let current_pos = player.get_position();
            let new_pos = (current_pos + offset).max(0.0);
            player.seek(new_pos)?;
            log::info!(
                "Seeked {} seconds (from {} to {})",
                offset,
                current_pos,
                new_pos
            );
        }
        PlayerCommand::ToggleMute => {
            app.muted = !app.muted;
            player.set_mute(app.muted)?;
            log::info!("Mute toggled: {}", app.muted);
        }
    }
    Ok(())
}

fn update_media_controls(
    app: &App,
    player: &Player,
    media_controls: &mut Option<TuiMediaControls>,
) {
    let Some(mc) = media_controls.as_mut() else {
        return;
    };

    // Update metadata from current track
    let track = app.current_track();
    let album_title = app
        .current_queue_index
        .and_then(|idx| app.queue.get(idx))
        .map(|entry| entry.item.album.title.as_str());

    let title_owned: String;
    let artist_owned: String;

    let (title, artist) = match track {
        Some(t) => {
            title_owned = t.title.clone().unwrap_or_default();
            artist_owned = t.artist.clone().unwrap_or_default();
            (
                if title_owned.is_empty() {
                    None
                } else {
                    Some(title_owned.as_str())
                },
                if artist_owned.is_empty() {
                    None
                } else {
                    Some(artist_owned.as_str())
                },
            )
        }
        None => (None, None),
    };

    let duration = track.and_then(|t| t.duration_secs).map(Duration::from_secs);

    // Build cover URL from album art path (file:// URL for macOS)
    let cover_url_owned = app
        .current_queue_index
        .and_then(|idx| app.queue.get(idx))
        .and_then(|entry| entry.item.album.album_art_path.as_ref())
        .filter(|path| path.exists())
        .map(|path| format!("file://{}", path.display()));
    let cover_url = cover_url_owned.as_deref();

    mc.set_metadata(title, artist, album_title, duration, cover_url);

    // Update playback state
    let position_secs = player.get_position();
    let progress = Some(MediaPosition(Duration::from_secs_f64(position_secs)));

    let playback = if app.is_playing {
        MediaPlayback::Playing { progress }
    } else if app.current_queue_index.is_some() {
        MediaPlayback::Paused { progress }
    } else {
        MediaPlayback::Stopped
    };

    mc.set_playback(playback);
}
