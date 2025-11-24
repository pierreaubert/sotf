//! Example: Monitor MIDI input from a device

use sotf_audio_player_midi::MidiManager;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("SOTF MIDI Monitor\n");

    let mut manager = MidiManager::new()?;

    // List available input devices
    let devices = manager.list_input_devices()?;

    if devices.is_empty() {
        println!("No MIDI input devices found!");
        return Ok(());
    }

    println!("Available MIDI input devices:");
    for device in &devices {
        println!("  [{}] {}", device.index, device.name);
    }
    println!();

    // Get user selection
    print!("Select device index (or press Enter for device 0): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let device_index: usize = if input.trim().is_empty() {
        0
    } else {
        input.trim().parse()?
    };

    // Connect to the device
    println!("\nConnecting to device {}...", device_index);
    println!("Listening for MIDI messages (press Ctrl+C to stop)...\n");

    manager.connect_input(device_index, |message| {
        println!("[{}] {}",
                 chrono::Local::now().format("%H:%M:%S%.3f"),
                 message.description());
    })?;

    // Keep the program running
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
