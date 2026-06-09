use clap::Parser;
use std::str::FromStr;

/// Audio recorder for test signals with analysis
#[derive(Parser)]
#[command(name = "sotf_recorder")]
#[command(about = "Generate and record test signals with analysis", long_about = None)]
struct Cli {
    /// Signal type: tone, two-tone, sweep, white-noise, pink-noise, m-noise, mls, dirac
    #[arg(long)]
    signal: Option<String>,

    /// Duration in seconds
    #[arg(long)]
    duration: Option<f32>,

    /// Sample rate in Hz
    #[arg(long, default_value = "48000")]
    sample_rate: u32,

    /// Number of signal channels (must be 1)
    #[arg(long, default_value = "1")]
    channels: u16,

    /// Hardware output channel to send signal to (0-based, single channel only)
    #[arg(long)]
    hwaudio_send_to: Option<String>,

    /// Hardware input channels to record from (0-based, comma-separated)
    #[arg(long)]
    hwaudio_record_from: Option<String>,

    /// Optional filename prefix
    #[arg(long)]
    name: Option<String>,

    /// Audio device name (use --list-devices to see available devices). If not specified, uses default device.
    #[arg(long)]
    device: Option<String>,

    /// List available audio devices and exit
    #[arg(long)]
    list_devices: bool,

    // Signal-specific parameters
    /// Tone frequency in Hz (for tone signal)
    #[arg(long)]
    freq: Option<f32>,

    /// First frequency in Hz (for two-tone signal)
    #[arg(long)]
    freq1: Option<f32>,

    /// Second frequency in Hz (for two-tone signal)
    #[arg(long)]
    freq2: Option<f32>,

    /// Start frequency in Hz (for sweep signal)
    #[arg(long, default_value = "5")]
    start_freq: Option<f32>,

    /// End frequency in Hz (for sweep signal)
    #[arg(long, default_value = "22000")]
    end_freq: Option<f32>,

    /// Amplitude (0.0-1.0)
    #[arg(long)]
    amp: Option<f32>,

    /// First amplitude (0.0-1.0, for two-tone signal)
    #[arg(long)]
    amp1: Option<f32>,

    /// Second amplitude (0.0-1.0, for two-tone signal)
    #[arg(long)]
    amp2: Option<f32>,

    /// MLS order (2-24, for MLS signal)
    #[arg(long)]
    mls_order: Option<u8>,

    /// Microphone compensation file (freq/SPL pairs in CSV format)
    /// When provided, inverse compensation is applied to the CSV output.
    /// Applies to all channels as a default fallback.
    #[arg(long)]
    microphone_compensation: Option<String>,

    /// Per-channel microphone calibration file in channel:path format.
    /// Can be specified multiple times. Example: --mic-calibration 0:/path/to/umik1.txt
    #[arg(long = "mic-calibration", value_name = "CHANNEL:PATH")]
    mic_calibration: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    // Handle --list-devices flag
    if cli.list_devices {
        list_audio_devices();
        return;
    }

    // Validate required arguments when not listing devices
    let signal = cli.signal.unwrap_or_else(|| {
        eprintln!("Error: --signal is required");
        std::process::exit(1);
    });
    let duration = cli.duration.unwrap_or_else(|| {
        eprintln!("Error: --duration is required");
        std::process::exit(1);
    });
    let hwaudio_send_to = cli.hwaudio_send_to.unwrap_or_else(|| {
        eprintln!("Error: --hwaudio-send-to is required");
        std::process::exit(1);
    });
    let hwaudio_record_from = cli.hwaudio_record_from.unwrap_or_else(|| {
        eprintln!("Error: --hwaudio-record-from is required");
        std::process::exit(1);
    });

    // Parse per-channel calibration args into a map
    let mut mic_calibration_map: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    for entry in &cli.mic_calibration {
        if let Some((ch_str, path)) = entry.split_once(':') {
            match ch_str.parse::<usize>() {
                Ok(ch) => {
                    mic_calibration_map.insert(ch, path.to_string());
                }
                Err(_) => {
                    eprintln!(
                        "Error: invalid channel number in --mic-calibration: {}",
                        entry
                    );
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!(
                "Error: --mic-calibration must be in CHANNEL:PATH format, got: {}",
                entry
            );
            std::process::exit(1);
        }
    }

    if let Err(e) = record_signal(
        signal,
        duration,
        cli.sample_rate,
        cli.channels,
        hwaudio_send_to,
        hwaudio_record_from,
        cli.name,
        cli.device,
        cli.freq,
        cli.freq1,
        cli.freq2,
        cli.start_freq,
        cli.end_freq,
        cli.amp,
        cli.amp1,
        cli.amp2,
        cli.mls_order,
        cli.microphone_compensation,
        mic_calibration_map,
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn list_audio_devices() {
    println!("{}", "=".repeat(80));
    println!("Available Audio Devices");
    println!("{}", "=".repeat(80));

    let devices = match sotf_audio::devices::get_audio_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to get devices: {}", e);
            return;
        }
    };

    println!("\n📥 INPUT DEVICES:");
    println!("{}", "-".repeat(80));

    if let Some(input_devices) = devices.get("input") {
        for (idx, device) in input_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = format_sample_rate_range(&device.available_sample_rates);

                println!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
            } else {
                println!("  [{}] {}{}", idx, device.name, default_marker);
            }
        }
    }

    println!("\n📤 OUTPUT DEVICES:");
    println!("{}", "-".repeat(80));

    if let Some(output_devices) = devices.get("output") {
        for (idx, device) in output_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = format_sample_rate_range(&device.available_sample_rates);

                println!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
            } else {
                println!("  [{}] {}{}", idx, device.name, default_marker);
            }
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("💡 Usage: Use --device \"Device Name\" to select a device");
    println!("{}", "=".repeat(80));
}

fn format_sample_rate_range(rates: &[u32]) -> String {
    match (rates.first(), rates.last()) {
        (None, _) => "unknown".to_string(),
        (Some(rate), Some(_)) if rates.len() == 1 => format!("{} Hz", rate),
        (Some(first), Some(last)) => format!("{}-{} Hz", first, last),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_sample_rate_range;

    #[test]
    fn sample_rate_range_handles_empty_single_and_multiple_rates() {
        assert_eq!(format_sample_rate_range(&[]), "unknown");
        assert_eq!(format_sample_rate_range(&[48_000]), "48000 Hz");
        assert_eq!(
            format_sample_rate_range(&[44_100, 48_000, 96_000]),
            "44100-96000 Hz"
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_signal(
    signal: String,
    duration: f32,
    sample_rate: u32,
    channels: u16,
    hwaudio_send_to: String,
    hwaudio_record_from: String,
    name: Option<String>,
    device: Option<String>,
    freq: Option<f32>,
    freq1: Option<f32>,
    freq2: Option<f32>,
    start_freq: Option<f32>,
    end_freq: Option<f32>,
    amp: Option<f32>,
    amp1: Option<f32>,
    amp2: Option<f32>,
    mls_order: Option<u8>,
    microphone_compensation: Option<String>,
    mic_calibration_map: std::collections::HashMap<usize, String>,
) -> Result<(), String> {
    use sotf_audio::signal_recorder::*;

    println!("{}", "=".repeat(60));
    println!("Signal Recording and Analysis");
    println!("{}", "=".repeat(60));

    // Validate channels
    if channels != 1 {
        return Err(format!(
            "Channels must be 1 (mono signal generation), got {}",
            channels
        ));
    }

    // Parse signal type
    let signal_type = SignalType::from_str(&signal)?;

    // Parse channel lists
    let send_to_channels = parse_channel_list(&hwaudio_send_to)?;
    let record_from_channels = parse_channel_list(&hwaudio_record_from)?;

    // Validate that we have at least one send-to channel
    if send_to_channels.is_empty() {
        return Err("hwaudio-send-to must specify at least 1 channel".to_string());
    }

    // Validate channel configuration:
    // - Either the number of send and record channels must match (1:1 mapping)
    // - Or there must be exactly 1 record channel (record same channel for all outputs)
    if send_to_channels.len() != record_from_channels.len() && record_from_channels.len() != 1 {
        return Err(format!(
            "Invalid channel configuration: {} send-to channels, {} record-from channels.\n\
             Must be either:\n\
             - Equal number of channels (e.g., --hwaudio-send-to 0,1 --hwaudio-record-from 0,1)\n\
             - Single record channel (e.g., --hwaudio-send-to 0,1 --hwaudio-record-from 0)",
            send_to_channels.len(),
            record_from_channels.len()
        ));
    }

    // Build signal parameters based on signal type
    let params = match signal_type {
        SignalType::Tone => {
            let freq = freq.ok_or("--freq is required for tone signal")?;
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Tone { freq, amp }
        }
        SignalType::TwoTone => {
            let freq1 = freq1.ok_or("--freq1 is required for two-tone signal")?;
            let freq2 = freq2.ok_or("--freq2 is required for two-tone signal")?;
            let amp1 = amp1.unwrap_or(0.5).clamp(0.0, 1.0);
            let amp2 = amp2.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            }
        }
        SignalType::Sweep => {
            let start_freq = start_freq.ok_or("--start-freq is required for sweep signal")?;
            let end_freq = end_freq.ok_or("--end-freq is required for sweep signal")?;
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            }
        }
        SignalType::WhiteNoise | SignalType::PinkNoise | SignalType::MNoise => {
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Noise { amp }
        }
        SignalType::Mls => {
            let order = mls_order.unwrap_or(DEFAULT_MLS_ORDER);
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Mls { order, amp }
        }
        SignalType::Dirac => {
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Dirac { amp }
        }
    };

    // Validate parameters
    validate_signal_params(signal_type, &params, duration, sample_rate)?;

    println!("\nConfiguration:");
    println!("  Signal: {}", signal_type.as_str());
    println!("  Duration: {:.2}s", duration);
    println!("  Sample rate: {}Hz", sample_rate);
    if let Some(ref dev) = device {
        println!("  Audio device: {}", dev);
    } else {
        println!("  Audio device: [DEFAULT]");
    }
    println!("  Channel pairs (send → record):");
    // If there's only one record channel, it's used for all send channels
    if record_from_channels.len() == 1 {
        let record_ch = record_from_channels[0];
        for &send_ch in &send_to_channels {
            println!("    hw output {} → hw input {}", send_ch, record_ch);
        }
    } else {
        // 1:1 mapping
        for (&send_ch, &record_ch) in send_to_channels.iter().zip(record_from_channels.iter()) {
            println!("    hw output {} → hw input {}", send_ch, record_ch);
        }
    }
    println!(
        "  Total recordings: {} (one mono file per pair)",
        send_to_channels.len()
    );
    if let Some(ref n) = name {
        println!("  Output prefix: {}", n);
    }
    println!();

    // Generate the base signal
    let total_recordings = send_to_channels.len(); // One recording per send/record pair
    println!("[1/{}] Generating signal...", total_recordings + 2);
    let mut base_signal = generate_signal(signal_type, &params, duration, sample_rate)?;

    // Validate that the signal is mono (Vec<f32> represents mono)
    // All our signal generation functions return mono signals
    println!(
        "  ✓ Generated mono signal with {} samples",
        base_signal.len()
    );

    // Apply pre-compensation if provided (for sweeps only)
    if let Some(ref comp_path) = microphone_compensation {
        use sotf_audio::signal_analysis::MicrophoneCompensation;
        use std::path::Path;

        println!("\n  Loading microphone compensation for playback pre-compensation...");
        let compensation = MicrophoneCompensation::from_file(Path::new(comp_path))?;

        // Only apply to sweeps - other signal types don't have well-defined instantaneous frequency
        if signal_type == SignalType::Sweep {
            let start_freq = start_freq.unwrap_or(5.0);
            let end_freq = end_freq.unwrap_or(22000.0);

            println!(
                "  Applying inverse microphone compensation to sweep ({} Hz - {} Hz)...",
                start_freq, end_freq
            );
            println!("  This pre-compensates the playback signal so the microphone records flat");

            // Apply inverse compensation: boost where mic is weak, cut where mic is loud
            base_signal =
                compensation.apply_to_sweep(&base_signal, start_freq, end_freq, sample_rate, true);

            println!("  ✓ Applied pre-compensation to playback signal");
        } else {
            println!(
                "  Note: Pre-compensation only supported for sweep signals (got {})",
                signal_type.as_str()
            );
            println!("  Post-compensation will still be applied to CSV output");
        }
    }

    // Prepare mono signal with fades and padding
    println!("\n[2/{}] Preparing mono signal...", total_recordings + 2);
    let prepared_signal = prepare_signal(base_signal.clone(), sample_rate);
    println!(
        "  ✓ Prepared mono signal with {} samples",
        prepared_signal.len()
    );

    // Perform recording for each send/record channel pair
    // If there's only one record channel, it's reused for all send channels
    for (idx, &send_ch) in send_to_channels.iter().enumerate() {
        let record_ch = if record_from_channels.len() == 1 {
            record_from_channels[0]
        } else {
            record_from_channels[idx]
        };
        println!(
            "\n[{}/{}] Playing to hw channel {}, recording from hw channel {}...",
            idx + 3,
            total_recordings + 2,
            send_ch,
            record_ch
        );

        // Generate output filenames - include both send and record channels
        let (wav_path, csv_path) = generate_output_filenames_stereo(
            name.as_deref(),
            signal_type,
            send_ch,
            record_ch,
            sample_rate,
        );

        println!("  Output WAV: {:?}", wav_path);
        println!("  Output CSV: {:?}", csv_path);

        // Write mono signal to temporary WAV file
        println!("  Writing temporary mono WAV file...");
        let temp_wav = write_temp_wav(&prepared_signal, sample_rate, 1)?;
        println!("  Temp file: {:?}", temp_wav.path());

        // Perform actual playback and recording
        println!("  Starting playback and recording...");
        println!("  Playing mono signal to hw output channel {}", send_ch);
        println!("  Recording mono from hw input channel {}", record_ch);

        // Resolve per-channel calibration: per-channel override > global fallback
        let effective_mic_comp = mic_calibration_map
            .get(&(record_ch as usize))
            .map(|s| s.as_str())
            .or(microphone_compensation.as_deref());

        record_and_analyze(
            temp_wav.path(),  // Use the temporary WAV file for playback
            &wav_path,        // Record to the final output WAV file
            &prepared_signal, // Use the prepared mono signal for analysis
            sample_rate,
            &csv_path,
            send_ch,            // Output channel
            record_ch,          // Input channel
            device.as_deref(),  // Optional output device name
            device.as_deref(),  // Optional input device name
            effective_mic_comp, // Per-channel or global microphone compensation
            None,               // Optional sweep range
        )?;

        println!("  ✓ Recording complete");

        // Add pause between channel recordings if there are more to process
        if idx + 1 < total_recordings {
            println!("  Waiting 500ms before next recording...");
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("All recordings complete!");
    println!("{}", "=".repeat(60));

    Ok(())
}
