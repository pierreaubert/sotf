// ============================================================================
// Analyzer Plugins Integration Tests
// ============================================================================
//
// Demonstrates how to use analyzer plugins that compute metrics without
// producing audio output.

use sotf_host::{
    ChannelAssignment, ChannelLayout, ChannelRole, LoudnessData, LoudnessMonitorPlugin,
    ParameterId, ParameterValue, Plugin, ProcessContext, SpectrumAnalyzerPlugin, SpectrumConfig,
    SpectrumData,
};

fn explicit_5_1(order: [ChannelRole; 6]) -> ChannelLayout {
    ChannelLayout::new(
        order
            .into_iter()
            .enumerate()
            .map(|(index, role)| ChannelAssignment { index, role })
            .collect(),
    )
    .unwrap()
}

fn render_loudness(layout: ChannelLayout, role_samples: &[(ChannelRole, f32)]) -> LoudnessData {
    let channels = layout.channels.len();
    let frames = 48_000 * 4;
    let mut plugin = LoudnessMonitorPlugin::with_channel_layout(layout).unwrap();
    plugin.initialize(48_000).unwrap();
    let mut input = vec![0.0; frames * channels];
    for (frame_index, frame) in input.chunks_exact_mut(channels).enumerate() {
        let polarity = if frame_index.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        for (role, value) in role_samples {
            let index = plugin
                .channel_layout()
                .unwrap()
                .channels
                .iter()
                .find(|assignment| assignment.role == *role)
                .unwrap()
                .index;
            frame[index] = *value * polarity;
        }
    }
    let mut output = vec![0.0; input.len()];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, frames))
        .unwrap();
    plugin
        .get_data()
        .unwrap()
        .downcast_ref::<LoudnessData>()
        .unwrap()
        .clone()
}

#[test]
fn explicit_5_1_loudness_is_independent_of_physical_channel_order() {
    use ChannelRole::*;
    let canonical = explicit_5_1([FrontLeft, FrontRight, FrontCenter, Lfe, SideLeft, SideRight]);
    let reordered = explicit_5_1([Lfe, SideRight, FrontCenter, FrontLeft, SideLeft, FrontRight]);
    let signal = [
        (FrontLeft, 0.05),
        (FrontRight, 0.08),
        (FrontCenter, 0.11),
        (Lfe, 0.9),
        (SideLeft, 0.14),
        (SideRight, 0.17),
    ];
    let a = render_loudness(canonical, &signal);
    let b = render_loudness(reordered, &signal);
    assert!(a.channel_layout_is_compliant && b.channel_layout_is_compliant);
    assert!((a.momentary_lufs - b.momentary_lufs).abs() < 1.0e-9);
    assert!((a.shortterm_lufs - b.shortterm_lufs).abs() < 1.0e-9);
    assert!((a.integrated_lufs - b.integrated_lufs).abs() < 1.0e-9);
}

#[test]
fn explicit_layout_excludes_lfe_from_bs1770_loudness_but_keeps_peak() {
    use ChannelRole::*;
    let data = render_loudness(
        explicit_5_1([FrontLeft, FrontRight, FrontCenter, Lfe, SideLeft, SideRight]),
        &[(Lfe, 0.8)],
    );
    assert_eq!(data.momentary_lufs, f64::NEG_INFINITY);
    assert_eq!(data.shortterm_lufs, f64::NEG_INFINITY);
    assert_eq!(data.integrated_lufs, f64::NEG_INFINITY);
    assert!((data.peak - 0.8).abs() < 1.0e-6);
    assert!(data.channel_layout_is_compliant);
}

#[test]
fn explicit_layout_applies_the_bs1770_surround_energy_coefficient() {
    use ChannelRole::*;
    let layout = explicit_5_1([FrontLeft, FrontRight, FrontCenter, Lfe, SideLeft, SideRight]);
    let front = render_loudness(layout.clone(), &[(FrontLeft, 0.1)]);
    let surround = render_loudness(layout, &[(SideLeft, 0.1)]);
    let expected_delta = 10.0 * 1.41_f64.log10();
    let momentary_delta = surround.momentary_lufs - front.momentary_lufs;
    let integrated_delta = surround.integrated_lufs - front.integrated_lufs;
    assert!(
        (momentary_delta - expected_delta).abs() < 2.0e-5,
        "momentary delta {momentary_delta}, expected {expected_delta}"
    );
    assert!(
        (integrated_delta - expected_delta).abs() < 2.0e-5,
        "integrated delta {integrated_delta}, expected {expected_delta}"
    );
}

#[test]
fn explicit_large_layouts_are_compliant_and_count_only_multichannel_is_not() {
    for id in ["7.1", "7.1.4", "9.1.6"] {
        let layout = ChannelLayout::from_speaker_config(
            sotf_host::speaker_config::get_speaker_config(id).unwrap(),
        )
        .unwrap();
        let channels = layout.channels.len();
        let mut plugin = LoudnessMonitorPlugin::with_channel_layout(layout).unwrap();
        plugin.initialize(48_000).unwrap();
        let input = vec![0.0; channels];
        let mut output = vec![0.0; channels];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 1))
            .unwrap();
        let data = plugin.get_data().unwrap();
        assert!(
            data.downcast_ref::<LoudnessData>()
                .unwrap()
                .channel_layout_is_compliant,
            "{id}"
        );
    }

    let mut ambiguous = LoudnessMonitorPlugin::new(8).unwrap();
    ambiguous.initialize(48_000).unwrap();
    ambiguous
        .process(&[0.0; 8], &mut [0.0; 8], &ProcessContext::new(48_000, 1))
        .unwrap();
    let data = ambiguous.get_data().unwrap();
    assert!(
        !data
            .downcast_ref::<LoudnessData>()
            .unwrap()
            .channel_layout_is_compliant
    );
}

#[test]
fn loudness_monitor_rejects_malformed_or_width_mismatched_layouts() {
    use ChannelRole::*;
    let malformed = ChannelLayout {
        channels: vec![
            ChannelAssignment {
                index: 0,
                role: FrontLeft,
            },
            ChannelAssignment {
                index: 0,
                role: FrontRight,
            },
        ],
    };
    assert!(LoudnessMonitorPlugin::with_channel_layout(malformed).is_err());

    let stereo = ChannelLayout::new(vec![
        ChannelAssignment {
            index: 0,
            role: FrontLeft,
        },
        ChannelAssignment {
            index: 1,
            role: FrontRight,
        },
    ])
    .unwrap();
    assert!(LoudnessMonitorPlugin::new_with_layout(6, stereo).is_err());
}

#[test]
fn test_loudness_monitor_rejects_invalid_construction_and_initialization() {
    assert!(LoudnessMonitorPlugin::new(0).is_err());

    let mut monitor = LoudnessMonitorPlugin::new(2).unwrap();
    assert!(monitor.initialize(0).is_err());

    let input = vec![0.0; 8];
    let mut output = vec![0.0; 8];
    let context = ProcessContext::new(48_000, 4);
    assert!(monitor.process(&input, &mut output, &context).is_err());
}

#[test]
fn test_loudness_monitor_validates_context_and_exact_buffer_geometry() {
    let mut monitor = LoudnessMonitorPlugin::new(2).unwrap();
    monitor.initialize(48_000).unwrap();

    let context = ProcessContext::new(48_000, 4);
    let input = vec![0.0; 8];
    let mut short_output = vec![0.0; 7];
    assert!(
        monitor
            .process(&input, &mut short_output, &context)
            .is_err()
    );

    let short_input = vec![0.0; 7];
    let mut output = vec![0.0; 8];
    assert!(
        monitor
            .process(&short_input, &mut output, &context)
            .is_err()
    );

    let long_input = vec![0.0; 9];
    let mut long_output = vec![0.0; 9];
    assert!(
        monitor
            .process(&long_input, &mut long_output, &context)
            .is_err()
    );

    let wrong_rate = ProcessContext::new(44_100, 4);
    assert!(monitor.process(&input, &mut output, &wrong_rate).is_err());

    let overflow = ProcessContext::new(48_000, usize::MAX);
    assert!(monitor.process(&[], &mut [], &overflow).is_err());
}

#[test]
fn test_loudness_monitor_keeps_frames_across_former_ring_wrap() {
    let channels = 7;
    let mut monitor = LoudnessMonitorPlugin::new(channels).unwrap();
    monitor.initialize(48_000).unwrap();

    // 13,714 * 7 = 95,998 samples. The old 96,000-sample ring then split
    // the next two frames into 2- and 12-sample slices, neither frame-aligned.
    let first_frames = 13_714;
    let first = vec![0.0; first_frames * channels];
    let mut first_output = vec![0.0; first.len()];
    monitor
        .process(
            &first,
            &mut first_output,
            &ProcessContext::new(48_000, first_frames),
        )
        .unwrap();

    let second = vec![0.75; 2 * channels];
    let mut second_output = vec![0.0; second.len()];
    monitor
        .process(&second, &mut second_output, &ProcessContext::new(48_000, 2))
        .unwrap();

    let data = monitor.get_data().unwrap();
    let loudness = data.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(second_output, second);
    assert!((loudness.peak - 0.75).abs() < 1.0e-6);
}

#[test]
fn test_loudness_monitor_does_not_truncate_blocks_larger_than_old_ring() {
    let channels = 32;
    let frames = 3_001;
    let mut monitor = LoudnessMonitorPlugin::new(channels).unwrap();
    monitor.initialize(48_000).unwrap();

    let mut input = vec![0.0; frames * channels];
    input[(frames - 1) * channels..].fill(0.9);
    let mut output = vec![0.0; input.len()];
    monitor
        .process(&input, &mut output, &ProcessContext::new(48_000, frames))
        .unwrap();

    let data = monitor.get_data().unwrap();
    let loudness = data.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(output, input);
    assert!((loudness.peak - 0.9).abs() < 1.0e-6);
}

#[test]
fn test_loudness_monitor_disable_clears_and_reenable_starts_fresh() {
    let mut monitor = LoudnessMonitorPlugin::new(2).unwrap();
    monitor.initialize(48_000).unwrap();
    let input = vec![0.8; 2_048];
    let mut output = vec![0.0; input.len()];
    let context = ProcessContext::new(48_000, 1_024);
    monitor.process(&input, &mut output, &context).unwrap();

    monitor
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let disabled = monitor.get_data().unwrap();
    let disabled = disabled.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(disabled.peak, 0.0);
    assert!(disabled.correlation_lr.is_none());
    assert!(!disabled.measurement_enabled);
    assert!(!disabled.measurement_valid);

    monitor
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    let silence = vec![0.0; input.len()];
    monitor.process(&silence, &mut output, &context).unwrap();
    let reenabled = monitor.get_data().unwrap();
    let reenabled = reenabled.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(reenabled.peak, 0.0);
    assert!(reenabled.measurement_enabled);
}

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
    assert_eq!(
        output, input,
        "the loudness analyzer must be bit-transparent"
    );

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
    assert!(
        (loudness.momentary_lufs - -20.0).abs() < 0.2,
        "1 kHz stereo sine at -20 dBFS measured {:.3} LUFS",
        loudness.momentary_lufs
    );
}

#[test]
fn loudness_monitor_stereo_correlation_is_centered_and_partition_invariant() {
    fn render(partitions: &[usize]) -> f64 {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(48_000).unwrap();
        let frames = 8192;
        let input: Vec<f32> = (0..frames)
            .flat_map(|frame| {
                let signal = (std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0).sin();
                [signal + 3.0, signal * 0.2 - 7.0]
            })
            .collect();
        let mut offset = 0;
        for &partition in partitions {
            if offset == frames {
                break;
            }
            let count = partition.min(frames - offset);
            let start = offset * 2;
            let end = (offset + count) * 2;
            let mut output = vec![0.0; count * 2];
            plugin
                .process(
                    &input[start..end],
                    &mut output,
                    &ProcessContext::new(48_000, count),
                )
                .unwrap();
            offset += count;
        }
        if offset < frames {
            let mut output = vec![0.0; (frames - offset) * 2];
            plugin
                .process(
                    &input[offset * 2..],
                    &mut output,
                    &ProcessContext::new(48_000, frames - offset),
                )
                .unwrap();
        }
        let data = plugin.get_data().unwrap();
        data.downcast_ref::<LoudnessData>()
            .unwrap()
            .correlation_lr
            .unwrap()
    }

    let whole = render(&[8192]);
    let partitioned = render(&[1, 7, 63, 511, 2048, 4096]);
    assert!(
        whole > 0.999,
        "DC/gain invariant Pearson expected, got {whole}"
    );
    assert!((whole - partitioned).abs() < 1.0e-6);
}

#[test]
fn loudness_data_exposes_validity_true_peak_scope_and_integrated_window() {
    for sample_rate in [44_100, 48_000, 88_200, 96_000, 192_000] {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(sample_rate).unwrap();
        let frames = sample_rate as usize * 4;
        let input = vec![0.1; frames * 2];
        let mut output = vec![0.0; input.len()];
        plugin
            .process(
                &input,
                &mut output,
                &ProcessContext::new(sample_rate, frames),
            )
            .unwrap();
        let data = plugin.get_data().unwrap();
        let data = data.downcast_ref::<LoudnessData>().unwrap();
        assert_eq!(data.true_peak_is_compliant, sample_rate == 48_000);
        assert_eq!(data.integrated_window_seconds, 3_600);
        assert!(data.channel_layout_is_compliant);
        assert!(data.measurement_valid);
        assert_eq!(data.query_error_generation, 0);
    }
}

#[test]
fn count_only_multichannel_measurement_is_explicitly_noncompliant() {
    for channels in [5, 6, 8, 12, 16] {
        let mut plugin = LoudnessMonitorPlugin::new(channels).unwrap();
        plugin.initialize(48_000).unwrap();
        let input = vec![0.1; 48_000 * channels];
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 48_000))
            .unwrap();
        let data = plugin.get_data().unwrap();
        let data = data.downcast_ref::<LoudnessData>().unwrap();
        assert!(!data.channel_layout_is_compliant);
    }
}

#[test]
fn incomplete_loudness_windows_are_invalid_not_plausible_minus_120() {
    let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
    plugin.initialize(48_000).unwrap();
    let input = vec![0.1; 256 * 2];
    let mut output = vec![0.0; input.len()];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 256))
        .unwrap();
    let data = plugin.get_data().unwrap();
    let data = data.downcast_ref::<LoudnessData>().unwrap();
    assert!(!data.measurement_valid);
    assert!(data.query_error_generation > 0);
    assert!(data.momentary_lufs.is_infinite() && data.momentary_lufs.is_sign_negative());
    assert!(data.shortterm_lufs.is_infinite() && data.shortterm_lufs.is_sign_negative());
    assert!(data.integrated_lufs.is_infinite() && data.integrated_lufs.is_sign_negative());
}

#[test]
fn test_loudness_monitor_keeps_32_channel_peak_slots() {
    let channels = 32;
    let mut monitor = LoudnessMonitorPlugin::new(channels).unwrap();
    monitor.initialize(48000).unwrap();

    let num_frames = 1024;
    let input = vec![0.05_f32; num_frames * channels];
    let mut output = vec![0.0; input.len()];
    let context = ProcessContext::new(48000, num_frames);

    monitor.process(&input, &mut output, &context).unwrap();

    let data = monitor.get_data().unwrap();
    let loudness = data.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(loudness.channel_peaks.len(), channels);
    assert_eq!(loudness.true_peaks_dbtp.len(), channels);
}

#[test]
fn test_loudness_monitor_spatial_matrix_is_opt_in_and_survives_initialize() {
    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 48_000.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;
    }

    let context = ProcessContext::new(48_000, num_frames);
    let mut output = vec![0.0_f32; input.len()];

    let mut disabled = LoudnessMonitorPlugin::new(2).unwrap();
    disabled.initialize(48_000).unwrap();
    disabled.process(&input, &mut output, &context).unwrap();
    let disabled_data = disabled.get_data().unwrap();
    let disabled_loudness = disabled_data.downcast_ref::<LoudnessData>().unwrap();
    assert!(disabled_loudness.correlation_matrix.is_empty());
    assert_eq!(disabled_loudness.correlation_samples_seen, 0);

    let mut enabled = LoudnessMonitorPlugin::new(2).unwrap().with_spatial();
    enabled.initialize(48_000).unwrap();
    enabled.process(&input, &mut output, &context).unwrap();
    let enabled_data = enabled.get_data().unwrap();
    let enabled_loudness = enabled_data.downcast_ref::<LoudnessData>().unwrap();
    assert_eq!(enabled_loudness.correlation_matrix.len(), 4);
    assert!(enabled_loudness.correlation_samples_seen >= num_frames as u64);
    assert!(enabled_loudness.correlation_matrix[0] > 0.99);
    assert!(enabled_loudness.correlation_matrix[3] > 0.99);
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
    assert_eq!(
        output, input,
        "the spectrum analyzer must be bit-transparent"
    );

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
        input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 138.0 * t).sin() * 0.1;
        // RR
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
