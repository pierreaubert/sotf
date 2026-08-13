use super::crossfeed_plugin::CrossfeedPlugin;
use super::crossfeed_plugin_params::CrossfeedPluginParams;
use super::delay_line::DelayLine;
use super::types::CrossfeedMode;
use super::types::CrossfeedPreset;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

#[test]
fn test_crossfeed_basic() {
    let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    p.initialize(48000).unwrap();
    let mut b = vec![1.0, 0.0, 1.0, 0.0];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 2))
        .unwrap();
    assert!(b[1].abs() > 0.0);
}

#[test]
fn test_yaw_only_itd_advances_delay_for_every_algorithm() {
    for mode in [
        CrossfeedMode::Bauer,
        CrossfeedMode::Meier,
        CrossfeedMode::Mb,
        CrossfeedMode::Hrtf,
    ] {
        let params = CrossfeedPluginParams {
            mode,
            head_yaw_deg: 45.0,
            itd_delay_ms: 0.0,
            ..Default::default()
        };
        let mut plugin = CrossfeedPlugin::new(params).unwrap();
        plugin.initialize(48_000).unwrap();
        let mut buffer = vec![0.0; 128 * 2];
        buffer[0] = 1.0;
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(48_000, 128))
            .unwrap();
        assert_ne!(
            plugin.itd_delay_l.write_pos, 0,
            "yaw-only ITD must run the delay line in {mode:?} mode"
        );
    }
}

/// Regression for block-rate yaw/ITD automation: processing the same ramp in
/// different callback partitions must produce the same samples.  The delay
/// must follow the yaw smoother at sample rate, not jump to the end-of-block
/// value once per callback.
#[test]
fn test_yaw_itd_automation_is_partition_invariant() {
    const SAMPLE_RATE: u32 = 48_000;
    const FRAMES: usize = 1_024;

    fn render(partitions: &[usize]) -> Vec<f32> {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.bauer_feed_db = 6.0;
        params.itd_delay_ms = 0.0;
        params.head_yaw_deg = 0.0;
        params.mix = 1.0;
        let mut plugin = CrossfeedPlugin::new(params).unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("head_yaw_deg"),
                ParameterValue::Float(45.0),
            )
            .unwrap();

        let mut output = Vec::with_capacity(FRAMES * 2);
        let mut frame = 0;
        let mut partition = 0;
        while frame < FRAMES {
            let block = partitions[partition % partitions.len()].min(FRAMES - frame);
            let mut buffer = Vec::with_capacity(block * 2);
            for i in 0..block {
                let absolute = frame + i;
                let phase =
                    2.0 * std::f32::consts::PI * 311.0 * absolute as f32 / SAMPLE_RATE as f32;
                buffer.extend_from_slice(&[phase.sin() * 0.5, (phase * 0.73).cos() * 0.25]);
            }
            plugin
                .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, block))
                .unwrap();
            output.extend_from_slice(&buffer);
            frame += block;
            partition += 1;
        }
        output
    }

    let one_block = render(&[FRAMES]);
    let varied_blocks = render(&[1, 7, 31, 64, 3, 127, 17]);
    assert_eq!(one_block.len(), varied_blocks.len());
    let max_error = one_block
        .iter()
        .zip(&varied_blocks)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_error < 1e-5,
        "yaw/ITD automation must be callback-partition invariant; max error={max_error}"
    );
}

#[test]
fn test_public_preset_selection_applies_complete_preset() {
    let cases = [
        (0, CrossfeedPreset::Default),
        (1, CrossfeedPreset::Cmoy),
        (2, CrossfeedPreset::Meier),
        (3, CrossfeedPreset::Mb),
        (4, CrossfeedPreset::Off),
        (5, CrossfeedPreset::Hrtf),
    ];
    for (index, preset) in cases {
        let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("crossfeed_preset"),
                ParameterValue::Int(index),
            )
            .unwrap();
        let expected = CrossfeedPluginParams::from_preset(preset);
        assert_eq!(plugin.params.preset, preset);
        assert_eq!(plugin.params.mode, expected.mode);
        assert_eq!(plugin.params.bauer_fcut_hz, expected.bauer_fcut_hz);
        assert_eq!(plugin.params.bauer_feed_db, expected.bauer_feed_db);
        assert_eq!(plugin.params.meier_level, expected.meier_level);
        assert_eq!(plugin.params.mb_low_feed_db, expected.mb_low_feed_db);
    }
}

#[test]
fn public_presets_converge_to_fresh_reference_audio() {
    const SR: u32 = 48_000;
    const FRAMES: usize = 8_192;
    for (index, preset) in [
        (0, CrossfeedPreset::Default),
        (1, CrossfeedPreset::Cmoy),
        (2, CrossfeedPreset::Meier),
        (3, CrossfeedPreset::Mb),
        (4, CrossfeedPreset::Off),
        (5, CrossfeedPreset::Hrtf),
    ] {
        let mut selected = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
        selected.initialize(SR).unwrap();
        selected
            .set_parameter(
                ParameterId::from("crossfeed_preset"),
                ParameterValue::Int(index),
            )
            .unwrap();
        let mut reference =
            CrossfeedPlugin::new(CrossfeedPluginParams::from_preset(preset)).unwrap();
        reference.initialize(SR).unwrap();

        let input: Vec<f32> = (0..FRAMES)
            .flat_map(|frame| {
                let phase = 2.0 * std::f32::consts::PI * 347.0 * frame as f32 / SR as f32;
                [phase.sin() * 0.4, (phase * 0.73).cos() * 0.2]
            })
            .collect();
        let mut actual = input.clone();
        let mut expected = input;
        selected
            .process_in_place(&mut actual, &ProcessContext::new(SR, FRAMES))
            .unwrap();
        reference
            .process_in_place(&mut expected, &ProcessContext::new(SR, FRAMES))
            .unwrap();
        let tail_start = (FRAMES - 512) * 2;
        let max_error = actual[tail_start..]
            .iter()
            .zip(&expected[tail_start..])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_error < 2e-3,
            "preset {preset:?} did not converge to its fresh reference; max error={max_error}"
        );
    }
}

#[test]
fn test_unrelated_parameter_update_preserves_filter_history() {
    let params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    let mut changed = CrossfeedPlugin::new(params.clone()).unwrap();
    let mut control = CrossfeedPlugin::new(params).unwrap();
    changed.initialize(48_000).unwrap();
    control.initialize(48_000).unwrap();
    let mut warm_a = vec![0.0; 256 * 2];
    let mut warm_b = vec![0.0; 256 * 2];
    warm_a[0] = 1.0;
    warm_b[0] = 1.0;
    changed
        .process_in_place(&mut warm_a, &ProcessContext::new(48_000, 256))
        .unwrap();
    control
        .process_in_place(&mut warm_b, &ProcessContext::new(48_000, 256))
        .unwrap();
    changed
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();
    let mut tail_a = vec![0.0; 256 * 2];
    let mut tail_b = tail_a.clone();
    changed
        .process_in_place(&mut tail_a, &ProcessContext::new(48_000, 256))
        .unwrap();
    control
        .process_in_place(&mut tail_b, &ProcessContext::new(48_000, 256))
        .unwrap();
    assert_eq!(tail_a, tail_b, "mix setter must not reset DSP filters");
}

#[test]
fn test_process_rejects_non_exact_stereo_buffer_lengths() {
    let cases = [(true, CrossfeedMode::Bauer), (false, CrossfeedMode::Off)];
    for (enabled, mode) in cases {
        let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams {
            enabled,
            mode,
            ..Default::default()
        })
        .unwrap();
        plugin.initialize(48_000).unwrap();
        for len in [7, 9] {
            let mut buffer = vec![0.0; len];
            assert!(
                plugin
                    .process_in_place(&mut buffer, &ProcessContext::new(48_000, 4))
                    .is_err()
            );
        }
        let mut empty = [];
        assert_eq!(
            plugin
                .process_in_place(&mut empty, &ProcessContext::new(48_000, 0))
                .unwrap(),
            0
        );
        assert!(
            plugin
                .process_in_place(&mut empty, &ProcessContext::new(48_000, usize::MAX))
                .is_err()
        );
    }
}

#[test]
fn test_non_finite_yaw_is_rejected() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("head_yaw_deg"),
                ParameterValue::Float(f32::NAN),
            )
            .is_err()
    );
}

#[test]
fn test_invalid_construction_and_sample_rate_are_rejected() {
    let invalid = CrossfeedPluginParams {
        mb_low_freq_hz: 6_000.0,
        mb_mid_high_freq_hz: 5_000.0,
        ..Default::default()
    };
    assert!(CrossfeedPlugin::new(invalid).is_err());

    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    assert!(plugin.initialize(0).is_err());
}

#[test]
fn process_requires_initialized_matching_sample_rate() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    let mut buffer = vec![0.0; 8];
    assert!(
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(44_100, 4))
            .is_err()
    );
    plugin.initialize(48_000).unwrap();
    assert!(
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(44_100, 4))
            .is_err()
    );
}

#[test]
fn failed_parameter_batch_is_transactional() {
    use sotf_host::parametric_plugin::ParameterSet;

    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(48_000).unwrap();
    let before = plugin.current_values();
    let mut update = ParameterSet::new();
    update.insert(ParameterId::from("mix"), ParameterValue::Float(0.25));
    update.insert(ParameterId::from("zz_unknown"), ParameterValue::Float(1.0));
    assert!(plugin.apply_values(update).is_err());
    assert_eq!(plugin.current_values(), before);
}

#[test]
fn scratch_capacity_matches_setup_contract() {
    let params = CrossfeedPluginParams {
        max_block_frames: 257,
        ..Default::default()
    };
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    assert_eq!(plugin.dry_l.len(), 257);
    plugin.initialize(48_000).unwrap();
    let mut exact = vec![0.0; 257 * 2];
    assert_eq!(
        plugin
            .process_in_place(&mut exact, &ProcessContext::new(48_000, 257))
            .unwrap(),
        257
    );
    let mut too_large = vec![0.0; 258 * 2];
    assert!(
        plugin
            .process_in_place(&mut too_large, &ProcessContext::new(48_000, 258))
            .is_err()
    );
}

#[test]
fn non_finite_audio_is_sanitized_before_dsp_state() {
    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Meier,
        ..Default::default()
    };
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(48_000).unwrap();
    let mut poisoned = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.25];
    plugin
        .process_in_place(&mut poisoned, &ProcessContext::new(48_000, 2))
        .unwrap();
    assert!(poisoned.iter().all(|sample| sample.is_finite()));

    let mut follow_up = vec![0.1; 128 * 2];
    plugin
        .process_in_place(&mut follow_up, &ProcessContext::new(48_000, 128))
        .unwrap();
    assert!(follow_up.iter().all(|sample| sample.is_finite()));
}

#[test]
fn test_zero_delay_still_advances_delay_history() {
    let mut delay = DelayLine::new(0.0, 48_000);
    assert_eq!(delay.process(0.25), 0.25);
    assert_eq!(delay.write_pos, 1);
    delay.set_delay(0.5, 48_000);
    let peak = (0..25)
        .map(|_| delay.process(0.0).abs())
        .fold(0.0, f32::max);
    assert!(peak > 0.1);
}

#[test]
fn test_mb_feed_linear_cache_updates_on_parameter_change() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    params.mb_low_feed_db = 0.0;
    params.mb_mid_feed_db = 6.0;
    params.mb_high_feed_db = 3.0;
    let mut p = CrossfeedPlugin::new(params).unwrap();

    let before = p.mb_feed_linear;
    p.set_parameter(
        ParameterId::from("mb_mid_feed_db"),
        ParameterValue::Float(0.0),
    )
    .unwrap();

    assert_ne!(
        before, p.mb_feed_linear,
        "linear feed cache should change when feed dB changes"
    );
    assert!(
        (p.mb_feed_linear[1] - 1.0).abs() < 1e-4,
        "0 dB mid feed should cache as unity gain"
    );
}

/// Regression: Bauer mode used a plain lowpass on the per-channel crossfeed path,
/// which caused a bass boost on mono signals and a steep roll-off. A proper Bauer
/// crossfeed applies a low-shelf cut to the difference signal (L-R), preserving
/// mono energy and gently attenuating low-frequency stereo width.
#[test]
fn test_bauer_mono_preserved() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.bauer_feed_db = 6.0;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 4000;
    let mut buf: Vec<f32> = (0..n).flat_map(|_| [0.5f32, 0.5]).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, n))
        .unwrap();

    let last_l = buf[(n - 1) * 2];
    let last_r = buf[(n - 1) * 2 + 1];
    // Old lowpass code boosted mono by feed*lowpass(mono) ≈ 3.0.
    // Proper low-shelf on difference leaves mono unchanged.
    assert!(
        (last_l - 0.5).abs() < 0.01 && (last_r - 0.5).abs() < 0.01,
        "Mono signal should be preserved, got L={last_l}, R={last_r}"
    );
}

/// Regression: Bauer mode should apply a low-shelf cut to the difference signal,
/// attenuating low-frequency stereo width while preserving high-frequency width.
#[test]
fn test_bauer_difference_shelved() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.bauer_feed_db = 6.0;
    let sr = 48000u32;
    let n = 8000;

    // Low-frequency stereo difference
    let mut buf_lf: Vec<f32> = (0..n)
        .flat_map(|i| {
            let t = i as f32 / sr as f32;
            let s = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
            [s, -s]
        })
        .collect();
    let mut p = CrossfeedPlugin::new(params.clone()).unwrap();
    p.initialize(sr).unwrap();
    p.process_in_place(&mut buf_lf, &ProcessContext::new(sr, n))
        .unwrap();

    let tail_start = (n - 2000) * 2;
    let diff_rms_lf: f32 = buf_lf[tail_start..]
        .chunks(2)
        .map(|c| {
            let d = c[0] - c[1];
            d * d
        })
        .sum::<f32>()
        .sqrt()
        / (2000.0f32).sqrt();

    // With a -6 dB shelf, low-frequency difference should be attenuated
    assert!(
        diff_rms_lf < 0.5,
        "Low-frequency difference should be attenuated by shelf, got {diff_rms_lf}"
    );

    // High-frequency stereo difference
    let mut buf_hf: Vec<f32> = (0..n)
        .flat_map(|i| {
            let t = i as f32 / sr as f32;
            let s = (2.0 * std::f32::consts::PI * 10000.0 * t).sin() * 0.5;
            [s, -s]
        })
        .collect();
    let mut p2 = CrossfeedPlugin::new(params).unwrap();
    p2.initialize(sr).unwrap();
    p2.process_in_place(&mut buf_hf, &ProcessContext::new(sr, n))
        .unwrap();

    let diff_rms_hf: f32 = buf_hf[tail_start..]
        .chunks(2)
        .map(|c| {
            let d = c[0] - c[1];
            d * d
        })
        .sum::<f32>()
        .sqrt()
        / (2000.0f32).sqrt();

    // High-frequency difference should be nearly unchanged
    assert!(
        diff_rms_hf > 0.6,
        "High-frequency difference should be preserved, got {diff_rms_hf}"
    );
}

#[test]
fn test_bauer_basic() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let mut buffer = vec![1.0, 0.0, 0.0, 1.0];
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();

    assert!(buffer[1].abs() > 0.0);
}

#[test]
fn test_bauer_uses_lowpass() {
    // Bauer mode should crossfeed low frequencies (low-shelf on difference, per bs2b spec).
    // A DC signal should produce significant crossfeed;
    // a high-frequency signal should produce minimal crossfeed.
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.bauer_feed_db = 6.0;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    // DC signal: all energy in left channel
    let n = 4000;
    let mut dc_buf: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
    p.process_in_place(&mut dc_buf, &ProcessContext::new(48000, n))
        .unwrap();

    // After settling, DC should bleed significantly into right channel via LPF crossfeed
    let last_r = dc_buf[(n - 1) * 2 + 1];
    assert!(
        last_r.abs() > 0.1,
        "Bauer LPF crossfeed: DC should bleed to right channel, got {}",
        last_r
    );
}

#[test]
fn test_meier_basic() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
    params.mode = CrossfeedMode::Meier;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let mut buffer = vec![1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, 4))
        .unwrap();
    assert!(buffer[1].abs() > 0.0);
}

#[test]
fn test_mb_basic() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 100;
    let mut buffer: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, n))
        .unwrap();
    // Right channel should get some crossfeed
    let last_r = buffer[(n - 1) * 2 + 1];
    assert!(
        last_r.abs() > 0.0,
        "MB crossfeed should bleed, got {}",
        last_r
    );
}

#[test]
fn test_mb_mono_signal_is_headroom_normalized() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    params.mix = 1.0;
    params.autogain_enabled = false;

    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 4096;
    let mut buffer: Vec<f32> = (0..n).flat_map(|_| [0.5f32, 0.5f32]).collect();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, n))
        .unwrap();

    let tail_peak = buffer[(n / 2) * 2..]
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0, f32::max);
    assert!(
        tail_peak <= 0.75,
        "default multiband mono output should stay headroom-normalized, got peak {tail_peak}"
    );
}

#[test]
fn test_mb_feed_has_true_off_endpoint_and_per_band_constant_power_norm() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    params.mb_low_feed_db = -60.0;
    params.mb_mid_feed_db = 0.0;
    params.mb_high_feed_db = 6.0;
    let plugin = CrossfeedPlugin::new(params).unwrap();

    // -60 dB is the UI's explicit Off endpoint, not a merely very quiet bleed.
    assert_eq!(plugin.mb_feed_linear[0], 0.0);
    assert!((plugin.mb_feed_linear[1] - 1.0).abs() < 1e-6);
    assert!((plugin.mb_feed_linear[2] - 10.0_f32.powf(6.0 / 20.0)).abs() < 1e-2);

    // Each band is normalized independently, so changing one feed cannot
    // attenuate unrelated bands.  The constant-power factor is 1/sqrt(1+g²).
    assert!((plugin.mb_wet_norm[0] - 1.0).abs() < 1e-6);
    assert!((plugin.mb_wet_norm[1] - 1.0 / 2.0_f32.sqrt()).abs() < 1e-6);
    let high_gain = 10.0_f32.powf(6.0 / 20.0);
    assert!((plugin.mb_wet_norm[2] - 1.0 / (1.0 + high_gain * high_gain).sqrt()).abs() < 5e-3);
}

#[test]
fn test_itd_delay() {
    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Bauer,
        itd_delay_ms: 0.5, // 0.5ms = 24 samples at 48kHz
        ..CrossfeedPluginParams::default()
    };
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    // Impulse in left channel only
    let n = 100;
    let mut buffer = vec![0.0f32; n * 2];
    buffer[0] = 1.0; // impulse at frame 0, left channel

    p.process_in_place(&mut buffer, &ProcessContext::new(48000, n))
        .unwrap();

    // The crossfeed to right channel should be delayed by ~24 samples
    // Check that right channel has near-zero for the first few frames
    // and nonzero later
    let early_r: f32 = (0..10).map(|f| buffer[f * 2 + 1].abs()).sum();
    let late_r: f32 = (25..50).map(|f| buffer[f * 2 + 1].abs()).sum();
    assert!(
        late_r > early_r,
        "ITD delay: later right channel samples should exceed early ones. early={}, late={}",
        early_r,
        late_r
    );
}

#[test]
fn test_itd_delay_zero() {
    // With itd_delay_ms = 0, delay line should be transparent
    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Bauer,
        itd_delay_ms: 0.0,
        ..CrossfeedPluginParams::default()
    };
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 100;
    let mut buffer: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, n))
        .unwrap();
    // Should still work and produce crossfeed
    assert!(buffer[1].is_finite());
}

#[test]
fn test_delay_line_supports_fractional_and_high_sample_rate_delay() {
    let mut delay = DelayLine::new(1.0, 192000);
    assert!(
        delay.capacity >= 194,
        "1ms at 192kHz needs at least 192 samples plus interpolation headroom"
    );

    delay.set_delay(0.5, 48000);
    assert!(
        (delay.delay_samples - 24.0).abs() < 1e-5,
        "0.5ms at 48kHz should be represented as 24 samples"
    );

    delay.set_delay(0.25, 44100);
    assert!(
        delay.delay_samples.fract() > 0.0,
        "0.25ms at 44.1kHz should preserve a fractional delay"
    );
}

#[test]
fn test_itd_parameter() {
    let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    p.initialize(48000).unwrap();

    // Set ITD delay
    p.set_parameter(
        ParameterId::from("itd_delay_ms"),
        ParameterValue::Float(0.3),
    )
    .unwrap();

    let val = p.get_parameter(&ParameterId::from("itd_delay_ms"));
    assert_eq!(val, Some(ParameterValue::Float(0.3)));
}

#[test]
fn test_itd_delay_accuracy() {
    // Set itd_delay_ms=0.5. Process an impulse on L only.
    // Verify the crossfeed to R arrives later than with itd_delay_ms=0.
    let n = 200;
    let sr = 48000;

    // Helper: process an L-only impulse and find the frame where R channel
    // first exceeds a threshold.
    let find_r_onset = |itd_ms: f32| -> usize {
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            bauer_feed_db: 6.0,
            itd_delay_ms: itd_ms,
            mix: 1.0,
            ..CrossfeedPluginParams::default()
        };
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(sr).unwrap();

        let mut buffer = vec![0.0f32; n * 2];
        buffer[0] = 1.0; // impulse at frame 0, L channel

        p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
            .unwrap();

        // Find the first frame where |R| > threshold
        let threshold = 0.001;
        for f in 0..n {
            if buffer[f * 2 + 1].abs() > threshold {
                return f;
            }
        }
        n // never found
    };

    let onset_no_delay = find_r_onset(0.0);
    let onset_with_delay = find_r_onset(0.5);

    // With the differential-ITD model, itd_delay_ms is split equally across the two
    // crossfeed paths (base = itd_ms / 2 per path when yaw = 0).
    // So 0.5ms → each path gets 0.25ms = 12 samples at 48kHz.
    // The delayed version should arrive later.
    assert!(
        onset_with_delay > onset_no_delay,
        "ITD 0.5ms should delay R onset: no_delay_onset={}, delayed_onset={}",
        onset_no_delay,
        onset_with_delay
    );

    // The difference should be approximately 12 samples (0.25ms at 48kHz, half of 0.5ms ITD)
    let diff = onset_with_delay - onset_no_delay;
    assert!(
        (diff as i32 - 12).unsigned_abs() <= 3,
        "ITD difference should be ~12 samples (0.25ms per path at 48kHz), got {} \
             (onset_no={}, onset_with={})",
        diff,
        onset_no_delay,
        onset_with_delay
    );
}

#[test]
fn test_disabled_passthrough() {
    let params = CrossfeedPluginParams {
        enabled: false,
        ..CrossfeedPluginParams::default()
    };
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let mut buffer = vec![1.0, 0.5, 0.3, 0.7];
    let original = buffer.clone();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    assert_eq!(
        buffer, original,
        "Disabled crossfeed should pass through unchanged"
    );
}

#[test]
fn test_crossfeed_frequency_response_low_vs_high() {
    // Crossfeed should affect low frequencies more than high frequencies.
    // Generate pure left-channel tones at 200Hz and 8kHz, measure how much
    // crossfeed leaks into the right channel at each frequency.
    let sr = 48000u32;
    let n = 10000; // frames
    let ctx = ProcessContext::new(sr, n);

    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.bauer_feed_db = 6.0;

    // Helper: generate left-only sine, process, measure right channel energy in tail
    let measure_crossfeed = |freq: f32| -> f32 {
        let mut p = CrossfeedPlugin::new(params.clone()).unwrap();
        p.initialize(sr).unwrap();

        let mut buf: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
                [s, 0.0] // left only
            })
            .collect();
        p.process_in_place(&mut buf, &ctx).unwrap();

        // Measure right channel RMS in the last 2000 frames (skip transient)
        let tail_start = (n - 2000) * 2;
        let right_energy: f32 = buf[tail_start..]
            .chunks(2)
            .map(|c| c[1] * c[1])
            .sum::<f32>();
        (right_energy / 2000.0).sqrt()
    };

    let low_crossfeed = measure_crossfeed(200.0);
    let high_crossfeed = measure_crossfeed(8000.0);

    assert!(
        low_crossfeed > 0.001,
        "200Hz should produce measurable crossfeed: {low_crossfeed}"
    );
    assert!(
        low_crossfeed > high_crossfeed * 1.5,
        "Low-frequency crossfeed ({low_crossfeed:.4}) should be significantly more than high-frequency ({high_crossfeed:.4})"
    );
}

/// Bug: Meier filters were not updated when sample rate changed from 44100 to 48000.
/// Verify that both sample rates produce consistent crossfeed RMS for a tone well
/// below the 650 Hz LPF cutoff (should pass freely at both rates).
#[test]
fn test_meier_filter_coefficients_correct_after_sample_rate_change() {
    let n = 8000usize;
    let freq = 200.0f32;

    let measure_rms = |sr: u32| -> f32 {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
        params.mode = CrossfeedMode::Meier;
        params.mix = 1.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(sr).unwrap();

        let mut buf: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
                [s, 0.0f32]
            })
            .collect();
        p.process_in_place(&mut buf, &ProcessContext::new(sr, n))
            .unwrap();

        let tail_start = (n * 3 / 4) * 2;
        let rms: f32 = buf[tail_start..]
            .chunks(2)
            .map(|c| c[1] * c[1])
            .sum::<f32>();
        (rms / (n / 4) as f32).sqrt()
    };

    let rms_44 = measure_rms(44100);
    let rms_48 = measure_rms(48000);

    assert!(
        rms_44 > 0.001,
        "Meier crossfeed should produce output at 44100: rms={rms_44}"
    );
    assert!(
        rms_48 > 0.001,
        "Meier crossfeed should produce output at 48000: rms={rms_48}"
    );
    // At 200 Hz (well below cutoff) both rates should produce similar gain. 20% tolerance.
    let ratio = if rms_44 > rms_48 {
        rms_44 / rms_48
    } else {
        rms_48 / rms_44
    };
    assert!(
        ratio < 1.2,
        "Meier crossfeed at 200 Hz should be consistent across sample rates \
             (44100={rms_44:.4}, 48000={rms_48:.4}, ratio={ratio:.3})"
    );
}

/// Bug: ITD was modeled symmetrically — both crossfeed paths got the same delay.
/// With positive yaw, the L→R path should be longer than the R→L path.
#[test]
fn test_itd_yaw_asymmetry() {
    let sr = 48000u32;
    let n = 300usize;

    let find_onset = |impulse_on_left: bool, yaw_deg: f32| -> usize {
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            bauer_feed_db: 6.0,
            itd_delay_ms: 0.5,
            head_yaw_deg: yaw_deg,
            mix: 1.0,
            ..CrossfeedPluginParams::default()
        };
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(sr).unwrap();

        let mut buffer = vec![0.0f32; n * 2];
        if impulse_on_left {
            buffer[0] = 1.0;
        } else {
            buffer[1] = 1.0;
        }

        p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
            .unwrap();

        let threshold = 0.001;
        for f in 0..n {
            let idx = if impulse_on_left { f * 2 + 1 } else { f * 2 };
            if buffer[idx].abs() > threshold {
                return f;
            }
        }
        n
    };

    // At yaw=0: symmetric — both paths carry equal delay (base = 0.25 ms each)
    let onset_l_to_r_yaw0 = find_onset(true, 0.0);
    let onset_r_to_l_yaw0 = find_onset(false, 0.0);
    assert!(
        (onset_l_to_r_yaw0 as i32 - onset_r_to_l_yaw0 as i32).unsigned_abs() <= 2,
        "At yaw=0 both paths should have equal delay: L→R={onset_l_to_r_yaw0}, R→L={onset_r_to_l_yaw0}"
    );

    // At positive yaw: L→R path should be longer (larger onset index)
    let onset_l_to_r_pos = find_onset(true, 45.0);
    let onset_r_to_l_pos = find_onset(false, 45.0);
    assert!(
        onset_l_to_r_pos >= onset_r_to_l_pos,
        "Positive yaw: L→R delay ({onset_l_to_r_pos}) should be >= R→L ({onset_r_to_l_pos})"
    );
}

/// Bug: mix smoother advanced to end-of-block value and applied it uniformly,
/// causing a step discontinuity instead of a ramp.
/// Verify that after a mix change the right channel output increases across the block.
#[test]
fn test_mix_ramp_no_step_discontinuity() {
    let sr = 48000u32;
    let n = 512usize;

    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.mix = 0.0;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(sr).unwrap();

    // Warm-up block to settle smoother at mix=0
    let mut warmup = vec![0.5f32; n * 2];
    p.process_in_place(&mut warmup, &ProcessContext::new(sr, n))
        .unwrap();

    // Jump mix to 1.0
    p.set_parameter(
        sotf_host::parameters::ParameterId::from("mix"),
        sotf_host::parameters::ParameterValue::Float(1.0),
    )
    .unwrap();

    // Process DC on L only
    let mut buf: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0f32]).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(sr, n))
        .unwrap();

    // Right channel: dry_r=0, wet_r>0 (crossfeed).  With a ramp, early < late.
    let first_r = buf[1].abs();
    let last_r = buf[(n - 1) * 2 + 1].abs();
    assert!(
        last_r > first_r,
        "Mix ramp: last right sample ({last_r:.6}) should exceed first ({first_r:.6})"
    );
}

/// Changing a Bauer cutoff while processing must not reset the shelf history.
/// A reset changes the first output sample at the automation boundary by much
/// more than the surrounding sine-wave slope, which is audible as a click.
#[test]
fn test_bauer_frequency_automation_is_click_free() {
    let sr = 48_000u32;
    let block = 1024usize;
    let freq = 440.0f32;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.mix = 1.0;
    params.bauer_feed_db = 12.0;
    params.bauer_fcut_hz = 400.0;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(sr).unwrap();

    let render = |start: usize| -> Vec<f32> {
        (0..block)
            .flat_map(|i| {
                let phase = 2.0 * std::f32::consts::PI * freq * (start + i) as f32 / sr as f32;
                let sample = phase.sin() * 0.5;
                [sample, -sample]
            })
            .collect()
    };

    let mut before = render(0);
    plugin
        .process_in_place(&mut before, &ProcessContext::new(sr, block))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("bauer_fcut_hz"),
            ParameterValue::Float(1000.0),
        )
        .unwrap();
    let mut after = render(block);
    plugin
        .process_in_place(&mut after, &ProcessContext::new(sr, block))
        .unwrap();

    let previous = before[(block - 1) * 2];
    let boundary_jump = (after[0] - previous).abs();
    let local_slope = (1..32)
        .map(|i| (before[i * 2] - before[(i - 1) * 2]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        boundary_jump <= local_slope * 2.0 + 1.0e-3,
        "Bauer cutoff automation produced a discontinuity: boundary={boundary_jump}, local_slope={local_slope}"
    );
}

#[test]
fn test_multiband_frequency_automation_preserves_crossover_state() {
    let sr = 48_000u32;
    let block = 1024usize;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    params.mix = 1.0;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();

    plugin.initialize(sr).unwrap();
    let render = |start: usize| -> Vec<f32> {
        (0..block)
            .flat_map(|i| {
                let phase = 2.0 * std::f32::consts::PI * 220.0 * (start + i) as f32 / sr as f32;
                let sample = phase.sin() * 0.5;
                [sample, -sample]
            })
            .collect()
    };
    let mut before = render(0);
    plugin
        .process_in_place(&mut before, &ProcessContext::new(sr, block))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mb_low_freq_hz"),
            ParameterValue::Float(500.0),
        )
        .unwrap();
    let mut after = render(block);
    plugin
        .process_in_place(&mut after, &ProcessContext::new(sr, block))
        .unwrap();

    let boundary_jump = (after[0] - before[(block - 1) * 2]).abs();
    let local_slope = (1..32)
        .map(|i| (before[i * 2] - before[(i - 1) * 2]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        boundary_jump <= local_slope * 3.0 + 0.02,
        "multiband cutoff automation lost crossover state: boundary={boundary_jump}, local_slope={local_slope}"
    );
}

#[test]
fn disabled_crossfeed_resets_state_before_reentry() {
    let sr = 48_000;
    let n = 128;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(sr).unwrap();

    let mut warm = vec![0.0; n * 2];
    warm[0] = 1.0;
    plugin
        .process_in_place(&mut warm, &ProcessContext::new(sr, n))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut bypass = vec![0.0; n * 2];
    plugin
        .process_in_place(&mut bypass, &ProcessContext::new(sr, n))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    let mut silent = vec![0.0; n * 2];
    plugin
        .process_in_place(&mut silent, &ProcessContext::new(sr, n))
        .unwrap();
    assert!(silent.iter().all(|sample| sample.abs() < 1e-7));
}

/// Mode changes must not resurrect filter history from an earlier activation
/// of that mode.  Switching away from Meier and back while processing silence
/// should therefore remain silent rather than replaying the old LPF/all-pass
/// tail.
#[test]
fn mode_transition_resets_inactive_filter_state() {
    let sr = 48_000;
    let n = 8;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
    params.mode = CrossfeedMode::Meier;
    params.mix = 1.0;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(sr).unwrap();

    let mut impulse = vec![0.0; n * 2];
    impulse[0] = 1.0;
    plugin
        .process_in_place(&mut impulse, &ProcessContext::new(sr, n))
        .unwrap();
    assert!(
        impulse.iter().any(|sample| sample.abs() > 1e-5),
        "warm-up impulse must exercise the Meier state"
    );

    plugin
        .set_parameter(ParameterId::from("crossfeed_mode"), ParameterValue::Int(1))
        .unwrap();
    let mut bauer = vec![0.0; n * 2];
    plugin
        .process_in_place(&mut bauer, &ProcessContext::new(sr, n))
        .unwrap();

    plugin
        .set_parameter(ParameterId::from("crossfeed_mode"), ParameterValue::Int(2))
        .unwrap();
    let mut silent = vec![0.0; n * 2];
    plugin
        .process_in_place(&mut silent, &ProcessContext::new(sr, n))
        .unwrap();
    assert!(
        silent.iter().all(|sample| sample.abs() < 1e-7),
        "mode re-entry must not replay stale filter state: max={:.9e}",
        silent.iter().map(|sample| sample.abs()).fold(0.0, f32::max)
    );
}

/// Regression: reset() must clear all filter state so that a second
/// playback pass starts from the same deterministic state as a fresh
/// plugin. Previously bauer_shelf, meier LPF/allpass, and yaw_smoother
/// were not reset, causing stale filter tails and wrong yaw values.
#[test]
fn test_reset_clears_all_filter_state() {
    let sr = 48000;
    let n = 512;

    // Create two identical plugins
    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Meier,
        meier_level: 0.5,
        head_yaw_deg: 30.0,
        itd_delay_ms: 0.3,
        mix: 1.0,
        ..CrossfeedPluginParams::default()
    };
    let mut p1 = CrossfeedPlugin::new(params.clone()).unwrap();
    p1.initialize(sr).unwrap();
    let mut p2 = CrossfeedPlugin::new(params.clone()).unwrap();
    p2.initialize(sr).unwrap();

    // Run p1 for one block to warm up filter state
    let mut block1: Vec<f32> = (0..n)
        .flat_map(|i| [(i as f32 * 0.01).sin(), 0.0f32])
        .collect();
    p1.process_in_place(&mut block1, &ProcessContext::new(sr, n))
        .unwrap();

    // Reset p1 — after this it should behave like a fresh p2
    p1.reset();

    // Process the same impulse on both
    let mut impulse1 = vec![0.0f32; n * 2];
    impulse1[0] = 1.0;
    let mut impulse2 = impulse1.clone();

    p1.process_in_place(&mut impulse1, &ProcessContext::new(sr, n))
        .unwrap();
    p2.process_in_place(&mut impulse2, &ProcessContext::new(sr, n))
        .unwrap();

    // Outputs should match exactly (or very closely)
    for i in 0..(n * 2) {
        assert!(
            (impulse1[i] - impulse2[i]).abs() < 1e-5,
            "reset() did not fully clear state: sample {} differs by {}",
            i,
            (impulse1[i] - impulse2[i]).abs()
        );
    }
}

/// Bug: process_in_place must NOT resize buffers on the audio thread.
/// Pre-allocate to a safe size in initialize() and error if exceeded.
#[test]
fn test_process_does_not_resize_buffers() {
    let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    p.initialize(48000).unwrap();

    let initial_len = p.dry_l.len();
    let initial_cap = p.dry_l.capacity();

    let nf = 8192; // exceeds old 4096 limit but under new 16384 limit
    let mut buffer: Vec<f32> = (0..nf).flat_map(|_| [0.5f32, 0.3f32]).collect();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, nf))
        .unwrap();

    assert_eq!(
        p.dry_l.len(),
        initial_len,
        "process_in_place must not resize dry_l (audio-thread allocation bug)"
    );
    assert_eq!(
        p.dry_l.capacity(),
        initial_cap,
        "process_in_place must not reallocate dry_l (audio-thread allocation bug)"
    );
}

/// Verify that blocks larger than the pre-allocated capacity are rejected.
#[test]
fn test_oversized_block_returns_error() {
    let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    p.initialize(48000).unwrap();

    let nf = p.dry_l.len() + 1;
    let mut buffer: Vec<f32> = vec![0.0f32; nf * 2];
    let err = p
        .process_in_place(&mut buffer, &ProcessContext::new(48000, nf))
        .unwrap_err();
    assert!(
        err.contains("exceeds pre-allocated capacity"),
        "Expected capacity error, got: {}",
        err
    );
}

// -------------------------------------------------------------------------
// process_in_place focused tests (Off mode, autogain, yaw, ITD, mix=0)
// -------------------------------------------------------------------------

#[test]
fn test_process_in_place_enabled_off_mode_passthrough() {
    let params = CrossfeedPluginParams {
        enabled: true,
        mode: CrossfeedMode::Off,
        ..Default::default()
    };
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let mut buffer = vec![1.0, 0.5, 0.3, 0.7];
    let original = buffer.clone();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    assert_eq!(
        buffer, original,
        "enabled=true with mode=Off should pass through unchanged"
    );
}

#[test]
fn test_process_in_place_autogain_enabled() {
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.autogain_enabled = true;
    params.mode = CrossfeedMode::Bauer;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 4096;
    let mut buffer: Vec<f32> = (0..n).flat_map(|_| [0.5f32, 0.3f32]).collect();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, n))
        .unwrap();

    assert!(
        buffer.iter().all(|s| s.is_finite()),
        "autogain should produce finite output"
    );
}

#[test]
fn autogain_target_lufs_changes_compensation() {
    fn converged_gain(target_lufs: f32) -> f32 {
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            bauer_feed_db: 4.5,
            autogain_enabled: true,
            autogain_target_lufs: target_lufs,
            autogain_smoothing_ms: 20.0,
            ..Default::default()
        };
        let mut plugin = CrossfeedPlugin::new(params).unwrap();
        plugin.initialize(48_000).unwrap();

        let block_size = 1024;
        for block in 0..120 {
            let mut buffer = vec![0.0_f32; block_size * 2];
            for frame in 0..block_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (block * block_size + frame) as f32
                        / 48_000.0;
                buffer[2 * frame] = phase.sin() * 0.25;
                buffer[2 * frame + 1] = phase.sin() * 0.25;
            }
            plugin
                .process_in_place(&mut buffer, &ProcessContext::new(48_000, block_size))
                .unwrap();
        }

        plugin.auto_gain.current_gain_db()
    }

    let quiet_target_gain = converged_gain(-36.0);
    let loud_target_gain = converged_gain(-12.0);
    assert!(
        (quiet_target_gain - loud_target_gain).abs() > 6.0,
        "autogain target must affect converged compensation: -36={quiet_target_gain} dB, -12={loud_target_gain} dB"
    );
}

#[test]
fn autogain_target_lufs_updates_the_helper() {
    let mut params = CrossfeedPluginParams::default();
    params.autogain_enabled = true;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(48_000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("autogain_target_lufs"),
            ParameterValue::Float(-24.0),
        )
        .unwrap();
    assert_eq!(plugin.auto_gain.target_lufs(), Some(-24.0));
}

#[test]
fn test_process_in_place_yaw_parameter_affects_itd() {
    let sr = 48000u32;
    let n = 300usize;

    let find_onset = |yaw_deg: f32| -> usize {
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            bauer_feed_db: 6.0,
            itd_delay_ms: 0.5,
            head_yaw_deg: 0.0,
            mix: 1.0,
            ..CrossfeedPluginParams::default()
        };
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(sr).unwrap();

        p.set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(yaw_deg),
        )
        .unwrap();
        p.reset(); // snap yaw smoother to target immediately

        let mut buffer = vec![0.0f32; n * 2];
        buffer[0] = 1.0;

        p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
            .unwrap();

        let threshold = 0.001;
        for f in 0..n {
            if buffer[f * 2 + 1].abs() > threshold {
                return f;
            }
        }
        n
    };

    let onset_yaw0 = find_onset(0.0);
    let onset_yaw45 = find_onset(45.0);

    assert!(
        onset_yaw45 >= onset_yaw0,
        "positive yaw should delay L→R crossfeed: yaw0={onset_yaw0}, yaw45={onset_yaw45}"
    );
}

#[test]
fn test_process_in_place_meier_with_itd() {
    let sr = 48000u32;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
    params.mode = CrossfeedMode::Meier;
    params.itd_delay_ms = 0.5;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(sr).unwrap();

    let n = 200;
    let mut buffer = vec![0.0f32; n * 2];
    buffer[0] = 1.0;

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
        .unwrap();

    let early_r: f32 = (0..10).map(|f| buffer[f * 2 + 1].abs()).sum();
    let late_r: f32 = (25..50).map(|f| buffer[f * 2 + 1].abs()).sum();
    assert!(
        late_r > early_r,
        "Meier with ITD: delayed crossfeed should arrive later. early={}, late={}",
        early_r,
        late_r
    );
}

#[test]
fn test_process_in_place_mb_with_itd() {
    let sr = 48000u32;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
    params.mode = CrossfeedMode::Mb;
    params.itd_delay_ms = 0.5;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(sr).unwrap();

    let n = 200;
    let mut buffer = vec![0.0f32; n * 2];
    buffer[0] = 1.0;

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
        .unwrap();

    let early_r: f32 = (0..10).map(|f| buffer[f * 2 + 1].abs()).sum();
    let late_r: f32 = (25..50).map(|f| buffer[f * 2 + 1].abs()).sum();
    assert!(
        late_r > early_r,
        "MB with ITD: delayed crossfeed should arrive later. early={}, late={}",
        early_r,
        late_r
    );
}

#[test]
fn test_process_in_place_mix_zero_passthrough() {
    let sr = 48000u32;
    let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    params.mode = CrossfeedMode::Bauer;
    params.mix = 0.0;
    let mut p = CrossfeedPlugin::new(params).unwrap();
    p.initialize(sr).unwrap();

    let n = 100;
    let mut buffer: Vec<f32> = (0..n)
        .flat_map(|i| [(i as f32 * 0.1).sin(), 0.0f32])
        .collect();
    let original = buffer.clone();

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, n))
        .unwrap();

    for (i, (&out, &inp)) in buffer.iter().zip(original.iter()).enumerate() {
        assert!(
            (out - inp).abs() < 1e-5,
            "mix=0 should pass through unchanged at sample {}: out={}, in={}",
            i,
            out,
            inp
        );
    }
}

fn render_hrtf(input: &[f32], partitions: &[usize]) -> Vec<f32> {
    let frames = input.len() / 2;
    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Hrtf,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(48_000).unwrap();
    let mut output = Vec::with_capacity(input.len());
    let mut frame = 0;
    let mut part = 0;
    while frame < frames {
        let count = partitions[part % partitions.len()].min(frames - frame);
        let mut block = input[frame * 2..(frame + count) * 2].to_vec();
        plugin
            .process_in_place(&mut block, &ProcessContext::new(48_000, count))
            .unwrap();
        output.extend_from_slice(&block);
        frame += count;
        part += 1;
    }
    output
}

#[test]
fn hrtf_hard_pan_bleeds_after_interaural_delay_and_reports_zero_latency() {
    let mut input = vec![0.0; 256 * 2];
    input[0] = 1.0;
    let output = render_hrtf(&input, &[256]);
    let first_right = (0..256).find(|&i| output[i * 2 + 1].abs() > 1e-6).unwrap();
    assert!((11..=14).contains(&first_right), "onset={first_right}");
    assert!(output.iter().all(|sample| sample.is_finite()));

    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams {
        mode: CrossfeedMode::Hrtf,
        ..Default::default()
    })
    .unwrap();
    plugin.initialize(48_000).unwrap();
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn hrtf_preserves_mono_fold_and_bounds_antiphase_input() {
    let input: Vec<f32> = (0..4096)
        .flat_map(|i| {
            let sample = (std::f32::consts::TAU * 220.0 * i as f32 / 48_000.0).sin() * 0.4;
            [sample, -sample]
        })
        .collect();
    let output = render_hrtf(&input, &[4096]);
    for (source, rendered) in input.chunks_exact(2).zip(output.chunks_exact(2)) {
        assert!(((source[0] + source[1]) - (rendered[0] + rendered[1])).abs() < 1e-6);
    }
    let peak = output
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!(peak < 0.8, "anti-phase peak={peak}");
}

#[test]
fn hrtf_render_is_partition_invariant() {
    let input: Vec<f32> = (0..4096)
        .flat_map(|i| {
            let phase = std::f32::consts::TAU * 347.0 * i as f32 / 48_000.0;
            [phase.sin() * 0.4, (phase * 0.73).cos() * 0.2]
        })
        .collect();
    let one = render_hrtf(&input, &[4096]);
    let varied = render_hrtf(&input, &[1, 7, 31, 64, 3, 127]);
    assert_eq!(one, varied);
}
