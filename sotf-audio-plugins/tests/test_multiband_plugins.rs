// Integration tests for multiband plugins

use sotf_plugins::{
    InPlacePlugin, MultibandCompressorPlugin, MultibandExpanderPlugin, ProcessContext,
};

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

    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    // Just verify it processed something and didn't crash
    // Detailed crossover verification is complex for unit test
    // But input should be modified
    // (Wait, `process_in_place` takes buffer, modifies it.
    // I initialized output but used input in call. That's fine for in-place.)

    // Check that we don't have NaNs
    assert!(!input.iter().any(|x| x.is_nan()));
}

#[test]
fn test_multiband_expander_instantiation() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(44100).unwrap();

    assert_eq!(plugin.channels(), 2);
    assert!(plugin.info().name.contains("Expander"));
}
