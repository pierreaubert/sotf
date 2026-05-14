use sotf_audio_player_gpui::{
    calculate_room_eq_log_trend, room_eq_channel_sort_key, room_eq_passband_trend_fit_domain,
    room_eq_progress_chart_series, room_eq_report_channel_has_renderable_data,
    room_eq_report_data_from_dsp_output, room_eq_report_eq_y_range, room_eq_report_y_range,
    room_eq_trend_fit_domain, sum_room_eq_responses_db,
};
use std::collections::HashMap;

#[test]
fn subwoofer_trend_uses_reduced_minus_3db_passband() {
    let freqs = vec![
        20.0, 30.0, 40.0, 50.0, 60.0, 80.0, 100.0, 140.0, 180.0, 240.0, 320.0, 500.0,
    ];
    let values = vec![
        -20.0, -12.0, -4.0, -1.0, 0.5, 1.0, 0.8, 0.4, -0.2, -3.2, -12.0, -24.0,
    ];

    let domain =
        room_eq_passband_trend_fit_domain(&freqs, &values).expect("sub passband trend domain");
    assert!(
        domain.0 > 40.0 && domain.0 < 80.0,
        "lower trend bound should move inside the -3 dB point, got {}",
        domain.0
    );
    assert!(
        domain.1 > 140.0 && domain.1 < 240.0,
        "upper trend bound should move inside the -3 dB point, got {}",
        domain.1
    );

    let (slope, _) = calculate_room_eq_log_trend(&freqs, &values, domain).expect("sub trend fit");
    assert!(
        slope.abs() < 3.0,
        "rolloff outside the passband should not dominate sub trend, got {slope}"
    );
}

#[test]
fn main_channel_trend_keeps_full_range_analysis_band() {
    let freqs = vec![20.0, 100.0, 1000.0, 10_000.0, 20_000.0];
    let domain = room_eq_trend_fit_domain("L", &freqs).expect("main trend domain");
    assert_eq!(domain, (100.0, 10_000.0));
}

#[test]
fn room_eq_sum_uses_phase_when_available() {
    let main = vec![(80.0, 0.0)];
    let sub = vec![(80.0, 0.0)];
    let main_phase = vec![(80.0, 0.0)];
    let sub_phase = vec![(80.0, 180.0)];

    let sum = sum_room_eq_responses_db(&main, &sub, Some(&main_phase), Some(&sub_phase));
    assert!(
        sum[0].1 < -200.0,
        "opposite-phase equal-level responses should cancel, got {} dB",
        sum[0].1
    );
}

#[test]
fn room_eq_report_uses_dsp_output_curves_without_recomputing() {
    let mut channels = HashMap::new();
    channels.insert(
        "R".to_string(),
        make_report_channel("R", vec![-2.0, -28.0, -8.0], vec![-1.0, -27.0, -7.0]),
    );
    channels.insert(
        "LFE".to_string(),
        make_report_channel("LFE", vec![-20.0, -5.0, -18.0], vec![-18.0, -4.0, -20.0]),
    );
    channels.insert(
        "L".to_string(),
        make_report_channel("L", vec![-1.0, -30.0, -9.0], vec![-2.0, -31.0, -10.0]),
    );
    let output = autoeq::roomeq::DspChainOutput {
        version: "test".to_string(),
        global_plugins: Vec::new(),
        channels,
        metadata: None,
    };

    let report = room_eq_report_data_from_dsp_output(&output);

    let names: Vec<_> = report
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(names, vec!["L", "R", "LFE"]);
    let left = &report.channels[0];
    assert_eq!(
        left.initial_curve.as_ref().expect("initial").spl,
        vec![-1.0, -30.0, -9.0]
    );
    assert_eq!(
        left.final_curve.as_ref().expect("final").spl,
        vec![-2.0, -31.0, -10.0]
    );
    assert_eq!(
        left.eq_response.as_ref().expect("eq").spl,
        vec![0.0, -3.0, 1.0]
    );
}

#[test]
fn room_eq_report_helpers_match_python_display_rules() {
    assert!(room_eq_channel_sort_key("L [mic 1]") < room_eq_channel_sort_key("R"));
    assert!(room_eq_channel_sort_key("LFE") > room_eq_channel_sort_key("C"));

    let spl_curve = sotf_audio_player_gpui::RoomEqReportCurve {
        freq: vec![20.0, 100.0, 1000.0],
        spl: vec![-25.0, -8.0, 12.0],
        phase: None,
    };
    assert_eq!(room_eq_report_y_range([Some(&spl_curve)]), (-35.0, 15.0));

    let eq_curve = sotf_audio_player_gpui::RoomEqReportCurve {
        freq: vec![20.0, 100.0, 1000.0],
        spl: vec![-4.0, 8.0, 2.0],
        phase: None,
    };
    assert_eq!(room_eq_report_eq_y_range([Some(&eq_curve)]), (-10.0, 15.0));
}

#[test]
fn room_eq_report_channel_without_embedded_data_uses_legacy_fallback_guard() {
    let mut channels = HashMap::new();
    channels.insert("L".to_string(), make_empty_report_channel("L"));
    channels.insert(
        "R".to_string(),
        make_report_channel("R", vec![-2.0, -28.0, -8.0], vec![-1.0, -27.0, -7.0]),
    );
    let output = autoeq::roomeq::DspChainOutput {
        version: "test".to_string(),
        global_plugins: Vec::new(),
        channels,
        metadata: None,
    };

    let report = room_eq_report_data_from_dsp_output(&output);
    let left = report
        .channels
        .iter()
        .find(|channel| channel.name == "L")
        .expect("left channel");
    let right = report
        .channels
        .iter()
        .find(|channel| channel.name == "R")
        .expect("right channel");

    assert!(
        !room_eq_report_channel_has_renderable_data(left),
        "channels with no embedded report curves should fall back to legacy results"
    );
    assert!(room_eq_report_channel_has_renderable_data(right));
}

#[test]
fn room_eq_progress_chart_splits_channel_when_iteration_resets() {
    let history = vec![
        (1, 4.0, "R".to_string(), Some(5.0)),
        (2, 3.0, "R".to_string(), Some(4.8)),
        (1, 2.5, "R".to_string(), Some(4.7)),
        (2, 2.0, "R".to_string(), Some(4.6)),
        (1, 3.5, "L".to_string(), None),
    ];

    let (channels, series, losses) = room_eq_progress_chart_series(&history);

    assert_eq!(channels, vec!["R".to_string(), "L".to_string()]);
    assert_eq!(losses, vec![4.0, 3.0, 2.5, 2.0, 3.5]);
    assert_eq!(series.len(), 3);
    assert_eq!(series[0].channel, "R");
    assert_eq!(series[0].pass, 1);
    assert_eq!(series[0].iterations, vec![1.0, 2.0]);
    assert_eq!(series[1].channel, "R");
    assert_eq!(series[1].pass, 2);
    assert_eq!(series[1].iterations, vec![1.0, 2.0]);
    assert_eq!(series[2].channel, "L");
    assert_eq!(series[2].pass, 1);
}

fn make_report_channel(
    name: &str,
    initial_spl: Vec<f64>,
    final_spl: Vec<f64>,
) -> autoeq::roomeq::ChannelDspChain {
    autoeq::roomeq::ChannelDspChain {
        channel: name.to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: Some(curve_data(initial_spl)),
        final_curve: Some(curve_data(final_spl)),
        eq_response: Some(curve_data(vec![0.0, -3.0, 1.0])),
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
    }
}

fn make_empty_report_channel(name: &str) -> autoeq::roomeq::ChannelDspChain {
    autoeq::roomeq::ChannelDspChain {
        channel: name.to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: None,
        final_curve: None,
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
    }
}

fn curve_data(spl: Vec<f64>) -> autoeq::roomeq::CurveData {
    autoeq::roomeq::CurveData {
        freq: vec![20.0, 50.0, 100.0],
        spl,
        phase: None,
        norm_range: None,
    }
}
