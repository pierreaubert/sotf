use super::*;
use math_audio_iir_fir::BiquadFilterType;
use ndarray::array;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn test_chain(channel: &str, final_curve: &Curve) -> ChannelDspChain {
    ChannelDspChain {
        channel: channel.to_string(),
        plugins: Vec::new(),
        drivers: None,
        initial_curve: None,
        final_curve: Some(final_curve.into()),
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
    }
}

#[test]
fn phase_alignment_reporting_adjusts_phase_without_touching_magnitude() {
    let mut curve = Curve {
        freq: array![50.0, 100.0],
        spl: array![1.0, 2.0],
        phase: Some(array![10.0, -20.0]),
        ..Default::default()
    };

    apply_phase_only_adjustment_to_reported_curve(&mut curve, 2.0, true);

    assert_eq!(curve.spl, array![1.0, 2.0]);
    let phase = curve.phase.as_ref().unwrap();
    assert_close(phase[0], 154.0);
    assert_close(phase[1], 88.0);
}

#[test]
fn phase_alignment_reporting_creates_phase_when_missing() {
    let mut curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: None,
        ..Default::default()
    };

    apply_phase_only_adjustment_to_reported_curve(&mut curve, 1.0, false);

    let phase = curve.phase.as_ref().unwrap();
    assert_close(phase[0], -36.0);
}

#[test]
fn reported_phase_sync_updates_channel_result_and_chain_curve() {
    let curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

    sync_reported_phase_adjustment("L", &mut channel_results, &mut channel_chains, 1.0, true);

    let result_phase = channel_results["L"].final_curve.phase.as_ref().unwrap()[0];
    let chain_phase = channel_chains["L"]
        .final_curve
        .as_ref()
        .unwrap()
        .phase
        .as_ref()
        .unwrap()[0];
    assert_close(result_phase, 144.0);
    assert_close(chain_phase, 144.0);
}

#[test]
fn reported_gain_sync_updates_result_and_chain_magnitude() {
    let curve = Curve {
        freq: array![100.0],
        spl: array![1.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

    sync_reported_gain_adjustment("L", &mut channel_results, &mut channel_chains, 2.5, true);

    assert_close(channel_results["L"].final_curve.spl[0], 3.5);
    assert_close(
        channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
        3.5,
    );
    assert_close(
        channel_results["L"].final_curve.phase.as_ref().unwrap()[0],
        180.0,
    );
}

#[test]
fn reported_biquad_sync_updates_result_and_chain_curve() {
    let curve = Curve {
        freq: array![1000.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);
    let filters = vec![Biquad::new(
        BiquadFilterType::Peak,
        1000.0,
        48000.0,
        1.0,
        6.0,
    )];

    sync_reported_biquad_adjustment(
        "L",
        &mut channel_results,
        &mut channel_chains,
        &filters,
        48000.0,
    );

    assert!(
        channel_results["L"].final_curve.spl[0] > 5.5,
        "expected PEQ boost in reported result"
    );
    assert_close(
        channel_results["L"].final_curve.spl[0],
        channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
    );
}

#[test]
fn reported_fir_sync_updates_result_and_chain_curve() {
    let curve = Curve {
        freq: array![1000.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);

    sync_reported_fir_adjustment(
        "L",
        &mut channel_results,
        &mut channel_chains,
        &[1.0, -1.0],
        48000.0,
    );

    assert!(
        channel_results["L"].final_curve.spl[0] < -17.0,
        "expected differentiator FIR attenuation at 1 kHz"
    );
    assert_close(
        channel_results["L"].final_curve.spl[0],
        channel_chains["L"].final_curve.as_ref().unwrap().spl[0],
    );
}

#[test]
fn perceptual_metrics_report_epa_and_timing_confidence() {
    let score = |preference| crate::loss::epa::score::EpaScore {
        evaluation: 0.0,
        potency: 0.0,
        activity: 0.0,
        preference,
        sharpness_acum: 0.0,
        roughness: 0.0,
        total_loudness_sone: 0.0,
        loudness_balance: 0.0,
    };
    let group_delay = super::super::gd_opt::GroupDelayOptSummary {
        band: (20.0, 120.0),
        channel_names: Vec::new(),
        per_channel_delay_ms: Vec::new(),
        per_channel_polarity_inverted: Vec::new(),
        per_channel_ap_count: Vec::new(),
        sum_gd_pre_rms_ms: 1.0,
        sum_gd_post_rms_ms: 0.5,
        mean_coherence: 1.0,
        improvement_db: 0.0,
        advisory: "success".to_string(),
        applied: true,
    };
    let mut metadata = OptimizationMetadata {
        pre_score: 0.0,
        post_score: 0.0,
        algorithm: "test".to_string(),
        loss_type: Some("flat".to_string()),
        iterations: 0,
        timestamp: "test".to_string(),
        inter_channel_deviation: Some(super::super::types::InterChannelDeviation {
            deviation_per_freq: Vec::new(),
            midrange_rms_db: 1.25,
            passband_rms_db: 2.0,
            midrange_peak_db: 3.0,
            midrange_peak_freq: 1000.0,
        }),
        epa_per_channel: Some(HashMap::from([
            (
                "L".to_string(),
                super::super::types::EpaChannelMetrics {
                    pre: score(4.0),
                    post: score(6.0),
                },
            ),
            (
                "R".to_string(),
                super::super::types::EpaChannelMetrics {
                    pre: score(5.0),
                    post: score(7.0),
                },
            ),
        ])),
        group_delay: Some(group_delay),
        perceptual_metrics: None,
        home_cinema_layout: None,
        multi_seat_coverage: None,
        multi_seat_correction: None,
        bass_management: None,
        timing_diagnostics: None,
    };

    update_perceptual_metrics(&mut metadata, None, None);

    let metrics = metadata.perceptual_metrics.expect("metrics");
    assert_close(metrics.epa_preference_pre, 4.5);
    assert_close(metrics.epa_preference_post, 6.5);
    assert_close(metrics.epa_preference_delta, 2.0);
    assert_close(metrics.channel_matching_midrange_rms_db.unwrap(), 1.25);
    assert_eq!(metrics.timing_confidence.as_deref(), Some("gd_applied"));
}

#[test]
fn perceptual_metrics_report_role_bass_dialog_and_headroom_guards() {
    let score = |preference| crate::loss::epa::score::EpaScore {
        evaluation: 0.0,
        potency: 0.0,
        activity: 0.0,
        preference,
        sharpness_acum: 0.0,
        roughness: 0.0,
        total_loudness_sone: 0.0,
        loudness_balance: 0.0,
    };
    let freqs = array![40.0, 80.0, 300.0, 1000.0, 4000.0];
    let left = Curve {
        freq: freqs.clone(),
        spl: array![0.0, 0.0, 0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let right = Curve {
        freq: freqs.clone(),
        spl: array![0.0, 0.0, 0.0, 2.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let center = Curve {
        freq: freqs.clone(),
        spl: array![0.0, 0.0, -1.0, 1.0, -1.0],
        phase: None,
        ..Default::default()
    };
    let sub = Curve {
        freq: freqs.clone(),
        spl: array![1.0, 1.0, 0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let lfe = Curve {
        freq: freqs,
        spl: array![-1.0, -1.0, 0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let mut boosted_left = test_chain("L", &left);
    boosted_left.plugins.push(output::create_gain_plugin(5.0));
    let channels = HashMap::from([
        ("L".to_string(), boosted_left),
        ("R".to_string(), test_chain("R", &right)),
        ("C".to_string(), test_chain("C", &center)),
        ("Sub".to_string(), test_chain("Sub", &sub)),
        ("LFE".to_string(), test_chain("LFE", &lfe)),
    ]);
    let mut metadata = OptimizationMetadata {
        pre_score: 0.0,
        post_score: 0.0,
        algorithm: "test".to_string(),
        loss_type: Some("flat".to_string()),
        iterations: 0,
        timestamp: "test".to_string(),
        inter_channel_deviation: None,
        epa_per_channel: Some(HashMap::from([(
            "L".to_string(),
            super::super::types::EpaChannelMetrics {
                pre: score(1.0),
                post: score(2.0),
            },
        )])),
        group_delay: None,
        perceptual_metrics: None,
        home_cinema_layout: None,
        multi_seat_coverage: None,
        multi_seat_correction: None,
        bass_management: None,
        timing_diagnostics: None,
    };

    update_perceptual_metrics(&mut metadata, Some(&channels), None);

    let metrics = metadata.perceptual_metrics.expect("metrics");
    assert!(metrics.role_channel_matching_rms_db.unwrap() > 0.0);
    assert_close(metrics.bass_consistency_rms_db.unwrap(), 1.0);
    assert!(metrics.dialog_band_roughness_rms_db.unwrap() > 0.8);
    assert_close(metrics.headroom_peak_boost_db.unwrap(), 5.0);
    assert_eq!(
        metrics.headroom_risk.as_deref(),
        Some("moderate_boost_uses_headroom")
    );
}

#[test]
fn role_channel_matching_metric_skips_invalid_groups() {
    let valid_l = Curve {
        freq: array![300.0, 1000.0, 4000.0],
        spl: array![0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let valid_r = Curve {
        freq: array![300.0, 1000.0, 4000.0],
        spl: array![0.0, 2.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let invalid_sl = Curve {
        freq: array![300.0, 1000.0, 4000.0],
        spl: array![0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let invalid_sr = Curve {
        freq: array![301.0, 1000.0, 4000.0],
        spl: array![0.0, 0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let channels = HashMap::from([
        ("L".to_string(), test_chain("L", &valid_l)),
        ("R".to_string(), test_chain("R", &valid_r)),
        ("SL".to_string(), test_chain("SL", &invalid_sl)),
        ("SR".to_string(), test_chain("SR", &invalid_sr)),
    ]);

    let rms = role_channel_matching_rms_db(&channels).expect("valid L/R group");

    assert!(rms > 0.0);
}

#[test]
fn timing_diagnostics_reflect_measured_arrivals_and_exported_delays() {
    let curve = Curve {
        freq: array![100.0, 1000.0],
        spl: array![0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let mut left = test_chain("L", &curve);
    left.plugins.push(output::create_delay_plugin(2.0));
    let channels = HashMap::from([
        ("L".to_string(), left),
        ("R".to_string(), test_chain("R", &curve)),
        ("SL".to_string(), test_chain("SL", &curve)),
    ]);
    let arrivals = HashMap::from([
        ("L".to_string(), 8.0),
        ("R".to_string(), 10.0),
        ("SL".to_string(), 7.0),
    ]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: None,
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let report = build_timing_diagnostics(&config, &arrivals, &channels).expect("timing");
    assert_eq!(report.reference_channel.as_deref(), Some("L"));
    assert_close(report.reference_arrival_ms.unwrap(), 10.0);
    assert_close(report.arrival_spread_before_ms, 3.0);
    assert_close(report.arrival_spread_after_ms, 3.0);
    let left_report = report
        .channels
        .iter()
        .find(|channel| channel.name == "L")
        .unwrap();
    assert_close(left_report.measured_arrival_ms, 8.0);
    assert_close(left_report.applied_delay_ms, 2.0);
    assert_close(left_report.final_arrival_ms, 10.0);
    assert!(
        report
            .advisories
            .contains(&"surround_or_height_precedence_risk".to_string())
    );
}

#[test]
fn total_chain_delay_sums_stacked_delay_plugins() {
    let curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: None,
        ..Default::default()
    };
    let mut chain = test_chain("L", &curve);
    chain.plugins.push(output::create_delay_plugin(2.0));
    chain.plugins.push(output::create_gain_plugin(1.5));
    chain.plugins.push(output::create_delay_plugin(3.5));

    assert_close(total_chain_delay_ms(&chain), 5.5);
}

#[test]
fn channel_matching_role_key_groups_like_roles_only() {
    assert_eq!(channel_matching_role_key("L"), Some("front_lr"));
    assert_eq!(channel_matching_role_key("front-right"), Some("front_lr"));
    assert_eq!(channel_matching_role_key("C"), None);
    assert_eq!(channel_matching_role_key("SL"), Some("side_surrounds"));
    assert_eq!(
        channel_matching_role_key("surround right"),
        Some("side_surrounds")
    );
    assert_eq!(channel_matching_role_key("TFL"), Some("top_front"));
    assert_eq!(
        channel_matching_role_key("rear.height.right"),
        Some("top_rear")
    );
}

#[test]
fn role_aware_channel_matching_groups_exclude_center() {
    let curve = Curve {
        freq: array![500.0, 1000.0],
        spl: array![0.0, 0.0],
        phase: None,
        ..Default::default()
    };
    let curves = HashMap::from([
        ("L".to_string(), curve.clone()),
        ("R".to_string(), curve.clone()),
        ("C".to_string(), curve.clone()),
        ("SL".to_string(), curve.clone()),
        ("SR".to_string(), curve),
    ]);

    let groups = role_aware_channel_matching_groups(&curves);
    let mut group_names: Vec<Vec<String>> = groups
        .into_iter()
        .map(|group| {
            let mut names: Vec<String> = group.keys().cloned().collect();
            names.sort();
            names
        })
        .collect();
    group_names.sort();

    assert_eq!(
        group_names,
        vec![
            vec!["L".to_string(), "R".to_string()],
            vec!["SL".to_string(), "SR".to_string()],
        ]
    );
}

#[test]
fn phase_alignment_delay_schedule_preserves_shared_sub_constraints() {
    let phase_results = HashMap::from([
        ("L".to_string(), (-5.0, false, "LFE".to_string())),
        ("R".to_string(), (2.0, false, "LFE".to_string())),
    ]);

    let schedule = compute_phase_alignment_delay_schedule(&phase_results);

    assert_close(*schedule.get("L").unwrap_or(&0.0), 0.0);
    assert_close(schedule["LFE"], 5.0);
    assert_close(schedule["R"], 7.0);
    assert_close(
        schedule.get("L").unwrap_or(&0.0) - schedule.get("LFE").unwrap_or(&0.0),
        -5.0,
    );
    assert_close(schedule["R"] - schedule["LFE"], 2.0);
}

#[test]
fn phase_alignment_delay_schedule_normalizes_independent_components() {
    let phase_results = HashMap::from([
        ("L".to_string(), (-3.0, false, "SubL".to_string())),
        ("R".to_string(), (4.0, false, "SubR".to_string())),
    ]);

    let schedule = compute_phase_alignment_delay_schedule(&phase_results);

    assert_close(*schedule.get("L").unwrap_or(&0.0), 0.0);
    assert_close(schedule["SubL"], 3.0);
    assert_close(*schedule.get("SubR").unwrap_or(&0.0), 0.0);
    assert_close(schedule["R"], 4.0);
}

#[test]
fn negative_phase_alignment_delay_applies_sub_delay_and_reported_phase() {
    let main_curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let sub_curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([
        (
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: main_curve.clone(),
                final_curve: main_curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        ),
        (
            "LFE".to_string(),
            ChannelOptimizationResult {
                name: "LFE".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: sub_curve.clone(),
                final_curve: sub_curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        ),
    ]);
    let mut channel_chains = HashMap::from([
        ("L".to_string(), test_chain("L", &main_curve)),
        ("LFE".to_string(), test_chain("LFE", &sub_curve)),
    ]);
    let phase_results = HashMap::from([("L".to_string(), (-5.0, false, "LFE".to_string()))]);

    let applied = apply_phase_alignment_delay_schedule(
        &phase_results,
        &mut channel_results,
        &mut channel_chains,
    );

    assert!(!applied.contains_key("L"));
    assert_close(applied["LFE"], 5.0);
    assert_close(total_chain_delay_ms(&channel_chains["LFE"]), 5.0);
    assert_eq!(total_chain_delay_ms(&channel_chains["L"]), 0.0);
    assert_close(
        channel_results["LFE"].final_curve.phase.as_ref().unwrap()[0],
        -180.0,
    );
    assert_close(
        channel_chains["LFE"]
            .final_curve
            .as_ref()
            .unwrap()
            .phase
            .as_ref()
            .unwrap()[0],
        -180.0,
    );
}

#[test]
fn phase_alignment_reported_curve_collection_sees_applied_delay() {
    let main_curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let sub_curve = Curve {
        freq: array![100.0],
        spl: array![0.0],
        phase: Some(array![0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([
        (
            "L".to_string(),
            ChannelOptimizationResult {
                name: "L".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: main_curve.clone(),
                final_curve: main_curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        ),
        (
            "LFE".to_string(),
            ChannelOptimizationResult {
                name: "LFE".to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: sub_curve.clone(),
                final_curve: sub_curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
        ),
    ]);
    let mut channel_chains = HashMap::from([
        ("L".to_string(), test_chain("L", &main_curve)),
        ("LFE".to_string(), test_chain("LFE", &sub_curve)),
    ]);
    let phase_results = HashMap::from([("L".to_string(), (-2.5, false, "LFE".to_string()))]);

    apply_phase_alignment_delay_schedule(&phase_results, &mut channel_results, &mut channel_chains);
    let current_curves = collect_current_final_curves(&channel_results);

    assert_close(current_curves["LFE"].phase.as_ref().unwrap()[0], -90.0);
    assert_close(current_curves["L"].phase.as_ref().unwrap()[0], 0.0);
}

#[test]
fn gd_result_application_inserts_dsp_and_updates_reported_phase() {
    let curve = Curve {
        freq: array![100.0, 200.0],
        spl: array![0.0, 0.0],
        phase: Some(array![0.0, 0.0]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: curve.clone(),
            final_curve: curve.clone(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let mut channel_chains = HashMap::from([("L".to_string(), test_chain("L", &curve))]);
    let result = super::super::gd_opt::GroupDelayOptResult {
        band: (20.0, 160.0),
        per_channel: vec![super::super::gd_opt::ChannelGdResult {
            delay_ms: 1.0,
            polarity_inverted: true,
            ap_filters: vec![Biquad::new(
                BiquadFilterType::AllPass,
                80.0,
                48000.0,
                1.0,
                0.0,
            )],
            channel_gd_pre_rms_ms: 0.0,
            channel_gd_post_rms_ms: 0.0,
        }],
        sum_gd_pre_rms_ms: 1.0,
        sum_gd_post_rms_ms: 0.1,
        mean_coherence: 1.0,
        improvement_db: 20.0,
    };

    let applied = apply_gd_opt_result(
        &result,
        &["L".to_string()],
        &mut channel_results,
        &mut channel_chains,
        48000.0,
    );

    assert!(applied);
    let plugins = &channel_chains["L"].plugins;
    assert_eq!(plugins.len(), 3);
    assert_eq!(plugins[0].plugin_type, "gain");
    assert_eq!(plugins[1].plugin_type, "delay");
    assert_eq!(plugins[2].plugin_type, "eq");

    let result_phase = channel_results["L"].final_curve.phase.as_ref().unwrap()[0];
    let chain_phase = channel_chains["L"]
        .final_curve
        .as_ref()
        .unwrap()
        .phase
        .as_ref()
        .unwrap()[0];
    assert!(
        result_phase.abs() > 1.0,
        "GD DSP must be reflected in final_curve phase"
    );
    assert_close(result_phase, chain_phase);
}

#[test]
fn generic_path_score_band_uses_optimizer_bounds() {
    let mut config = test_room_config_with_gd(Default::default());
    config.system = None;
    config.optimizer.min_freq = 35.0;
    config.optimizer.max_freq = 12_500.0;

    assert_eq!(
        final_score_band_for_channel(&config, "left"),
        (35.0, 12_500.0)
    );
}

#[test]
fn gd_opt_rejects_same_length_mismatched_frequency_grids() {
    let curve_l = Curve {
        freq: array![20.0, 40.0, 80.0, 160.0],
        spl: array![0.0, 0.0, 0.0, 0.0],
        phase: Some(array![0.0, 0.0, 0.0, 0.0]),
        coherence: Some(array![0.95, 0.95, 0.95, 0.95]),
        ..Default::default()
    };
    let curve_r = Curve {
        freq: array![20.0, 41.0, 80.0, 160.0],
        spl: array![0.0, 0.0, 0.0, 0.0],
        phase: Some(array![0.0, 0.0, 0.0, 0.0]),
        coherence: Some(array![0.95, 0.95, 0.95, 0.95]),
        ..Default::default()
    };
    let mut channel_results = HashMap::from([
        ("L".to_string(), test_channel_result("L", &curve_l)),
        ("R".to_string(), test_channel_result("R", &curve_r)),
    ]);
    let mut channel_chains = HashMap::from([
        ("L".to_string(), test_chain("L", &curve_l)),
        ("R".to_string(), test_chain("R", &curve_r)),
    ]);
    let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
        enabled: true,
        ..Default::default()
    });

    let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
        .expect("GD summary should explain the skip");

    assert_eq!(summary.advisory, "frequency_grid_mismatch");
    assert!(!summary.applied);
    assert!(channel_chains["L"].plugins.is_empty());
    assert!(channel_chains["R"].plugins.is_empty());
}

#[test]
fn gd_opt_missing_coherence_downgrades_to_delay_only() {
    let curve_l = gd_test_curve(0.0, None);
    let curve_r = gd_test_curve(2.0, None);
    let mut channel_results = HashMap::from([
        ("L".to_string(), test_channel_result("L", &curve_l)),
        ("R".to_string(), test_channel_result("R", &curve_r)),
    ]);
    let mut channel_chains = HashMap::from([
        ("L".to_string(), test_chain("L", &curve_l)),
        ("R".to_string(), test_chain("R", &curve_r)),
    ]);
    let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
        enabled: true,
        min_improvement_db: -100.0,
        max_iter: 300,
        popsize: 10,
        adaptive_allpass: false,
        ap_per_channel: 2,
        optimize_polarity: true,
        ..Default::default()
    });

    let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
        .expect("GD summary");

    assert_eq!(summary.advisory, "missing_coherence_delay_only");
    assert!(summary.per_channel_ap_count.iter().all(|&n| n == 0));
    assert!(
        summary
            .per_channel_polarity_inverted
            .iter()
            .all(|&inverted| !inverted)
    );
}

#[test]
fn gd_opt_disables_allpass_without_bootstrap_realisations() {
    let coherence = Some(array![0.95, 0.95, 0.95, 0.95, 0.95, 0.95]);
    let curve_l = gd_test_curve(0.0, coherence.clone());
    let curve_r = gd_test_curve(2.0, coherence);
    let mut channel_results = HashMap::from([
        ("L".to_string(), test_channel_result("L", &curve_l)),
        ("R".to_string(), test_channel_result("R", &curve_r)),
    ]);
    let mut channel_chains = HashMap::from([
        ("L".to_string(), test_chain("L", &curve_l)),
        ("R".to_string(), test_chain("R", &curve_r)),
    ]);
    let config = test_room_config_with_gd(super::super::types::GroupDelayOptimizationConfig {
        enabled: true,
        min_improvement_db: -100.0,
        max_iter: 300,
        popsize: 10,
        adaptive_allpass: true,
        ap_per_channel: 2,
        ..Default::default()
    });

    let summary = try_run_gd_opt(&config, &mut channel_results, &mut channel_chains, 48000.0)
        .expect("GD summary");

    assert_eq!(
        summary.advisory,
        "allpass_disabled_no_bootstrap_realisations"
    );
    assert!(summary.per_channel_ap_count.iter().all(|&n| n == 0));
}

fn gd_test_curve(delay_ms: f64, coherence: Option<ndarray::Array1<f64>>) -> Curve {
    let freq = array![20.0, 40.0, 80.0, 120.0, 160.0, 200.0];
    let phase = freq.mapv(|f| -360.0 * f * delay_ms * 1e-3);
    Curve {
        freq,
        spl: array![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        phase: Some(phase),
        coherence,
        ..Default::default()
    }
}

fn test_channel_result(name: &str, curve: &Curve) -> ChannelOptimizationResult {
    ChannelOptimizationResult {
        name: name.to_string(),
        pre_score: 0.0,
        post_score: 0.0,
        initial_curve: curve.clone(),
        final_curve: curve.clone(),
        biquads: Vec::new(),
        fir_coeffs: None,
    }
}

fn spectral_gate_curve(spl_fn: impl Fn(f64) -> f64) -> Curve {
    let n = 200;
    let log_start = 20f64.log10();
    let log_end = 20000f64.log10();
    let freq: Vec<f64> = (0..n)
        .map(|i| 10f64.powf(log_start + (log_end - log_start) * i as f64 / (n - 1) as f64))
        .collect();
    let spl: Vec<f64> = freq.iter().map(|&f| spl_fn(f)).collect();
    Curve {
        freq: ndarray::Array1::from(freq),
        spl: ndarray::Array1::from(spl),
        phase: None,
        ..Default::default()
    }
}

#[test]
fn spectral_shelf_gate_accepts_icd_improvement_despite_flatness_regression() {
    let sample_rate = 48_000.0;
    let mut candidate = None;

    for left_low in [-3.0, -1.5, 0.0, 1.5, 3.0] {
        for left_high in [-3.0, -1.5, 0.0, 1.5, 3.0] {
            for right_low in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                for right_high in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                    let make_shaped = |low_gain, high_gain| {
                        let low = Biquad::new(
                            math_audio_iir_fir::BiquadFilterType::Lowshelf,
                            super::super::spectral_align::LOWSHELF_FREQ,
                            sample_rate,
                            math_audio_iir_fir::DEFAULT_Q_HIGH_LOW_SHELF,
                            low_gain,
                        );
                        let high = Biquad::new(
                            math_audio_iir_fir::BiquadFilterType::Highshelf,
                            super::super::spectral_align::HIGHSHELF_FREQ,
                            sample_rate,
                            math_audio_iir_fir::DEFAULT_Q_HIGH_LOW_SHELF,
                            high_gain,
                        );
                        spectral_gate_curve(|f| {
                            low.np_log_result(&ndarray::arr1(&[f]))[0]
                                + high.np_log_result(&ndarray::arr1(&[f]))[0]
                        })
                    };

                    let curves = HashMap::from([
                        ("L".to_string(), make_shaped(left_low, left_high)),
                        ("R".to_string(), make_shaped(right_low, right_high)),
                    ]);
                    let alignment = super::super::spectral_align::compute_spectral_alignment(
                        &curves,
                        sample_rate,
                        20.0,
                        20_000.0,
                    );

                    for channel in ["L", "R"] {
                        let shelf_filters = super::super::spectral_align::create_alignment_filters(
                            &alignment[channel],
                            sample_rate,
                        );
                        if shelf_filters.is_empty() {
                            continue;
                        }

                        let before_flat =
                            recompute_curve_flatness_score(&curves[channel], 20.0, 20_000.0);
                        let response = crate::response::compute_peq_complex_response(
                            &shelf_filters,
                            &curves[channel].freq,
                            sample_rate,
                        );
                        let corrected =
                            crate::response::apply_complex_response(&curves[channel], &response);
                        let after_flat = recompute_curve_flatness_score(&corrected, 20.0, 20_000.0);
                        if after_flat <= before_flat + 1e-3 {
                            continue;
                        }

                        let icd_before =
                            super::super::spectral_align::compute_inter_channel_deviation(
                                &curves, 20.0,
                            );
                        let mut corrected_curves = curves.clone();
                        corrected_curves.insert(channel.to_string(), corrected);
                        let icd_after =
                            super::super::spectral_align::compute_inter_channel_deviation(
                                &corrected_curves,
                                20.0,
                            );
                        let flatness_regression = after_flat - before_flat;
                        let icd_improvement =
                            icd_before.passband_rms_db - icd_after.passband_rms_db;
                        if icd_improvement > flatness_regression + 1e-6 {
                            candidate = Some((
                                curves,
                                channel.to_string(),
                                shelf_filters,
                                before_flat,
                                after_flat,
                                icd_before.passband_rms_db,
                                icd_after.passband_rms_db,
                            ));
                            break;
                        }
                    }
                    if candidate.is_some() {
                        break;
                    }
                }
                if candidate.is_some() {
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }
        if candidate.is_some() {
            break;
        }
    }

    let Some((curves, channel, shelf_filters, before_flat, after_flat, icd_before, icd_after)) =
        candidate
    else {
        panic!("test setup could not find an ICD-improving shelf with a flatness regression");
    };
    assert!(
        after_flat > before_flat + 1e-3 && icd_before - icd_after > after_flat - before_flat + 1e-6,
        "invalid test candidate: flatness {before_flat}->{after_flat}, ICD {icd_before}->{icd_after}"
    );

    assert!(
        should_apply_spectral_shelves(
            &curves,
            &channel,
            &shelf_filters,
            sample_rate,
            20.0,
            20_000.0,
        ),
        "shelves should be accepted when they improve inter-channel deviation"
    );
}

fn test_room_config_with_gd(
    group_delay: super::super::types::GroupDelayOptimizationConfig,
) -> RoomConfig {
    RoomConfig {
        version: super::super::types::default_config_version(),
        system: None,
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig {
            group_delay: Some(group_delay),
            seed: Some(11),
            ..Default::default()
        },
        recording_config: None,
        cea2034_cache: None,
    }
}
