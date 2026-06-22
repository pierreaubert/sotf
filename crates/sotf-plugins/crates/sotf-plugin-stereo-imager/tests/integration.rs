// ============================================================================
// Integration tests for sotf-plugin-stereo-imager
//
// These tests exercise the crate's public API through the `InPlacePlugin`
// trait as a black box. No internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePluginAdapter;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_stereo_imager::{StereoImagerPlugin, StereoImagerPluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 512;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    let info = plugin.info();
    assert_eq!(info.name, "StereoImager");
    assert_eq!(info.author, "SotF");
    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn from_params_uses_provided_values() {
    let params = StereoImagerPluginParams {
        width: 1.5,
        low_mid_freq: 300.0,
        mid_high_freq: 3500.0,
        low_width: 0.5,
        mid_width: 1.0,
        high_width: 1.5,
        mono_bass: true,
        mix: 0.75,
    };
    let plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::from_params(2, params));
    assert_eq!(plugin.channels(), 2);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("width")),
        Some(ParameterValue::Float(1.5))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mono_bass")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.75))
    );
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_expected_controls() {
    let plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"width"));
    assert!(ids.contains(&"low_mid_freq"));
    assert!(ids.contains(&"mid_high_freq"));
    assert!(ids.contains(&"low_width"));
    assert!(ids.contains(&"mid_width"));
    assert!(ids.contains(&"high_width"));
    assert!(ids.contains(&"mono_bass"));
    assert!(ids.contains(&"mix"));
}

#[test]
fn all_parameters_roundtrip() {
    let cases: &[(&str, ParameterValue)] = &[
        ("width", ParameterValue::Float(1.5)),
        ("low_mid_freq", ParameterValue::Float(500.0)),
        ("mid_high_freq", ParameterValue::Float(3000.0)),
        ("low_width", ParameterValue::Float(0.5)),
        ("mid_width", ParameterValue::Float(1.5)),
        ("high_width", ParameterValue::Float(2.0)),
        ("mono_bass", ParameterValue::Bool(true)),
        ("mix", ParameterValue::Float(0.75)),
    ];

    for &(id, ref value) in cases {
        let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
            2,
            StereoImagerPluginParams::default(),
        ));
        plugin.initialize(SR).unwrap();
        plugin
            .set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from(id));
        assert_eq!(got, Some(value.clone()), "roundtrip failed for {}", id);
    }
}

#[test]
fn unknown_parameter_get_returns_none() {
    let plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("nonexistent")),
        None
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn initialize_then_process_works() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

#[test]
fn reset_then_process_still_works() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut buffer2: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();
    let frames = plugin
        .process_in_place(&mut buffer2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn initialize_at_multiple_sample_rates() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();
    let mut buffer = vec![0.5f32; FRAMES * 2];
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(96000, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn mix_zero_is_passthrough() {
    let params = StereoImagerPluginParams {
        mix: 0.0,
        ..StereoImagerPluginParams::default()
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(2, params));
    plugin.initialize(SR).unwrap();

    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.7)
        .collect();
    let original = buffer.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    for (i, (&orig, &out)) in original.iter().zip(buffer.iter()).enumerate() {
        assert_eq!(
            orig, out,
            "sample {i}: mix=0 changed output: expected {orig}, got {out}"
        );
    }
}

#[test]
fn width_one_is_passthrough_for_constant_signal() {
    let params = StereoImagerPluginParams {
        width: 1.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
        ..StereoImagerPluginParams::default()
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(2, params));
    plugin.initialize(SR).unwrap();

    // Long constant stereo signal to let crossover transients settle.
    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.7);
        buffer.push(0.3);
    }
    let original = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, num_frames))
        .unwrap();

    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        assert!(
            (buffer[idx] - original[idx]).abs() < 0.02,
            "frame {frame} L: expected {}, got {}",
            original[idx],
            buffer[idx]
        );
        assert!(
            (buffer[idx + 1] - original[idx + 1]).abs() < 0.02,
            "frame {frame} R: expected {}, got {}",
            original[idx + 1],
            buffer[idx + 1]
        );
    }
}

#[test]
fn width_zero_collapses_to_mono() {
    let params = StereoImagerPluginParams {
        width: 0.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
        ..StereoImagerPluginParams::default()
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(2, params));
    plugin.initialize(SR).unwrap();

    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        buffer.push((i as f32 * 0.01).sin() * 0.5);
        buffer.push((i as f32 * 0.02).cos() * 0.3);
    }

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, num_frames))
        .unwrap();

    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        let diff = (buffer[idx] - buffer[idx + 1]).abs();
        assert!(
            diff < 0.01,
            "frame {frame}: L={} R={} diff={} (expected mono)",
            buffer[idx],
            buffer[idx + 1],
            diff
        );
    }
}

#[test]
fn width_two_widens_stereo() {
    let params = StereoImagerPluginParams {
        width: 2.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
        ..StereoImagerPluginParams::default()
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(2, params));
    plugin.initialize(SR).unwrap();

    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.8);
        buffer.push(0.2);
    }

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, num_frames))
        .unwrap();

    let last = (num_frames - 1) * 2;
    let l = buffer[last];
    let r = buffer[last + 1];
    assert!(l > 0.9, "wide L should be > 0.9, got {l}");
    assert!(r < 0.1, "wide R should be < 0.1, got {r}");
}

#[test]
fn mono_bass_collapses_low_frequencies() {
    let params = StereoImagerPluginParams {
        width: 1.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: true,
        mix: 1.0,
    };
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(2, params));
    plugin.initialize(SR).unwrap();

    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.8);
        buffer.push(0.2);
    }

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, num_frames))
        .unwrap();

    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        let diff = (buffer[idx] - buffer[idx + 1]).abs();
        assert!(
            diff < 0.05,
            "frame {frame}: L={} R={} diff={} (expected mono bass)",
            buffer[idx],
            buffer[idx + 1],
            diff
        );
    }
}

#[test]
fn output_is_finite_for_sine_input() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// Error paths and edge cases visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("nonexistent"));
}

#[test]
fn set_parameter_out_of_range_fails() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();

    assert!(
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(-0.1))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(2.1))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.1))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.1))
            .is_err()
    );
}

#[test]
fn non_stereo_channels_pass_through_unchanged() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        1,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let mut buffer = vec![0.5, 0.3, 0.7, 0.1];
    let original = buffer.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, 4))
        .unwrap();
    assert_eq!(buffer, original);
}

#[test]
fn process_empty_buffer_returns_zero() {
    let mut plugin = ParametricInPlacePluginAdapter::new(StereoImagerPlugin::new(
        2,
        StereoImagerPluginParams::default(),
    ));
    plugin.initialize(SR).unwrap();
    let mut buffer = vec![];
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, 0))
        .unwrap();
    assert_eq!(frames, 0);
}
