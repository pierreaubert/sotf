// Integration tests for sotf-plugin-limiter — exercises the public API only.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_limiter::{LimiterData, LimiterPlugin, LimiterPluginParams};

fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

#[test]
fn info_and_channels_match_construction() {
    let plugin = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    assert_eq!(plugin.channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "Limiter");
    assert_eq!(info.version, "1.3.0");
}

#[test]
fn initialize_changes_sample_rate_and_latency() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();
    assert_eq!(plugin.latency_samples(), 240);

    plugin.initialize(96000).unwrap();
    assert_eq!(plugin.latency_samples(), 480);
}

#[test]
fn parameter_roundtrip() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, ParameterValue)] = &[
        ("threshold", ParameterValue::Float(-12.0)),
        ("release", ParameterValue::Float(100.0)),
        ("lookahead", ParameterValue::Float(10.0)),
        ("soft", ParameterValue::Bool(true)),
        ("true_peak", ParameterValue::Bool(true)),
        ("isp_mode", ParameterValue::Bool(true)),
        ("dual_release", ParameterValue::Bool(true)),
        ("mix", ParameterValue::Float(0.75)),
        ("feed_forward", ParameterValue::Bool(true)),
        ("link_amount", ParameterValue::Float(0.5)),
    ];

    for &(id, ref value) in cases {
        plugin
            .set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(value.clone()),
            "roundtrip failed for parameter {}",
            id
        );
    }
}

#[test]
fn invalid_parameter_rejected() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();

    // Out of range.
    assert!(
        plugin
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(5.0))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.1))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(25.0))
            .is_err()
    );
    // NaN / infinity.
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("threshold"),
                ParameterValue::Float(f32::NAN)
            )
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("release"),
                ParameterValue::Float(f32::INFINITY)
            )
            .is_err()
    );
    // Unknown parameter.
    assert!(
        plugin
            .set_parameter(ParameterId::from("unknown"), ParameterValue::Float(1.0))
            .is_err()
    );

    assert!(
        plugin
            .get_parameter(&ParameterId::from("unknown"))
            .is_none()
    );
}

#[test]
fn process_zero_frames_returns_zero() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();
    let mut buffer = [0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 0);
}

#[test]
fn reset_clears_state() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(sr).unwrap();

    let mut buf = vec![0.9f32; 1024];
    let ctx = ProcessContext::new(sr, 1024);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    plugin.reset();

    // After reset, silence should remain silence.
    let mut silence = vec![0.0f32; 1024];
    plugin.process_in_place(&mut silence, &ctx).unwrap();
    for &s in &silence {
        assert_eq!(s, 0.0);
    }
}

#[test]
fn limiter_clamps_loud_signal() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(sr).unwrap();

    let num_frames = 4096;
    let mut buf = vec![1.0f32; num_frames];
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-6.0);
    for &s in &buf[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.05,
            "output {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

#[test]
fn soft_knee_respects_ceiling() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, true);
    plugin.initialize(sr).unwrap();

    let num_frames = 4096;
    let mut buf = vec![0.0f32; num_frames];
    for (i, sample) in buf.iter_mut().enumerate() {
        *sample = 0.9 * (i as f32 * 0.1).sin();
    }

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-6.0);
    for &s in &buf[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.1,
            "soft knee: output {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

#[test]
fn mix_zero_is_dry_passthrough() {
    let sr = 48000u32;
    let num_frames = 4096;

    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    plugin.initialize(sr).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    // Warm up the mix smoother.
    let mut warmup = vec![0.0f32; 4800];
    let warmup_ctx = ProcessContext::new(sr, warmup.len());
    plugin.process_in_place(&mut warmup, &warmup_ctx).unwrap();

    let input = make_sine(440.0, sr, num_frames, 0.5);
    let mut buf = input.clone();
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // With lookahead=0 there is still a 1-sample delay.
    let max_error = (1..num_frames)
        .map(|i| (buf[i] - input[i - 1]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-4,
        "mix=0 should pass dry signal through (delayed by 1), max_error={max_error}"
    );
}

#[test]
fn true_peak_detection_limits_inter_sample_peaks() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin
        .set_parameter(ParameterId::from("true_peak"), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(sr).unwrap();

    let frames = 2048;
    let mut buf = vec![0.0f32; frames];
    for (i, sample) in buf.iter_mut().enumerate() {
        *sample = if i % 2 == 0 { 0.8 } else { -0.8 };
    }

    let ctx = ProcessContext::new(sr, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-6.0);
    for &s in &buf[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.15,
            "true peak: output {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

#[test]
fn isp_mode_limits_output_true_peaks() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -3.0, 50.0, 5.0, false);
    plugin
        .set_parameter(ParameterId::from("isp_mode"), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(sr).unwrap();

    let frames = 8192;
    let mut buf = vec![0.0f32; frames];
    for (i, sample) in buf.iter_mut().enumerate() {
        *sample = 0.65 * (2.0 * std::f32::consts::PI * 12000.0 * i as f32 / sr as f32).sin();
    }

    let ctx = ProcessContext::new(sr, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-3.0);
    for &s in &buf[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.05,
            "ISP mode: output {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

#[test]
fn dual_release_limits_normally() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin
        .set_parameter(
            ParameterId::from("dual_release"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin.initialize(sr).unwrap();

    let frames = 4096;
    let mut buf = vec![0.0f32; frames];
    for (i, sample) in buf.iter_mut().enumerate() {
        *sample = 0.9 * (i as f32 * 0.1).sin();
    }

    let ctx = ProcessContext::new(sr, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-6.0);
    for &s in &buf[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.1,
            "dual release: output {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

#[test]
fn feed_forward_pre_empts_transient() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
    plugin.initialize(sr).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("feed_forward"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let mut buffer = vec![0.0f32; 512 * 2];
    for i in 0..512 {
        let amp = if i == 200 { 2.0 } else { 0.1 };
        buffer[i * 2] = amp;
        buffer[i * 2 + 1] = amp;
    }

    let context = ProcessContext::new(sr, 512);
    plugin.process_in_place(&mut buffer, &context).unwrap();

    let max_out = buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_out < 1.0,
        "feed-forward should pre-emptively limit transient, max_out={}",
        max_out
    );
}

#[test]
fn link_amount_zero_preserves_independence() {
    let sr = 48000u32;
    let mut plugin = LimiterPlugin::new(2, -6.0, 50.0, 0.0, false);
    plugin.initialize(sr).unwrap();
    plugin
        .set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(0.0))
        .unwrap();

    let frames = 1024;
    let mut buf = vec![0.0f32; frames * 2];
    for frame in 0..frames {
        buf[frame * 2] = 0.9;
        buf[frame * 2 + 1] = 0.1;
    }

    let ctx = ProcessContext::new(sr, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = db_to_linear(-6.0);
    let ch0_max = buf[500..]
        .iter()
        .step_by(2)
        .map(|&s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        ch0_max <= thresh_lin * 1.1,
        "ch0 should be limited, max={ch0_max}"
    );

    let ch1_max = buf[500..]
        .iter()
        .skip(1)
        .step_by(2)
        .map(|&s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        (ch1_max - 0.1).abs() < 1e-4,
        "ch1 should pass through unchanged, max={ch1_max}"
    );
}

#[test]
fn get_data_returns_typed_cache() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();

    let data = plugin.get_data();
    assert!(data.is_some());
    assert!(data.unwrap().is::<LimiterData>());
}

#[test]
fn from_params_wires_all_fields() {
    let params = LimiterPluginParams {
        threshold_db: -3.0,
        release_ms: 100.0,
        lookahead_ms: 10.0,
        soft: true,
        true_peak: true,
        isp_mode: true,
        dual_release: true,
        mix: 0.8,
        feed_forward: true,
        link_amount: 0.75,
    };
    let plugin = LimiterPlugin::from_params(2, params);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("true_peak")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("isp_mode")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dual_release")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("feed_forward")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.8))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("link_amount")),
        Some(ParameterValue::Float(0.75))
    );
}

#[test]
fn parameters_list_contains_expected_ids() {
    let plugin = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&ParameterId::from("threshold")));
    assert!(ids.contains(&ParameterId::from("release")));
    assert!(ids.contains(&ParameterId::from("lookahead")));
    assert!(ids.contains(&ParameterId::from("soft")));
    assert!(ids.contains(&ParameterId::from("true_peak")));
    assert!(ids.contains(&ParameterId::from("isp_mode")));
    assert!(ids.contains(&ParameterId::from("dual_release")));
    assert!(ids.contains(&ParameterId::from("mix")));
    assert!(ids.contains(&ParameterId::from("feed_forward")));
    assert!(ids.contains(&ParameterId::from("link_amount")));
}

#[test]
fn lookahead_parameter_change_uses_preallocated_storage() {
    let mut plugin = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    plugin.initialize(48000).unwrap();

    let initial_latency = plugin.latency_samples();
    assert_eq!(initial_latency, 240);

    plugin
        .set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(20.0))
        .unwrap();
    assert_eq!(plugin.latency_samples(), 960);

    plugin
        .set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(1.0))
        .unwrap();
    assert_eq!(plugin.latency_samples(), 48);
}
