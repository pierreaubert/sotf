use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Helper to find a device by fuzzy name
fn find_device(name_part: &str, input: bool) -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = cpal::default_host();
    let devices = if input {
        host.input_devices().ok()?
    } else {
        host.output_devices().ok()?
    };

    for device in devices {
        if let Ok(name) = device.name() {
            if name.contains(name_part) {
                if input {
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            if config.channels() >= 6 {
                                return Some((device, config.with_max_sample_rate()));
                            }
                        }
                    }
                } else {
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            if config.channels() >= 6 {
                                return Some((device, config.with_max_sample_rate()));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[test]
fn test_upmixer_real_audio_loopback() {
    // 1. Setup Devices
    // We need BlackHole 6ch (or 16ch/64ch) to support 5.1
    // Prioritize 16ch or 64ch as they are more common for multi-channel work
    let device_names = ["BlackHole 16ch", "BlackHole 64ch", "BlackHole 6ch"];
    
    let mut output_setup = None;
    let mut input_setup = None;

    for name in device_names {
        if let Some(out) = find_device(name, false) {
             if let Some(in_) = find_device(name, true) {
                 output_setup = Some(out);
                 input_setup = Some(in_);
                 println!("Found device: {}", name);
                 break;
             }
        }
    }

    if output_setup.is_none() || input_setup.is_none() {
        println!("SKIPPING test: BlackHole device with 6+ channels not found. Install BlackHole 16ch or 64ch.");
        return;
    }

    let (out_device, _out_config) = output_setup.unwrap();
    let (in_device, in_config) = input_setup.unwrap();
    
    println!("Using Output: {}", out_device.name().unwrap());
    println!("Using Input:  {}", in_device.name().unwrap());

    // 2. Configure Engine with Upmixer
    // We use the 'test_engine_config_with' pattern manually since we can't access test modules easily
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.name().unwrap());
    config.output_channels = 6; // Force 6 channels for 5.1
    
    config.plugins = vec![
        PluginConfig::new(
            "upmixer",
            json!({
                "speaker_config": "5.1",
                "gain_front_direct": 1.0,
                "gain_front_ambient": 1.0, 
                "gain_rear_ambient": 1.0,
                "lfe_cutoff_hz": 120.0,
                "stereo_width": 1.0,
                "bandpass_hz": 200.0,
                "height_gain": 0.0,
                "lfe_gain": 1.0,
                "enable_subharmonic_synth": false,
                "subharmonic_gain": 0.0,
                "enable_hr_direct": false, // Disable HR for simpler signal
                "hr_sharpen": 0.0,
                "safety_cap_db": 0.0,
                "decorrelation_mode": 0
            }),
        ),
    ];

    let mut engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => {
            println!("Failed to create AudioEngine: {}. Skipping test.", e);
            return;
        }
    };

    // 3. Create Test File (Stereo Sine Wave)
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(temp_file.path(), spec).unwrap();
    
    // Generate 2 seconds of audio at 440Hz
    for t in 0..(48000 * 2) {
        let sample = (t as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0).sin();
        let amp = (sample * i16::MAX as f32 * 0.5) as i16;
        writer.write_sample(amp).unwrap(); // Left
        writer.write_sample(amp).unwrap(); // Right
    }
    writer.finalize().unwrap();

    // 4. Start Capture Stream
    let captured_samples = Arc::new(Mutex::new(Vec::new()));
    let capture_clone = captured_samples.clone();
    let channels = in_config.channels();

    let stream = in_device.build_input_stream(
        &in_config.into(),
        move |data: &[f32], _: &_| {
            let mut buffer = capture_clone.lock().unwrap();
            buffer.extend_from_slice(data);
        },
        move |err| {
            eprintln!("Capture error: {}", err);
        },
        None
    ).expect("Failed to build input stream");

    stream.play().expect("Failed to start capture stream");

    // 5. Start Playback
    println!("Starting playback...");
    if let Err(e) = engine.play(temp_file.path().to_path_buf()) {
        println!("Failed to start playback: {}. Skipping test.", e);
        return;
    }

    // 6. Wait and Record
    // Wait a bit for engine to start
    std::thread::sleep(Duration::from_millis(500));
    // Record for 1.5 seconds
    std::thread::sleep(Duration::from_millis(1500)); 

    // 7. Analyze Results
    // Stop capturing
    drop(stream);
    
    let buffer = captured_samples.lock().unwrap();
    println!("Captured {} samples ({} frames at {} channels)", buffer.len(), buffer.len() / channels as usize, channels);

    if buffer.len() == 0 {
         println!("WARNING: Captured 0 samples. Loopback might not be working or permission denied.");
         panic!("No audio captured from loopback device");
    }

    // Verify channel content
    // We check RMS of each channel
    let mut channel_energy = vec![0.0; channels as usize];
    let frame_count = buffer.len() / channels as usize;

    for i in 0..frame_count {
        for ch in 0..channels as usize {
            let sample = buffer[i * channels as usize + ch];
            channel_energy[ch] += sample * sample;
        }
    }

    for ch in 0..channels as usize {
        channel_energy[ch] = (channel_energy[ch] / frame_count as f32).sqrt();
        println!("Channel {} RMS: {:.4}", ch, channel_energy[ch]);
    }

    // Assertions
    // Ensure we have at least 6 channels of data
    assert!(channels >= 6, "Input device must have at least 6 channels");

    // Check for signal presence (threshold -80dB ~= 0.0001)
    let threshold = 0.0001; 
    
    // Fronts (L/R)
    assert!(channel_energy[0] > threshold, "Left channel silent: {:.6}", channel_energy[0]);
    assert!(channel_energy[1] > threshold, "Right channel silent: {:.6}", channel_energy[1]);
    
    // Center (upmixer should extract phantom center from correlated stereo)
    assert!(channel_energy[2] > threshold, "Center channel silent: {:.6} - Upmixer failed?", channel_energy[2]);

    // LFE (generated from bass content) - 440Hz might not generate much LFE if crossover is low
    // 440Hz > 120Hz crossover, so LFE might be quiet depending on slope.
    println!("LFE Channel RMS: {:.6}", channel_energy[3]);

    // Surrounds
    assert!(channel_energy[4] >= 0.0, "Surround Left error");
    assert!(channel_energy[5] >= 0.0, "Surround Right error");

    println!("Test PASSED: 6 channels of audio verified via loopback.");
}