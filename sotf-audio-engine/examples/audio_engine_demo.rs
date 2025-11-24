// ============================================================================
// Audio Engine Demo
// ============================================================================
//
// Demonstrates the new native audio engine (replacing CamillaDSP).

use sotf_audio::engine::{AudioEngine, EngineConfig};
use std::env;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get audio file from command line
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <audio_file>", args[0]);
        eprintln!("Example: {} test.flac", args[0]);
        return Ok(());
    }

    let audio_file = &args[1];

    println!("=== Audio Engine Demo ===");
    println!("File: {}", audio_file);
    println!();

    // Create engine with custom config
    let config = EngineConfig {
        frame_size: 1024,
        buffer_ms: 200, // 200ms latency
        output_sample_rate: 48000,
        input_channels: 2, // Stereo input
        output_channels: 2,
        output_device: None, // Use default output device
        plugins: Vec::new(), // No plugins for now
        volume: 0.8,         // 80% volume
        muted: false,
        config_path: None,   // No config file watching
        watch_config: false, // Disable config watching for demo
    };

    println!("Creating audio engine...");
    println!("  Frame size: {} frames", config.frame_size);
    println!("  Buffer: {}ms", config.buffer_ms);
    println!(
        "  Output: {}Hz, {} channels",
        config.output_sample_rate, config.output_channels
    );
    println!();

    let mut engine = AudioEngine::new(config)?;

    // Play the file
    println!("Playing: {}", audio_file);
    engine.play(audio_file)?;

    // Monitor playback
    for i in 0..100 {
        thread::sleep(Duration::from_millis(100));

        let state = engine.get_state();
        print!(
            "\r[{:3}s] State: {:?}, Position: {:.2}s   ",
            i / 10,
            state.playback_state,
            state.position
        );
        std::io::Write::flush(&mut std::io::stdout())?;

        // Test pause/resume at 3 seconds
        if i == 30 {
            println!("\n\nPausing...");
            engine.pause()?;
        }

        if i == 50 {
            println!("Resuming...");
            engine.resume()?;
        }

        // Test seek at 7 seconds
        if i == 70 {
            println!("\nSeeking to 10.0s...");
            engine.seek(10.0)?;
        }

        // Test volume change at 8 seconds
        if i == 80 {
            println!("Setting volume to 50%...");
            engine.set_volume(0.5)?;
        }
    }

    println!("\n\nStopping...");
    engine.stop()?;

    println!("\nFinal state:");
    let state = engine.get_state();
    println!("  Playback state: {:?}", state.playback_state);
    println!("  Position: {:.2}s", state.position);
    println!("  Underruns: {}", state.underruns);

    println!("\nShutting down...");
    engine.shutdown()?;

    println!("Done!");
    Ok(())
}
