// ============================================================================
// Multi-Sample-Rate Integration Tests
// ============================================================================
//
// Tests that all plugins correctly handle various sample rates including
// 22050, 44100, 48000, 96000, and 192000 Hz.

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_plugins::{
    CompressorPlugin, CrossoverPlugin, DelayPlugin, EqPlugin, ExpanderPlugin, GainPlugin,
    GatePlugin, InPlacePlugin, InPlacePluginAdapter, LimiterPlugin, MatrixPlugin, Plugin,
    PluginHost, ProcessContext,
};

const SAMPLE_RATES: [u32; 5] = [22050, 44100, 48000, 96000, 192000];
const NUM_FRAMES: usize = 512;

/// Generate a stereo sine wave buffer
fn generate_sine_stereo(
    sample_rate: u32,
    freq: f32,
    amplitude: f32,
    num_frames: usize,
) -> Vec<f32> {
    (0..num_frames)
        .flat_map(|i| {
            let t = i as f32 / sample_rate as f32;
            let sample = (t * freq * 2.0 * std::f32::consts::PI).sin() * amplitude;
            vec![sample, sample]
        })
        .collect()
}

/// Generate a mono sine wave buffer
#[allow(dead_code)]
fn generate_sine_mono(sample_rate: u32, freq: f32, amplitude: f32, num_frames: usize) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (t * freq * 2.0 * std::f32::consts::PI).sin() * amplitude
        })
        .collect()
}

/// Assert all samples in a buffer are finite (no NaN or Inf)
fn assert_all_finite(buffer: &[f32], label: &str) {
    for (i, &s) in buffer.iter().enumerate() {
        assert!(
            s.is_finite(),
            "{}: non-finite value at index {} (value: {})",
            label,
            i,
            s
        );
    }
}

// ============================================================================
// Gain Plugin
// ============================================================================

#[test]
fn test_gain_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = GainPlugin::new(2, -6.0);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Gain@{}Hz", sr));

        // -6dB should halve amplitude
        let peak = buffer.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            peak < 0.4,
            "Gain@{}Hz: -6dB should reduce 0.5 amplitude, peak={}",
            sr,
            peak
        );
    }
}

// ============================================================================
// EQ Plugin
// ============================================================================

#[test]
fn test_eq_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let filters = vec![
            Biquad::new(BiquadFilterType::Peak, 1000.0, sr as f64, 1.0, 6.0),
            Biquad::new(BiquadFilterType::Lowshelf, 200.0, sr as f64, 0.707, -3.0),
        ];

        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, filters));
        plugin.initialize(sr).unwrap();

        let input = generate_sine_stereo(sr, 1000.0, 0.3, NUM_FRAMES);
        let mut output = vec![0.0_f32; NUM_FRAMES * 2];
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process(&input, &mut output, &context).unwrap();
        assert_all_finite(&output, &format!("EQ@{}Hz", sr));

        let output_energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(
            output_energy > 0.0,
            "EQ@{}Hz: output should not be silent",
            sr
        );
    }
}

// ============================================================================
// Compressor Plugin
// ============================================================================

#[test]
fn test_compressor_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 0.8, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Compressor@{}Hz", sr));
    }
}

// ============================================================================
// Gate Plugin
// ============================================================================

#[test]
fn test_gate_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = GatePlugin::new(2, -30.0, 10.0, 1.0, 10.0, 100.0);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Gate@{}Hz", sr));
    }
}

// ============================================================================
// Limiter Plugin
// ============================================================================

#[test]
fn test_limiter_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 2.0, NUM_FRAMES); // Hot signal
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Limiter@{}Hz", sr));
    }
}

// ============================================================================
// Expander Plugin
// ============================================================================

#[test]
fn test_expander_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = ExpanderPlugin::new(2);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 0.01, NUM_FRAMES); // Quiet signal
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Expander@{}Hz", sr));
    }
}

// ============================================================================
// Delay Plugin
// ============================================================================

#[test]
fn test_delay_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
        plugin.initialize(sr).unwrap();

        let mut buffer = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert_all_finite(&buffer, &format!("Delay@{}Hz", sr));
    }
}

// ============================================================================
// Crossover Plugin
// ============================================================================

#[test]
fn test_crossover_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        // Lowpass crossover
        let mut lp = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        lp.initialize(sr).unwrap();

        let input = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        let mut output = vec![0.0f32; input.len()];
        lp.process(&input, &mut output, &context).unwrap();
        assert_all_finite(&output, &format!("CrossoverLP@{}Hz", sr));

        // Highpass crossover
        let mut hp = CrossoverPlugin::new(2, "LR24", 1000.0, "high").unwrap();
        hp.initialize(sr).unwrap();

        let input2 = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let mut output2 = vec![0.0f32; input2.len()];
        hp.process(&input2, &mut output2, &context).unwrap();
        assert_all_finite(&output2, &format!("CrossoverHP@{}Hz", sr));
    }
}

// ============================================================================
// Matrix Plugin
// ============================================================================

#[test]
fn test_matrix_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.initialize(sr).unwrap();

        let input = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let mut output = vec![0.0_f32; NUM_FRAMES * 2];
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process(&input, &mut output, &context).unwrap();
        assert_all_finite(&output, &format!("Matrix@{}Hz", sr));
    }
}

// ============================================================================
// Plugin Chain at Various Sample Rates
// ============================================================================

#[test]
fn test_plugin_chain_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut host = PluginHost::new(2, sr);

        // Build a chain: Gain -> EQ -> Compressor -> Limiter
        let gain = GainPlugin::new(2, -3.0);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
            .unwrap();

        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            sr as f64,
            1.0,
            3.0,
        )];
        let eq = EqPlugin::new(2, filters);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(eq)))
            .unwrap();

        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(compressor)))
            .unwrap();

        let limiter = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(limiter)))
            .unwrap();

        // Use enough frames to exceed the limiter's lookahead at all sample rates.
        // At 192kHz, 5ms lookahead = 960 samples, so we need >960 frames.
        let chain_frames = 2048;
        let input = generate_sine_stereo(sr, 440.0, 0.5, chain_frames);
        let mut output = vec![0.0_f32; chain_frames * 2];

        host.process(&input, &mut output).unwrap();
        assert_all_finite(&output, &format!("Chain@{}Hz", sr));

        let output_energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(
            output_energy > 0.0,
            "Chain@{}Hz: output should not be silent",
            sr
        );
    }
}

// ============================================================================
// Dynamics Chain at Various Sample Rates
// ============================================================================

#[test]
fn test_dynamics_chain_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        let mut host = PluginHost::new(2, sr);

        // Gate -> Compressor -> Limiter chain
        let gate = GatePlugin::new(2, -50.0, 10.0, 1.0, 10.0, 100.0);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(gate)))
            .unwrap();

        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(compressor)))
            .unwrap();

        let limiter = LimiterPlugin::new(2, -0.5, 50.0, 5.0, false);
        host.add_plugin(Box::new(InPlacePluginAdapter::new(limiter)))
            .unwrap();

        let input = generate_sine_stereo(sr, 440.0, 0.8, NUM_FRAMES);
        let mut output = vec![0.0_f32; NUM_FRAMES * 2];

        host.process(&input, &mut output).unwrap();
        assert_all_finite(&output, &format!("DynamicsChain@{}Hz", sr));
    }
}

// ============================================================================
// Crossover + Delay Chain at Various Sample Rates
// ============================================================================

#[test]
fn test_crossover_delay_chain_multi_sample_rate() {
    for &sr in &SAMPLE_RATES {
        // Test crossover followed by delay at each rate
        let mut crossover = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        crossover.initialize(sr).unwrap();

        let mut delay = DelayPlugin::new(2, 50.0, 0.2, 0.5);
        delay.initialize(sr).unwrap();

        let input = generate_sine_stereo(sr, 440.0, 0.5, NUM_FRAMES);
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        let mut crossover_output = vec![0.0f32; input.len()];
        crossover.process(&input, &mut crossover_output, &context).unwrap();
        delay.process_in_place(&mut crossover_output, &context).unwrap();
        assert_all_finite(&crossover_output, &format!("CrossoverDelay@{}Hz", sr));
    }
}

// ============================================================================
// EQ at Different Frequencies Relative to Nyquist
// ============================================================================

#[test]
fn test_eq_near_nyquist() {
    for &sr in &SAMPLE_RATES {
        let nyquist = sr as f64 / 2.0;
        // Filter at 80% of Nyquist - should still work
        let freq = nyquist * 0.8;

        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            freq,
            sr as f64,
            1.0,
            3.0,
        )];

        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, filters));
        plugin.initialize(sr).unwrap();

        let input = generate_sine_stereo(sr, freq as f32, 0.3, NUM_FRAMES);
        let mut output = vec![0.0_f32; NUM_FRAMES * 2];
        let context = ProcessContext::new(sr, NUM_FRAMES,);

        plugin.process(&input, &mut output, &context).unwrap();
        assert_all_finite(&output, &format!("EQ@{}Hz_near_nyquist", sr));
    }
}

// ============================================================================
// Reinitialize at Different Sample Rate
// ============================================================================

#[test]
fn test_reinitialize_sample_rate_change() {
    let mut plugin = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);

    // Initialize at 44100, process some data
    plugin.initialize(44100).unwrap();
    let mut buffer = generate_sine_stereo(44100, 440.0, 0.5, NUM_FRAMES);
    let context = ProcessContext::new(44100, NUM_FRAMES,);
    plugin.process_in_place(&mut buffer, &context).unwrap();

    // Reinitialize at 96000, process more data
    plugin.initialize(96000).unwrap();
    let mut buffer2 = generate_sine_stereo(96000, 440.0, 0.5, NUM_FRAMES);
    let context2 = ProcessContext::new(96000, NUM_FRAMES,);
    plugin.process_in_place(&mut buffer2, &context2).unwrap();
    assert_all_finite(&buffer2, "Compressor_reinit@96kHz");
}
