//! Integration tests for the SOTF Convolution plugin.
//!
//! These tests exercise the public API through the `Plugin` trait (via
//! `InPlacePluginAdapter`) as a black box: instantiation, parameter get/set,
//! audio processing, error paths and state transitions.

use sotf_host::{
    ParameterId, ParameterValue, ParametricInPlacePluginAdapter, Plugin, ProcessContext,
};
use sotf_plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
use std::io::Write;

/// Write a minimal mono 16-bit PCM WAV file containing a single-sample impulse.
fn write_delta_ir(path: &std::path::Path, sample_rate: u32) -> std::io::Result<()> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * (channels as u32) * (bits_per_sample as u32) / 8;
    let block_align = channels * bits_per_sample / 8;
    let data: Vec<u8> = {
        let mut v = Vec::with_capacity(4);
        v.extend_from_slice(&0i16.to_le_bytes()); // sample 0
        v.extend_from_slice(&i16::MAX.to_le_bytes()); // impulse
        v
    };
    let data_len = data.len() as u32;
    let riff_len = 36 + data_len;

    let mut file = std::fs::File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&riff_len.to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // fmt chunk size
    file.write_all(&1u16.to_le_bytes())?; // PCM
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;
    file.write_all(&data)?;
    Ok(())
}

#[test]
fn convolution_plugin_info_and_channels() {
    let plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    assert_eq!(plugin.info().name, "Convolution");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn convolution_instantiate_from_params() {
    let params = ConvolutionPluginParams {
        ir_file: String::new(),
        mix: 0.75,
        gain_db: -3.0,
        use_nupc: false,
        zero_latency_head: false,
        head_taps: 64,
    };
    let plugin = ParametricInPlacePluginAdapter::new(
        ConvolutionPlugin::from_params(2, 44100, params).unwrap(),
    );
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn convolution_parameter_roundtrip() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();

    let params_before = plugin.parameters();
    assert!(params_before.iter().any(|p| p.id.as_str() == "mix"));
    assert!(params_before.iter().any(|p| p.id.as_str() == "gain_db"));
    assert!(params_before.iter().any(|p| p.id.as_str() == "use_nupc"));

    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.25))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-6.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("use_nupc"), ParameterValue::Bool(false))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("zero_latency_head"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("head_taps"), ParameterValue::Int(64))
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.25))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_db")),
        Some(ParameterValue::Float(-6.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_nupc")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("zero_latency_head")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_taps")),
        Some(ParameterValue::Int(64))
    );
}

#[test]
fn convolution_unknown_parameter_error() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    let err = plugin
        .set_parameter(
            ParameterId::from("does_not_exist"),
            ParameterValue::Float(1.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("does_not_exist"));
}

#[test]
fn convolution_process_without_ir_passes_input() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();

    let num_frames = 128;
    let input = vec![0.3_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);

    let frames = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(frames, num_frames);

    // With no IR loaded the plugin is a pass-through (adapter copies input).
    for (i, o) in input.iter().zip(output.iter()) {
        assert!((i - o).abs() < 1e-6, "expected pass-through without IR");
    }
}

#[test]
fn convolution_process_with_ir() {
    let tmp_dir = std::env::temp_dir().join("sotf_convolution_integration_test");
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let ir_path = tmp_dir.join("delta_ir.wav");
    write_delta_ir(&ir_path, 44100).unwrap();

    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(ir_path.to_string_lossy().to_string()),
        )
        .unwrap();

    // Process enough frames for the asynchronous IR load to finish.
    let warmup = vec![0.0_f32; 128 * 2];
    let mut warmup_out = vec![0.0_f32; 128 * 2];
    plugin
        .process(&warmup, &mut warmup_out, &ProcessContext::new(44100, 128))
        .unwrap();

    let num_frames = 2048;
    let mut input = vec![0.0_f32; num_frames * 2];
    // Place an impulse in the left channel of frame 100.
    input[100 * 2] = 1.0;
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);

    let frames = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(frames, num_frames);

    let energy: f32 = output.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "convolved output should have energy");

    // The left channel should carry some non-zero energy somewhere.
    let left_energy: f32 = output.iter().step_by(2).map(|s| s * s).sum();
    assert!(
        left_energy > 0.0,
        "left channel should have convolved energy"
    );
}

#[test]
fn convolution_ir_file_not_found_error() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();

    let err = plugin
        .set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String("/nonexistent/path/to/ir.wav".to_string()),
        )
        .unwrap_err();
    assert!(
        err.to_lowercase().contains("no such file")
            || err.to_lowercase().contains("not found")
            || err.to_lowercase().contains("io:"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn convolution_reset_clears_processing_state() {
    let tmp_dir = std::env::temp_dir().join("sotf_convolution_integration_test");
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let ir_path = tmp_dir.join("delta_ir.wav");
    write_delta_ir(&ir_path, 44100).unwrap();

    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(ir_path.to_string_lossy().to_string()),
        )
        .unwrap();

    // Let the IR load complete.
    let dummy = vec![0.0_f32; 128 * 2];
    let mut dummy_out = vec![0.0_f32; 128 * 2];
    plugin
        .process(&dummy, &mut dummy_out, &ProcessContext::new(44100, 128))
        .unwrap();

    plugin.reset();

    // After reset, processing should return to pass-through behavior.
    let num_frames = 128;
    let input = vec![0.2_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    for (i, o) in input.iter().zip(output.iter()) {
        assert!((i - o).abs() < 1e-6, "expected pass-through after reset");
    }
}

#[test]
fn convolution_mix_zero_is_dry() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    let num_frames = 128;
    let input = vec![0.4_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    for (i, o) in input.iter().zip(output.iter()) {
        assert!((i - o).abs() < 1e-6, "mix=0 should be dry pass-through");
    }
}

#[test]
fn convolution_gain_db_state_change() {
    let mut plugin = ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(2, 44100));
    plugin.initialize(44100).unwrap();

    plugin
        .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-12.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_db")),
        Some(ParameterValue::Float(-12.0))
    );

    plugin
        .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(6.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_db")),
        Some(ParameterValue::Float(6.0))
    );
}
