// ============================================================================
// FFT Plugin Chain Integration Tests
// ============================================================================
// Verifies that chains containing multiple FFT-based plugins (Upmixer, Denoiser, etc.)
// correctly handle arbitrary frame sizes and STFT latency without drift or glitches.

use sotf_audio::engine::{AudioEngine, EngineConfig};
use sotf_audio::plugins::{PluginChain, PluginSettings, PluginType};
use std::time::Duration;

#[test]
fn test_fft_chain_arbitrary_frame_sizes() {
    // 1. Setup a chain with Upmixer and Denoiser
    // This chain changes channel count (2 -> 6) and has multiple STFT windows.
    let mut chain = PluginChain::new();

    // Add Upmixer (5.1 config = 6 channels)
    let upmixer_idx = chain.add_plugin(&PluginType::Upmixer);

    // Add Denoiser (processes 6 channels)
    let _denoiser_idx = chain.add_plugin(&PluginType::Denoiser);

    // Add Spectrum Analyzer (processes 6 channels)
    let _spectrum_idx = chain.add_plugin(&PluginType::SpectrumAnalyzer);

    // Configure Upmixer for 5.1
    if let Some(plugin) = chain.get_plugin_mut(upmixer_idx)
        && let PluginSettings::Upmixer { speaker_config, .. } = &mut plugin.settings
    {
        *speaker_config = "5.1".to_string();
    }

    chain.update_channel_dependent_plugins();

    let sample_rate = 48000;
    let configs = chain.to_plugin_configs(sample_rate as f64);

    // 2. Create Engine with a specific non-power-of-two frame size
    // This often happens in practice due to resampling or hardware constraints.
    let config = EngineConfig {
        version: 1,
        frame_size: 1115, // Arbitrary non-power-of-two size
        buffer_ms: 100,
        output_sample_rate: sample_rate,
        input_channels: 2,
        output_channels: 6,
        output_device: None,
        plugins: configs,
        volume: 1.0,
        muted: false,
        config_path: None,
        watch_config: false,
        hal_mode: false,
        allow_virtual_output: true,
    };

    let engine = AudioEngine::new(config).expect("Failed to create engine");

    // Generate a test file (sine wave at 1kHz)
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_sine.wav");
    create_test_sine_wav(&file_path, sample_rate, 2.0, 1000.0); // 2 seconds, 1000Hz

    // 3. Play and verify no errors
    engine.play(&file_path).expect("Failed to start playback");

    // Let it run for a bit to process many arbitrary-sized blocks
    std::thread::sleep(Duration::from_millis(500));

    let state = engine.get_state();
    assert!(
        state.last_error.is_none(),
        "Engine reported error: {:?}",
        state.last_error
    );

    // 4. Verify Spectrum Analyzer data
    // The engine index for spectrum analyzer should be 2 (Upmixer=0, Denoiser=1, Spectrum=2)
    // Actually, according to PluginChain logic, analyzers are moved to the end.
    // In our case: Upmixer(0), Denoiser(1) are processing, Spectrum(2) is analyzer.
    let spectrum_data_any = engine
        .get_cached_plugin_data(2)
        .expect("Failed to get spectrum data");

    let spectrum_data = spectrum_data_any
        .downcast_ref::<sotf_audio::SpectrumData>()
        .expect("Failed to downcast spectrum data");

    // Verify we have a peak around 1000Hz
    assert!(
        spectrum_data.peak_magnitude > -50.0,
        "Spectrum peak too low: {}dB",
        spectrum_data.peak_magnitude
    );

    let (peak_bin_idx, _) = spectrum_data
        .magnitudes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    let peak_freq = spectrum_data.frequencies[peak_bin_idx];
    assert!(
        (peak_freq - 1000.0).abs() < 200.0,
        "Peak frequency mismatch: {}Hz (expected ~1000Hz)",
        peak_freq
    );
}

fn create_test_sine_wav(path: &std::path::Path, sample_rate: u32, duration_secs: f32, freq: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
        let amplitude = (i16::MAX as f32 * 0.5) as i16;
        writer
            .write_sample((sample * amplitude as f32) as i16)
            .unwrap();
        writer
            .write_sample((sample * amplitude as f32) as i16)
            .unwrap();
    }
    writer.finalize().unwrap();
}
