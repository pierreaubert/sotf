use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use sotf_audio::run_preflight_checks;
use sotf_audio_player::{MusicLibrary, Player, PluginSettings, PluginType};
use sotf_audio_player_tui::app::{App, InputMode, Screen};
use sotf_audio_player_tui::events::{
    AppEvent, PlayerCommand, handle_events, handle_key_event, handle_media_control_event,
    poll_headphone_eq_optimization, poll_room_eq_optimization, poll_spinorama_optimization,
    poll_spinorama_speaker_load,
};
use sotf_audio_player_tui::media_controls::{self, TuiMediaControls};
use sotf_audio_player_tui::theme;
use sotf_audio_player_tui::ui;
use souvlaki::{MediaPlayback, MediaPosition};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "player")]
#[command(version, about = "SOTF TUI Music Player", long_about = None)]
struct Args {
    /// Initial directories to scan for music
    #[arg(short, long)]
    directories: Vec<PathBuf>,

    /// Auto-scan on startup
    #[arg(short, long)]
    scan: bool,

    /// Enable binaural decoder plugin
    #[arg(long)]
    binaural: bool,

    /// Path to SOFA file for binaural decoder
    #[arg(long)]
    sofa_file: Option<PathBuf>,

    /// Theme to use (dark or light)
    #[arg(long, default_value = "dark")]
    theme: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to file to avoid corrupting the TUI
    let log_path =
        sotf_audio_player::config::get_tui_log_path().ok_or("Could not determine log file path")?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .filter_level(log::LevelFilter::Debug)
        // Log all modules including Symphonia at debug level
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF UI Player starting...");

    // Run pre-flight checks before initializing the player
    if let Err(e) = run_preflight_checks() {
        eprintln!("\nPre-flight check failed:\n");
        eprintln!("{}\n", e);
        log::error!("Pre-flight check failed: {}", e);
        std::process::exit(1);
    }

    let args = Args::parse();

    // Parse theme
    let theme_type = theme::ThemeType::from_str(&args.theme).ok_or_else(|| {
        format!(
            "Invalid theme '{}', valid options are: dark, light",
            args.theme
        )
    })?;
    let theme = theme::Theme::from_type(theme_type);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and player
    let mut app = App::new(theme);
    let mut player = Player::new();

    // Show loading screen immediately
    app.current_screen = Screen::Loading;
    terminal.draw(|f| ui::draw(f, &mut app))?;

    // Set initial volume
    player.set_volume(app.volume)?;

    // Configure binaural decoder if requested
    if args.binaural {
        // Validate that SOFA file is provided
        let sofa_file = args
            .sofa_file
            .clone()
            .ok_or("Binaural decoder requires --sofa-file to be specified")?;

        // Determine input channels from existing plugin chain
        // This allows proper configuration when used after an upmixer
        let input_channels = app.plugin_chain.output_channels();

        // Add binaural decoder plugin to the chain (before permanent tail)
        let insert_idx = app.plugin_chain.user_plugin_insert_index();
        app.plugin_chain
            .insert_plugin(insert_idx, &PluginType::BinauralDecoder);

        // Configure the plugin with SOFA file path and detected input channels
        if let Some(plugin) = app.plugin_chain.get_plugin_mut(insert_idx) {
            plugin.settings = PluginSettings::BinauralDecoder {
                sofa_file: sofa_file.to_string_lossy().to_string(),
                input_channels,
                enable_optimization: true,
                externalization: 0.0,
                near_field_strength: 0.0,
            };
            log::info!(
                "Binaural decoder enabled with {} input channels, SOFA file: {:?}",
                input_channels,
                sofa_file
            );
        }
    }

    // Load available audio devices
    app.load_output_devices();
    app.load_recording_devices();

    // Add initial directories (without triggering rescan)
    for dir in args.directories {
        app.add_directory_quiet(dir);
    }

    // Load library from database in a background thread so we can animate the
    // loading screen. The library is moved to the thread and moved back when done.
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

                        if let Err(e) = app.start_waveform_scan() {
                            log::warn!("Failed to start waveform scan: {}", e);
                        }
                        if let Err(e) = app.start_bliss_scan() {
                            log::warn!("Failed to start bliss scan: {}", e);
                        }
                        db_empty = album_count == 0;
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    log::error!("Library loading thread panicked");
                    // Restore a fresh library with database
                    app.library = MusicLibrary::with_database().unwrap_or_else(|_| MusicLibrary::new());
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
    if let Err(e) = app.load_config() {
        log::warn!("Could not load saved configuration: {}", e);
    }

    // Auto-scan if:
    // 1. Explicit --scan flag provided, OR
    // 2. Database is empty and we have directories to scan
    let will_scan = (args.scan || db_is_empty) && !app.library.directories.is_empty();
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

    // Set initial screen based on queue state (if not scanning)
    if !will_scan {
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
    }

    // Initialize media controls (best-effort: works without them on headless servers)
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

    // Main loop
    let result = run_app(&mut terminal, &mut app, &mut player, &mut media_controls);

    // Save configuration before exit
    if let Err(e) = app.save_config() {
        log::error!("Failed to save configuration: {}", e);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    log::info!("SOTF UI Player exiting...");

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
    loop {
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
                            app.input_mode = InputMode::ShowError;
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
                            app.input_mode = InputMode::ShowError;
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
                    if poll_room_eq_optimization(app) {
                        app.needs_redraw = true;
                    }
                    // Poll speaker-load result (non-blocking, no-op when not loading)
                    if poll_spinorama_speaker_load(app) {
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
                        .plugin_chain
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
                        app.input_mode = InputMode::ShowError;
                        app.is_playing = false;
                    } else if let Some(err) = state.last_error {
                        log::error!("[TUI] Playback error: {}", err);
                        app.error_message = Some(err);
                        app.input_mode = InputMode::ShowError;
                        app.is_playing = false;
                    } else if state.engine_restarted {
                        log::info!("[TUI] Engine auto-restarted after crash, resuming playback");
                    } else if app.is_playing
                        && !state.is_playing
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

                            // Check for upmixer + non-stereo conflict
                            if track_channels != 2
                                && let Some(upmixer_idx) =
                                    app.plugin_chain.find_plugin_index(&PluginType::Upmixer)
                                && app
                                    .plugin_chain
                                    .plugins()
                                    .get(upmixer_idx)
                                    .is_some_and(|p| p.enabled)
                            {
                                log::info!(
                                    "[TUI] Auto-advance channel conflict: {}ch file with upmixer enabled",
                                    track_channels
                                );
                                app.channel_conflict_path = Some(path);
                                app.channel_conflict_selection = 0;
                                app.channel_conflict_track_channels = track_channels;
                                app.input_mode = InputMode::ChannelConflict;
                                update_media_controls(app, player, media_controls);
                                continue;
                            }

                            if let Err(e) = start_playback(player, app, path, track_channels) {
                                log::error!("[TUI] Failed to auto-advance: {}", e);
                                app.error_message = Some(format!("Auto-advance failed: {}", e));
                                app.input_mode = InputMode::ShowError;
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
                                app.plugin_chain.set_replay_gain(rg_gain);

                                let sample_rate = app
                                    .current_sample_rate
                                    .map(|r| r as f64)
                                    .unwrap_or_else(|| app.get_current_sample_rate());
                                let plugins = app.plugin_chain.to_plugin_configs(sample_rate);

                                match player.update_plugins(plugins) {
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
                }
                AppEvent::Resize => {
                    // Terminal resized, will redraw on next iteration
                }
            }
        }

        // Pump macOS event loop so media key callbacks are delivered
        media_controls::pump_macos_event_loop();

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Start playback for a file, handling matrix adaptation and channel clamping.
fn start_playback(
    player: &mut Player,
    app: &mut App,
    path: PathBuf,
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

    app.plugin_chain.adapt_matrix_to_input(track_channels);

    // Apply ReplayGain correction to the permanent Gain plugin
    let rg_gain = app.get_replay_gain_for_current_track();
    app.plugin_chain.set_replay_gain(rg_gain);

    let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
    let mut output_channels = app.plugin_chain.output_channels_for_input(track_channels);

    // Clamp output channels to device max — the playback thread will
    // downmix automatically when the processing chain outputs more
    // channels than the hardware supports.
    if let Some(max_channels) = app.get_device_max_channels() {
        if output_channels > max_channels {
            log::info!(
                "[TUI] Clamping output from {} to {} channels (device limit)",
                output_channels,
                max_channels
            );
            output_channels = max_channels;
        }
    }

    let path_clone = path.clone();
    player.load_and_play(
        path,
        plugins,
        output_channels,
        app.current_output_device_name.clone(),
    )?;

    app.start_track_tracking(path_clone);
    Ok(())
}

fn handle_player_command(
    player: &mut Player,
    app: &mut App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Stop tracking previous track if any
            app.stop_track_tracking();

            // Load album images when starting playback
            app.load_album_images();

            let track_channels = app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;

            // Check for upmixer + non-stereo conflict before attempting playback.
            // The upmixer only accepts stereo (2ch) input.
            if track_channels != 2
                && let Some(upmixer_idx) = app.plugin_chain.find_plugin_index(&PluginType::Upmixer)
                && app
                    .plugin_chain
                    .plugins()
                    .get(upmixer_idx)
                    .is_some_and(|p| p.enabled)
            {
                log::info!(
                    "[TUI] Channel conflict: {}ch file with upmixer enabled",
                    track_channels
                );
                app.channel_conflict_path = Some(path);
                app.channel_conflict_selection = 0;
                app.channel_conflict_track_channels = track_channels;
                app.input_mode = InputMode::ChannelConflict;
                return Ok(());
            }

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
