mod app;
mod events;
mod library;
mod player;
mod plugins;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{handle_events, handle_key_event, AppEvent, PlayerCommand};
use player::Player;
use ratatui::{backend::CrosstermBackend, Terminal};
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
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

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

    // Add initial directories
    for dir in args.directories {
        app.add_directory(dir);
    }

    // Auto-scan if requested
    if args.scan && !app.library.directories.is_empty() {
        if let Err(e) = app.scan_library() {
            log::error!("Failed to scan library: {}", e);
        }
    }

    // Main loop
    let result = run_app(&mut terminal, &mut app, &player).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Stop playback
    let _ = player.stop().await;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    player: &Player,
) -> Result<(), Box<dyn std::error::Error>> {
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
                    // Update position if playing
                    if app.is_playing {
                        if let Ok(pos) = player.get_position().await {
                            app.position_secs = pos;
                        }

                        // Check if playback ended and auto-advance
                        if let Ok(is_playing) = player.is_playing().await {
                            if !is_playing && app.current_queue_index.is_some() {
                                // Track ended, advance to next
                                if let Some(path) = app.next_track() {
                                    let sample_rate = 48000.0;
                                    let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
                                    let output_channels = app.plugin_chain.output_channels();
                                    let _ = player.load_and_play(path, plugins, output_channels).await;
                                } else {
                                    app.is_playing = false;
                                }
                            }
                        }
                    }

                    // Perform library scan if needed
                    if app.needs_rescan {
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
    app: &App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Get plugin configs and output channels
            let sample_rate = 48000.0; // Default sample rate
            let plugins = app.plugin_chain.to_plugin_configs(sample_rate);
            let output_channels = app.plugin_chain.output_channels();

            player.load_and_play(path, plugins, output_channels).await?;
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
    }
    Ok(())
}
