// ============================================================================
// Config Watcher Demo
// ============================================================================
//
// Demonstrates config file watching and Unix signal handling.
//
// Usage:
//   cargo run --example config_watcher_demo --release <audio_file> <config_file>
//
// Then try:
//   - Edit the config file to trigger a reload
//   - Send SIGHUP: kill -HUP <pid>
//   - Send SIGTERM/SIGINT: Ctrl-C or kill <pid>

use sotf_audio::engine::{AudioEngine, EngineConfig};
use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <audio_file> <config_file>", args[0]);
        eprintln!("Example: {} test.flac config.yaml", args[0]);
        eprintln!();
        eprintln!("Controls:");
        eprintln!("  - Edit config.yaml to trigger hot-reload");
        eprintln!(
            "  - Send SIGHUP to reload: kill -HUP {}",
            std::process::id()
        );
        eprintln!("  - Send SIGTERM or Ctrl-C to shutdown");
        return Ok(());
    }

    let audio_file = &args[1];
    let config_file = PathBuf::from(&args[2]);

    println!("=== Config Watcher Demo ===");
    println!("Audio file: {}", audio_file);
    println!("Config file: {:?}", config_file);
    println!("Process ID: {}", std::process::id());
    println!();

    // Load initial config from file
    let config_contents = std::fs::read_to_string(&config_file)?;
    let mut config: EngineConfig = serde_yaml::from_str(&config_contents)?;

    // Enable config watching
    config.config_path = Some(config_file.clone());
    config.watch_config = true;

    println!("Initial config:");
    println!("  Frame size: {} frames", config.frame_size);
    println!("  Buffer: {}ms", config.buffer_ms);
    println!(
        "  Output: {}Hz, {} channels",
        config.output_sample_rate, config.output_channels
    );
    println!("  Plugins: {} configured", config.plugins.len());
    println!();

    println!("Creating audio engine with config watching enabled...");
    let mut engine = AudioEngine::new(config)?;

    // Play the file
    println!("Playing: {}", audio_file);
    println!();
    println!("Try editing {:?} to trigger a hot-reload!", config_file);
    println!("Or send SIGHUP: kill -HUP {}", std::process::id());
    println!("Send SIGTERM or Ctrl-C to shutdown");
    println!();

    engine.play(audio_file)?;

    // Monitor playback
    for i in 0..600 {
        // Run for up to 60 seconds
        thread::sleep(Duration::from_millis(100));

        let state = engine.get_state();
        print!(
            "\r[{:3}s] State: {:?}, Position: {:.2}s, Underruns: {}   ",
            i / 10,
            state.playback_state,
            state.position,
            state.underruns
        );
        std::io::Write::flush(&mut std::io::stdout())?;

        // Check if playback stopped
        if state.playback_state == sotf_audio::engine::PlaybackState::Stopped {
            println!("\n\nPlayback finished");
            break;
        }
    }

    println!("\n\nShutting down...");
    engine.shutdown()?;

    println!("Done!");
    Ok(())
}
