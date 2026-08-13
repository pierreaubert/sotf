// ============================================================================
// Integration tests for sotf-plugin-band-split
//
// These tests exercise the public `Plugin` trait and crate-specific API as a
// black box — no internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_split::BandSplitPlugin;

const SR: u32 = 48000;
const FRAMES: usize = 256;

fn settled_band_amplitudes(
    sample_rate: u32,
    crossover_type: &str,
    frequencies: &[f64],
    tone_hz: f32,
) -> Vec<f32> {
    let frames = sample_rate as usize / 2;
    let mut plugin = BandSplitPlugin::new_multiband(1, frequencies, crossover_type).unwrap();
    plugin.initialize(sample_rate).unwrap();
    let input: Vec<f32> = (0..frames)
        .map(|frame| {
            (2.0 * std::f32::consts::PI * tone_hz * frame as f32 / sample_rate as f32).sin()
        })
        .collect();
    let bands = frequencies.len() + 1;
    let mut output = vec![0.0; frames * bands];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(sample_rate, frames),
        )
        .unwrap();
    let start = frames / 2;
    (0..bands)
        .map(|band| {
            let power = (start..frames)
                .map(|frame| output[frame * bands + band].powi(2))
                .sum::<f32>()
                / (frames - start) as f32;
            (2.0 * power).sqrt()
        })
        .collect()
}

fn summed_impulse_response(kind: &str, frequencies: &[f64], frames: usize) -> Vec<f32> {
    let bands = frequencies.len() + 1;
    let mut plugin = BandSplitPlugin::new_multiband(1, frequencies, kind).unwrap();
    plugin.initialize(SR).unwrap();
    let mut input = vec![0.0; frames];
    input[0] = 1.0;
    let mut split = vec![0.0; frames * bands];
    plugin
        .process(&input, &mut split, &ProcessContext::new(SR, frames))
        .unwrap();
    split
        .chunks_exact(bands)
        .map(|frame| frame.iter().sum())
        .collect()
}

fn dft_at(signal: &[f32], frequency: f32) -> (f32, f32) {
    signal
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (index, sample)| {
            let phase = -2.0 * std::f32::consts::PI * frequency * index as f32 / SR as f32;
            (re + sample * phase.cos(), im + sample * phase.sin())
        })
}

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_two_band_plugin_has_expected_metadata() {
    let plugin = BandSplitPlugin::new(2, 1000.0, "LR24").unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "BandSplit");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 4); // 2 in * 2 bands
}

#[test]
fn new_multiband_plugin_has_expected_channel_counts() {
    let plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR48").unwrap();
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 3); // 1 in * 3 bands
}

#[test]
fn new_with_no_frequencies_fails() {
    let err = match BandSplitPlugin::new_multiband(1, &[], "LR24") {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("At least one crossover frequency"));
}

#[test]
fn new_with_too_many_frequencies_fails() {
    let err = match BandSplitPlugin::new_multiband(1, &[100.0, 500.0, 1000.0, 4000.0], "LR24") {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("Too many bands") || err.contains("max"));
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_frequency_and_gains() {
    let plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"frequency"));
    assert!(ids.contains(&"crossover_type"));
    assert!(ids.contains(&"band_0_gain_db"));
    assert!(ids.contains(&"band_1_gain_db"));
}

#[test]
fn multiband_parameters_include_additional_frequencies() {
    let plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"frequency"));
    assert!(ids.contains(&"frequency_2"));
    assert!(ids.contains(&"band_2_gain_db"));
}

#[test]
fn frequency_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(500.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(500.0))
    );
}

#[test]
fn crossover_type_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin
        .set_parameter(ParameterId::from("crossover_type"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossover_type")),
        Some(ParameterValue::Int(1))
    );
    plugin.initialize(SR).unwrap();
}

#[test]
fn band_gain_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("band_1_gain_db"),
            ParameterValue::Float(-6.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_1_gain_db")),
        Some(ParameterValue::Float(-6.0))
    );
}

#[test]
fn multiband_frequency_2_roundtrip() {
    let mut plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(1500.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency_2")),
        Some(ParameterValue::Float(1500.0))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn process_zero_input_produces_finite_output() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.0f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn dc_reconstructs_approximately() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let low = output[(FRAMES - 1) * 2];
    let high = output[(FRAMES - 1) * 2 + 1];
    let sum = low + high;
    assert!(
        (sum - dc).abs() < 0.05,
        "split bands should reconstruct DC: got {} (low={}, high={})",
        sum,
        low,
        high
    );
}

#[test]
fn multiband_dc_reconstructs_approximately() {
    let mut plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 3];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let mut sum = 0.0f32;
    for band in 0..3 {
        sum += output[(FRAMES - 1) * 3 + band];
    }
    assert!(
        (sum - dc).abs() < 0.05,
        "3-band split should reconstruct DC: got {} expected {}",
        sum,
        dc
    );
}

#[test]
fn band_gain_attenuates_band() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let frames = 4096;
    let dc = 0.5f32;
    let input = vec![dc; frames];
    let mut output_ref = vec![0.0f32; frames * 2];
    plugin
        .process(&input, &mut output_ref, &ProcessContext::new(SR, frames))
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(-24.0),
        )
        .unwrap();

    let mut output_gain = vec![0.0f32; frames * 2];
    plugin
        .process(&input, &mut output_gain, &ProcessContext::new(SR, frames))
        .unwrap();

    let low_ref = output_ref[(frames - 1) * 2].abs();
    let low_gain = output_gain[(frames - 1) * 2].abs();
    assert!(
        low_gain < low_ref * 0.1,
        "band 0 should be strongly attenuated: ref={} gained={}",
        low_ref,
        low_gain
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_continues() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut output2 = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();

    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(96000, FRAMES))
        .unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_band_gain_for_out_of_range_band_fails() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("band_7_gain_db"),
            ParameterValue::Float(-6.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("band_7"));
}

#[test]
fn set_frequency_with_non_numeric_type_fails() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::String("five hundred".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("frequency") || err.contains("type mismatch"));
}

#[test]
fn process_with_correct_output_size_succeeds() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

#[test]
fn two_band_response_matches_linkwitz_riley_slope_and_complementarity() {
    for (kind, order, tolerance) in [("LR24", 4_i32, 0.025_f32), ("LR48", 8, 0.035)] {
        for ratio in [0.5_f32, 1.0, 2.0] {
            let amplitudes = settled_band_amplitudes(48_000, kind, &[1_000.0], 1_000.0 * ratio);
            let ratio_power = ratio.powi(order);
            let expected_low = 1.0 / (1.0 + ratio_power);
            let expected_high = ratio_power / (1.0 + ratio_power);
            assert!(
                (amplitudes[0] - expected_low).abs() < tolerance,
                "{kind} low response at {ratio}fc: actual={}, expected={expected_low}",
                amplitudes[0]
            );
            assert!(
                (amplitudes[1] - expected_high).abs() < tolerance,
                "{kind} high response at {ratio}fc: actual={}, expected={expected_high}",
                amplitudes[1]
            );
            assert!(
                (amplitudes.iter().sum::<f32>() - 1.0).abs() < tolerance * 2.0,
                "{kind} complementary magnitude failed at {ratio}fc: {amplitudes:?}"
            );
        }
    }
}

#[test]
fn multiband_response_is_finite_isolated_and_valid_across_sample_rates() {
    for sample_rate in [32_000, 44_100, 48_000, 96_000, 192_000] {
        for kind in ["LR24", "LR48"] {
            for (tone, expected_band) in [(100.0, 0), (1_000.0, 1), (4_000.0, 2), (12_000.0, 3)] {
                let amplitudes =
                    settled_band_amplitudes(sample_rate, kind, &[500.0, 2_000.0, 8_000.0], tone);
                assert!(amplitudes.iter().all(|value| value.is_finite()));
                let wanted = amplitudes[expected_band];
                let strongest_other = amplitudes
                    .iter()
                    .enumerate()
                    .filter(|(band, _)| *band != expected_band)
                    .map(|(_, value)| *value)
                    .fold(0.0_f32, f32::max);
                assert!(
                    wanted > strongest_other * 2.0,
                    "{kind} {sample_rate} Hz, tone {tone}: wrong band response {amplitudes:?}"
                );
            }
        }
    }
}

#[test]
fn twelve_channel_processing_has_no_cross_channel_leakage() {
    let frames = 8_192;
    let channels = 12;
    let bands = 4;
    let mut plugin =
        BandSplitPlugin::new_multiband(channels, &[500.0, 2_000.0, 8_000.0], "LR48").unwrap();
    plugin.initialize(SR).unwrap();
    let mut input = vec![0.0; frames * channels];
    for frame in 0..frames {
        input[frame * channels + 7] =
            (2.0 * std::f32::consts::PI * 1_000.0 * frame as f32 / SR as f32).sin();
    }
    let mut output = vec![0.0; frames * channels * bands];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, frames))
        .unwrap();
    for frame in 0..frames {
        for band in 0..bands {
            for channel in 0..channels {
                if channel != 7 {
                    assert_eq!(
                        output[frame * channels * bands + band * channels + channel],
                        0.0
                    );
                }
            }
        }
    }
}

#[test]
fn impulse_response_characterizes_reconstruction_magnitude_and_phase() {
    let frames = 12_288;
    for kind in ["LR24", "LR48"] {
        let two_band = summed_impulse_response(kind, &[1_000.0], frames);
        let mut maximum_phase = 0.0_f32;
        for frequency in [125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0] {
            let (re, im) = dft_at(&two_band, frequency);
            let magnitude = re.hypot(im);
            maximum_phase = maximum_phase.max(im.atan2(re).abs());
            assert!(
                (magnitude - 1.0).abs() < 0.015,
                "{kind} two-band reconstruction magnitude at {frequency} Hz: {magnitude}"
            );
        }
        assert!(
            maximum_phase > 0.5,
            "{kind} crossover sum should expose its frequency-dependent phase, got {maximum_phase} rad"
        );

        // Cascaded multiband sums intentionally are not phase-perfect. Bound
        // their measured broadband magnitude so future topology changes cannot
        // silently introduce severe cancellation or gain.
        for frequencies in [&[500.0, 2_000.0][..], &[250.0, 1_000.0, 4_000.0][..]] {
            let response = summed_impulse_response(kind, frequencies, frames);
            for frequency in [125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0] {
                let (re, im) = dft_at(&response, frequency);
                let magnitude = re.hypot(im);
                assert!(
                    (0.45..=1.05).contains(&magnitude),
                    "{kind} {}-band summed magnitude at {frequency} Hz: {magnitude}",
                    frequencies.len() + 1
                );
            }
        }
    }
}

#[test]
fn deterministic_white_noise_split_sum_has_bounded_gain_and_correlation() {
    let frames = 48_000;
    let mut state = 0x5eed_1234_u32;
    let input: Vec<f32> = (0..frames)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state as f32 / u32::MAX as f32 - 0.5) * 0.5
        })
        .collect();
    for kind in ["LR24", "LR48"] {
        for frequencies in [
            &[1_000.0][..],
            &[500.0, 2_000.0][..],
            &[250.0, 1_000.0, 4_000.0][..],
        ] {
            let bands = frequencies.len() + 1;
            let mut plugin = BandSplitPlugin::new_multiband(1, frequencies, kind).unwrap();
            plugin.initialize(SR).unwrap();
            let mut output = vec![0.0; frames * bands];
            plugin
                .process(&input, &mut output, &ProcessContext::new(SR, frames))
                .unwrap();
            let mut input_power = 0.0;
            let mut output_power = 0.0;
            let mut cross = 0.0;
            for frame in 2_048..frames {
                let dry = input[frame];
                let sum: f32 = output[frame * bands..(frame + 1) * bands].iter().sum();
                input_power += dry * dry;
                output_power += sum * sum;
                cross += dry * sum;
            }
            let gain = (output_power / input_power).sqrt();
            let correlation = cross / (input_power * output_power).sqrt();
            let gain_bounds = if bands == 2 { 0.98..=1.02 } else { 0.55..=1.05 };
            assert!(
                gain_bounds.contains(&gain),
                "{kind} {bands}-band noise reconstruction gain={gain}"
            );
            assert!(
                correlation.is_finite() && correlation.abs() > 0.15,
                "{kind} {bands}-band reconstruction correlation={correlation}"
            );
        }
    }
}
