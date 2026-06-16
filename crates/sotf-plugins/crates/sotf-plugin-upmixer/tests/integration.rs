//! Integration tests for the SOTF Stereo-to-Surround Upmixer plugin.
//!
//! Tests exercise the public `Plugin` trait: instantiation, parameter get/set,
//! audio processing, output channel configuration, bypass modes and reset.

use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};

#[test]
fn upmixer_plugin_info_and_channels() {
    let plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    assert!(plugin.info().name.contains("Upmixer"));
    assert_eq!(plugin.input_channels(), 2);
    // Default config is 5.1 => 6 channels.
    assert_eq!(plugin.output_channels(), 6);
}

#[test]
fn upmixer_instantiate_from_params_custom_config() {
    let mut params = UpmixerPluginParams::default();
    params.core.speaker_config = "7.1".to_string();
    let plugin = UpmixerPlugin::from_params(params);
    assert_eq!(plugin.output_channels(), 8);
}

#[test]
fn upmixer_parameter_roundtrip() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let params = plugin.parameters();
    assert!(params.iter().any(|p| p.id.as_str() == "gain_front_direct"));
    assert!(params.iter().any(|p| p.id.as_str() == "gain_rear_ambient"));
    assert!(params.iter().any(|p| p.id.as_str() == "lfe_gain"));

    plugin
        .set_parameter(
            ParameterId::from("gain_front_direct"),
            ParameterValue::Float(0.8),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("gain_rear_ambient"),
            ParameterValue::Float(1.25),
        )
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_front_direct")),
        Some(ParameterValue::Float(0.8))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_rear_ambient")),
        Some(ParameterValue::Float(1.25))
    );
}

#[test]
fn upmixer_unknown_parameter_error() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    let err = plugin
        .set_parameter(
            ParameterId::from("no_such_param"),
            ParameterValue::Float(1.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("no_such_param"));
}

#[test]
fn upmixer_process_silence() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * plugin.output_channels()];
    let context = ProcessContext::new(44100, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    let energy: f32 = output.iter().map(|s| s * s).sum();
    assert_eq!(energy, 0.0, "silent input should produce silent output");
}

#[test]
fn upmixer_process_stereo_to_surround() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3;
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    let total_energy: f32 = output.iter().map(|s| s * s).sum();
    assert!(total_energy > 0.0, "upmixed output should have energy");

    // Each output channel should carry some energy.
    for ch in 0..out_ch {
        let ch_energy: f32 = (0..num_frames)
            .map(|i| output[i * out_ch + ch].powi(2))
            .sum();
        assert!(ch_energy > 0.0, "channel {} should have energy", ch);
    }
}

#[test]
fn upmixer_bypass_all_processing_passes_stereo() {
    let mut params = UpmixerPluginParams::default();
    params.bypass.bypass_all_processing = true;
    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(44100).unwrap();

    let num_frames = 512;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = 0.4;
        input[i * 2 + 1] = -0.4;
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    for i in 0..num_frames {
        assert!((output[i * out_ch] - 0.4).abs() < 1e-5);
        assert!((output[i * out_ch + 1] - (-0.4)).abs() < 1e-5);
        for ch in 2..out_ch {
            assert_eq!(output[i * out_ch + ch], 0.0);
        }
    }
}

#[test]
fn upmixer_state_change_low_latency_fft() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let latency_before = plugin.latency_samples();
    plugin
        .set_parameter(ParameterId::from("low_latency"), ParameterValue::Bool(true))
        .unwrap();
    let latency_after = plugin.latency_samples();

    assert!(
        latency_after < latency_before,
        "low-latency mode should reduce reported latency"
    );
}

#[test]
fn upmixer_reset_clears_state() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let latency = plugin.latency_samples();
    let num_frames = latency + 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 0.5;
    }

    let out_ch = plugin.output_channels();
    let mut output1 = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output1, &context).unwrap();

    // Output after the initial latency should be non-silent.
    let energy1: f32 = output1[latency * out_ch..].iter().map(|s| s * s).sum();
    assert!(
        energy1 > 0.0,
        "first process should produce non-silent output"
    );

    plugin.reset();

    let mut output2 = vec![0.0_f32; num_frames * out_ch];
    plugin.process(&input, &mut output2, &context).unwrap();

    // After reset, the plugin should still recover and produce non-silent output.
    let energy2: f32 = output2[latency * out_ch..].iter().map(|s| s * s).sum();
    assert!(
        energy2 > 0.0,
        "output after reset should still have energy beyond latency"
    );
}

#[test]
fn upmixer_invalid_sample_rate_error() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    let err = plugin.initialize(0).unwrap_err();
    assert!(err.contains("Invalid sample rate"));
}

#[test]
fn upmixer_params_serde_flatten_roundtrip() {
    let original = UpmixerPluginParams::default();
    let json = serde_json::to_value(&original).unwrap();
    // Flat serialization: no nested "core"/"gains" objects.
    assert!(json.get("fft_size").is_some());
    assert!(json.get("core").is_none());

    // Empty JSON and flat keys deserialize correctly.
    let from_empty: UpmixerPluginParams = serde_json::from_str("{}").unwrap();
    assert_eq!(from_empty.core.fft_size, original.core.fft_size);

    let from_flat: UpmixerPluginParams =
        serde_json::from_str(r#"{"fft_size":1024,"speaker_config":"7.1","height_gain":0.8}"#)
            .unwrap();
    assert_eq!(from_flat.core.fft_size, 1024);
    assert_eq!(from_flat.core.speaker_config, "7.1");
    assert_eq!(from_flat.height.height_gain, 0.8);
}
