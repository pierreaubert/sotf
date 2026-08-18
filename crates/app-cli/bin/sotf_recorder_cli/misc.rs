use sotf_audio_player::recording_helpers::capture_signal_params;
use std::str::FromStr;

/// Validate a user-supplied amplitude flag (R3): reject anything outside
/// `(0.0, 1.0]` with a clear error instead of silently clamping, so a level
/// above 0 dBFS can never clip the stimulus unnoticed.
fn validated_amp(amp: Option<f32>, flag: &str) -> Result<f32, String> {
    let amp = amp.unwrap_or(0.5);
    if !amp.is_finite() || amp <= 0.0 || amp > 1.0 {
        return Err(format!(
            "{} must be in the range (0.0, 1.0] — level must be ≤ 0 dBFS to avoid clipping the stimulus, got {}",
            flag, amp
        ));
    }
    Ok(amp)
}

pub(super) fn list_audio_devices() {
    println!("{}", "=".repeat(80));
    println!("Available Audio Devices");
    println!("{}", "=".repeat(80));

    let devices = match sotf_audio::devices::get_audio_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{}",
                sotf_audio::signal_recorder::actionable_capture_error(
                    "Failed to enumerate audio devices",
                    &e
                )
            );
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

pub(super) fn format_sample_rate_range(rates: &[u32]) -> String {
    match (rates.first(), rates.last()) {
        (None, _) => "unknown".to_string(),
        (Some(rate), Some(_)) if rates.len() == 1 => format!("{} Hz", rate),
        (Some(first), Some(last)) => format!("{}-{} Hz", first, last),
        _ => "unknown".to_string(),
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
    output_dir: Option<std::path::PathBuf>,
    device: Option<String>,
    freq: Option<f32>,
    freq1: Option<f32>,
    freq2: Option<f32>,
    start_freq: f32,
    end_freq: f32,
    amp: Option<f32>,
    amp1: Option<f32>,
    amp2: Option<f32>,
    mls_order: Option<u8>,
    microphone_compensation: Option<String>,
    mic_calibration_map: std::collections::HashMap<usize, String>,
) -> Result<(), String> {
    use sotf_audio::signal_recorder::*;

    let output_dir = output_dir.unwrap_or_else(|| {
        std::env::current_dir().expect("current directory should be accessible")
    });
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("failed to create output directory: {e}"))?;

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

    // Build signal parameters based on signal type. The sweep case routes
    // through the shared sotf-player helper so every frontend builds sweep
    // params identically; Tone/TwoTone/MLS stay local because the CLI has
    // dedicated flags (--freq/--freq1/--freq2/--mls-order) the shared
    // helper does not model.
    let params = match signal_type {
        SignalType::Tone => {
            let freq = freq.ok_or("--freq is required for tone signal")?;
            let amp = validated_amp(amp, "--amp")?;
            SignalParams::Tone { freq, amp }
        }
        SignalType::TwoTone => {
            let freq1 = freq1.ok_or("--freq1 is required for two-tone signal")?;
            let freq2 = freq2.ok_or("--freq2 is required for two-tone signal")?;
            let amp1 = validated_amp(amp1, "--amp1")?;
            let amp2 = validated_amp(amp2, "--amp2")?;
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            }
        }
        SignalType::Sweep => capture_signal_params(
            SignalType::Sweep,
            start_freq,
            end_freq,
            validated_amp(amp, "--amp")?,
            None,
            None,
            None,
        ),
        SignalType::WhiteNoise | SignalType::PinkNoise | SignalType::MNoise => {
            let amp = validated_amp(amp, "--amp")?;
            SignalParams::Noise { amp }
        }
        SignalType::Mls => {
            let order = mls_order.unwrap_or(DEFAULT_MLS_ORDER);
            let amp = validated_amp(amp, "--amp")?;
            SignalParams::Mls { order, amp }
        }
        SignalType::Dirac => {
            let amp = validated_amp(amp, "--amp")?;
            SignalParams::Dirac { amp }
        }
    };

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

    // Prepare the stimulus (validate → generate → 20 ms fades + 250 ms
    // padding) through the shared engine helper so all frontends share one
    // prepare+validate path (R4).
    let total_recordings = send_to_channels.len(); // One recording per send/record pair
    println!("[1/{}] Preparing signal...", total_recordings + 1);

    // Load playback pre-compensation up front (sweeps only). It modulates
    // the raw sweep sample-by-sample using the buffer length as the sweep
    // duration, so it must run between generation and `prepare_signal` —
    // the one case that cannot go through `prepare_measurement_signal`
    // end to end; it composes the same engine steps around it.
    let pre_compensation = match microphone_compensation {
        Some(ref comp_path) => {
            use sotf_audio::signal_analysis::MicrophoneCompensation;
            use std::path::Path;

            println!("\n  Loading microphone compensation for playback pre-compensation...");
            let compensation = MicrophoneCompensation::from_file(Path::new(comp_path))?;
            if signal_type == SignalType::Sweep {
                println!(
                    "  Applying inverse microphone compensation to sweep ({} Hz - {} Hz)...",
                    start_freq, end_freq
                );
                println!(
                    "  This pre-compensates the playback signal so the microphone records flat"
                );
                Some(compensation)
            } else {
                // Only sweeps have a well-defined instantaneous frequency
                println!(
                    "  Note: Pre-compensation only supported for sweep signals (got {})",
                    signal_type.as_str()
                );
                println!("  Post-compensation will still be applied to CSV output");
                None
            }
        }
        None => None,
    };

    // Validate that the signal is mono (Vec<f32> represents mono)
    // All our signal generation functions return mono signals
    let prepared_signal = match pre_compensation {
        Some(ref compensation) => {
            validate_signal_params(signal_type, &params, duration, sample_rate)?;
            let raw_signal = generate_signal(signal_type, &params, duration, sample_rate)?;
            // Apply inverse compensation: boost where mic is weak, cut where mic is loud
            let compensated = compensation.apply_to_sweep(
                &raw_signal,
                start_freq,
                end_freq,
                sample_rate,
                true,
            );
            println!("  ✓ Applied pre-compensation to playback signal");
            prepare_signal(compensated, sample_rate)
        }
        None => prepare_measurement_signal(signal_type, &params, duration, sample_rate)?,
    };
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
            idx + 2,
            total_recordings + 1,
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
        let wav_path = output_dir.join(wav_path);
        let csv_path = output_dir.join(csv_path);

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
            1,                  // num_sweeps: CLI captures a single sweep (no repeat flag)
            None,               // Optional cancel flag
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

#[cfg(test)]
mod tests {

    use super::super::format_sample_rate_range;
    use super::validated_amp;

    #[test]
    fn sample_rate_range_handles_empty_single_and_multiple_rates() {
        assert_eq!(format_sample_rate_range(&[]), "unknown");
        assert_eq!(format_sample_rate_range(&[48_000]), "48000 Hz");
        assert_eq!(
            format_sample_rate_range(&[44_100, 48_000, 96_000]),
            "44100-96000 Hz"
        );
    }

    #[test]
    fn validated_amp_defaults_and_passes_valid_values() {
        assert_eq!(validated_amp(None, "--amp").unwrap(), 0.5);
        assert_eq!(validated_amp(Some(1.0), "--amp").unwrap(), 1.0);
        assert_eq!(validated_amp(Some(0.25), "--amp1").unwrap(), 0.25);
    }

    #[test]
    fn validated_amp_rejects_clipping_and_invalid_levels() {
        for bad in [1.5, 0.0, -0.5, f32::NAN, f32::INFINITY] {
            let err = validated_amp(Some(bad), "--amp").unwrap_err();
            assert!(err.contains("--amp"), "error names the flag: {}", err);
            assert!(
                err.contains("0 dBFS"),
                "error explains the dBFS limit: {}",
                err
            );
        }
    }

    #[test]
    fn sweep_defaults_match_shared_player_defaults() {
        use clap::Parser;
        let cli = crate::types::Cli::try_parse_from(["sotf_recorder"]).expect("defaults parse");
        assert_eq!(
            cli.start_freq,
            sotf_audio_player::recording_helpers::DEFAULT_SWEEP_START_FREQ
        );
        assert_eq!(
            cli.end_freq,
            sotf_audio_player::recording_helpers::DEFAULT_SWEEP_END_FREQ
        );
    }
}
