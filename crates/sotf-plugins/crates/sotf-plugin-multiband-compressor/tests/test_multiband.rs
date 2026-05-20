// Integration tests for Multiband Compressor plugin

use sotf_host::{InPlacePlugin, ProcessContext};
use sotf_plugin_multiband_compressor::MultibandCompressorPlugin;

#[test]
fn test_multiband_compressor_instantiation() {
    // Default bands
    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(44100).unwrap();

    assert_eq!(plugin.channels(), 2);
    // Info should say Multiband
    assert!(plugin.info().name.contains("Multiband"));
}

#[test]
fn test_multiband_compressor_processing() {
    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(48000).unwrap();

    // Create signal with low freq (100Hz) and high freq (10kHz)
    let num_frames = 2048;
    let mut input = vec![0.0; num_frames * 2];

    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let low = (2.0 * std::f32::consts::PI * 100.0 * t).sin();
        let high = (2.0 * std::f32::consts::PI * 10000.0 * t).sin();
        input[i * 2] = (low + high) * 0.5;
        input[i * 2 + 1] = (low + high) * 0.5;
    }

    // Process in 1024-frame chunks to stay within max block size
    let chunk_size = 1024;
    let mut offset = 0;
    while offset < num_frames {
        let end = (offset + chunk_size).min(num_frames);
        let context = ProcessContext::new(48000, end - offset);
        plugin
            .process_in_place(&mut input[offset * 2..end * 2], &context)
            .unwrap();
        offset = end;
    }

    // Check that we don't have NaNs
    assert!(!input.iter().any(|x| x.is_nan()));
}

#[test]
fn test_multiband_compressor_ms_mode_roundtrip() {
    // M/S mode should: encode L/R → Mid/Side, compress, decode Mid/Side → L/R.
    // With no compression (ratio=1), the output should equal the input (perfect roundtrip).
    use sotf_host::{ParameterId, ParameterValue};

    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(48000).unwrap();

    // Enable M/S mode, set ratio=1 (no compression) to test encode/decode roundtrip
    plugin
        .set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(1.0))
        .unwrap();

    // Generate a stereo signal with different L/R content.
    // Stay within the 4096-frame pre-alloc limit; use two 2400-frame blocks.
    let block = 2400usize;
    let total = block * 2;
    let mut signal = vec![0.0f32; total * 2];
    for i in 0..total {
        let t = i as f32 / 48000.0;
        // Left: 440Hz, Right: 880Hz — different content per channel
        signal[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3;
        signal[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3;
    }
    let original = signal.clone();

    let context = ProcessContext::new(48000, block);

    // Process both blocks; the second block is in the settled region.
    plugin
        .process_in_place(&mut signal[..block * 2], &context)
        .unwrap();
    plugin
        .process_in_place(&mut signal[block * 2..], &context)
        .unwrap();

    // With ratio=1 (no compression), output should preserve energy and stereo image.
    // The crossover filters shift phase, so sample-level identity is not expected.
    // Instead verify: (1) no NaNs, (2) RMS is preserved within ~4dB, (3) stereo channels differ.
    assert!(!signal.iter().any(|x| x.is_nan()), "No NaNs in output");

    // Compare only the settled second block (skip first block entirely)
    let skip = block * 2;
    let orig_rms: f32 = (original[skip..].iter().map(|x| x * x).sum::<f32>()
        / (original.len() - skip) as f32)
        .sqrt();
    let out_rms: f32 =
        (signal[skip..].iter().map(|x| x * x).sum::<f32>() / (signal.len() - skip) as f32).sqrt();
    let rms_ratio_db = 20.0 * (out_rms / orig_rms).log10();

    assert!(
        rms_ratio_db.abs() < 4.0,
        "M/S mode with ratio=1 should preserve RMS within 4dB. Got {rms_ratio_db:.1}dB"
    );

    // Stereo channels should still differ sample-by-sample (M/S decode preserved separation).
    // Since L=440Hz and R=880Hz, they cannot be equal at all frames simultaneously.
    let different_frames = signal[skip..]
        .chunks(2)
        .filter(|c| (c[0] - c[1]).abs() > 0.001)
        .count();
    assert!(
        different_frames > 10,
        "L and R channels should remain different after M/S roundtrip; only {different_frames} different frames found"
    );
}
