use super::gate_data::GateData;
use super::gate_plugin::GatePlugin;
use super::types::GatePluginParams;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::ParameterSet;
use sotf_host::plugin::{PluginCostClass, ProcessContext};

#[test]
fn threshold_decision_uses_the_canonical_db_smoother_trajectory() {
    let mut plugin = GatePlugin::new(1, -60.0, 4.0, 5.0, 0.0, 50.0);
    plugin.initialize(48_000).unwrap();
    plugin
        .parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(0.0))
        .unwrap();

    for _ in 0..512 {
        let (threshold_db, threshold_linear) = plugin.advance_threshold();
        let expected = math_audio_dsp::fast_math::fast_pow10(
            threshold_db / super::consts::DB_CONVERSION_FACTOR,
        );
        assert!((threshold_linear - expected).abs() < 1.0e-7);
    }
}

#[test]
fn test_gate_basic() {
    let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 10.0, 50.0);
    p.initialize(48000).unwrap();
    let mut b = vec![0.05; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(b[999] < 0.05);
}

#[test]
fn test_hold_samples_precomputed_and_updated() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 10.0, 50.0);
    assert_eq!(p.hold_samples, 441);

    p.initialize(96000).unwrap();
    assert_eq!(p.hold_samples, 960);

    p.parametric_set_parameter(ParameterId::from("hold"), ParameterValue::Float(1.5))
        .unwrap();
    assert_eq!(p.hold_samples, 144);
}

/// CRITICAL: Attack must control gate opening speed, Release must control closing speed.
/// With fast attack (1 ms) and slow release (500 ms), the gate should open quickly
/// when the signal rises above threshold.
#[test]
fn test_attack_controls_opening_speed() {
    let sr = 48000u32;
    let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 0.0, 500.0);
    p.initialize(sr).unwrap();

    // Close the gate with very quiet signal (-100 dBFS)
    let quiet_len = sr as usize;
    let mut quiet = vec![0.00001f32; quiet_len];
    let ctx = ProcessContext::new(sr, quiet_len);
    p.process_in_place(&mut quiet, &ctx).unwrap();

    // Switch to loud signal (-6 dBFS, well above threshold)
    let loud_len = sr as usize / 10; // 100 ms
    let input_level = 0.5f32;
    let mut loud = vec![input_level; loud_len];
    let ctx2 = ProcessContext::new(sr, loud_len);
    p.process_in_place(&mut loud, &ctx2).unwrap();

    // With fast attack (1 ms) the gate should be essentially fully open
    // within the last 10 ms of the loud section.
    let tail_start = loud_len - sr as usize / 100;
    let avg_output: f32 = loud[tail_start..].iter().sum::<f32>() / (loud_len - tail_start) as f32;
    assert!(
        avg_output > input_level * 0.95,
        "Gate should open quickly with fast attack (1 ms), but avg output was {avg_output}"
    );
}

/// CRITICAL: Release must control gate closing speed.
/// With the slowest supported attack (50 ms) and fastest release (10 ms), the gate should close quickly
/// when the signal drops below threshold.
#[test]
fn test_release_controls_closing_speed() {
    let sr = 48000u32;
    let mut p = GatePlugin::new(1, -20.0, 100.0, 50.0, 0.0, 10.0);
    p.initialize(sr).unwrap();

    // Open the gate with loud signal (-6 dBFS)
    let loud_len = sr as usize;
    let mut loud = vec![0.5f32; loud_len];
    let ctx = ProcessContext::new(sr, loud_len);
    p.process_in_place(&mut loud, &ctx).unwrap();

    // Switch to very quiet signal (-60 dBFS, well below threshold)
    let quiet_len = sr as usize / 10; // 100 ms
    let quiet_input = 0.001f32;
    let mut quiet = vec![quiet_input; quiet_len];
    let ctx2 = ProcessContext::new(sr, quiet_len);
    p.process_in_place(&mut quiet, &ctx2).unwrap();

    // With the fastest supported release (10 ms) the gate should be essentially fully closed
    // within the last 10 ms of the quiet section.
    let tail_start = quiet_len - sr as usize / 100;
    let avg_output: f32 = quiet[tail_start..].iter().sum::<f32>() / (quiet_len - tail_start) as f32;
    assert!(
        avg_output < quiet_input * 0.1,
        "Gate should close quickly with fast release (10 ms), but avg output was {avg_output}"
    );
}

/// CRITICAL: In linked stereo mode the monitoring cache `is_open` must reflect
/// the actual gate state. When the gate is fully closed it must report false.
#[test]
fn test_linked_stereo_monitoring_cache_reports_closed() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    let block_size = 1024;
    let num_blocks = 20; // enough to trigger cache update (every 10 blocks)
    let quiet = vec![0.0001f32; block_size * 2];
    for _ in 0..num_blocks {
        let mut buf = quiet.clone();
        let ctx = ProcessContext::new(sr, block_size);
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    let data = p.get_data().unwrap();
    let gate_data = data.downcast_ref::<GateData>().unwrap();
    assert!(
        !gate_data.is_open,
        "Linked stereo gate should report is_open=false when fully closed"
    );
}

/// Sidechain HPF at 200 Hz: a 50 Hz signal below threshold should NOT open
/// the gate (HPF filters out the low-freq detection signal). A 1 kHz signal
/// at the same level should open it.
#[test]
fn test_sidechain_hpf_filters_low_freq_detection() {
    let sr = 48000u32;
    let threshold_db = -20.0;
    // Signal amplitude is above threshold in raw dB but below after HPF
    let amplitude = 10.0_f32.powf(-15.0 / 20.0); // -15 dBFS (above -20 threshold)

    // --- Test 1: 50 Hz signal with HPF=200 Hz. Gate should stay closed. ---
    let mut p_low = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 200.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p_low.initialize(sr).unwrap();

    let num_frames = 9600; // 200ms
    let mut buf_low = vec![0.0f32; num_frames];
    for (i, sample) in buf_low.iter_mut().enumerate() {
        *sample = amplitude * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr as f32).sin();
    }

    let ctx = ProcessContext::new(sr, num_frames);
    p_low.process_in_place(&mut buf_low, &ctx).unwrap();

    // The 50 Hz signal should be significantly attenuated because the HPF
    // at 200 Hz filters out the 50 Hz from the sidechain detection.
    let rms_low: f32 =
        buf_low[4800..].iter().map(|x| x * x).sum::<f32>() / (num_frames - 4800) as f32;
    let rms_low = rms_low.sqrt();

    // --- Test 2: 1 kHz signal with HPF=200 Hz. Gate should open. ---
    let mut p_high = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 200.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p_high.initialize(sr).unwrap();

    let mut buf_high = vec![0.0f32; num_frames];
    for (i, sample) in buf_high.iter_mut().enumerate() {
        *sample = amplitude * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
    }

    p_high.process_in_place(&mut buf_high, &ctx).unwrap();

    let rms_high: f32 =
        buf_high[4800..].iter().map(|x| x * x).sum::<f32>() / (num_frames - 4800) as f32;
    let rms_high = rms_high.sqrt();

    // 1 kHz should pass through much louder than 50 Hz (gate open vs closed)
    assert!(
        rms_high > rms_low * 2.0,
        "1kHz (RMS={rms_high:.5}) should pass through gate much louder than 50Hz (RMS={rms_low:.5}) \
             when sidechain HPF=200Hz"
    );
}

/// Hysteresis test: a signal that oscillates +/-2 dB around the threshold should
/// not cause the gate to "chatter" (rapidly open and close).
///
/// Setup:
///   threshold = -20 dB, hysteresis = 4 dB
///   -> open threshold  = -20 dB
///   -> close threshold = -24 dB
///
/// The test signal alternates every 100 samples between -18 dBFS and -22 dBFS.
/// Both levels are between -24 dB and -20 dB when the gate is open, so once
/// opened the gate should remain open for the entire alternating region.
///
/// Without hysteresis the gate would open on -18 dB and close on -22 dB every
/// 100-sample segment, producing many transitions.  With hysteresis it should
/// stay open after the first opening.
#[test]
fn test_gate_hysteresis_no_chatter() {
    let sr = 48000u32;
    // Fast attack/release so the envelope reacts within the 100-sample segments
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            hysteresis_db: 4.0,
            ratio: 100.0,
            attack_ms: 0.5,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    // Build 1-second buffer that alternates every 100 samples between
    // -18 dBFS (above open threshold -20 dB) and -22 dBFS (between open and
    // close thresholds, so gate should stay open once opened).
    let amp_high = 10.0_f32.powf(-18.0 / 20.0); // -18 dBFS
    let amp_low = 10.0_f32.powf(-22.0 / 20.0); // -22 dBFS  (above close threshold -24 dB)
    let num_frames = sr as usize; // 1 second
    let mut buffer: Vec<f32> = (0..num_frames)
        .map(|i| {
            if (i / 100) % 2 == 0 {
                amp_high
            } else {
                amp_low
            }
        })
        .collect();

    let ctx = ProcessContext::new(sr, num_frames);
    p.process_in_place(&mut buffer, &ctx).unwrap();

    // Count how many times the output crosses a "gate closed" boundary.
    // If the gate chatters, the output will swing between near-zero and amp_low
    // each 100-sample segment.  With hysteresis the output should be consistently
    // passed through after the initial opening.
    //
    // Threshold for "effectively gated": output below 10 % of amp_low.
    let closed_threshold = amp_low * 0.1;

    // Skip the first 500 samples (attack / settling period).
    let steady_state = &buffer[500..];

    // Count sign-changes between "open" and "closed" state.
    let mut transitions = 0usize;
    let mut prev_open = steady_state[0] > closed_threshold;
    for &s in steady_state.iter().skip(1) {
        let cur_open = s > closed_threshold;
        if cur_open != prev_open {
            transitions += 1;
            prev_open = cur_open;
        }
    }

    // With hysteresis the gate should open once and stay open: 0 or at most 1
    // transition (the initial opening) throughout the steady-state region.
    // Without hysteresis we would expect ~2 * (num_frames / 100) ~ 190 transitions.
    assert!(
        transitions <= 2,
        "Gate with hysteresis=4dB should not chatter on a +/-2dB oscillating signal, \
             but observed {transitions} open/closed transitions in steady-state"
    );
}

/// Regression test: gate open/close decisions use linear-space thresholds.
///
/// Prior to this refactor, `process_in_place` called `fast_log10(det)` on every
/// frame to compare the detected envelope against the threshold in dB space.
/// The gate now compares the linear envelope directly against pre-computed
/// `threshold_linear` and `close_threshold_linear`, eliminating `fast_log10`
/// from the hot per-frame audio path.  `fast_log10` is only called when the
/// gate is actually closing and we need to compute the attenuation curve in
/// dB space.
#[test]
fn test_gate_linear_threshold_no_fast_log10_in_decision() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    // Signal exactly at the linear threshold (-20 dBFS = 0.1).
    // With exact linear comparison the gate should remain open.
    let input_level = 0.1f32;
    let frames = sr as usize / 10;
    let mut buf = vec![input_level; frames];
    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut buf, &ctx).unwrap();

    let tail_start = frames - sr as usize / 100;
    let avg_output: f32 = buf[tail_start..].iter().sum::<f32>() / (frames - tail_start) as f32;
    assert!(
        (avg_output - input_level).abs() < 0.001,
        "Gate should stay open for signal at exact threshold (linear comparison), \
             avg output was {avg_output}"
    );
}

#[test]
fn test_soft_knee_curve_is_continuous_at_boundaries() {
    let mut p = GatePlugin::new(1, -20.0, 4.0, 1.0, 0.0, 10.0);
    p.range_db = 80.0;
    p.knee_db = 6.0;

    let threshold = -20.0;
    let upper = threshold + p.knee_db / 2.0;
    let lower = threshold - p.knee_db / 2.0;
    let slope = 1.0 - 1.0 / p.ratio.max(1.0);

    let at_upper = p.calculate_gate_attenuation(upper, threshold);
    let just_inside_upper = p.calculate_gate_attenuation(upper - 0.001, threshold);
    assert!(at_upper.abs() < 1e-6);
    assert!(
        just_inside_upper < 0.001,
        "knee should enter smoothly near upper boundary, got {just_inside_upper}"
    );

    let at_lower = p.calculate_gate_attenuation(lower, threshold);
    let expected_lower = (threshold - lower) * slope;
    assert!(
        (at_lower - expected_lower).abs() < 1e-5,
        "lower boundary should meet below-threshold line: got {at_lower}, expected {expected_lower}"
    );
}

#[test]
fn range_zero_means_unlimited_attenuation() {
    let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 0.0, 10.0);
    p.knee_db = 0.0;
    for (range, expected) in [(0.0, 79.2), (20.0, 20.0), (80.0, 79.2), (120.0, 79.2)] {
        p.range_db = range;
        let attenuation = p.calculate_gate_attenuation(-100.0, -20.0);
        assert!(
            (attenuation - expected).abs() < 1e-4,
            "range={range}: {attenuation}"
        );
    }
    p.range_db = 0.0;
    assert_eq!(p.calculate_gate_attenuation(-10_000.0, -20.0), 240.0);
}

#[test]
fn invalid_factory_parameters_are_rejected() {
    let base = GatePluginParams {
        threshold_db: -40.0,
        ratio: 10.0,
        attack_ms: 1.0,
        hold_ms: 0.0,
        release_ms: 50.0,
        mix: 1.0,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
        sidechain_hpf_order: "2nd".into(),
        detection_mode: "peak".into(),
        sidechain_external: false,
        range_db: 80.0,
        hysteresis_db: 0.0,
        knee_db: 0.0,
        lookahead_ms: 0.0,
    };
    for bad in [
        GatePluginParams {
            threshold_db: f32::NAN,
            ..base.clone()
        },
        GatePluginParams {
            ratio: f32::INFINITY,
            ..base.clone()
        },
        GatePluginParams {
            hold_ms: f32::NEG_INFINITY,
            ..base.clone()
        },
        GatePluginParams {
            attack_ms: 0.0,
            ..base.clone()
        },
        GatePluginParams {
            release_ms: -1.0,
            ..base.clone()
        },
        GatePluginParams {
            sidechain_hpf_order: "8th".into(),
            ..base.clone()
        },
        GatePluginParams {
            detection_mode: "invalid".into(),
            ..base.clone()
        },
    ] {
        assert!(GatePlugin::try_from_params(1, bad).is_err());
    }
    assert!(GatePlugin::try_from_params(0, base).is_err());
}

#[test]
fn metadata_and_structural_schema_match_runtime_contracts() {
    let mut gate = GatePlugin::try_from_params(
        2,
        GatePluginParams {
            lookahead_ms: 5.0,
            ..GatePluginParams::default()
        },
    )
    .unwrap();
    gate.initialize(48_000).unwrap();

    assert_eq!(gate.info().version, env!("CARGO_PKG_VERSION"));
    assert_eq!(gate.cost_class(), PluginCostClass::Dynamics);
    let metadata = gate.compile_metadata();
    assert!(!metadata.linear);
    assert!(metadata.stateful && metadata.boundary);
    assert!(metadata.channel_mixing);
    assert_eq!(metadata.latency_samples, gate.latency_samples());

    let structural = [
        "link_channels",
        "sidechain_hpf_hz",
        "sidechain_hpf_order",
        "detection_mode",
        "sidechain_external",
        "lookahead_ms",
    ];
    let schema = gate.parameter_schema();
    for id in structural {
        let parameter = schema
            .iter()
            .find(|parameter| parameter.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing structural parameter {id}"));
        assert_eq!(parameter.update_mode, UpdateMode::Structural, "{id}");
    }
}

#[test]
fn structural_parameters_are_rejected_after_initialize() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.initialize(48_000).unwrap();
    for id in [
        "lookahead_ms",
        "sidechain_hpf_hz",
        "detection_mode",
        "sidechain_external",
    ] {
        let value = if id == "detection_mode" {
            ParameterValue::Int(1)
        } else if id == "sidechain_external" {
            ParameterValue::Bool(true)
        } else {
            ParameterValue::Float(5.0)
        };
        assert!(
            p.parametric_set_parameter(ParameterId::from(id), value)
                .is_err(),
            "{id}"
        );
    }
}

fn process_diagnostic_blocks(gate: &mut GatePlugin, samples: &[f32]) {
    let channels = 2;
    let block_frames = 512;
    for block in samples.chunks_exact(channels * block_frames) {
        let mut buffer = block.to_vec();
        gate.process_in_place(&mut buffer, &ProcessContext::new(48_000, block_frames))
            .unwrap();
    }
}

#[test]
fn diagnostics_publish_distinct_input_and_attenuation_vectors_when_linked() {
    let mut gate = GatePlugin::new(2, -10.0, 100.0, 1.0, 0.0, 10.0);
    gate.initialize(48_000).unwrap();
    let mut samples = Vec::new();
    for _ in 0..8 {
        for _ in 0..512 {
            samples.extend([0.1_f32, 0.01_f32]);
        }
    }
    process_diagnostic_blocks(&mut gate, &samples);

    let data = gate.get_data().unwrap();
    let data = data.downcast_ref::<GateData>().unwrap();
    assert!((data.input_levels_db[0] + 20.0).abs() < 0.5);
    assert!((data.input_levels_db[1] + 40.0).abs() < 0.5);
    assert!(data.attenuation_db[0] > 1.0);
    assert!(data.attenuation_db[1] > 1.0);
}

#[test]
fn diagnostics_publish_per_channel_values_when_unlinked() {
    let mut gate = GatePlugin::new(2, -10.0, 100.0, 1.0, 0.0, 10.0);
    gate.parametric_set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    gate.initialize(48_000).unwrap();
    let mut samples = Vec::new();
    for _ in 0..8 {
        for _ in 0..512 {
            samples.extend([0.1_f32, 0.01_f32]);
        }
    }
    process_diagnostic_blocks(&mut gate, &samples);

    let data = gate.get_data().unwrap();
    let data = data.downcast_ref::<GateData>().unwrap();
    assert!((data.input_levels_db[0] + 20.0).abs() < 0.5);
    assert!((data.input_levels_db[1] + 40.0).abs() < 0.5);
    assert!(data.attenuation_db[0] > 1.0);
    assert!(data.attenuation_db[1] > 1.0);
}

// -------------------------------------------------------------------------
// set_parameter smoke tests and edge cases
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_all_params_roundtrip() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.parametric_set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert_eq!(
        p.parametric_get_parameter(&ParameterId::from("link_channels")),
        Some(ParameterValue::Bool(false))
    );
    p.initialize(48000).unwrap();

    let cases: &[(&str, ParameterValue)] = &[
        ("threshold", ParameterValue::Float(-30.0)),
        ("ratio", ParameterValue::Float(20.0)),
        ("attack", ParameterValue::Float(5.0)),
        ("hold", ParameterValue::Float(15.0)),
        ("release", ParameterValue::Float(200.0)),
        ("mix", ParameterValue::Float(0.5)),
        ("range_db", ParameterValue::Float(40.0)),
        ("hysteresis_db", ParameterValue::Float(2.0)),
        ("knee_db", ParameterValue::Float(3.0)),
    ];

    for &(id, ref value) in cases {
        p.parametric_set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = p.parametric_get_parameter(&ParameterId::from(id));
        assert_eq!(got, Some(value.clone()), "roundtrip failed for {}", id);
    }
}

#[test]
fn test_set_parameter_clamps_out_of_bounds() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.initialize(48000).unwrap();

    // param_bridge clamps floats to range instead of returning Err
    p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(10.0))
        .unwrap();
    assert_eq!(p.threshold_db, 0.0);

    p.parametric_set_parameter(
        ParameterId::from("threshold"),
        ParameterValue::Float(-100.0),
    )
    .unwrap();
    assert_eq!(p.threshold_db, -80.0);

    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(2.0))
        .unwrap();
    assert_eq!(p.mix, 1.0);

    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.5))
        .unwrap();
    assert_eq!(p.mix, 0.0);
}

#[test]
fn test_set_parameter_unknown_id_returns_error() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.initialize(48000).unwrap();

    let result =
        p.parametric_set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0));
    assert!(result.is_err());
}

#[test]
fn test_set_parameter_type_mismatch_returns_error() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.initialize(48000).unwrap();

    // threshold is float, not bool
    let result =
        p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Bool(true));
    assert!(result.is_err());
}

#[test]
fn test_process_empty_buffer_returns_zero() {
    let mut p = GatePlugin::new(1, -20.0, 10.0, 1.0, 0.0, 50.0);
    p.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    let frames = p.process_in_place(&mut buf, &ctx).unwrap();
    assert_eq!(frames, 0);
}

#[test]
fn test_input_channels_doubles_with_external_sidechain() {
    let mut p = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: true,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(48000).unwrap();

    assert_eq!(p.channels(), 2);
    assert_eq!(p.input_channels(), 4);
}

#[test]
fn test_from_params_detection_mode_rms() {
    let p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "rms".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    assert_eq!(p.detection_mode_index, 1);
}

#[test]
fn test_from_params_hpf_order_4th() {
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 100.0,
            sidechain_hpf_order: "4th".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(48000).unwrap();
    assert_eq!(p.sidechain_hpf_order_index, 1);
    // Biquads should be built for 4th order (2 sections)
    assert_eq!(p.sidechain_hpf_biquads.len(), 1);
    assert_eq!(p.sidechain_hpf_biquads[0].len(), 2);
}

#[test]
fn test_info_and_latency() {
    let p = GatePlugin::new(2, -20.0, 10.0, 1.0, 0.0, 50.0);
    assert_eq!(p.channels(), 2);
    let info = p.info();
    assert_eq!(info.name, "Gate");
    assert_eq!(p.latency_samples(), 0);

    let mut p2 = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 5.0,
        },
    );
    p2.initialize(48000).unwrap();
    assert!(p2.latency_samples() > 0);
}

// -------------------------------------------------------------------------
// process_in_place focused tests (lookahead, external SC, RMS, mix, hold)
// -------------------------------------------------------------------------

#[test]
fn test_process_in_place_rms_detection() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "rms".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    let mut quiet = vec![0.0001f32; sr as usize];
    p.process_in_place(&mut quiet, &ProcessContext::new(sr, sr as usize))
        .unwrap();
    let avg_quiet: f32 = quiet.iter().sum::<f32>() / quiet.len() as f32;
    assert!(
        avg_quiet < 0.00001,
        "RMS detection should gate quiet signal, avg={}",
        avg_quiet
    );
}

#[test]
fn test_process_in_place_lookahead_delays_output() {
    let sr = 48000u32;
    let lookahead_ms = 5.0;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms,
        },
    );
    p.initialize(sr).unwrap();

    let delay_samples = (lookahead_ms * 0.001 * sr as f32).round() as usize;
    let num_frames = delay_samples + 100;
    let mut buffer = vec![0.0f32; num_frames];
    buffer[0] = 1.0;

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    assert!(
        buffer[0].abs() < 0.001,
        "lookahead should delay output, got {} at sample 0",
        buffer[0]
    );
    let peak_idx = buffer
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .unwrap()
        .0;
    assert!(
        (peak_idx as i32 - delay_samples as i32).abs() <= 2,
        "lookahead peak should be at ~{}, got {}",
        delay_samples,
        peak_idx
    );
}

#[test]
fn test_process_in_place_external_sidechain() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: true,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    let num_frames = 1000;
    let mut buffer = vec![0.0f32; num_frames * 2];
    for i in 0..num_frames {
        buffer[i * 2] = 0.5; // loud audio
    }
    for i in 0..num_frames {
        buffer[i * 2 + 1] = 0.00001; // quiet sidechain
    }
    let original_sidechain: Vec<f32> = buffer.iter().skip(1).step_by(2).copied().collect();

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let avg_output: f32 = buffer.iter().step_by(2).sum::<f32>() / num_frames as f32;
    assert!(
        avg_output < 0.05,
        "external sidechain: quiet SC should close gate on loud audio, avg={}",
        avg_output
    );
    assert_eq!(
        buffer
            .iter()
            .skip(1)
            .step_by(2)
            .copied()
            .collect::<Vec<_>>(),
        original_sidechain,
        "external sidechain input must remain read-only"
    );
}

#[test]
fn test_process_in_place_mix_half() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 0.5,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    // from_params sets p.mix but does not update the mix smoother target.
    // Explicitly set the parameter so the smoother ramps to 0.5.
    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();

    // Warm-up: let the mix smoother settle (5ms time constant → ~50ms enough)
    let warmup = sr as usize / 10; // 100ms
    let mut buf = vec![0.001f32; warmup];
    p.process_in_place(&mut buf, &ProcessContext::new(sr, warmup))
        .unwrap();

    let num_frames = sr as usize / 10; // 100ms
    let input_level = 0.001f32;
    let mut buffer = vec![input_level; num_frames];
    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let avg_output: f32 = buffer.iter().sum::<f32>() / num_frames as f32;
    assert!(
        avg_output > input_level * 0.3 && avg_output < input_level * 0.7,
        "mix=0.5 should blend dry and wet, avg={} (input={})",
        avg_output,
        input_level
    );
}

#[test]
fn test_process_in_place_unlinked_dual_channel() {
    let sr = 48000u32;
    let mut p = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    let num_frames = sr as usize;
    let mut buffer: Vec<f32> = (0..num_frames).flat_map(|_| [0.5f32, 0.0001f32]).collect();

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let avg_ch0: f32 = buffer.iter().step_by(2).sum::<f32>() / num_frames as f32;
    let avg_ch1: f32 = buffer.iter().skip(1).step_by(2).sum::<f32>() / num_frames as f32;

    assert!(
        avg_ch0 > 0.4,
        "unlinked ch0 (loud) should pass, avg={}",
        avg_ch0
    );
    assert!(
        avg_ch1 < 0.00005,
        "unlinked ch1 (quiet) should be gated, avg={}",
        avg_ch1
    );
}

#[test]
fn test_process_in_place_hold_counter() {
    let sr = 48000u32;
    let hold_ms = 10.0;
    let mut p = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -20.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms,
            release_ms: 10.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    p.initialize(sr).unwrap();

    // Open gate with loud signal
    let mut loud = vec![0.5f32; sr as usize];
    p.process_in_place(&mut loud, &ProcessContext::new(sr, sr as usize))
        .unwrap();

    // Switch to quiet signal for exactly hold_samples frames
    let hold_samples = p.hold_samples;
    let input_level = 0.0001f32;
    let mut quiet = vec![input_level; hold_samples];
    p.process_in_place(&mut quiet, &ProcessContext::new(sr, hold_samples))
        .unwrap();

    let avg_during_hold: f32 = quiet.iter().sum::<f32>() / hold_samples as f32;
    assert!(
        avg_during_hold > input_level * 0.9,
        "output should pass during hold period, avg={}",
        avg_during_hold
    );
    assert_eq!(p.hold_counter[0], 0, "hold_counter should be exhausted");

    // After hold expires, process more quiet signal — gate should attenuate
    let extra = sr as usize / 10; // 100ms
    let mut quiet2 = vec![input_level; extra];
    p.process_in_place(&mut quiet2, &ProcessContext::new(sr, extra))
        .unwrap();
    let avg_after_hold: f32 = quiet2[extra / 2..].iter().sum::<f32>() / (extra / 2) as f32;
    assert!(
        avg_after_hold < input_level * 0.1,
        "output should be attenuated after hold expires, avg={}",
        avg_after_hold
    );
}

#[test]
fn process_requires_initialize_and_matching_sample_rate() {
    let mut gate = GatePlugin::new(1, -40.0, 10.0, 1.0, 0.0, 100.0);
    let mut buffer = vec![0.0; 16];
    let err = gate
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 16))
        .unwrap_err();
    assert!(err.contains("initialized"), "unexpected error: {err}");

    gate.initialize(48_000).unwrap();
    let err = gate
        .process_in_place(&mut buffer, &ProcessContext::new(44_100, 16))
        .unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn process_requires_exact_buffer_length() {
    let mut gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 0.0, 100.0);
    gate.initialize(48_000).unwrap();
    let mut oversized = vec![0.0; 9];
    let err = gate
        .process_in_place(&mut oversized, &ProcessContext::new(48_000, 4))
        .unwrap_err();
    assert!(err.contains("expected 8"), "unexpected error: {err}");

    let mut empty = [];
    let err = gate
        .process_in_place(&mut empty, &ProcessContext::new(48_000, usize::MAX))
        .unwrap_err();
    assert!(err.contains("overflow"), "unexpected error: {err}");
}

#[test]
fn non_finite_audio_and_sidechain_do_not_poison_state() {
    let mut gate = GatePlugin::try_from_params(
        1,
        GatePluginParams {
            sidechain_external: true,
            ..GatePluginParams::default()
        },
    )
    .unwrap();
    gate.initialize(48_000).unwrap();
    let mut poisoned = [f32::NAN, f32::INFINITY, 0.25, f32::NEG_INFINITY];
    gate.process_in_place(&mut poisoned, &ProcessContext::new(48_000, 2))
        .unwrap();
    assert!(poisoned[0].is_finite() && poisoned[2].is_finite());

    let mut recovery = vec![0.25; 512 * 2];
    for frame in recovery.chunks_exact_mut(2) {
        frame[1] = 0.25;
    }
    gate.process_in_place(&mut recovery, &ProcessContext::new(48_000, 512))
        .unwrap();
    assert!(recovery.iter().step_by(2).all(|sample| sample.is_finite()));
}

#[test]
fn structural_updates_allow_noop_but_reject_actual_change_transactionally() {
    let mut gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 0.0, 100.0);
    gate.initialize(48_000).unwrap();
    let noops = [
        ("link_channels", ParameterValue::Bool(true)),
        ("sidechain_hpf_hz", ParameterValue::Float(0.0)),
        ("sidechain_hpf_order", ParameterValue::Int(0)),
        ("detection_mode", ParameterValue::Int(0)),
        ("sidechain_external", ParameterValue::Bool(false)),
        ("lookahead_ms", ParameterValue::Float(0.0)),
    ];
    for (id, value) in noops {
        gate.parametric_set_parameter(ParameterId::from(id), value)
            .unwrap();
    }

    for (id, value) in [
        ("link_channels", ParameterValue::Bool(false)),
        ("sidechain_hpf_hz", ParameterValue::Float(100.0)),
        ("sidechain_hpf_order", ParameterValue::Int(1)),
        ("detection_mode", ParameterValue::Int(1)),
        ("sidechain_external", ParameterValue::Bool(true)),
        ("lookahead_ms", ParameterValue::Float(5.0)),
    ] {
        let before_channels = gate.input_channels();
        let before_latency = gate.latency_samples();
        assert!(
            gate.parametric_set_parameter(ParameterId::from(id), value)
                .is_err(),
            "{id} must require graph rebuild"
        );
        assert_eq!(gate.input_channels(), before_channels);
        assert_eq!(gate.latency_samples(), before_latency);
    }
}

#[test]
fn failed_batch_update_does_not_partially_mutate_state() {
    let mut gate = GatePlugin::new(1, -40.0, 10.0, 1.0, 0.0, 100.0);
    let mut values = ParameterSet::new();
    values.insert(ParameterId::from("threshold"), ParameterValue::Float(-20.0));
    values.insert(ParameterId::from("zz_unknown"), ParameterValue::Float(1.0));
    assert!(gate.apply_values(values).is_err());
    assert_eq!(gate.threshold_db, -40.0);
}

#[test]
fn lowering_hold_clamps_an_active_hold_counter() {
    let mut gate = GatePlugin::new(1, -40.0, 10.0, 1.0, 100.0, 100.0);
    gate.initialize(48_000).unwrap();
    gate.hold_counter[0] = gate.hold_samples;
    gate.parametric_set_parameter(ParameterId::from("hold"), ParameterValue::Float(1.0))
        .unwrap();
    assert!(gate.hold_counter[0] <= gate.hold_samples);
}

#[test]
fn held_diagnostic_snapshot_is_immutable_while_new_data_publishes() {
    let mut gate = GatePlugin::new(1, -20.0, 100.0, 1.0, 0.0, 10.0);
    gate.initialize(48_000).unwrap();
    let mut loud = vec![0.5; 2_000];
    gate.process_in_place(&mut loud, &ProcessContext::new(48_000, 2_000))
        .unwrap();
    let held = gate.get_data().unwrap();
    let held = held.downcast::<GateData>().unwrap();
    let held_level = held.input_levels_db[0];

    let mut quiet = vec![0.0001; 4_000];
    gate.process_in_place(&mut quiet, &ProcessContext::new(48_000, 4_000))
        .unwrap();
    let latest = gate.get_data().unwrap();
    let latest = latest.downcast::<GateData>().unwrap();
    assert_eq!(held.input_levels_db[0], held_level);
    assert!(latest.input_levels_db[0] < held_level - 20.0);
    assert!(latest.attenuation_db[0] > held.attenuation_db[0]);
}

#[test]
fn diagnostics_are_callback_partition_invariant() {
    fn run(block_size: usize) -> (usize, f32, f32) {
        let mut gate = GatePlugin::new(1, -20.0, 100.0, 1.0, 0.0, 10.0);
        gate.initialize(48_000).unwrap();
        let mut remaining = 48_000;
        while remaining > 0 {
            let frames = remaining.min(block_size);
            let mut block = vec![0.001; frames];
            gate.process_in_place(&mut block, &ProcessContext::new(48_000, frames))
                .unwrap();
            remaining -= frames;
        }
        let data = gate.get_data().unwrap();
        let data = data.downcast::<GateData>().unwrap();
        (
            gate.diagnostic_samples,
            data.input_levels_db[0],
            data.attenuation_db[0],
        )
    }

    let reference = run(32);
    for block_size in [127, 512, 4_096] {
        let actual = run(block_size);
        assert_eq!(actual.0, reference.0, "block_size={block_size}");
        assert!((actual.1 - reference.1).abs() < 1e-4);
        assert!((actual.2 - reference.2).abs() < 1e-3);
    }
}

#[test]
fn steady_state_audio_gain_obeys_range_contract() {
    fn output_gain(range_db: f32) -> f32 {
        let mut gate = GatePlugin::new(1, -20.0, 100.0, 0.1, 0.0, 10.0);
        gate.range_db = range_db;
        gate.initialize(48_000).unwrap();
        let input = 1.0e-8;
        let mut buffer = vec![input; 48_000];
        gate.process_in_place(&mut buffer, &ProcessContext::new(48_000, 48_000))
            .unwrap();
        buffer.last().copied().unwrap() / input
    }

    let unlimited = output_gain(0.0);
    let range_20 = output_gain(20.0);
    let range_80 = output_gain(80.0);
    let range_120 = output_gain(120.0);
    assert!(unlimited < range_120);
    assert!(range_120 < range_80);
    assert!(range_80 < range_20);
    assert!((range_20 - 0.1).abs() < 0.002);
}
