use sotf_audio_player_gpui::{
    room_eq_channel_chain_by_name, room_eq_display_response_points,
    room_eq_initial_response_points, should_render_filter_plot,
};

#[test]
fn filter_plot_shown_when_only_broadband_filters_exist() {
    assert!(
        should_render_filter_plot(
            true,  // has_response_data
            false, // has_main — empty
            true,  // has_broadband — present
        ),
        "Broadband-only optimizations must still render the filter plot — \
         this was Issue-6 problem 1: broadband filters invisible in the \
         Review graph."
    );
}

#[test]
fn filter_plot_shown_when_only_main_filters_exist() {
    assert!(should_render_filter_plot(true, true, false));
}

#[test]
fn filter_plot_shown_when_both_stages_exist() {
    assert!(should_render_filter_plot(true, true, true));
}

#[test]
fn filter_plot_hidden_when_no_filters_at_all() {
    assert!(
        !should_render_filter_plot(true, false, false),
        "No filters of any kind -> do not render the plot"
    );
}

#[test]
fn filter_plot_hidden_when_no_response_data() {
    assert!(
        !should_render_filter_plot(false, true, true),
        "Without response data the plot has nothing to overlay against — \
         the predicate must block even if filters exist"
    );
}

#[test]
fn review_display_curve_prefers_dsp_chain_final_curve() {
    let fallback = autoeq::Curve {
        freq: ndarray::Array1::from(vec![20.0, 80.0, 200.0]),
        spl: ndarray::Array1::from(vec![10.0, 10.0, 10.0]),
        ..Default::default()
    };
    let chain = autoeq::roomeq::ChannelDspChain {
        channel: "L".to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: None,
        final_curve: Some(autoeq::roomeq::CurveData {
            freq: vec![20.0, 80.0, 200.0],
            spl: vec![-80.0, -10.0, -8.0],
            phase: None,
            norm_range: None,
        }),
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
        direct_early_late_correction: None,
    };

    let points = room_eq_display_response_points(Some(&chain), Some(&fallback)).unwrap();

    assert_eq!(points, vec![(20.0, -80.0), (80.0, -10.0), (200.0, -8.0)]);
    assert!(
        points.iter().any(|(_, db)| *db < -50.0),
        "Review must use the exported DSP-chain final_curve, which carries \
         the bass-management crossover shape, instead of the full-range \
         optimizer fallback curve."
    );
}

#[test]
fn review_initial_curve_prefers_dsp_chain_initial_curve() {
    let fallback = autoeq::Curve {
        freq: ndarray::Array1::from(vec![20.0, 80.0, 200.0]),
        spl: ndarray::Array1::from(vec![5.0, 5.0, 5.0]),
        ..Default::default()
    };
    let chain = autoeq::roomeq::ChannelDspChain {
        channel: "L".to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: Some(autoeq::roomeq::CurveData {
            freq: vec![20.0, 80.0, 200.0],
            spl: vec![-60.0, -15.0, -9.0],
            phase: None,
            norm_range: None,
        }),
        final_curve: None,
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
        direct_early_late_correction: None,
    };

    let points = room_eq_initial_response_points(Some(&chain), Some(&fallback)).unwrap();

    assert_eq!(points, vec![(20.0, -60.0), (80.0, -15.0), (200.0, -9.0)]);
}

#[test]
fn review_display_curve_falls_back_to_optimizer_curve_when_dsp_curve_missing() {
    let fallback = autoeq::Curve {
        freq: ndarray::Array1::from(vec![20.0, 80.0, 200.0]),
        spl: ndarray::Array1::from(vec![1.0, 2.0, 3.0]),
        ..Default::default()
    };

    assert_eq!(
        room_eq_display_response_points(None, Some(&fallback)).unwrap(),
        vec![(20.0, 1.0), (80.0, 2.0), (200.0, 3.0)]
    );
}

#[test]
fn review_display_curve_finds_chain_by_embedded_channel_name() {
    let chain = autoeq::roomeq::ChannelDspChain {
        channel: "R".to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: None,
        final_curve: Some(autoeq::roomeq::CurveData {
            freq: vec![20.0, 100.0],
            spl: vec![-70.0, -12.0],
            phase: None,
            norm_range: None,
        }),
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
        direct_early_late_correction: None,
    };
    let mut channels = std::collections::HashMap::new();
    channels.insert("Front Right".to_string(), chain);

    let chain = room_eq_channel_chain_by_name(&channels, "R").unwrap();
    let points = room_eq_display_response_points(Some(chain), None).unwrap();

    assert_eq!(points, vec![(20.0, -70.0), (100.0, -12.0)]);
}
