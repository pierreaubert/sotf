// Integration tests for sotf-plugin-multiband-expander exercising the public Plugin trait.

use sotf_host::{ParametricInPlacePluginAdapter, ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_multiband_expander::{MultibandExpanderPlugin, MultibandExpanderPluginParams};

fn sine_buffer(num_frames: usize, channels: usize, freq: f32, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

fn rms(samples: &[f32]) -> f32 {
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len().max(1) as f32).sqrt()
}

fn process_blocks(
    adapter: &mut ParametricInPlacePluginAdapter<MultibandExpanderPlugin>,
    input: &[f32],
    output: &mut [f32],
    sample_rate: u32,
    channels: usize,
) {
    assert_eq!(input.len(), output.len());
    let num_frames = input.len() / channels;
    let block_size = 1024;
    let mut offset = 0;
    while offset < num_frames {
        let end = (offset + block_size).min(num_frames);
        let ctx = ProcessContext::new(sample_rate, end - offset);
        let in_chunk = &input[offset * channels..end * channels];
        let out_chunk = &mut output[offset * channels..end * channels];
        out_chunk.copy_from_slice(in_chunk);
        adapter.process(in_chunk, out_chunk, &ctx).unwrap();
        offset = end;
    }
}

#[test]
fn plugin_info_and_channels() {
    let plugin = MultibandExpanderPlugin::new(2);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    assert!(adapter.info().name.contains("Multiband Expander"));
    assert_eq!(adapter.input_channels(), 2);
    assert_eq!(adapter.output_channels(), 2);
    assert!(!adapter.parameters().is_empty());
}

#[test]
fn plugin_processes_stereo_sine() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let input = sine_buffer(2048, 2, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 2048))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
    assert!(rms(&output) > 0.0);
}

#[test]
fn parameter_roundtrip() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    adapter
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-50.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(-50.0))
    );

    adapter
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(6.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("ratio")),
        Some(ParameterValue::Float(6.0))
    );

    adapter
        .set_parameter(ParameterId::from("range"), ParameterValue::Float(30.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("range")),
        Some(ParameterValue::Float(30.0))
    );

    adapter
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.25))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.25))
    );

    adapter
        .set_parameter(ParameterId::from("processing_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("processing_mode")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn expansion_attenuates_quiet_signal() {
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        threshold_db: -40.0,
        ratio: 8.0,
        range_db: 40.0,
        attack_ms: 1.0,
        release_ms: 10.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(MultibandExpanderPlugin::with_params(2, params));
    plugin.initialize(48000).unwrap();

    let num_frames = 8192;
    let amp = 10.0f32.powf(-50.0 / 20.0);
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| {
            let frame = i / 2;
            let t = frame as f32 / 48000.0;
            (2.0 * std::f32::consts::PI * 200.0 * t).sin() * amp
        })
        .collect();
    let mut output = input.clone();

    process_blocks(&mut plugin, &input, &mut output, 48000, 2);

    let input_rms = rms(&input[input.len() / 2..]);
    let output_rms = rms(&output[output.len() / 2..]);
    assert!(
        output_rms < input_rms * 0.9,
        "Expander should attenuate a signal below the threshold (input_rms={input_rms:.4}, output_rms={output_rms:.4})"
    );
}

#[test]
fn dry_mix_passthrough() {
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        mix: 0.0,
        ..Default::default()
    };
    let plugin = MultibandExpanderPlugin::with_params(2, params);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 2048;
    let input = sine_buffer(num_frames, 2, 440.0, 48000);
    let mut output = input.clone();

    process_blocks(&mut adapter, &input, &mut output, 48000, 2);

    let max_error = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_error < 1e-5,
        "mix=0 should pass the dry signal through: max_error={}",
        max_error
    );
}

#[test]
fn spectral_mode_processes_audio() {
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        processing_mode: "spectral".to_string(),
        ..Default::default()
    };
    let plugin = MultibandExpanderPlugin::with_params(2, params);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let input = sine_buffer(4096, 2, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 4096))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn changing_num_bands_works() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    adapter
        .set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(4))
        .unwrap();

    let input = sine_buffer(2048, 2, 500.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 2048))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_then_process_is_stable() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let input = sine_buffer(1024, 2, 800.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 1024))
        .unwrap();

    adapter.reset();

    let mut output2 = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output2, &ProcessContext::new(48000, 1024))
        .unwrap();

    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn unknown_parameter_is_rejected() {
    let plugin = MultibandExpanderPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result = adapter.set_parameter(
        ParameterId::from("does_not_exist"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
}

#[test]
fn invalid_parameter_value_is_rejected() {
    let plugin = MultibandExpanderPlugin::new(2);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result =
        adapter.validate_parameter(&ParameterId::from("ratio"), &ParameterValue::Float(25.0));
    assert!(result.is_err(), "ratio must be within the documented range");
}
