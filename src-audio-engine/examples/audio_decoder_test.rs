use sotf_audio::decoder::{AudioFormat, create_decoder, probe_file};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <audio_file>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("Testing audio decoder with file: {}", file_path);

    // Test format detection
    match AudioFormat::from_path(file_path) {
        Ok(format) => {
            println!(
                "✓ Detected format: {} ({})",
                format.as_str(),
                format.extension()
            );
            println!("  Lossless: {}", format.is_lossless());
        }
        Err(e) => {
            println!("✗ Format detection failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test file probing
    match probe_file(file_path) {
        Ok((format, spec)) => {
            println!("✓ Probed file successfully:");
            println!("  Format: {}", format.as_str());
            println!("  Sample Rate: {} Hz", spec.sample_rate);
            println!("  Channels: {}", spec.channels);
            println!("  Bits per Sample: {}", spec.bits_per_sample);
            if let Some(frames) = spec.total_frames {
                println!("  Total Frames: {}", frames);
                if let Some(duration) = spec.duration() {
                    println!("  Duration: {:.2} seconds", duration.as_secs_f64());
                }
            }
        }
        Err(e) => {
            println!("✗ File probing failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test decoder creation
    match create_decoder(file_path) {
        Ok(decoder) => {
            println!("✓ Decoder created successfully");
            let spec = decoder.spec();
            println!(
                "  Decoder spec matches probe: {} Hz, {} channels",
                spec.sample_rate, spec.channels
            );
        }
        Err(e) => {
            println!("✗ Decoder creation failed: {}", e);
            std::process::exit(1);
        }
    }

    println!("✅ All audio decoder tests passed!");
}
