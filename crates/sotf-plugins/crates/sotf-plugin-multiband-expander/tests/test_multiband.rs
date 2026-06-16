// Integration tests for Multiband Expander plugin

use sotf_host::{InPlacePlugin, ParametricInPlacePluginAdapter};
use sotf_plugin_multiband_expander::MultibandExpanderPlugin;

#[test]
fn test_multiband_expander_instantiation() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(44100).unwrap();

    assert_eq!(adapter.channels(), 2);
    assert!(adapter.info().name.contains("Expander"));
}

#[test]
fn test_multiband_expander_processes_audio() {
    use sotf_host::{ParameterId, ParameterValue, ProcessContext};

    let sr = 48000u32;
    let plugin = MultibandExpanderPlugin::new(1);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(sr).unwrap();

    adapter
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
        .unwrap();
    adapter
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(4.0))
        .unwrap();

    let num_frames = sr as usize;
    let amp = 10.0f32.powf(-40.0 / 20.0);
    let mut buffer: Vec<f32> = (0..num_frames)
        .map(|i| {
            let t = i as f32 / sr as f32;
            (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amp
        })
        .collect();

    let _input_rms: f32 = (buffer.iter().map(|x| x * x).sum::<f32>() / num_frames as f32).sqrt();

    let block_size = 1024;
    for pos in (0..num_frames).step_by(block_size) {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext::new(sr, end - pos);
        adapter
            .process_in_place(&mut buffer[pos..end], &ctx)
            .unwrap();
    }

    assert!(
        !buffer.iter().any(|x| x.is_nan()),
        "Output should not contain NaNs"
    );
    assert!(
        buffer.iter().all(|x| x.is_finite()),
        "All output samples should be finite"
    );

    let output_energy: f32 = buffer[num_frames / 2..].iter().map(|x| x * x).sum();
    assert!(output_energy > 0.0, "Output should have non-zero energy");
}
