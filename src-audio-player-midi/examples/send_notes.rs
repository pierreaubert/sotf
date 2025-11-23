//! Example: Send MIDI notes to an output device

use sotf_audio_player_midi::{MidiManager, MidiMessage};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("SOTF MIDI Note Sender\n");

    let mut manager = MidiManager::new()?;

    // List available output devices
    let devices = manager.list_output_devices()?;

    if devices.is_empty() {
        println!("No MIDI output devices found!");
        return Ok(());
    }

    println!("Available MIDI output devices:");
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
    manager.connect_output(device_index)?;
    println!("Connected! Playing a C major scale...\n");

    // Play a C major scale
    let scale = [60, 62, 64, 65, 67, 69, 71, 72]; // C major scale (MIDI note numbers)
    let channel = 0;
    let velocity = 100;

    for &note in &scale {
        println!("Playing note {}", note);

        // Note on
        manager.send_message(&MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        })?;

        thread::sleep(Duration::from_millis(500));

        // Note off
        manager.send_message(&MidiMessage::NoteOff {
            channel,
            note,
            velocity: 0,
        })?;

        thread::sleep(Duration::from_millis(100));
    }

    println!("\nDone!");

    Ok(())
}
