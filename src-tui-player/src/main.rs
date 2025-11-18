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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let player = Player::new();

    // Enable loudness monitoring
    let _ = player.enable_loudness_monitoring().await;

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
    if (args.scan || db_is_empty) && !app.library.directories.is_empty() {
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

    // Main loop
    let result = run_app(&mut terminal, &mut app, &player).await;

    // Save configuration before exit
    if let Err(e) = app.save_config() {
        log::error!("Failed to save configuration: {}", e);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Stop playback
    let _ = player.stop().await;

    log::info!("SOTF UI Player exiting...");

    // Propagate any errors from the main loop
    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    player: &Player,
) -> Result<(), Box<dyn std::error::Error>> {
    // Track previous spectrum visibility state
    let mut spectrum_was_visible = app.spectrum_visible;

    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events
        if let Some(event) = handle_events(Duration::from_millis(100))? {
            match event {
                AppEvent::Key(key) => {
                    if let Some(cmd) = handle_key_event(app, key) {
                        handle_player_command(player, app, cmd).await?;
                    }
                }
                AppEvent::Tick => {
                    // Enable/disable spectrum monitoring when visibility changes
                    // Do this in Tick to avoid blocking the UI thread
                    if app.spectrum_visible != spectrum_was_visible {
                        if app.spectrum_visible {
                            let _ = player.enable_spectrum_monitoring().await;
                            log::info!("Spectrum analyzer enabled");
                        } else {
                            let _ = player.disable_spectrum_monitoring().await;
                            // Keep the last spectrum data to avoid flickering
                            log::info!("Spectrum analyzer disabled (keeping last data)");
                        }
                        spectrum_was_visible = app.spectrum_visible;
                    }

                    // Update position if playing
                    if app.is_playing {
                        if let Ok(pos) = player.get_position().await {
                            app.position_secs = pos;
                        }

                        // Check if playback ended and auto-advance
                        if let Ok(is_playing) = player.is_playing().await {
                            if !is_playing && app.current_queue_index.is_some() {
                                log::info!("[TUI] Track ended, attempting auto-advance...");
                                // Track ended, advance to next
                                if let Some(path) = app.next_track() {
                                    log::info!("[TUI] Auto-advancing to: {:?}", path);
                                    let sample_rate = 48000.0;
                                    let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
                                    let output_channels = app.plugin_chain.output_channels();
                                    if let Err(e) = player
                                        .load_and_play(
                                            path,
                                            plugins,
                                            output_channels,
                                            app.current_output_device_name.clone(),
                                        )
                                        .await {
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
                        }
                    }

                    // Update loudness data
                    app.loudness_info = player.get_loudness().await;

                    // Update spectrum data only if spectrum panel is visible
                    if app.spectrum_visible {
                        app.spectrum_info = player.get_spectrum().await;
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

async fn handle_player_command(
    player: &Player,
    app: &mut App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Get plugin configs and output channels
            let sample_rate = 48000.0; // Default sample rate
            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
            let output_channels = app.plugin_chain.output_channels();

            player
                .load_and_play(
                    path,
                    plugins,
                    output_channels,
                    app.current_output_device_name.clone(),
                )
                .await?;
        }
        PlayerCommand::Pause => {
            player.pause().await?;
        }
        PlayerCommand::Resume => {
            player.resume().await?;
        }
        PlayerCommand::Stop => {
            player.stop().await?;
        }
        PlayerCommand::SetVolume(volume) => {
            player.set_volume(volume).await?;
        }
        PlayerCommand::UpdatePlugins => {
            // Update plugins in real-time
            let sample_rate = 48000.0;
            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
            player.update_plugins(plugins).await?;
        }
        PlayerCommand::SetOutputDevice(device_name) => {
            // Store the device name for future playback
            app.current_output_device_name = Some(device_name.clone());
            player.set_output_device(device_name).await?;
            log::info!("Output device changed");
        }
    }
    Ok(())
}
