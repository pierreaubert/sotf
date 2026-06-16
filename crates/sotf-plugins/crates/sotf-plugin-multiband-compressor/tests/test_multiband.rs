// Integration tests for Multiband Compressor plugin

use sotf_host::{InPlacePlugin, ParametricInPlacePluginAdapter};
use sotf_host::ProcessContext;
use sotf_plugin_multiband_compressor::MultibandCompressorPlugin;

#[test]
fn test_multiband_compressor_instantiation() {
    let plugin = MultibandCompressorPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(44100).unwrap();

    assert_eq!(adapter.channels(), 2);
    assert!(adapter.info().name.contains("Multiband"));
}

#[test]
fn test_multiband_compressor_processing() {
    let plugin = MultibandCompressorPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 2048;
    let mut input = vec![0.0; num_frames * 2];

    for i in 0..num_frames {
        let t = i as f32 / 48000.0;
        let low = (2.0 * std::f32::consts::PI * 100.0 * t).sin();
        let high = (2.0 * std::f32::consts::PI * 10000.0 * t).sin();
        input[i * 2] = (low + high) * 0.5;
        input[i * 2 + 1] = (low + high) * 0.5;
    }

    let chunk_size = 1024;
    let mut offset = 0;
    while offset < num_frames {
        let end = (offset + chunk_size).min(num_frames);
        let context = ProcessContext::new(48000, end - offset);
        adapter
            .process_in_place(&mut input[offset * 2..end * 2], &context)
            .unwrap();
        offset = end;
    }

    assert!(!input.iter().any(|x| x.is_nan()));
}

#[test]
fn test_multiband_compressor_ms_mode_roundtrip() {
    use sotf_host::{ParameterId, ParameterValue};

    let plugin = MultibandCompressorPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    adapter
        .set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();
    adapter
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(1.0))
        .unwrap();

    let block = 2400usize;
    let total = block * 2;
    let mut signal = vec![0.0f32; total * 2];
    for i in 0..total {
        let t = i as f32 / 48000.0;
        signal[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3;
        signal[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3;
    }
    let original = signal.clone();

    let context = ProcessContext::new(48000, block);
    adapter
        .process_in_place(&mut signal[..block * 2], &context)
        .unwrap();
    adapter
        .process_in_place(&mut signal[block * 2..], &context)
        .unwrap();

    assert!(!signal.iter().any(|x| x.is_nan()), "No NaNs in output");

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

    let different_frames = signal[skip..]
        .chunks(2)
        .filter(|c| (c[0] - c[1]).abs() > 0.001)
        .count();
    assert!(
        different_frames > 10,
        "L and R channels should remain different after M/S roundtrip; only {different_frames} different frames found"
    );
}
