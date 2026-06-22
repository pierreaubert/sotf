// ============================================================================
// Integration tests for sotf-plugin-fir-designer
//
// These tests exercise the crate's public API as a black box through the
// InPlacePlugin trait (and the Plugin adapter) with realistic end-to-end
// workflows.
// ============================================================================

use sotf_host::ParametricInPlacePluginAdapter;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_fir_designer::{BandConfig, FirDesignerPlugin, FirDesignerPluginParams};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 64;

// ----------------------------------------------------------------------------
// Instantiation and metadata
// ----------------------------------------------------------------------------

#[test]
fn info_returns_expected_metadata() {
    let plugin = FirDesignerPlugin::new(2, SAMPLE_RATE);
    let info = plugin.info();
    assert_eq!(info.name, "FIR Designer");
    assert_eq!(info.author, "SOTF");
    assert!(info.description.contains("FIR"));
}

#[test]
fn channels_matches_constructor() {
    let plugin = FirDesignerPlugin::new(2, SAMPLE_RATE);
    assert_eq!(plugin.channels(), 2);

    let plugin = FirDesignerPlugin::new(1, SAMPLE_RATE);
    assert_eq!(plugin.channels(), 1);
}

#[test]
fn parameters_include_global_and_band_params() {
    let params = FirDesignerPluginParams {
        num_filters: 2,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 0.0,
            active: true,
        }],
    };
    let plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();

    let ids: Vec<String> = plugin.parametric_parameters().iter().map(|p| p.id.to_string()).collect();

    assert!(ids.contains(&"num_filters".to_string()));
    assert!(ids.contains(&"fir_length".to_string()));
    assert!(ids.contains(&"phase_mode".to_string()));
    assert!(ids.contains(&"auto_gain".to_string()));
    assert!(ids.contains(&"mix".to_string()));
    assert!(ids.contains(&"band_0_type".to_string()));
    assert!(ids.contains(&"band_0_freq".to_string()));
    assert!(ids.contains(&"band_0_q".to_string()));
    assert!(ids.contains(&"band_0_gain".to_string()));
    assert!(ids.contains(&"band_0_active".to_string()));
}

// ----------------------------------------------------------------------------
// Happy-path processing
// ----------------------------------------------------------------------------

#[test]
fn default_plugin_processes_finite_output() {
    let mut plugin = FirDesignerPlugin::new(2, SAMPLE_RATE);
    let mut buffer = vec![0.25f32; FRAMES * 2];

    let processed = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    assert_eq!(processed, FRAMES);
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn from_params_with_active_peak_processes_finite_output() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };

    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();
    let mut buffer = vec![0.1f32; FRAMES];

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn dry_mix_is_passthrough() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 0.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 12.0,
            active: true,
        }],
    };

    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();
    let input: Vec<f32> = (0..FRAMES).map(|i| ((i % 11) as f32 - 5.0) / 6.0).collect();
    let mut output = input.clone();

    plugin
        .process_in_place(&mut output, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    let max_error: f32 = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-4,
        "mix=0 should be dry passthrough: max_error={}",
        max_error
    );
}

#[test]
fn plugin_adapter_exposes_plugin_trait() {
    let mut plugin = ParametricInPlacePluginAdapter::new(FirDesignerPlugin::new(1, SAMPLE_RATE));
    plugin.initialize(SAMPLE_RATE).unwrap();

    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 1);

    let input = vec![0.2f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// Parameter roundtrips and state transitions
// ----------------------------------------------------------------------------

#[test]
fn parameter_roundtrip_global_params() {
    let mut plugin = FirDesignerPlugin::new(1, SAMPLE_RATE);

    plugin
        .parametric_set_parameter(ParameterId::from("num_filters"), ParameterValue::Int(3))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("num_filters")),
        Some(ParameterValue::Int(3))
    );

    plugin
        .parametric_set_parameter(ParameterId::from("fir_length"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("fir_length")),
        Some(ParameterValue::Int(2))
    );

    plugin
        .parametric_set_parameter(ParameterId::from("phase_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("phase_mode")),
        Some(ParameterValue::Int(1))
    );

    plugin
        .parametric_set_parameter(ParameterId::from("auto_gain"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("auto_gain")),
        Some(ParameterValue::Bool(true))
    );

    plugin
        .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    let got = plugin.parametric_get_parameter(&ParameterId::from("mix"));
    assert!(
        matches!(got, Some(ParameterValue::Float(v)) if (v - 0.5).abs() < 0.001),
        "mix round-trip drift: {:?}",
        got
    );
}

#[test]
fn parameter_roundtrip_band_params() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 0.0,
            active: true,
        }],
    };
    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();

    plugin
        .parametric_set_parameter(
            ParameterId::from("band_0_freq"),
            ParameterValue::Float(500.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_freq")),
        Some(ParameterValue::Float(500.0))
    );

    plugin
        .parametric_set_parameter(ParameterId::from("band_0_q"), ParameterValue::Float(2.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_q")),
        Some(ParameterValue::Float(2.0))
    );

    plugin
        .parametric_set_parameter(
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_gain")),
        Some(ParameterValue::Float(-3.0))
    );

    plugin
        .parametric_set_parameter(
            ParameterId::from("band_0_active"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_active")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn phase_mode_transition_changes_latency() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0, // 1024 taps
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };
    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();
    assert_eq!(plugin.latency_samples(), (1024 - 1) / 2);

    plugin
        .parametric_set_parameter(ParameterId::from("phase_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn reset_returns_deterministic_state() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };
    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();

    let input: Vec<f32> = (0..FRAMES).map(|i| ((i % 7) as f32) / 7.0).collect();

    let mut run1 = input.clone();
    plugin
        .process_in_place(&mut run1, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    plugin.reset();

    let mut run2 = input.clone();
    plugin
        .process_in_place(&mut run2, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    let max_error: f32 = run1
        .iter()
        .zip(run2.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "reset should restore deterministic state: max_error={}",
        max_error
    );
}

// ----------------------------------------------------------------------------
// Error paths and edge cases
// ----------------------------------------------------------------------------

#[test]
fn unknown_band_parameter_errors() {
    let mut plugin = FirDesignerPlugin::new(1, SAMPLE_RATE);
    let result = plugin.parametric_set_parameter(
        ParameterId::from("band_0_not_real"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err(), "unknown band parameter should error");
    assert!(result.unwrap_err().contains("Unknown parameter"));
}

#[test]
fn large_block_is_chunked_without_error() {
    let params = FirDesignerPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };
    let mut plugin = FirDesignerPlugin::from_params(1, SAMPLE_RATE, params).unwrap();

    // fir_length=1024, fft_size=2048, max_chunk_frames = 2048 - 1023 = 1025.
    // Process 4096 frames to force internal chunking.
    let large_frames = 4096;
    let mut buffer = vec![0.1f32; large_frames];

    let processed = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, large_frames))
        .unwrap();

    assert_eq!(processed, large_frames);
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn zero_frames_returns_zero() {
    let mut plugin = FirDesignerPlugin::new(1, SAMPLE_RATE);
    let mut buffer = vec![0.0f32; 0];
    let processed = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, 0))
        .unwrap();
    assert_eq!(processed, 0);
}
