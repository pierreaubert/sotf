//! RME TotalMix FX control example
//!
//! Demonstrates comprehensive control of RME TotalMix FX via MIDI.

use sotf_audio_player_midi::profiles::{RMETotalMixProfile, TotalMixControl, TotalMixRow};
use sotf_audio_player_midi::MidiManager;
use std::io::{self, Write};

fn print_menu() {
    println!("\n═══════════════════════════════════════");
    println!("   RME TotalMix FX MIDI Controller");
    println!("═══════════════════════════════════════");
    println!();
    println!("1. Set Main Volume");
    println!("2. Set Output Fader");
    println!("3. Set Input Fader");
    println!("4. Mute Channel");
    println!("5. Solo Channel");
    println!("6. Set Pan");
    println!("7. Recall Snapshot");
    println!("8. Show TotalMix Info");
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

fn get_number(prompt: &str, max: u8) -> Option<u8> {
    print!("{} (0-{}): ", prompt, max);
    io::stdout().flush().unwrap();
    let input = get_input();
    input.parse::<u8>().ok().filter(|&n| n <= max)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("\nRME TotalMix FX MIDI Controller\n");

    let mut manager = MidiManager::new()?;

    // Find TotalMix output
    let output_devices = manager.list_output_devices()?;
    let totalmix_idx = output_devices
        .iter()
        .position(|d| d.name.contains("TotalMix") || d.name.contains("UFX") || d.name.contains("RME"))
        .or_else(|| {
            if !output_devices.is_empty() {
                println!("TotalMix not found. Available devices:");
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

    let Some(idx) = totalmix_idx else {
        eprintln!("No MIDI output devices available!");
        return Ok(());
    };

    println!("Connecting to: {}", output_devices[idx].name);
    manager.connect_output(idx)?;

    let totalmix = TotalMixControl::new(&mut manager)?;

    println!("\n✓ Connected to TotalMix FX");
    println!("\nTotalMix Layout:");
    println!("  - Input Row:    Channels 1-4, Banks 0-3 (64 faders max)");
    println!("  - Playback Row: Channels 5-8, Banks 0-3 (64 faders max)");
    println!("  - Output Row:   Channels 9-12, Banks 0-3 (64 faders max)");
    println!("  - Each bank has 16 faders (CC 102-117)");

    loop {
        print_menu();
        let choice = get_input();

        match choice.as_str() {
            "1" => {
                if let Some(vol) = get_number("Enter main volume", 127) {
                    totalmix.set_main_volume(vol)?;
                    println!("✓ Main volume set to {}", vol);
                }
            }
            "2" => {
                println!("\nOutput Row Fader:");
                if let Some(bank) = get_number("  Bank", 3) {
                    if let Some(fader) = get_number("  Fader", 15) {
                        if let Some(value) = get_number("  Value", 127) {
                            totalmix.set_fader(TotalMixRow::Output, bank, fader, value)?;
                            println!(
                                "✓ Output row, bank {}, fader {} set to {}",
                                bank, fader, value
                            );
                        }
                    }
                }
            }
            "3" => {
                println!("\nInput Row Fader:");
                if let Some(bank) = get_number("  Bank", 3) {
                    if let Some(fader) = get_number("  Fader", 15) {
                        if let Some(value) = get_number("  Value", 127) {
                            totalmix.set_fader(TotalMixRow::Input, bank, fader, value)?;
                            println!(
                                "✓ Input row, bank {}, fader {} set to {}",
                                bank, fader, value
                            );
                        }
                    }
                }
            }
            "4" => {
                println!("\nMute Channel (Mackie Control):");
                println!("Select row:");
                println!("  1. Input");
                println!("  2. Playback");
                println!("  3. Output");
                print!("> ");
                io::stdout().flush().unwrap();

                let row = match get_input().as_str() {
                    "1" => Some(TotalMixRow::Input),
                    "2" => Some(TotalMixRow::Playback),
                    "3" => Some(TotalMixRow::Output),
                    _ => None,
                };

                if let Some(r) = row {
                    if let Some(bank) = get_number("  Bank", 3) {
                        if let Some(ch) = get_number("  Channel (Mackie)", 7) {
                            totalmix.mute_channel(r, bank, ch)?;
                            println!("✓ Channel muted");
                        }
                    }
                }
            }
            "5" => {
                println!("\nSolo Channel (Mackie Control):");
                println!("Select row:");
                println!("  1. Input");
                println!("  2. Playback");
                println!("  3. Output");
                print!("> ");
                io::stdout().flush().unwrap();

                let row = match get_input().as_str() {
                    "1" => Some(TotalMixRow::Input),
                    "2" => Some(TotalMixRow::Playback),
                    "3" => Some(TotalMixRow::Output),
                    _ => None,
                };

                if let Some(r) = row {
                    if let Some(bank) = get_number("  Bank", 3) {
                        if let Some(ch) = get_number("  Channel (Mackie)", 7) {
                            totalmix.solo_channel(r, bank, ch)?;
                            println!("✓ Channel soloed");
                        }
                    }
                }
            }
            "6" => {
                println!("\nSet Pan (Mackie Control):");
                println!("Select row:");
                println!("  1. Input");
                println!("  2. Playback");
                println!("  3. Output");
                print!("> ");
                io::stdout().flush().unwrap();

                let row = match get_input().as_str() {
                    "1" => Some(TotalMixRow::Input),
                    "2" => Some(TotalMixRow::Playback),
                    "3" => Some(TotalMixRow::Output),
                    _ => None,
                };

                if let Some(r) = row {
                    if let Some(bank) = get_number("  Bank", 3) {
                        if let Some(ch) = get_number("  Channel (Mackie)", 7) {
                            if let Some(pan) = get_number("  Pan (0=left, 64=center, 127=right)", 127)
                            {
                                totalmix.set_pan(r, bank, ch, pan)?;
                                println!("✓ Pan set to {}", pan);
                            }
                        }
                    }
                }
            }
            "7" => {
                if let Some(snapshot) = get_number("Snapshot number", 127) {
                    totalmix.recall_snapshot(snapshot)?;
                    println!("✓ Recalled snapshot {}", snapshot);
                }
            }
            "8" => {
                println!("\n═══════════════════════════════════════");
                println!("   TotalMix FX MIDI Implementation");
                println!("═══════════════════════════════════════");
                println!();
                println!("CC Control:");
                println!("  CC 102-117: Controls faders 1-16 in selected bank");
                println!("  CC 7: Main output volume (channel 1)");
                println!();
                println!("Row/Bank Selection:");
                println!("  Input Row:    MIDI channels 1-4 (banks 0-3)");
                println!("  Playback Row: MIDI channels 5-8 (banks 0-3)");
                println!("  Output Row:   MIDI channels 9-12 (banks 0-3)");
                println!();
                println!("Mackie Control Protocol:");
                println!("  Mute:   Note 16-23 (channels 0-7)");
                println!("  Solo:   Note 8-15 (channels 0-7)");
                println!("  Pan:    CC 16-23 (channels 0-7)");
                println!("  Select: Note 0-7 (channels 0-7)");
                println!();
                println!("Snapshots:");
                println!("  Program Change: 0-127");
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
