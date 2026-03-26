// ============================================================================
// Analyzer Plugins Integration Tests
// ============================================================================
//
// Demonstrates how to use analyzer plugins that compute metrics without
// producing audio output.

use sotf_host::{
    LoudnessData, LoudnessMonitorPlugin, Plugin, ProcessContext, SpectrumAnalyzerPlugin,
    SpectrumConfig, SpectrumData,
};

#[test]
fn test_loudness_monitor_stereo() {
    // Create a loudness monitor for stereo audio
    let mut monitor = LoudnessMonitorPlugin::new(2).unwrap();
    monitor.initialize(48000).unwrap();

    // Generate test signal: -20dBFS tone
    let num_frames = 4800; // 100ms at 48kHz
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        let sample = phase.sin() * 0.1; // -20dBFS
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let context = ProcessContext::new(48000, num_frames);

    // Process audio
    let mut output = vec![0.0; input.len()];
    monitor.process(&input, &mut output, &context).unwrap();

    // Get loudness data
    let data_opt = monitor.get_data();
    let data = data_opt.unwrap();
    let loudness = data.downcast_ref::<LoudnessData>().unwrap();

    println!("Loudness Monitor Results:");
    println!("  Momentary: {:.1} LUFS", loudness.momentary_lufs);
    println!("  Short-term: {:.1} LUFS", loudness.shortterm_lufs);
    println!(
        "  Peak: {:.3} ({:.1} dBFS)",
        loudness.peak,
        20.0 * loudness.peak.log10()
    );

    // Peak should be around 0.1
    assert!(loudness.peak > 0.05 && loudness.peak < 0.15);
}

#[test]
fn test_spectrum_analyzer_stereo() {
    // Create a spectrum analyzer for stereo audio
    let config = SpectrumConfig {
        num_bins: 30,
        min_freq: 20.0,
        max_freq: 20000.0,
        smoothing: 0.0, // No smoothing for testing
    };

    let mut analyzer = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
    analyzer.initialize(48000).unwrap();

    // Generate test signal: 440Hz sine wave
    let num_frames = 2048;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0;
        let sample = phase.sin() * 0.5;
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let context = ProcessContext::new(48000, num_frames);

    // Process audio
    let mut output = vec![0.0; input.len()];
    analyzer.process(&input, &mut output, &context).unwrap();

    // Get spectrum data
    let data_opt = analyzer.get_data();
    let data = data_opt.unwrap();
    let spectrum = data.downcast_ref::<SpectrumData>().unwrap();

    println!("\nSpectrum Analyzer Results:");
    println!("  Number of bins: {}", spectrum.frequencies.len());
    println!(
        "  Frequency range: {:.0}Hz - {:.0}Hz",
        spectrum.frequencies.first().unwrap_or(&0.0),
        spectrum.frequencies.last().unwrap_or(&0.0)
    );
    println!("  Peak magnitude: {:.1} dB", spectrum.peak_magnitude);

    // Print all bins
    println!("\n  Bins:");
    for (i, (&freq, &mag)) in spectrum
        .frequencies
        .iter()
        .zip(spectrum.magnitudes.iter())
        .enumerate()
    {
        println!("    {:2}. {:6.0}Hz: {:6.1} dB", i, freq, mag);
    }

    assert_eq!(spectrum.frequencies.len(), 30);
}

#[test]
fn test_both_analyzers_together() {
    // Demonstrate using both analyzers on the same audio stream

    let mut loudness = LoudnessMonitorPlugin::new(2).unwrap();
    let mut spectrum = SpectrumAnalyzerPlugin::new(2).unwrap();

    loudness.initialize(48000).unwrap();
    spectrum.initialize(48000).unwrap();

    // Generate complex signal (mix of frequencies)
    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];

    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let mut sample = 0.0;

        // Mix of harmonics
        sample += (2.0 * std::f32::consts::PI * 100.0 * t).sin() * 0.2; // Bass
        sample += (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3; // A4
        sample += (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2; // Mid
        sample += (2.0 * std::f32::consts::PI * 5000.0 * t).sin() * 0.1; // Treble

        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let context = ProcessContext::new(48000, num_frames);

    // Process with both analyzers
    let mut output = vec![0.0; input.len()];
    loudness.process(&input, &mut output, &context).unwrap();
    spectrum.process(&input, &mut output, &context).unwrap();

    // Get results from both
    let loudness_data = loudness.get_data().unwrap();
    let spectrum_data = spectrum.get_data().unwrap();

    let ld = loudness_data.downcast_ref::<LoudnessData>().unwrap();
    let sd = spectrum_data.downcast_ref::<SpectrumData>().unwrap();

    println!("\nCombined Analysis Results:");
    println!(
        "  Loudness: {:.1} LUFS, Peak: {:.3}",
        ld.momentary_lufs, ld.peak
    );
    println!(
        "  Spectrum: {} bins, Peak: {:.1} dB",
        sd.frequencies.len(),
        sd.peak_magnitude
    );

    // Both should have computed something
    assert!(ld.peak > 0.0);
    assert!(sd.peak_magnitude > f32::NEG_INFINITY);
}

#[test]
fn test_analyzer_with_5ch_audio() {
    // Test analyzers with 5.0 surround audio (after upmixing)

    let mut loudness = LoudnessMonitorPlugin::new(5).unwrap();
    let mut spectrum = SpectrumAnalyzerPlugin::new(5).unwrap();

    loudness.initialize(48000).unwrap();
    spectrum.initialize(48000).unwrap();

    // Generate 5-channel audio
    let num_frames = 2048;
    let mut input = vec![0.0_f32; num_frames * 5];

    for i in 0..num_frames {
        let t = i as f32 / 48000.0;

        // Different content on each channel
        input[i * 5] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2; // FL
        input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 554.0 * t).sin() * 0.2; // FR
        input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.2; // C
        input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 110.0 * t).sin() * 0.1; // RL
        input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 138.0 * t).sin() * 0.1; // RR
    }

    let context = ProcessContext::new(48000, num_frames);

    // Process
    let mut output = vec![0.0; input.len()];
    loudness.process(&input, &mut output, &context).unwrap();
    spectrum.process(&input, &mut output, &context).unwrap();

    // Get results
    let loudness_data = loudness.get_data().unwrap();
    let spectrum_data = spectrum.get_data().unwrap();

    let ld = loudness_data.downcast_ref::<LoudnessData>().unwrap();
    let sd = spectrum_data.downcast_ref::<SpectrumData>().unwrap();

    println!("\n5-Channel Analysis:");
    println!(
        "  Loudness: {:.1} LUFS, Peak: {:.3}",
        ld.momentary_lufs, ld.peak
    );
    println!(
        "  Spectrum: {} bins, Peak: {:.1} dB",
        sd.frequencies.len(),
        sd.peak_magnitude
    );

    // Should have analyzed all channels
    assert!(ld.peak > 0.0);
}

#[test]
fn test_analyzer_reset() {
    let mut monitor = LoudnessMonitorPlugin::new(2).unwrap();
    monitor.initialize(48000).unwrap();

    // Process some audio
    let num_frames = 1024;
    let input = vec![0.5_f32; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    let mut output = vec![0.0; input.len()];
    monitor.process(&input, &mut output, &context).unwrap();

    // Get data before reset
    let data_before = monitor.get_data().unwrap();
    let ld_before = data_before.downcast_ref::<LoudnessData>().unwrap();
    let peak_before = ld_before.peak;

    println!("\nBefore reset: Peak = {:.3}", peak_before);

    // Reset
    monitor.reset();

    // Get data after reset
    let data_after = monitor.get_data().unwrap();
    let ld_after = data_after.downcast_ref::<LoudnessData>().unwrap();
    let peak_after = ld_after.peak;

    println!("After reset: Peak = {:.3}", peak_after);

    // Peak should be reset to 0
    assert!(peak_after < peak_before);
}
