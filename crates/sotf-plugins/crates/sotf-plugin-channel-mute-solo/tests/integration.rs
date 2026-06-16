// ============================================================================
// Integration tests for sotf-plugin-channel-mute-solo
//
// Exercises the public InPlacePlugin API and crate-specific helpers as a
// black box.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};

const SR: u32 = 48000;
const FRAMES: usize = 256;

// ----------------------------------------------------------------------------
// Construction and metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = ChannelMuteSoloPlugin::new(2, true);
    let info = plugin.info();
    assert_eq!(info.name, "Channel Mute/Solo");
    assert_eq!(info.author, "SotF");
    assert_eq!(plugin.channels(), 2);
}

#[test]
fn from_params_pads_and_truncates_channel_states() {
    let params = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: true,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: true,
            },
        ],
        dim_gain_db: -10.0,
        fade_ms: 1.0,
    };
    let plugin = ChannelMuteSoloPlugin::from_params(2, params);
    // Two-channel plugin ignores the third state.
    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.get_channel_state(0).unwrap().muted, true);
    assert_eq!(plugin.get_channel_state(1).unwrap().soloed, true);
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_registered_params() {
    let plugin = ChannelMuteSoloPlugin::new(2, true);
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"enabled"));
    assert!(ids.contains(&"channel_states"));
    assert!(ids.contains(&"dim_gain_db"));
    assert!(ids.contains(&"fade_ms"));
}

#[test]
fn enabled_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("enabled")),
        Some(ParameterValue::Bool(false))
    );
    assert!(!plugin.is_enabled());
}

#[test]
fn dim_gain_db_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("dim_gain_db"),
            ParameterValue::Float(-12.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dim_gain_db")),
        Some(ParameterValue::Float(-12.0))
    );
    assert!((plugin.dim_gain_db() - -12.0).abs() < 1e-6);
}

#[test]
fn fade_ms_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(10.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("fade_ms")),
        Some(ParameterValue::Float(10.0))
    );
    assert!((plugin.fade_ms() - 10.0).abs() < 1e-6);
}

#[test]
fn per_channel_mute_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mute_0"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mute_0")),
        Some(ParameterValue::Bool(true))
    );
    assert!(plugin.get_channel_state(0).unwrap().muted);
}

#[test]
fn per_channel_solo_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("solo_1"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("solo_1")),
        Some(ParameterValue::Bool(true))
    );
    assert!(plugin.get_channel_state(1).unwrap().soloed);
}

#[test]
fn per_channel_dim_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("dim_0"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dim_0")),
        Some(ParameterValue::Bool(true))
    );
    assert!(plugin.get_channel_state(0).unwrap().dimmed);
}

#[test]
fn channel_states_json_roundtrip() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    let states = serde_json::to_string(&[
        ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        },
        ChannelState {
            muted: false,
            soloed: true,
            dimmed: false,
        },
    ])
    .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String(states),
        )
        .unwrap();

    let retrieved = plugin
        .get_parameter(&ParameterId::from("channel_states"))
        .unwrap()
        .as_string()
        .unwrap()
        .to_string();
    let parsed: Vec<ChannelState> = serde_json::from_str(&retrieved).unwrap();
    assert!(parsed[0].muted);
    assert!(parsed[1].soloed);
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn disabled_plugin_passthrough() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, false);
    plugin.initialize(SR).unwrap();

    let dc_l = 0.5f32;
    let dc_r = 1.0f32;
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { dc_l } else { dc_r })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // After a tiny fade-in the values should be very close to the originals.
    assert!((buffer[(FRAMES - 1) * 2] - dc_l).abs() < 1e-4);
    assert!((buffer[(FRAMES - 1) * 2 + 1] - dc_r).abs() < 1e-4);
}

#[test]
fn muted_channel_is_silenced() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mute_0"), ParameterValue::Bool(true))
        .unwrap();

    let dc = 0.5f32;
    // Let the default 5 ms fade settle. Two blocks avoids the intra-block ramp
    // error at the very end of a single large block.
    let frames = 4096;
    let mut buffer = vec![dc; frames * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();

    assert!(
        buffer[(frames - 1) * 2].abs() < 1e-5,
        "channel 0 should be muted"
    );
    assert!(
        (buffer[(frames - 1) * 2 + 1] - dc).abs() < 1e-4,
        "channel 1 should remain unmuted"
    );
}

#[test]
fn soloed_channel_mutes_others() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("solo_1"), ParameterValue::Bool(true))
        .unwrap();

    let dc_l = 0.3f32;
    let dc_r = 0.7f32;
    let frames = 4096;
    let mut buffer: Vec<f32> = (0..frames * 2)
        .map(|i| if i % 2 == 0 { dc_l } else { dc_r })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();

    assert!(
        buffer[(frames - 1) * 2].abs() < 1e-5,
        "non-soloed channel 0 should be silent"
    );
    assert!(
        (buffer[(frames - 1) * 2 + 1] - dc_r).abs() < 1e-4,
        "soloed channel 1 should pass through"
    );
}

#[test]
fn dimmed_channel_is_attenuated() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("dim_0"), ParameterValue::Bool(true))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("dim_gain_db"),
            ParameterValue::Float(-20.0),
        )
        .unwrap();

    let dc = 0.5f32;
    let frames = 4096;
    let mut buffer = vec![dc; frames * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();

    let attenuation_db = 20.0 * (buffer[(frames - 1) * 2].abs() / dc).max(1e-12).log10();
    assert!(
        (attenuation_db - -20.0).abs() < 0.5,
        "dimmed channel should be near -20 dB, got {}",
        attenuation_db
    );
    assert!(
        (buffer[(frames - 1) * 2 + 1] - dc).abs() < 1e-4,
        "channel 1 should be unaffected"
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_continues() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.5f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut buffer2 = vec![0.5f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();

    let mut buffer = vec![0.5f32; FRAMES * 2];
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(96000, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn set_enabled_toggles_processing() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    plugin.set_channel_state(0, true, false, false).unwrap();

    let frames = 4096;

    // Enabled → channel 0 muted after fade settles (two blocks).
    let mut buffer = vec![0.5f32; frames * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, frames))
        .unwrap();
    assert!(buffer[(frames - 1) * 2].abs() < 1e-5);

    // Disabled → bypass. Use a larger block so the fade back to unity reaches
    // the sample at the end of the block within the test tolerance.
    plugin.set_enabled(false);
    let bypass_frames = 16384;
    let mut buffer = vec![0.5f32; bypass_frames * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, bypass_frames))
        .unwrap();
    assert!((buffer[(bypass_frames - 1) * 2] - 0.5).abs() < 1e-4);
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Bool(true))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_per_channel_param_with_out_of_range_index_fails() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("mute_5"), ParameterValue::Bool(true))
        .unwrap_err();
    assert!(err.contains("Invalid channel index") || err.contains("mute_5"));
}

#[test]
fn set_channel_states_with_invalid_json_fails() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String("not json".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("JSON") || err.contains("expected"));
}

#[test]
fn set_dim_gain_db_with_non_float_fails() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("dim_gain_db"), ParameterValue::Int(-12))
        .unwrap_err();
    assert!(err.contains("dim_gain_db") || err.contains("type mismatch"));
}
