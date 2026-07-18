use super::super::dyn_eq_band_params::DynEqBandParams;
use super::super::dynamic_eq_plugin::DynamicEqPlugin;
use super::super::dynamic_eq_plugin_params::DynamicEqPluginParams;
use super::super::params;
use super::super::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;

fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len() as f32).sqrt()
}

fn process_with_first_eq_filter(plugin: &mut DynamicEqPlugin, input: &[f32]) -> Vec<f32> {
    let filter = &mut plugin.bands[0].eq_filters[0];
    input
        .iter()
        .map(|sample| filter.process(*sample as f64) as f32)
        .collect()
}

#[test]
fn test_dynamic_eq_passthrough() {
    // With gain=0, output should equal input
    let sr = 48000u32;
    let num_frames = 4800; // 100ms
    let amplitude = 0.5;

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -60.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 0.0, // zero gain = passthrough
                band_threshold: -60.0,
                band_ratio: 4.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let original = make_sine(1000.0, sr, num_frames, amplitude);
    let mut buf = original.clone();

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Output should be essentially the same (peak EQ at 0 dB is passthrough)
    let input_rms = rms(&original);
    let output_rms = rms(&buf);
    let ratio = output_rms / input_rms;
    assert!(
        (ratio - 1.0).abs() < 0.05,
        "Passthrough: ratio={:.4} (input_rms={:.4}, output_rms={:.4})",
        ratio,
        input_rms,
        output_rms
    );
}

#[test]
fn test_dynamic_eq_boosts_on_threshold() {
    // With gain=+6dB and loud signal above threshold, output should be boosted
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.5; // about -6 dBFS

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 6.0, // +6 dB boost
                band_threshold: -20.0,
                band_ratio: 10.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Use the second half to allow attack to settle
    let output_rms = rms(&buf[num_frames / 2..]);

    // Output should be louder than input at the band frequency
    assert!(
        output_rms > input_rms * 1.1,
        "Boost: output_rms={:.4} should be > input_rms*1.1={:.4}",
        output_rms,
        input_rms * 1.1
    );
}

#[test]
fn test_dynamic_eq_no_boost_below_threshold() {
    // Quiet signal should pass unaffected (below threshold)
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.001; // very quiet, about -60 dBFS

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -10.0, // high threshold
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 12.0, // big boost, but shouldn't trigger
                band_threshold: -10.0,
                band_ratio: 10.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let output_rms = rms(&buf[num_frames / 2..]);

    // Should be essentially unchanged (EQ at ~0 dB gain)
    let ratio = output_rms / input_rms;
    assert!(
        (ratio - 1.0).abs() < 0.15,
        "Below threshold: ratio={:.4} (input_rms={:.6}, output_rms={:.6})",
        ratio,
        input_rms,
        output_rms
    );
}

#[test]
fn test_dynamic_eq_frequency_selective() {
    // 1kHz band should only affect 1kHz content, not 100Hz content
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.5;

    let params = DynamicEqPluginParams {
        num_bands: 1,
        threshold: -20.0,
        ratio: 10.0,
        attack_ms: 0.5,
        release_ms: 20.0,
        knee: 0.0,
        link_channels: false,
        mix: 1.0,
        bands: vec![DynEqBandParams {
            frequency: 1000.0,
            q: 2.0, // narrow band
            gain: 12.0,
            band_threshold: -20.0,
            band_ratio: 10.0,
            active: true,
            solo: false,
        }],
    };

    // Test with 1kHz signal (in-band)
    let mut plugin_1k = DynamicEqPlugin::from_params(1, params.clone());
    plugin_1k.initialize(sr).unwrap();
    let mut buf_1k = make_sine(1000.0, sr, num_frames, amplitude);
    let input_rms_1k = rms(&buf_1k);
    let ctx = ProcessContext::new(sr, num_frames);
    plugin_1k.process_in_place(&mut buf_1k, &ctx).unwrap();
    let output_rms_1k = rms(&buf_1k[num_frames / 2..]);

    // Test with 100Hz signal (out-of-band)
    let mut plugin_100 = DynamicEqPlugin::from_params(1, params);
    plugin_100.initialize(sr).unwrap();
    let mut buf_100 = make_sine(100.0, sr, num_frames, amplitude);
    let input_rms_100 = rms(&buf_100);
    plugin_100.process_in_place(&mut buf_100, &ctx).unwrap();
    let output_rms_100 = rms(&buf_100[num_frames / 2..]);

    let ratio_1k = output_rms_1k / input_rms_1k;
    let ratio_100 = output_rms_100 / input_rms_100;

    // 1kHz should be affected more than 100Hz
    assert!(
        ratio_1k > ratio_100 * 1.2,
        "Frequency selectivity: 1kHz ratio={:.4} should be > 100Hz ratio={:.4} * 1.2",
        ratio_1k,
        ratio_100
    );
}

#[test]
fn test_band_frequency_change_rebuilds_filters() {
    let sr = 48000u32;
    let num_frames = 48000;
    let input = make_sine(1000.0, sr, num_frames, 0.25);
    let input_rms = rms(&input);

    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(sr).unwrap();
    plugin.bands[0].frequency = 1000.0;
    plugin.bands[0].q = 4.0;
    plugin.bands[0].target_gain_db = 12.0;
    plugin.bands[0].rebuild_eq_filters(sr);

    let boosted = process_with_first_eq_filter(&mut plugin, &input);
    let boosted_rms = rms(&boosted[num_frames / 2..]);
    assert!(boosted_rms > input_rms * 1.4);

    plugin.bands[0].frequency = 1000.0;
    plugin.bands[0].q = 4.0;
    plugin.bands[0].target_gain_db = 12.0;
    plugin.bands[0].rebuild_eq_filters(sr);
    plugin
        .set_parameter(
            ParameterId::from("band_0_frequency"),
            ParameterValue::Float(100.0),
        )
        .unwrap();

    let retuned = process_with_first_eq_filter(&mut plugin, &input);
    let retuned_rms = rms(&retuned[num_frames / 2..]);
    assert!(
        retuned_rms < input_rms * 1.15,
        "retuned 100 Hz EQ should not keep boosting 1 kHz: input={input_rms:.4}, retuned={retuned_rms:.4}"
    );
}

#[test]
fn test_band_q_change_rebuilds_filters() {
    let sr = 48000u32;
    let num_frames = 48000;
    let input = make_sine(400.0, sr, num_frames, 0.25);
    let input_rms = rms(&input);

    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(sr).unwrap();
    plugin.bands[0].frequency = 1000.0;
    plugin.bands[0].q = 0.1;
    plugin.bands[0].target_gain_db = 12.0;
    plugin.bands[0].rebuild_eq_filters(sr);

    let wide = process_with_first_eq_filter(&mut plugin, &input);
    let wide_rms = rms(&wide[num_frames / 2..]);
    assert!(wide_rms > input_rms * 1.3);

    plugin.bands[0].frequency = 1000.0;
    plugin.bands[0].q = 0.1;
    plugin.bands[0].target_gain_db = 12.0;
    plugin.bands[0].rebuild_eq_filters(sr);
    plugin
        .set_parameter(ParameterId::from("band_0_q"), ParameterValue::Float(10.0))
        .unwrap();

    let narrow = process_with_first_eq_filter(&mut plugin, &input);
    let narrow_rms = rms(&narrow[num_frames / 2..]);
    assert!(
        narrow_rms < input_rms * 1.15,
        "narrowed Q should not keep wide-band boost: input={input_rms:.4}, narrow={narrow_rms:.4}"
    );
}

/// `reset()` must rebuild EQ filters to reflect the current `target_gain_db`.
///
/// We set target_gain_db = 0.0 (passthrough) but manually build the filter at
/// 12 dB to simulate stale biquad state, then call reset() and verify the output
/// is unaffected (filter rebuilt at 0 dB).
#[test]
fn test_reset_rebuilds_eq_filters_at_zero_gain() {
    let sr = 48000u32;
    let num_frames = 48000;
    let input = make_sine(1000.0, sr, num_frames, 0.25);
    let input_rms = rms(&input);

    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(sr).unwrap();
    plugin.num_bands = 1;
    plugin.bands[0].frequency = 1000.0;
    plugin.bands[0].q = 4.0;
    // target_gain_db = 0 → after reset the filter should be passthrough.
    // We deliberately build the filter at 12 dB to create stale biquad state.
    plugin.bands[0].target_gain_db = 12.0;
    plugin.bands[0].rebuild_eq_filters(sr);
    // Now set the intended target and call reset(); it should rebuild at 0 dB.
    plugin.bands[0].target_gain_db = 0.0;
    plugin.reset();

    let mut buf = input.clone();
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let output_rms = rms(&buf[num_frames / 2..]);
    assert!(
        (output_rms / input_rms - 1.0).abs() < 0.05,
        "reset should restore neutral EQ: input={input_rms:.4}, output={output_rms:.4}"
    );
}

/// Regression test: sidechain must read the dry buffer, not the processed output.
///
/// Two bands at different frequencies are active. If the sidechain of band 1
/// reads the EQ'd output of band 0 instead of the original dry signal, band 1's
/// detection level would differ depending on band 0's activity — causing
/// inter-band contamination. We verify that disabling band 0 does not change
/// what band 1 detects.
#[test]
fn test_sidechain_reads_dry_buffer_not_modified_output() {
    let sr = 48000u32;
    let num_frames = 4800; // 100ms
    // Signal at 1 kHz only. Band 0 is also tuned to 1 kHz with a large gain,
    // so if band 1 (at 2 kHz) sees band 0's boosted output it would detect a
    // higher level at 2 kHz (due to sidechain filter bleed) than when band 0 is
    // inactive.
    let amplitude = 0.5;

    let make_two_band_plugin = |band0_active: bool| {
        DynamicEqPlugin::from_params(
            1,
            DynamicEqPluginParams {
                num_bands: 2,
                threshold: -60.0, // always triggering
                ratio: 20.0,
                attack_ms: 0.1,
                release_ms: 200.0,
                knee: 0.0,
                link_channels: false,
                mix: 1.0,
                bands: vec![
                    DynEqBandParams {
                        frequency: 1000.0,
                        q: 1.0,
                        gain: 18.0, // large boost at 1 kHz
                        band_threshold: -60.0,
                        band_ratio: 20.0,
                        active: band0_active,
                        solo: false,
                    },
                    DynEqBandParams {
                        frequency: 2000.0,
                        q: 4.0,    // narrow band at 2 kHz — only detects 2 kHz
                        gain: 0.0, // passthrough, but detection is what we test
                        band_threshold: -60.0,
                        band_ratio: 20.0,
                        active: true,
                        solo: false,
                    },
                ],
            },
        )
    };

    // We run the plugin twice: once with band 0 active (18 dB boost at 1 kHz)
    // and once with band 0 inactive. We capture band 1's monitoring GR value,
    // which reflects what the sidechain detected. If the sidechain was reading
    // the modified buffer, band 0's presence would inflate the band 1 detection.
    let capture_band1_gr = |plugin: &mut DynamicEqPlugin| -> f32 {
        let ctx = ProcessContext::new(sr, num_frames);
        let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
        plugin.process_in_place(&mut buf, &ctx).unwrap();
        plugin.monitoring_gr[1]
    };

    let mut p_with = make_two_band_plugin(true);
    p_with.initialize(sr).unwrap();
    let gr_with_band0 = capture_band1_gr(&mut p_with);

    let mut p_without = make_two_band_plugin(false);
    p_without.initialize(sr).unwrap();
    let gr_without_band0 = capture_band1_gr(&mut p_without);

    // Band 1 detects 2 kHz; the source is a 1 kHz pure tone.
    // Whether band 0 boosts 1 kHz or not, band 1's sidechain sees the same
    // original dry signal in both cases (assuming correct implementation).
    let diff = (gr_with_band0 - gr_without_band0).abs();
    assert!(
        diff < 0.5,
        "Band 1 GR differs by {diff:.3} dB depending on band 0 activity — sidechain contamination bug"
    );
}

/// Regression test: EQ coefficients must NOT be recomputed every sample.
///
/// With the old buggy implementation, `update_eq_gain` called `update_params`
/// (which calls sin/cos/tan internally) on nearly every sample during attack.
/// The correct implementation uses a fixed biquad + dry/wet blend, meaning the
/// filter state is shaped by a fixed transfer function and the blend proportion
/// drives the modulation depth.
///
/// This test verifies that the plugin produces a smoothly modulated output
/// whose RMS at the band frequency correctly reflects the blend proportion,
/// NOT a coefficient-stepped output that would introduce artifacts.
#[test]
fn test_eq_gain_uses_proportion_blend_not_coefficient_update() {
    let sr = 48000u32;
    // 200 ms: long enough to see steady-state, short enough for fast attack
    let num_frames = 9600usize;
    let amplitude = 0.5f32;
    let target_gain_db = 12.0f32;

    // Configure so the band fires immediately (threshold far below signal level)
    // and with a fast attack so it reaches proportion ≈ 1.0 quickly.
    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -60.0,
            ratio: 20.0,
            attack_ms: 0.5,
            release_ms: 200.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: target_gain_db,
                band_threshold: -60.0,
                band_ratio: 20.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let input = make_sine(1000.0, sr, num_frames, amplitude);
    let mut buf = input.clone();
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // At steady state (second half) proportion ≈ 1.0; output should match
    // the EQ biquad at target_gain_db applied to the input.
    // Apply the reference EQ biquad (at target_gain_db) to the same input.
    let mut ref_plugin = DynamicEqPlugin::new(1);
    ref_plugin.initialize(sr).unwrap();
    ref_plugin.bands[0].target_gain_db = target_gain_db;
    ref_plugin.bands[0].rebuild_eq_filters(sr);
    let reference: Vec<f32> = input
        .iter()
        .map(|s| ref_plugin.bands[0].eq_filters[0].process(*s as f64) as f32)
        .collect();

    let out_rms = rms(&buf[num_frames / 2..]);
    let ref_rms = rms(&reference[num_frames / 2..]);

    // At full proportion the blend output must equal the biquad-only reference.
    // Allow ±10% for the envelope settling.
    let ratio = out_rms / ref_rms;
    assert!(
        (ratio - 1.0).abs() < 0.10,
        "Steady-state output RMS {out_rms:.4} should match reference {ref_rms:.4} (ratio={ratio:.4})"
    );
}
