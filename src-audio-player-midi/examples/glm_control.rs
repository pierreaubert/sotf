//! Genelec GLM control example
//!
//! Demonstrates MIDI control of Genelec SAM monitors via GLM software.

use sotf_audio_player_midi::profiles::{GLMControl, GenelecGLMProfile};
use sotf_audio_player_midi::MidiManager;
use std::io::{self, Write};

fn print_menu() {
    println!("\n═══════════════════════════════════════");
    println!("     Genelec GLM MIDI Controller");
    println!("═══════════════════════════════════════");
    println!();
    println!("1. Set Volume (0-100%)");
    println!("2. Mute On/Off");
    println!("3. Dim On/Off");
    println!("4. Solo On/Off");
    println!("5. Select Monitor Group");
    println!("6. Recall Volume Preset");
    println!("7. Toggle Bass Management");
    println!("8. System Power On/Off");
    println!("9. Solo Specific Monitor");
    println!("0. Show GLM Info");
    println!("q. Quit");
    println!();
    print!("> ");
    io::stdout().flush().unwrap();
}

fn get_input() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn get_yes_no(prompt: &str) -> bool {
    print!("{} (y/n): ", prompt);
    io::stdout().flush().unwrap();
    let input = get_input();
    input.to_lowercase().starts_with('y')
}

fn get_number(prompt: &str, max: u8) -> Option<u8> {
    print!("{} (0-{}): ", prompt, max);
    io::stdout().flush().unwrap();
    let input = get_input();
    input.parse::<u8>().ok().filter(|&n| n <= max)
}

fn get_percent(prompt: &str) -> Option<f32> {
    print!("{} (0-100): ", prompt);
    io::stdout().flush().unwrap();
    let input = get_input();
    input.parse::<f32>().ok().filter(|&n| n >= 0.0 && n <= 100.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("\nGenelec GLM MIDI Controller\n");

    let mut manager = MidiManager::new()?;

    // Find GLM output
    let output_devices = manager.list_output_devices()?;
    let glm_idx = output_devices
        .iter()
        .position(|d| d.name.contains("GLM") || d.name.contains("Genelec"))
        .or_else(|| {
            if !output_devices.is_empty() {
                println!("GLM not found. Available devices:");
                for (i, device) in output_devices.iter().enumerate() {
                    println!("  [{}] {}", i, device.name);
                }
                print!("\nSelect device index: ");
                io::stdout().flush().unwrap();
                get_input().parse::<usize>().ok()
            } else {
                None
            }
        });

    let Some(idx) = glm_idx else {
        eprintln!("No MIDI output devices available!");
        return Ok(());
    };

    println!("Connecting to: {}", output_devices[idx].name);
    manager.connect_output(idx)?;

    let glm = GLMControl::new(&mut manager);

    println!("\n✓ Connected to Genelec GLM");
    println!("\n⚠ Note: Make sure GLM software is running and MIDI control is");
    println!("  configured in GLM Settings > MIDI Remote.");
    println!("\n  Default CC assignments (verify in GLM):");
    println!("    - Volume: CC {}", GenelecGLMProfile::VOLUME_CC);
    println!("    - Mute:   CC {}", GenelecGLMProfile::MUTE_CC);
    println!("    - Dim:    CC {}", GenelecGLMProfile::DIM_CC);
    println!("    - Solo:   CC {}", GenelecGLMProfile::SOLO_CC);

    let mut muted = false;
    let mut dimmed = false;
    let mut solo = false;

    loop {
        print_menu();
        let choice = get_input();

        match choice.as_str() {
            "1" => {
                if let Some(percent) = get_percent("Enter volume percentage") {
                    glm.set_volume_percent(percent)?;
                    println!("✓ Volume set to {}%", percent);
                }
            }
            "2" => {
                muted = get_yes_no("Mute system?");
                glm.mute(muted)?;
                if muted {
                    println!("✓ System muted");
                } else {
                    println!("✓ System unmuted");
                }
            }
            "3" => {
                dimmed = get_yes_no("Enable dim (-20dB)?");
                glm.dim(dimmed)?;
                if dimmed {
                    println!("✓ Dim enabled (-20dB)");
                } else {
                    println!("✓ Dim disabled");
                }
            }
            "4" => {
                solo = get_yes_no("Enable solo?");
                glm.solo(solo)?;
                if solo {
                    println!("✓ Solo enabled");
                } else {
                    println!("✓ Solo disabled");
                }
            }
            "5" => {
                if let Some(group) = get_number("Enter monitor group number", 127) {
                    glm.select_monitor_group(group)?;
                    println!("✓ Switched to monitor group {}", group);
                }
            }
            "6" => {
                if let Some(preset) = get_number("Enter volume preset number", 127) {
                    glm.recall_volume_preset(preset)?;
                    println!("✓ Recalled volume preset {}", preset);
                }
            }
            "7" => {
                let enabled = get_yes_no("Enable bass management?");
                glm.bass_management(enabled)?;
                if enabled {
                    println!("✓ Bass management enabled");
                } else {
                    println!("✓ Bass management disabled");
                }
            }
            "8" => {
                let power_on = get_yes_no("Power on system?");
                glm.system_power(power_on)?;
                if power_on {
                    println!("✓ System powered on");
                } else {
                    println!("✓ System powered off");
                }
            }
            "9" => {
                println!("\nSolo Specific Monitor:");
                println!("The MIDI ID can be found in the monitor info popup in GLM.");
                if let Some(midi_id) = get_number("Enter monitor MIDI ID", 127) {
                    glm.solo_monitor(midi_id)?;
                    println!("✓ Soloed monitor with MIDI ID {}", midi_id);
                }
            }
            "0" => {
                println!("\n═══════════════════════════════════════");
                println!("   Genelec GLM MIDI Implementation");
                println!("═══════════════════════════════════════");
                println!();
                println!("GLM Version: 5.0+ (MIDI support improved)");
                println!();
                println!("Controllable Functions:");
                println!("  ✓ System Volume");
                println!("  ✓ System Mute");
                println!("  ✓ Dim (-20dB)");
                println!("  ✓ Solo");
                println!("  ✓ Monitor Groups");
                println!("  ✓ Volume Presets");
                println!("  ✓ Bass Management");
                println!("  ✓ System Power (GLM 5.0+)");
                println!("  ✓ Solo/Mute Individual Monitors (GLM 5.0+)");
                println!();
                println!("CC Assignments (configurable in GLM):");
                println!("  Volume:       CC {}", GenelecGLMProfile::VOLUME_CC);
                println!("  Mute:         CC {}", GenelecGLMProfile::MUTE_CC);
                println!("  Dim:          CC {}", GenelecGLMProfile::DIM_CC);
                println!("  Solo:         CC {}", GenelecGLMProfile::SOLO_CC);
                println!(
                    "  Mon. Group:   CC {}",
                    GenelecGLMProfile::MONITOR_GROUP_CC
                );
                println!(
                    "  Vol. Preset:  CC {}",
                    GenelecGLMProfile::VOLUME_PRESET_CC
                );
                println!(
                    "  Bass Mgmt:    CC {}",
                    GenelecGLMProfile::BASS_MGMT_CC
                );
                println!(
                    "  System Power: CC {}",
                    GenelecGLMProfile::SYSTEM_POWER_CC
                );
                println!(
                    "  Solo/Mute:    CC {} (value = MIDI ID)",
                    GenelecGLMProfile::SOLO_MUTE_DEV_CC
                );
                println!();
                println!("⚠ Important:");
                println!("  - GLM software must be running");
                println!("  - MIDI control must be configured in GLM Settings");
                println!("  - CC assignments are user-configurable");
                println!("  - SPL data cannot be read via MIDI (display only)");
                println!("  - No official API for programmatic control");
                println!();
            }
            "q" | "Q" => {
                println!("\nGoodbye!");
                break;
            }
            _ => {
                println!("Invalid choice. Try again.");
            }
        }
    }

    Ok(())
}
