mod app;
mod config;
mod database;
mod events;
mod library;
mod player;
mod plugins;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use events::{AppEvent, PlayerCommand, handle_events, handle_key_event};
use player::Player;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "sotf-player")]
#[command(about = "SOTF TUI Music Player", long_about = None)]
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to file to avoid corrupting the TUI
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("sotf_ui_player.log")?;

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .filter_level(log::LevelFilter::Debug)
        // Log all modules including Symphonia at debug level
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF UI Player starting...");

    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and player
    let mut app = App::new();
    let mut player = Player::new();

    // Enable loudness monitoring
    let _ = player.enable_loudness_monitoring();

    // Configure binaural decoder if requested
    if args.binaural {
        use plugins::{PluginSettings, PluginType};

        // Validate that SOFA file is provided
        let sofa_file = args.sofa_file.clone().ok_or(
            "Binaural decoder requires --sofa-file to be specified"
        )?;

        // Determine input channels from existing plugin chain
        // This allows proper configuration when used after an upmixer
        let input_channels = app.plugin_chain.output_channels();

        // Add binaural decoder plugin to the chain
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);

        // Configure the plugin with SOFA file path and detected input channels
        if let Some(plugin) = app.plugin_chain.get_plugin_mut(plugin_idx) {
            plugin.settings = PluginSettings::BinauralDecoder {
                sofa_file: sofa_file.to_string_lossy().to_string(),
                input_channels,
            };
            log::info!(
                "Binaural decoder enabled with {} input channels, SOFA file: {:?}",
                input_channels,
                sofa_file
            );
        }
    }

    // Load available output devices
    app.load_output_devices();

    // Add initial directories (without triggering rescan)
    for dir in args.directories {
        app.add_directory_quiet(dir);
    }

    // Load from database if no scan is requested
    let db_is_empty = if !args.scan {
        if let Err(e) = app.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
            true // Treat as empty if load fails
        } else {
            let album_count = app.library.albums.len();
            log::info!("Loaded library from database: {} albums", album_count);
            album_count == 0
        }
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
        if let Err(e) = app.scan_library() {
            log::error!("Failed to scan library: {}", e);
        }
    }

    // Set initial screen based on queue state (if not scanning)
    if !will_scan {
        if !app.queue.is_empty() {
            app.current_screen = app::Screen::Queue;
            log::info!(
                "Starting with Queue view (queue has {} items)",
                app.queue.len()
            );
        } else {
            app.current_screen = app::Screen::Library;
            log::info!("Starting with Library view (queue is empty)");
        }
    }

    // Main loop
    let result = run_app(&mut terminal, &mut app, &mut player);

    // Save configuration before exit
    if let Err(e) = app.save_config() {
        log::error!("Failed to save configuration: {}", e);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Stop playback
    let _ = player.stop();

    log::info!("SOTF UI Player exiting...");

    // Propagate any errors from the main loop
    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    player: &mut Player,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events
        if let Some(event) = handle_events(Duration::from_millis(100))? {
            match event {
                AppEvent::Key(key) => {
                    if let Some(cmd) = handle_key_event(app, key) {
                        handle_player_command(player, app, cmd)?;
                    }
                }
                AppEvent::Tick => {
                    // Get playback state
                    let state = player.get_playback_state();

                    // Update app state
                    app.position_secs = state.position_secs;
                    app.loudness_info = state.loudness;

                    // Check if playback ended and auto-advance
                    if app.is_playing && !state.is_playing && app.current_queue_index.is_some() {
                        log::info!("[TUI] Track ended, attempting auto-advance...");
                        // Track ended, advance to next
                        if let Some(path) = app.next_track() {
                            log::info!("[TUI] Auto-advancing to: {:?}", path);
                            let sample_rate = 48000.0;
                            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
                            let output_channels = app.plugin_chain.output_channels();

                            // Validate output channels against device max
                            if let Some(max_channels) = app.get_device_max_channels()
                                && output_channels > max_channels {
                                    log::error!(
                                        "[TUI] Plugin chain outputs {} channels but device only supports {}",
                                        output_channels,
                                        max_channels
                                    );
                                    app.is_playing = false;
                                    continue;
                                }

                            if let Err(e) = player.load_and_play(
                                path,
                                plugins,
                                output_channels,
                                app.current_output_device_name.clone(),
                            ) {
                                log::error!("[TUI] Failed to auto-advance: {}", e);
                                app.is_playing = false;
                            } else {
                                log::info!("[TUI] Auto-advance successful");
                            }
                        } else {
                            log::info!("[TUI] No more tracks in queue, stopping playback");
                            app.is_playing = false;
                        }
                    }

                    // Perform library scan if needed
                    // Note: This is intentionally synchronous and will block the UI
                    // But progress will be shown in the directory view
                    if app.needs_rescan {
                        // Switch to directory view so user can see scan progress
                        app.current_screen = app::Screen::DirectoryManager;

                        if let Err(e) = app.scan_library() {
                            log::error!("Failed to scan library: {}", e);
                        }
                    }
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

fn handle_player_command(
    player: &mut Player,
    app: &mut App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Get plugin configs and output channels
            let sample_rate = 48000.0; // Default sample rate
            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
            let output_channels = app.plugin_chain.output_channels();

            // Validate output channels against device max
            if let Some(max_channels) = app.get_device_max_channels()
                && output_channels > max_channels {
                    let error_msg = format!(
                        "Plugin chain outputs {} channels but device only supports {}",
                        output_channels, max_channels
                    );
                    log::error!("{}", error_msg);
                    return Err(error_msg.into());
                }

            player.load_and_play(
                path,
                plugins,
                output_channels,
                app.current_output_device_name.clone(),
            )?;
        }
        PlayerCommand::Pause => {
            player.pause()?;
        }
        PlayerCommand::Resume => {
            player.resume()?;
        }
        PlayerCommand::Stop => {
            player.stop()?;
        }
        PlayerCommand::SetVolume(volume) => {
            player.set_volume(volume)?;
        }
        PlayerCommand::UpdatePlugins => {
            // Update plugins in real-time
            let sample_rate = 48000.0;
            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
            player.update_plugins(plugins)?;
        }
        PlayerCommand::SetOutputDevice(device_name) => {
            // Store the device name for future playback
            app.current_output_device_name = Some(device_name.clone());
            player.set_output_device(device_name)?;
            log::info!("Output device changed");
        }
    }
    Ok(())
}
