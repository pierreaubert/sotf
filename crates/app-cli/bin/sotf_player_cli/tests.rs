use super::build::rack_eq_settings;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use super::build::downmix_settings;
use super::types::DownmixArgs;
use sotf_audio::plugins::PluginSettings;
use super::parse::parse_channel_mapping;
use super::parse::parse_loudness_compensation;
use super::types::Cli;

#[test]
fn cli_definition_has_unique_argument_ids() {
    use clap::CommandFactory;

    Cli::command().debug_assert();
}

// -- parse_channel_mapping ------------------------------------------------

#[test]
fn parse_channel_mapping_rejects_zero_input_channel() {
    // Regression: previously `ch - 1` underflowed to usize::MAX for ch == 0.
    let err = parse_channel_mapping("0,1->1,2").expect_err("0-indexed input must fail");
    assert!(
        err.contains("Input channel index must be >= 1"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_channel_mapping_rejects_zero_output_channel() {
    let err = parse_channel_mapping("1,2->0,1").expect_err("0-indexed output must fail");
    assert!(err.contains(">= 1"), "unexpected error: {err}");
}

#[test]
fn parse_channel_mapping_happy_path_is_one_indexed() {
    let (inp, out, matrix) = parse_channel_mapping("1,2->9,10").expect("valid mapping");
    assert_eq!(inp, vec![0, 1]);
    assert_eq!(out, vec![8, 9]);
    assert_eq!(matrix, vec![1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn parse_channel_mapping_supports_gap_underscore() {
    let (inp, out, _matrix) = parse_channel_mapping("1,2->_,9,10").expect("gap mapping");
    assert_eq!(inp, vec![0, 1]);
    assert_eq!(out, vec![8, 9]);
}

#[test]
fn parse_channel_mapping_rejects_unparseable_input() {
    assert!(parse_channel_mapping("x,1->1,2").is_err());
    assert!(parse_channel_mapping("1,2").is_err());
}

// -- parse_loudness_compensation -----------------------------------------

#[test]
fn parse_loudness_compensation_accepts_two_values() {
    let res = parse_loudness_compensation(&[70.0, 3.0]).expect("2 values valid");
    assert!(res.is_some());
}

#[test]
fn parse_loudness_compensation_accepts_three_values() {
    let res = parse_loudness_compensation(&[70.0, 3.0, 4.0]).expect("3 values valid");
    assert!(res.is_some());
}

#[test]
fn parse_loudness_compensation_rejects_wrong_arity() {
    assert!(parse_loudness_compensation(&[]).is_err());
    assert!(parse_loudness_compensation(&[70.0]).is_err());
    assert!(parse_loudness_compensation(&[70.0, 3.0, 4.0, 5.0]).is_err());
}

#[test]
fn downmix_builder_preserves_explicit_layout_and_defaults_to_unspecified() {
    let args = DownmixArgs {
        enabled: true,
        input_layout: Some("7.1".into()),
        center_gain_db: -3.0,
        surround_gain_db: -3.0,
        height_gain_db: -6.0,
        lfe_gain_db: -10.0,
        phase_coherence: false,
        phase_blend_low_hz: 500.0,
        phase_blend_high_hz: 2_000.0,
        itu_mode: false,
    };

    assert!(matches!(
        downmix_settings(8, &args),
        PluginSettings::Downmix {
            input_channels: 8,
            input_layout: Some(ref layout),
            ..
        } if layout == "7.1"
    ));

    let mut default_args = args;
    default_args.input_layout = None;
    assert!(matches!(
        downmix_settings(6, &default_args),
        PluginSettings::Downmix {
            input_channels: 6,
            input_layout: None,
            ..
        }
    ));
}


#[test]
fn rack_eq_builder_uses_canonical_global_control_defaults() {
    let filter = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 1.2, -3.0);
    let settings = rack_eq_settings(6, &[filter]);

    let PluginSettings::EQ {
        channels,
        filters,
        auto_gain_enabled,
        oversampling,
        ..
    } = settings
    else {
        panic!("rack EQ builder must produce EQ settings");
    };
    assert_eq!(channels, 6);
    assert_eq!(filters.len(), 1);
    assert!(!auto_gain_enabled, "CLI has no EQ Auto Gain option");
    assert_eq!(oversampling, 1.0, "CLI has no EQ oversampling option");
}
