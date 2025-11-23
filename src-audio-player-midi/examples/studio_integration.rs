//! Complete studio integration example
//!
//! This example demonstrates full integration of:
//! - RME UFX+ / TotalMix FX
//! - Genelec GLM
//! - Allen & Heath Xone K2/K3
//! - Novation Launch Control XL
//!
//! Shows how to use control surfaces to control audio hardware.

use sotf_audio_player_midi::profiles::{
    GLMControl, LaunchControlXLProfile, LCXLTemplate,
    TotalMixControl, TotalMixRow, XoneK2Profile,
    xone_k2::K2Control,
};
use sotf_audio_player_midi::{MidiManager, MidiMessage};
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("SOTF Studio Integration Example\n");
    println!("================================\n");

    let mut manager = MidiManager::new()?;

    // List all available MIDI devices
    println!("Available MIDI Devices:");
    println!("----------------------");

    let input_devices = manager.list_input_devices()?;
    println!("\nInputs:");
    for device in &input_devices {
        println!("  [{}] {}", device.index, device.name);
    }

    let output_devices = manager.list_output_devices()?;
    println!("\nOutputs:");
    for device in &output_devices {
        println!("  [{}] {}", device.index, device.name);
    }
    println!();

    // Find specific devices by name
    let totalmix_output = output_devices
        .iter()
        .find(|d| d.name.contains("TotalMix") || d.name.contains("UFX"))
        .map(|d| d.index);

    let glm_output = output_devices
        .iter()
        .find(|d| d.name.contains("GLM") || d.name.contains("Genelec"))
        .map(|d| d.index);

    let k2_input = input_devices
        .iter()
        .find(|d| d.name.contains("Xone") || d.name.contains("K2") || d.name.contains("K3"))
        .map(|d| d.index);

    let lcxl_input = input_devices
        .iter()
        .find(|d| d.name.contains("Launch Control"))
        .map(|d| d.index);

    // Connect to output devices
    if let Some(idx) = totalmix_output {
        println!("✓ Found RME TotalMix output at index {}", idx);
        manager.connect_output(idx)?;
        println!("  Connected!");

        // Demonstrate TotalMix control
        println!("\nDemonstrating TotalMix FX control:");
        let totalmix = TotalMixControl::new(&mut manager)?;

        println!("  Setting main volume to 100...");
        totalmix.set_main_volume(100)?;

        println!("  Setting output channel 1 to 90...");
        totalmix.set_fader(TotalMixRow::Output, 0, 0, 90)?;

        println!("  Setting pan to center for channel 1...");
        totalmix.set_pan(TotalMixRow::Output, 0, 0, 64)?;
    } else {
        println!("⚠ RME TotalMix output not found");
    }

    if let Some(idx) = glm_output {
        println!("\n✓ Found Genelec GLM output at index {}", idx);
        manager.connect_output(idx)?;
        println!("  Connected!");

        // Demonstrate GLM control
        println!("\nDemonstrating Genelec GLM control:");
        let glm = GLMControl::new(&mut manager);

        println!("  Setting volume to 75%...");
        glm.set_volume_percent(75.0)?;

        println!("  Activating dim (-20dB)...");
        glm.dim(true)?;

        std::thread::sleep(std::time::Duration::from_secs(1));

        println!("  Deactivating dim...");
        glm.dim(false)?;
    } else {
        println!("\n⚠ Genelec GLM output not found");
    }

    println!("\n");

    // Set up input monitoring
    let has_inputs = k2_input.is_some() || lcxl_input.is_some();

    if !has_inputs {
        println!("⚠ No control surfaces found. Exiting.");
        return Ok(());
    }

    // Shared state for volume control
    let current_volume = Arc::new(Mutex::new(100u8));

    // Connect to Xone K2/K3 input
    if let Some(idx) = k2_input {
        println!("✓ Found Xone K2/K3 input at index {}", idx);

        let volume = Arc::clone(&current_volume);
        manager.connect_input(idx, move |msg| {
            if let Some((control, value)) = XoneK2Profile::identify_control(&msg) {
                match control {
                    K2Control::RotaryPot(0) => {
                        // First pot controls main volume
                        *volume.lock().unwrap() = value;
                        println!("[K2] Main Volume: {}", value);
                    }
                    K2Control::Fader(n) => {
                        println!("[K2] Fader {}: {}", n + 1, value);
                    }
                    K2Control::Encoder(n) => {
                        println!("[K2] Encoder {}: {}", n + 1, value);
                    }
                    K2Control::EncoderSwitch(n) => {
                        if value > 0 {
                            println!("[K2] Encoder {} Switch Pressed", n + 1);
                        }
                    }
                    K2Control::Button(n) => {
                        if value > 0 {
                            println!("[K2] Button {} Pressed", n + 1);
                        }
                    }
                    _ => {}
                }
            }
        })?;
        println!("  Listening for K2 controls (pot 1 = main volume)...");
    }

    // Connect to Launch Control XL input
    if let Some(idx) = lcxl_input {
        println!("✓ Found Launch Control XL input at index {}", idx);

        let template = LCXLTemplate::factory_1();
        manager.connect_input(idx, move |msg| {
            if let Some(control) = LaunchControlXLProfile::identify_control(&msg, &template) {
                if let MidiMessage::ControlChange { value, .. } = msg {
                    println!("[LCXL] {}: {}", control, value);
                } else if let MidiMessage::NoteOn { velocity, .. } = msg {
                    if velocity > 0 {
                        println!("[LCXL] {} Pressed", control);
                    }
                }
            }
        })?;
        println!("  Listening for Launch Control XL input...");
    }

    println!("\n═══════════════════════════════════════");
    println!("Monitoring MIDI input (Ctrl+C to stop)");
    println!("═══════════════════════════════════════\n");

    // Keep running
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
