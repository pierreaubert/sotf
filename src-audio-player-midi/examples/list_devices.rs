//! Example: List all available MIDI input and output devices

use sotf_audio_player_midi::MidiManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("SOTF MIDI Device Lister\n");

    let mut manager = MidiManager::new()?;

    // List input devices
    println!("MIDI Input Devices:");
    println!("{:-<50}", "");
    let input_devices = manager.list_input_devices()?;
    if input_devices.is_empty() {
        println!("  No MIDI input devices found");
    } else {
        for device in &input_devices {
            println!("  [{}] {}", device.index, device.name);
        }
    }
    println!();

    // List output devices
    println!("MIDI Output Devices:");
    println!("{:-<50}", "");
    let output_devices = manager.list_output_devices()?;
    if output_devices.is_empty() {
        println!("  No MIDI output devices found");
    } else {
        for device in &output_devices {
            println!("  [{}] {}", device.index, device.name);
        }
    }
    println!();

    println!("Total: {} input(s), {} output(s)",
             input_devices.len(),
             output_devices.len());

    Ok(())
}
