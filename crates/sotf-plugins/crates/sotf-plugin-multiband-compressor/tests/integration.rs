// Integration tests for sotf-plugin-multiband-compressor exercising the public Plugin trait.

use sotf_host::{
    ParameterId, ParameterValue, ParametricInPlacePluginAdapter, Plugin, ProcessContext,
};
use sotf_plugin_multiband_compressor::{
    MultibandCompressorPlugin, MultibandCompressorPluginParams,
};

fn sine_buffer(num_frames: usize, channels: usize, freq: f32, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.9;
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
    adapter: &mut ParametricInPlacePluginAdapter<MultibandCompressorPlugin>,
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
    let plugin = MultibandCompressorPlugin::new(2);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    assert!(adapter.info().name.contains("Multiband Compressor"));
    assert_eq!(adapter.input_channels(), 2);
    assert_eq!(adapter.output_channels(), 2);
    assert!(!adapter.parameters().is_empty());
}

#[test]
fn plugin_processes_stereo_sine() {
    let plugin = MultibandCompressorPlugin::new(2);
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
    let plugin = MultibandCompressorPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    adapter
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(-30.0))
    );

    adapter
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(8.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("ratio")),
        Some(ParameterValue::Float(8.0))
    );

    adapter
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.5))
    );

    adapter
        .set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("ms_mode")),
        Some(ParameterValue::Bool(true))
    );

    adapter
        .set_parameter(
            ParameterId::from("band_0_active"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("band_0_active")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn compression_reduces_level() {
    let params = MultibandCompressorPluginParams {
        num_bands: 2,
        threshold_db: -25.0,
        ratio: 20.0,
        attack_ms: 1.0,
        release_ms: 10.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin =
        ParametricInPlacePluginAdapter::new(MultibandCompressorPlugin::with_params(2, params));
    plugin.initialize(48000).unwrap();

    let num_frames = 8192;
    let input = sine_buffer(num_frames, 2, 200.0, 48000);
    let mut output = input.clone();

    process_blocks(&mut plugin, &input, &mut output, 48000, 2);

    let input_rms = rms(&input[input.len() / 2..]);
    let output_rms = rms(&output[output.len() / 2..]);
    println!(
        "compression: input_rms={:.4}, output_rms={:.4}",
        input_rms, output_rms
    );

    assert!(
        output_rms < input_rms * 0.8,
        "Heavy compression should reduce the output level (input_rms={input_rms:.4}, output_rms={output_rms:.4})"
    );
}

#[test]
fn dry_mix_passthrough() {
    let params = MultibandCompressorPluginParams {
        num_bands: 2,
        mix: 0.0,
        ..Default::default()
    };
    let plugin = MultibandCompressorPlugin::with_params(2, params);
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
fn changing_num_bands_works() {
    let plugin = MultibandCompressorPlugin::new(2);
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
    let plugin = MultibandCompressorPlugin::new(2);
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
    let plugin = MultibandCompressorPlugin::new(2);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result = adapter.set_parameter(
        ParameterId::from("not_a_real_param"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
}

#[test]
fn invalid_parameter_value_is_rejected() {
    let plugin = MultibandCompressorPlugin::new(2);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result =
        adapter.validate_parameter(&ParameterId::from("ratio"), &ParameterValue::Float(25.0));
    assert!(result.is_err(), "ratio must be within the documented range");
}
