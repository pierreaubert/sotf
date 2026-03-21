// Integration tests for Multiband Expander plugin

use sotf_host::InPlacePlugin;
use sotf_plugin_multiband_expander::MultibandExpanderPlugin;

#[test]
fn test_multiband_expander_instantiation() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(44100).unwrap();

    assert_eq!(plugin.channels(), 2);
    assert!(plugin.info().name.contains("Expander"));
}

#[test]
fn test_multiband_expander_processes_audio() {
    // A quiet signal (-40dB) below threshold (-20dB) should be
    // expanded (attenuated further) by the multiband expander.
    use sotf_host::{ParameterId, ParameterValue, ProcessContext};

    let sr = 48000u32;
    let mut plugin = MultibandExpanderPlugin::new(1);
    plugin.initialize(sr).unwrap();

    // Set explicit threshold and ratio for clear expansion
    plugin
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(4.0))
        .unwrap();

    // Generate 1 second of quiet DC-ish signal at ~-40dB
    let num_frames = sr as usize;
    let amp = 10.0f32.powf(-40.0 / 20.0); // -40dB
    let mut buffer: Vec<f32> = (0..num_frames)
        .map(|i| {
            let t = i as f32 / sr as f32;
            (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amp
        })
        .collect();

    let input_rms: f32 = (buffer.iter().map(|x| x * x).sum::<f32>() / num_frames as f32).sqrt();

    // Process in blocks
    let block_size = 1024;
    for pos in (0..num_frames).step_by(block_size) {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: end - pos,
        };
        plugin.process_in_place(&mut buffer[pos..end], &ctx).unwrap();
    }

    // Output should be quieter than input (expansion attenuates below threshold)
    let skip = num_frames / 2; // skip first half for settling
    let output_rms: f32 = (buffer[skip..].iter().map(|x| x * x).sum::<f32>()
        / (num_frames - skip) as f32)
        .sqrt();

    assert!(
        output_rms < input_rms,
        "Expander should attenuate quiet signals. Input RMS: {input_rms:.6}, Output RMS: {output_rms:.6}"
    );
    assert!(
        !buffer.iter().any(|x| x.is_nan()),
        "Output should not contain NaNs"
    );
}
