//! Example: Create and use device profiles

use sotf_audio_player_midi::{DeviceConfig, DeviceProfile, MidiConfig, MidiManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("SOTF MIDI Device Profile Example\n");

    // Create a device profile for a MIDI controller
    let mut profile = DeviceProfile::new("My Controller".to_string());
    profile.description = Some("Custom MIDI controller configuration".to_string());

    // Configure device settings
    profile.device_config = DeviceConfig::new()
        .with_manufacturer("ACME".to_string())
        .with_model("MK-1000".to_string())
        .with_channel(0)
        .with_sysex(true);

    // Add MIDI control mappings
    profile.add_mapping(1, "modulation".to_string());
    profile.add_mapping(7, "volume".to_string());
    profile.add_mapping(10, "pan".to_string());
    profile.add_mapping(64, "sustain".to_string());
    profile.add_mapping(71, "resonance".to_string());
    profile.add_mapping(74, "cutoff".to_string());

    // Add initialization messages
    // Example: Set controller to specific mode (this is device-specific)
    profile.add_init_message(vec![0xB0, 0x00, 0x00]); // Bank select MSB
    profile.add_init_message(vec![0xC0, 0x00]); // Program change

    // Create MIDI configuration
    let mut config = MidiConfig::default();
    config.add_profile("my_controller".to_string(), profile.clone());
    config.set_active_profile("my_controller".to_string());
    config.default_input = Some("My Controller Input".to_string());
    config.default_output = Some("My Controller Output".to_string());

    // Display the configuration
    println!("Device Profile: {}", profile.name);
    if let Some(desc) = &profile.description {
        println!("Description: {}", desc);
    }
    println!("\nDevice Config:");
    println!("  Manufacturer: {:?}", profile.device_config.manufacturer);
    println!("  Model: {:?}", profile.device_config.model);
    println!("  Channel: {:?}", profile.device_config.channel);
    println!("  SysEx Enabled: {}", profile.device_config.sysex_enabled);

    println!("\nControl Mappings:");
    let mut mappings: Vec<_> = profile.mappings.iter().collect();
    mappings.sort_by_key(|(cc, _)| *cc);
    for (cc, function) in mappings {
        println!("  CC {} -> {}", cc, function);
    }

    println!(
        "\nInitialization Messages: {} message(s)",
        profile.init_messages.len()
    );

    // Save configuration to file
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().join("midi_config.json");
    config.save(&config_path)?;
    println!("\nConfiguration saved to: {}", config_path.display());

    // Load it back
    let loaded_config = MidiConfig::load(&config_path)?;
    println!("Configuration loaded successfully!");

    if let Some(loaded_profile) = loaded_config.get_profile("my_controller") {
        println!("Loaded profile: {}", loaded_profile.name);
        println!("Mappings count: {}", loaded_profile.mappings.len());
    }

    // Example: Use the profile with a manager
    let mut manager = MidiManager::with_config(config)?;

    // If devices are available, you could connect and send init messages:
    let outputs = manager.list_output_devices()?;
    if !outputs.is_empty() {
        println!("\nConnecting to first available output device...");
        manager.connect_output(0)?;
        println!("Sending initialization messages...");
        manager.send_init_messages()?;
        println!("Done!");
    } else {
        println!("\nNo MIDI output devices available for testing.");
    }

    Ok(())
}
