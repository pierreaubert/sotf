#![allow(dead_code)]
use super::eq_band::EqBand;
use super::linear_phase_eq_plugin::LinearPhaseEqPlugin;
use super::misc::{
    filter_type_to_index, fir_length_from_index, index_to_filter_type, parse_filter_type,
};
use super::types::BandConfig;
use super::types::LinearPhaseEqPluginParams;
use math_audio_iir_fir::BiquadFilterType;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

mod misc;

#[cfg(test)]
const DEFAULT_SAMPLE_RATE: u32 = 48000;

#[test]
fn test_large_block_is_chunked_not_silently_bypassed() {
    let channels = 1;
    let sr = 48000;
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 1,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 12.0,
            active: true,
        }],
    };
    let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
    let num_frames = plugin.fft_size + 512;
    let mut buffer = vec![0.0_f32; num_frames];
    for (i, sample) in buffer.iter_mut().enumerate() {
        let t = i as f32 / sr as f32;
        *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.25;
    }
    let input = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let changed = buffer
        .iter()
        .zip(input.iter())
        .any(|(&out, &inp)| (out - inp).abs() > 1.0e-5);
    assert!(
        changed,
        "large blocks must be processed, not passed through"
    );
}

#[test]
fn test_large_block_ola_matches_small_chunks() {
    let channels = 1;
    let sr = 48000;
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0, // 1024 taps, fft_size 2048, max valid block 1025
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 0.7,
            gain_db: 9.0,
            active: true,
        }],
    };
    let mut one_block = LinearPhaseEqPlugin::from_params(channels, sr, params.clone()).unwrap();
    let mut chunked = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();

    let num_frames = 1500;
    assert!(num_frames < one_block.fft_size);
    assert!(num_frames > one_block.fft_size - (one_block.fir_length() - 1));

    let mut input = vec![0.0f32; num_frames];
    for (i, sample) in input.iter_mut().enumerate() {
        let t = i as f32 / sr as f32;
        *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2
            + (2.0 * std::f32::consts::PI * 3000.0 * t).sin() * 0.1;
    }

    let mut large_buffer = input.clone();
    one_block
        .process_in_place(&mut large_buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let mut small_buffer = input.clone();
    for chunk in small_buffer.chunks_mut(512) {
        chunked
            .process_in_place(chunk, &ProcessContext::new(sr, chunk.len()))
            .unwrap();
    }

    let max_diff = large_buffer
        .iter()
        .zip(small_buffer.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff < 1.0e-4,
        "large-block OLA output should match explicit small chunks; max diff {max_diff}"
    );
}

#[test]
fn test_overlap_buffers_match_fir_tail_length() {
    let channels = 3;
    let sr = 48000;
    let mut plugin = LinearPhaseEqPlugin::new(channels, sr);

    let expected_tail = plugin.fir_length() - 1;
    assert!(expected_tail < plugin.fft_size);
    for overlap in &plugin.overlap {
        assert_eq!(overlap.len(), expected_tail);
    }

    plugin
        .set_parameter(ParameterId::from("fir_length"), ParameterValue::Int(3))
        .unwrap();
    let expected_tail = plugin.fir_length() - 1;
    assert!(expected_tail < plugin.fft_size);
    for overlap in &plugin.overlap {
        assert_eq!(overlap.len(), expected_tail);
    }
}

#[test]
fn test_rebuild_fir_reuses_design_scratch_vectors() {
    let channels = 1;
    let sr = 48000;
    let mut plugin = LinearPhaseEqPlugin::new(channels, sr);

    let initial_freq_capacity = plugin.design_freqs.capacity();
    let initial_mag_capacity = plugin.design_magnitudes_db.capacity();
    assert!(initial_freq_capacity >= plugin.design_freqs.len());
    assert!(initial_mag_capacity >= plugin.design_magnitudes_db.len());

    plugin
        .set_parameter(ParameterId::from("band_0_gain"), ParameterValue::Float(6.0))
        .unwrap();
    plugin.rebuild_fir();

    assert_eq!(plugin.design_freqs.capacity(), initial_freq_capacity);
    assert_eq!(plugin.design_magnitudes_db.capacity(), initial_mag_capacity);
}

#[test]
fn test_linear_phase_eq_latency() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let fir_len = plugin.fir_length();
    assert_eq!(plugin.latency_samples(), (fir_len - 1) / 2);
}

#[test]
fn test_parameter_roundtrip() {
    let mut plugin = LinearPhaseEqPlugin::new(2, 48000);

    // Set mix
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("mix"));
    match val {
        Some(ParameterValue::Float(v)) => assert!((v - 0.5).abs() < 0.01),
        other => panic!("Expected Float(0.5), got {other:?}"),
    }

    // Set band frequency
    plugin
        .set_parameter(
            ParameterId::from("band_0_freq"),
            ParameterValue::Float(2000.0),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("band_0_freq"));
    match val {
        Some(ParameterValue::Float(v)) => assert!((v - 2000.0).abs() < 1.0),
        other => panic!("Expected Float(2000.0), got {other:?}"),
    }
}

#[test]
fn test_dc_gain_not_hardcoded() {
    // CRITICAL: DC magnitude was hardcoded to 0 dB regardless of filter shape.
    // A lowshelf cut should produce a FIR with attenuated DC gain.
    let channels = 1;
    let sr = 48000;
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 2, // 4096 taps
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Lowshelf".to_string(),
            frequency: 200.0,
            q: 0.7,
            gain_db: -12.0,
            active: true,
        }],
    };

    let plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
    let dc_gain_linear: f32 = plugin.fir_coeffs.iter().sum();
    let dc_gain_db = 20.0 * dc_gain_linear.abs().max(1e-12).log10();
    // With the bug DC was forced to 0 dB, so sum ≈ 1.0 (0 dB).
    // After the fix the FIR should reflect the shelf cut.
    assert!(
        dc_gain_db < -6.0,
        "Expected DC gain significantly below 0 dB for a lowshelf cut, got {dc_gain_db:.2} dB"
    );
}

#[test]
fn test_parse_filter_type_cases() {
    assert_eq!(parse_filter_type("Peak").unwrap(), BiquadFilterType::Peak);
    assert_eq!(parse_filter_type("peak").unwrap(), BiquadFilterType::Peak);
    assert_eq!(
        parse_filter_type("Lowshelf").unwrap(),
        BiquadFilterType::Lowshelf
    );
    assert_eq!(
        parse_filter_type("lowshelf").unwrap(),
        BiquadFilterType::Lowshelf
    );
    assert_eq!(
        parse_filter_type("Highshelf").unwrap(),
        BiquadFilterType::Highshelf
    );
    assert_eq!(
        parse_filter_type("Lowpass").unwrap(),
        BiquadFilterType::Lowpass
    );
    assert_eq!(
        parse_filter_type("highpass").unwrap(),
        BiquadFilterType::Highpass
    );
    assert!(parse_filter_type("Notch").is_err());
    assert!(parse_filter_type("").is_err());
}

#[test]
fn test_filter_type_index_roundtrip() {
    let types = [
        BiquadFilterType::Peak,
        BiquadFilterType::Lowshelf,
        BiquadFilterType::Highshelf,
        BiquadFilterType::Lowpass,
        BiquadFilterType::Highpass,
    ];
    for (i, ft) in types.iter().enumerate() {
        assert_eq!(filter_type_to_index(*ft), i);
        assert_eq!(index_to_filter_type(i), *ft);
    }
    assert_eq!(index_to_filter_type(99), BiquadFilterType::Peak);
}

#[test]
fn test_fir_length_from_index_bounds() {
    assert_eq!(fir_length_from_index(0), 1024);
    assert_eq!(fir_length_from_index(1), 2048);
    assert_eq!(fir_length_from_index(2), 4096);
    assert_eq!(fir_length_from_index(3), 8192);
    assert_eq!(fir_length_from_index(99), 2048);
}

#[test]
fn test_set_parameter_num_filters_grows_bands() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    let original = plugin.num_filters;
    plugin
        .set_parameter(ParameterId::from("num_filters"), ParameterValue::Int(8))
        .unwrap();
    assert_eq!(plugin.num_filters, 8);
    assert!(plugin.bands.len() >= 8);
    plugin
        .set_parameter(
            ParameterId::from("num_filters"),
            ParameterValue::Int(original as i32),
        )
        .unwrap();
    assert_eq!(plugin.num_filters, original);
}

#[test]
fn test_set_parameter_fir_length_resizes() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    let original_fft = plugin.fft_size;
    plugin
        .set_parameter(ParameterId::from("fir_length"), ParameterValue::Int(3))
        .unwrap();
    assert_eq!(plugin.fir_length_index, 3);
    assert_eq!(plugin.fir_length(), 8192);
    assert!(plugin.fft_size > original_fft);
    assert_eq!(plugin.overlap[0].len(), 8191);
}

#[test]
fn test_set_parameter_auto_gain() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    assert!(!plugin.auto_gain);
    plugin
        .set_parameter(ParameterId::from("auto_gain"), ParameterValue::Bool(true))
        .unwrap();
    assert!(plugin.auto_gain);
}

#[test]
fn test_get_parameter_unknown_returns_none() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    assert!(
        plugin
            .get_parameter(&ParameterId::from("unknown"))
            .is_none()
    );
    assert!(
        plugin
            .get_parameter(&ParameterId::from("band_99_gain"))
            .is_none()
    );
}

#[test]
fn test_reset_clears_overlap() {
    let mut plugin = LinearPhaseEqPlugin::new(2, 48000);
    for ch in 0..plugin.channels {
        for sample in &mut plugin.overlap[ch] {
            *sample = 1.0;
        }
    }
    plugin.reset();
    for ch in 0..plugin.channels {
        assert!(plugin.overlap[ch].iter().all(|&s| s == 0.0));
    }
}

#[test]
fn test_process_zero_frames() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    let mut buffer = [0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 0);
}

#[test]
fn test_process_zero_channels() {
    let mut plugin = LinearPhaseEqPlugin::new(0, 48000);
    let mut buffer = [0.0f32; 64];
    let ctx = ProcessContext::new(48000, 64);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 64);
}

#[test]
fn test_initialize_changes_sample_rate() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 44100);
    assert_eq!(plugin.sample_rate, 44100);
    plugin.initialize(48000).unwrap();
    assert_eq!(plugin.sample_rate, 48000);
}

#[test]
fn test_from_params_fills_missing_bands() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 3,
        fir_length_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 500.0,
            q: 1.0,
            gain_db: 3.0,
            active: true,
        }],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
    assert_eq!(plugin.bands.len(), 3);
    assert_eq!(plugin.bands[0].frequency, 500.0);
    assert_eq!(plugin.bands[1].frequency, 1000.0);
}

#[test]
fn test_info_and_channels() {
    let plugin = LinearPhaseEqPlugin::new(4, 48000);
    assert_eq!(plugin.channels(), 4);
    let info = plugin.info();
    assert_eq!(info.name, "Linear-Phase EQ");
}

#[test]
fn test_parameters_reflect_state() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let before = plugin.parameters().len();
    assert!(before >= 4);
    assert!(
        plugin
            .parameters()
            .iter()
            .any(|p| p.id == ParameterId::from("band_0_gain"))
    );
}

#[test]
fn test_band_contribution_db_skips_inactive() {
    let sr = 48000.0;
    let active_band = EqBand::new(BiquadFilterType::Peak, 1000.0, 1.0, 6.0, true, sr);
    let inactive_band = EqBand::new(BiquadFilterType::Peak, 1000.0, 1.0, 6.0, false, sr);
    let with_active = LinearPhaseEqPlugin::band_contribution_db(&[active_band], 1000.0);
    let with_inactive = LinearPhaseEqPlugin::band_contribution_db(&[inactive_band], 1000.0);
    assert!(with_active > 1.0);
    assert!(with_inactive.abs() < 0.01);
}

#[test]
fn test_set_parameter_band_type() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin
        .set_parameter(ParameterId::from("band_0_type"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(plugin.bands[0].filter_type, BiquadFilterType::Lowshelf);
    assert!(plugin.fir_dirty);
    let val = plugin.get_parameter(&ParameterId::from("band_0_type"));
    assert_eq!(val, Some(ParameterValue::Int(1)));
}

#[test]
fn test_set_parameter_band_q() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin
        .set_parameter(ParameterId::from("band_0_q"), ParameterValue::Float(2.0))
        .unwrap();
    assert!((plugin.bands[0].q - 2.0).abs() < 1e-6);
    assert!(plugin.fir_dirty);
    let val = plugin.get_parameter(&ParameterId::from("band_0_q"));
    assert_eq!(val, Some(ParameterValue::Float(2.0)));
}

#[test]
fn test_set_parameter_band_gain() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin
        .set_parameter(ParameterId::from("band_0_gain"), ParameterValue::Float(6.0))
        .unwrap();
    assert!((plugin.bands[0].gain_db - 6.0).abs() < 1e-6);
    assert!(plugin.fir_dirty);
    let val = plugin.get_parameter(&ParameterId::from("band_0_gain"));
    assert_eq!(val, Some(ParameterValue::Float(6.0)));
}

#[test]
fn test_set_parameter_band_active() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin
        .set_parameter(
            ParameterId::from("band_0_active"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert!(!plugin.bands[0].active);
    assert!(plugin.fir_dirty);
    let val = plugin.get_parameter(&ParameterId::from("band_0_active"));
    assert_eq!(val, Some(ParameterValue::Bool(false)));
}

#[test]
fn test_set_parameter_num_filters_noop() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin.fir_dirty = false;
    let original = plugin.num_filters;
    plugin
        .set_parameter(
            ParameterId::from("num_filters"),
            ParameterValue::Int(original as i32),
        )
        .unwrap();
    assert!(!plugin.fir_dirty, "same num_filters should not mark dirty");
}

#[test]
fn test_set_parameter_fir_length_noop() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin.fir_dirty = false;
    let original = plugin.fir_length_index as i32;
    plugin
        .set_parameter(
            ParameterId::from("fir_length"),
            ParameterValue::Int(original),
        )
        .unwrap();
    assert!(!plugin.fir_dirty, "same fir_length should not mark dirty");
}

#[test]
fn test_set_parameter_mix() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert!((plugin.mix_value - 0.75).abs() < 1e-6);
    let val = plugin.get_parameter(&ParameterId::from("mix"));
    assert_eq!(val, Some(ParameterValue::Float(0.75)));
}

#[test]
fn test_set_parameter_unknown_band_param_returns_error() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    let result = plugin.set_parameter(
        ParameterId::from("band_0_unknown"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown parameter"));
}

#[test]
fn test_process_with_mix() {
    let channels = 1;
    let sr = 48000;
    let mut plugin = LinearPhaseEqPlugin::from_params(
        channels,
        sr,
        LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            auto_gain: false,
            mix: 0.5,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 12.0,
                active: true,
            }],
        },
    )
    .unwrap();
    let num_frames = 1024;
    let mut buffer = vec![0.0f32; num_frames];
    for (i, sample) in buffer.iter_mut().enumerate() {
        let t = i as f32 / sr as f32;
        *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.25;
    }
    let input = buffer.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();
    let diff_input: f32 = buffer
        .iter()
        .zip(input.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(diff_input > 0.01, "mixed output should differ from input");
}

#[test]
fn test_get_data_returns_none() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    assert!(plugin.get_data().is_none());
}

#[test]
fn test_initialize_same_sample_rate_no_rebuild() {
    let mut plugin = LinearPhaseEqPlugin::new(1, 48000);
    plugin.fir_dirty = false;
    plugin.initialize(48000).unwrap();
    assert!(!plugin.fir_dirty);
}
