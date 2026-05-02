use super::*;
use crate::roomeq::optimize::ChannelOptimizationResult;
use crate::roomeq::types::{
    BassManagementConfig, CrossoverConfig, MultiMeasurementStrategy, MultiSeatConfig,
    MultiSeatStrategy, OptimizerConfig, RoomConfig, SpeakerConfig, SubwooferStrategy,
    SubwooferSystemConfig, SystemConfig, SystemModel,
};
use crate::{Curve, MeasurementSource};
use ndarray::Array1;

fn flat_curve() -> Curve {
    curve_with_spl(vec![80.0, 80.0, 80.0])
}

fn curve_with_spl(spl: Vec<f64>) -> Curve {
    Curve {
        freq: Array1::from_vec(vec![20.0, 100.0, 1000.0]),
        spl: Array1::from_vec(spl),
        phase: Some(Array1::from_vec(vec![0.0, 0.0, 0.0])),
        ..Default::default()
    }
}

fn shifted_grid_curve() -> Curve {
    Curve {
        freq: Array1::from_vec(vec![25.0, 125.0, 1250.0]),
        spl: Array1::from_vec(vec![80.0, 80.0, 80.0]),
        phase: Some(Array1::from_vec(vec![0.0, 0.0, 0.0])),
        ..Default::default()
    }
}

fn home_cinema_system_for(names: &[&str]) -> SystemConfig {
    SystemConfig {
        model: SystemModel::HomeCinema,
        speakers: names
            .iter()
            .map(|name| ((*name).to_string(), (*name).to_string()))
            .collect(),
        subwoofers: None,
        bass_management: None,
    }
}

#[test]
fn detects_immersive_layout_from_standard_roles() {
    let mut speakers = HashMap::new();
    for name in ["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"] {
        speakers.insert(
            name.to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
        );
    }
    let config = RoomConfig {
        version: "test".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let layout = analyze_layout(&config);
    assert_eq!(layout.layout, "5.1.2");
    assert_eq!(layout.height_channels, 2);
    assert!(
        layout
            .channels
            .iter()
            .any(|ch| ch.role == HomeCinemaRole::Center)
    );
}

#[test]
fn system_roles_override_measurement_keys() {
    let mut speakers = HashMap::new();
    speakers.insert(
        "mic_file_1".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
    );
    speakers.insert(
        "mic_file_2".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
    );
    let system = SystemConfig {
        model: Default::default(),
        speakers: HashMap::from([
            ("L".to_string(), "mic_file_1".to_string()),
            ("R".to_string(), "mic_file_2".to_string()),
        ]),
        subwoofers: None,
        bass_management: None,
    };
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(system),
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let layout = analyze_layout(&config);
    assert_eq!(layout.layout, "2.0");
    assert!(
        layout
            .channels
            .iter()
            .all(|ch| ch.matching_group.as_deref() == Some("front_lr"))
    );
}

#[test]
fn role_targets_adjust_only_matching_roles() {
    let base = TargetResponseConfig {
        role_targets: Some(RoleTargetConfig {
            enabled: true,
            height_treble_shelf_db: -2.0,
            ..Default::default()
        }),
        ..Default::default()
    };

    let height = role_adjusted_target_response("TFL", &base);
    let front = role_adjusted_target_response("L", &base);

    assert_eq!(height.preference.treble_shelf_db, -2.0);
    assert_eq!(front.preference.treble_shelf_db, 0.0);
}

#[test]
fn role_targets_apply_role_specific_slope_offsets() {
    let base = TargetResponseConfig {
        shape: super::super::types::TargetShape::Harman,
        role_targets: Some(RoleTargetConfig {
            enabled: true,
            center_slope_offset_db_per_octave: -0.4,
            height_slope_offset_db_per_octave: -1.0,
            ..Default::default()
        }),
        ..Default::default()
    };

    let center = role_adjusted_target_response("C", &base);
    let height = role_adjusted_target_response("TFL", &base);
    let front = role_adjusted_target_response("L", &base);

    assert_eq!(center.shape, super::super::types::TargetShape::Custom);
    assert!((center.slope_db_per_octave - (-1.2)).abs() < 1e-9);
    assert!((height.slope_db_per_octave - (-1.8)).abs() < 1e-9);
    assert_eq!(front.shape, super::super::types::TargetShape::Harman);
}

#[test]
fn role_target_curve_shape_boosts_center_dialog_band() {
    let mut target_curve = Curve {
        freq: Array1::from_vec(vec![100.0, 1000.0, 8000.0]),
        spl: Array1::zeros(3),
        phase: None,
        ..Default::default()
    };
    let target = TargetResponseConfig {
        role_targets: Some(RoleTargetConfig {
            enabled: true,
            center_dialog_boost_db: 2.0,
            ..Default::default()
        }),
        ..Default::default()
    };

    apply_role_target_curve_shape("C", &mut target_curve, &target);

    assert!(target_curve.spl[1] > 1.0);
    assert!(target_curve.spl[1] > target_curve.spl[0]);
    assert!(target_curve.spl[1] > target_curve.spl[2]);
}

#[test]
fn role_target_curve_shape_applies_cinema_distance_rolloff() {
    let mut target_curve = Curve {
        freq: Array1::from_vec(vec![1000.0, 4000.0, 8000.0]),
        spl: Array1::zeros(3),
        phase: None,
        ..Default::default()
    };
    let target = TargetResponseConfig {
        role_targets: Some(RoleTargetConfig {
            enabled: true,
            cinema_x_curve_enabled: true,
            cinema_x_curve_db_per_octave: -0.5,
            listening_distance_m: Some(6.0),
            distance_treble_rolloff_db_per_doubling: 1.0,
            ..Default::default()
        }),
        ..Default::default()
    };

    apply_role_target_curve_shape("SL", &mut target_curve, &target);

    assert_eq!(target_curve.spl[0], 0.0);
    assert!(target_curve.spl[2] < target_curve.spl[1]);
    assert!(target_curve.spl[2] < -2.0);
}

#[test]
fn layout_metadata_reports_role_target_profile() {
    let mut speakers = HashMap::new();
    speakers.insert(
        "C".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
    );
    let config = RoomConfig {
        version: "test".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig {
            target_response: Some(TargetResponseConfig {
                role_targets: Some(RoleTargetConfig {
                    enabled: true,
                    center_dialog_boost_db: 1.5,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        recording_config: None,
        cea2034_cache: None,
    };

    let layout = analyze_layout(&config);
    let center = layout
        .channels
        .iter()
        .find(|channel| channel.role == HomeCinemaRole::Center)
        .unwrap();
    assert_eq!(center.target_profile, "center_dialog_role_target");
    assert_eq!(
        center.target_advisory.as_deref(),
        Some("center_dialog_band")
    );
}

#[test]
fn reports_non_sub_multiseat_coverage() {
    let mut speakers = HashMap::new();
    speakers.insert(
        "L".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
            flat_curve(),
            flat_curve(),
        ])),
    );
    speakers.insert(
        "Sub".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
            flat_curve(),
            flat_curve(),
            flat_curve(),
        ])),
    );
    let config = RoomConfig {
        version: "test".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let report = multi_seat_coverage(&config);
    assert_eq!(report.channels_with_multiple_measurements, 2);
    assert_eq!(report.non_sub_channel_count, 1);
    assert_eq!(report.non_sub_channels_with_multiple_measurements, 1);
    assert_eq!(report.max_seat_count, 3);
    assert!(report.all_channel_correction_ready);
    assert_eq!(report.recommended_scope, "all_channel_reporting_ready");
    assert_eq!(
        report.advisories,
        vec!["all_channel_multi_seat_reporting_ready".to_string()]
    );
}

#[test]
fn reports_partial_non_sub_multiseat_coverage() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: None,
        speakers: HashMap::from([
            (
                "L".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
                    flat_curve(),
                    flat_curve(),
                ])),
            ),
            (
                "R".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            ),
            (
                "Sub".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
                    flat_curve(),
                    flat_curve(),
                ])),
            ),
        ]),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let report = multi_seat_coverage(&config);
    assert_eq!(report.non_sub_channel_count, 2);
    assert_eq!(report.non_sub_channels_with_multiple_measurements, 1);
    assert!(!report.all_channel_correction_ready);
    assert_eq!(report.recommended_scope, "partial_non_sub_reporting_only");
    assert!(
        report
            .advisories
            .contains(&"partial_non_sub_multi_seat_coverage".to_string())
    );
}

#[test]
fn derives_all_channel_multiseat_primary_weights() {
    let mut optimizer = OptimizerConfig::default();
    optimizer.multi_seat = Some(MultiSeatConfig {
        enabled: false,
        strategy: MultiSeatStrategy::PrimaryWithConstraints,
        primary_seat: 1,
        primary_seat_weight: 3.0,
        ..Default::default()
    });
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        cea2034_cache: None,
    };

    let derived = derive_all_channel_multiseat_config(&config, "L", &source)
        .expect("home-cinema multi-seat config should derive");
    assert_eq!(
        derived.strategy,
        MultiMeasurementStrategy::SpatialRobustness
    );
    let weights = derived.weights.expect("weights should be normalized");
    assert!(weights[1] > weights[0]);
    assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-9);
}

#[test]
fn skips_all_channel_multiseat_on_grid_mismatch() {
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), shifted_grid_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    assert!(derive_all_channel_multiseat_config(&config, "L", &source).is_none());
}

#[test]
fn skips_all_channel_multiseat_on_invalid_weight_policy() {
    let mut optimizer = OptimizerConfig::default();
    optimizer.multi_seat = Some(MultiSeatConfig {
        seat_weights: Some(vec![1.0]),
        ..Default::default()
    });
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        cea2034_cache: None,
    };

    assert!(derive_all_channel_multiseat_config(&config, "L", &source).is_none());
    let channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: flat_curve(),
            final_curve: flat_curve(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let report = multi_seat_correction_report(&config, &channel_results, None);
    let left = report
        .channels
        .iter()
        .find(|channel| channel.channel == "L")
        .expect("left channel report");
    assert_eq!(left.status, "invalid_policy_skipped");
}

#[test]
fn reports_grid_mismatch_as_channel_skip() {
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), shifted_grid_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source))]),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };
    let channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: flat_curve(),
            final_curve: flat_curve(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);

    let report = multi_seat_correction_report(&config, &channel_results, None);
    let left = report
        .channels
        .iter()
        .find(|channel| channel.channel == "L")
        .expect("left channel report");
    assert_eq!(left.status, "frequency_grid_mismatch_skipped");
    assert!(left.seats.is_empty());
}

#[test]
fn skips_all_channel_multiseat_when_primary_seat_is_invalid() {
    let mut optimizer = OptimizerConfig::default();
    optimizer.multi_seat = Some(MultiSeatConfig {
        primary_seat: 2,
        ..Default::default()
    });
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        cea2034_cache: None,
    };

    assert!(derive_all_channel_multiseat_config(&config, "L", &source).is_none());
    let channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: flat_curve(),
            final_curve: flat_curve(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let report = multi_seat_correction_report(&config, &channel_results, None);
    let left = report
        .channels
        .iter()
        .find(|channel| channel.channel == "L")
        .expect("left channel report");
    assert_eq!(left.status, "invalid_policy_skipped");
    assert!(
        left.advisories
            .contains(&"primary_seat_out_of_range".to_string())
    );
}

#[test]
fn rejects_all_channel_multiseat_when_constraints_fail() {
    let mut optimizer = OptimizerConfig::default();
    optimizer.multi_seat = Some(MultiSeatConfig {
        strategy: MultiSeatStrategy::PrimaryWithConstraints,
        primary_seat: 0,
        max_deviation_db: 6.0,
        ..Default::default()
    });
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        cea2034_cache: None,
    };

    let acceptance = all_channel_multiseat_acceptance(
        &config,
        "L",
        &source,
        &flat_curve(),
        &curve_with_spl(vec![80.0, 95.0, 80.0]),
    );
    assert!(!acceptance.accepted);
    assert!(
        acceptance
            .advisories
            .contains(&"weighted_target_fit_collapsed".to_string())
            || acceptance
                .advisories
                .contains(&"primary_seat_constraint_failed".to_string())
    );
}

#[test]
fn rejects_all_channel_multiseat_when_broadband_level_collapses() {
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source.clone()))]),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let acceptance = all_channel_multiseat_acceptance(
        &config,
        "L",
        &source,
        &flat_curve(),
        &curve_with_spl(vec![60.0, 60.0, 60.0]),
    );

    assert!(!acceptance.accepted);
    assert!(
        acceptance
            .advisories
            .contains(&"weighted_target_level_collapsed".to_string()),
        "expected broadband level-collapse guard, got {:?}",
        acceptance.advisories
    );
}

#[test]
fn reports_guardrail_rejection_without_claiming_applied() {
    let source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L"])),
        speakers: HashMap::from([("L".to_string(), SpeakerConfig::Single(source))]),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };
    let channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: flat_curve(),
            final_curve: flat_curve(),
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);
    let rejections = HashMap::from([(
        "L".to_string(),
        vec!["weighted_target_fit_collapsed".to_string()],
    )]);

    let report = multi_seat_correction_report(&config, &channel_results, Some(&rejections));
    assert!(!report.applied);
    assert!(
        report
            .advisories
            .contains(&"all_channel_corrections_rejected_by_guardrails".to_string())
    );
    let left = report
        .channels
        .iter()
        .find(|channel| channel.channel == "L")
        .expect("left channel report");
    assert_eq!(left.status, "rejected_guardrails");
    assert!(left.seats.is_empty());
}

#[test]
fn reports_all_channel_multiseat_null_guard() {
    let mut optimizer = OptimizerConfig::default();
    optimizer.multi_seat = Some(MultiSeatConfig {
        enabled: false,
        strategy: MultiSeatStrategy::PrimaryWithConstraints,
        primary_seat: 0,
        max_deviation_db: 6.0,
        ..Default::default()
    });
    let primary = flat_curve();
    let null_seat = curve_with_spl(vec![80.0, 45.0, 80.0]);
    let source = MeasurementSource::InMemoryMultiple(vec![primary.clone(), null_seat]);
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L", "R"])),
        speakers: HashMap::from([
            ("L".to_string(), SpeakerConfig::Single(source)),
            (
                "R".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            ),
        ]),
        crossovers: None,
        target_curve: None,
        optimizer,
        recording_config: None,
        cea2034_cache: None,
    };
    let channel_results = HashMap::from([(
        "L".to_string(),
        ChannelOptimizationResult {
            name: "L".to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            initial_curve: primary.clone(),
            final_curve: primary,
            biquads: Vec::new(),
            fir_coeffs: None,
        },
    )]);

    let report = multi_seat_correction_report(&config, &channel_results, None);
    assert!(report.enabled);
    assert!(!report.applied);
    let left = report
        .channels
        .iter()
        .find(|channel| channel.channel == "L")
        .expect("left channel report");
    assert_eq!(left.status, "failed_constraints");
    assert!(left.primary_pass == Some(true));
    assert!(left.non_primary_pass == Some(false));
    assert!(
        left.advisories
            .contains(&"seat_specific_null_not_corrected".to_string())
    );
    assert!(
        report
            .advisories
            .contains(&"seat_specific_nulls_were_not_overcorrected".to_string())
    );
    let right = report
        .channels
        .iter()
        .find(|channel| channel.channel == "R")
        .expect("right channel report");
    assert_eq!(right.status, "single_seat_only");
}

#[test]
fn reports_all_channel_multiseat_by_role_group_and_excludes_subs() {
    let multiseat_source = MeasurementSource::InMemoryMultiple(vec![flat_curve(), flat_curve()]);
    let mut speakers = HashMap::new();
    for role in ["L", "C", "SL", "TFL"] {
        speakers.insert(
            role.to_string(),
            SpeakerConfig::Single(multiseat_source.clone()),
        );
    }
    speakers.insert(
        "R".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
    );
    speakers.insert(
        "LFE".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
            flat_curve(),
            flat_curve(),
        ])),
    );

    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(home_cinema_system_for(&["L", "R", "C", "SL", "TFL", "LFE"])),
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };
    let channel_results: HashMap<String, ChannelOptimizationResult> = ["L", "C", "SL", "TFL"]
        .into_iter()
        .map(|role| {
            (
                role.to_string(),
                ChannelOptimizationResult {
                    name: role.to_string(),
                    pre_score: 0.0,
                    post_score: 0.0,
                    initial_curve: flat_curve(),
                    final_curve: flat_curve(),
                    biquads: Vec::new(),
                    fir_coeffs: None,
                },
            )
        })
        .collect();

    let report = multi_seat_correction_report(&config, &channel_results, None);

    assert!(report.applied);
    assert!(
        report
            .channels
            .iter()
            .all(|channel| channel.channel != "LFE"),
        "sub/LFE channels must remain owned by bass management/MSO"
    );
    let front_lr = report
        .role_groups
        .iter()
        .find(|group| group.role_group == HomeCinemaRoleGroup::FrontLr)
        .expect("front LR role group");
    assert_eq!(front_lr.channel_count, 2);
    assert_eq!(front_lr.applied_channel_count, 1);
    let center = report
        .role_groups
        .iter()
        .find(|group| group.role_group == HomeCinemaRoleGroup::Center)
        .expect("center role group");
    assert_eq!(center.applied_channel_count, 1);
    let surrounds = report
        .role_groups
        .iter()
        .find(|group| group.role_group == HomeCinemaRoleGroup::SideSurrounds)
        .expect("surround role group");
    assert_eq!(surrounds.applied_channel_count, 1);
    let heights = report
        .role_groups
        .iter()
        .find(|group| group.role_group == HomeCinemaRoleGroup::TopFront)
        .expect("height role group");
    assert_eq!(heights.applied_channel_count, 1);

    let right = report
        .channels
        .iter()
        .find(|channel| channel.channel == "R")
        .expect("right channel report");
    assert_eq!(right.status, "single_seat_only");
}

#[test]
fn bass_management_reports_lfe_and_limits_sub_gain() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("R".to_string(), "R".to_string()),
                ("LFE".to_string(), "Sub".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig {
                max_sub_boost_db: 3.0,
                sub_trim_db: 1.0,
                ..Default::default()
            }),
        }),
        speakers: HashMap::from([
            (
                "L".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            ),
            (
                "R".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            ),
            (
                "Sub".to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            ),
        ]),
        crossovers: Some(HashMap::from([(
            "xo".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        )])),
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let effective = effective_bass_management(&config).expect("bass management");
    let (gain, limited) = limited_sub_gain(5.0, Some(&effective));
    let report = bass_management_report(&config, Some(gain), limited).expect("report");

    assert_eq!(gain, 3.0);
    assert!(limited);
    assert_eq!(report.crossover_type, "LR24");
    assert_eq!(report.crossover_frequency_hz, Some(80.0));
    assert_eq!(report.lfe_playback_gain_db, 10.0);
    assert!(report.optimization.is_none());
    assert_eq!(report.physical_sub_output, "LFE");
    assert_eq!(report.redirected_bass_channel_count, 2);
    assert_eq!(report.main_high_pass_hz, Some(80.0));
    assert_eq!(report.sub_low_pass_hz, Some(80.0));
    assert_eq!(report.lfe_headroom_required_db, 16.0);
    assert!(
        report
            .signal_flow_advisories
            .contains(&"lfe_gain_exceeds_headroom_margin".to_string())
    );
    let left = report
        .signal_flow
        .iter()
        .find(|entry| entry.source_channel == "L")
        .expect("left signal flow");
    assert_eq!(left.destination, "LFE");
    assert_eq!(left.high_pass_hz, Some(80.0));
    assert_eq!(left.low_pass_hz, Some(80.0));
    assert!(left.redirects_bass);
    let lfe = report
        .signal_flow
        .iter()
        .find(|entry| entry.source_channel == "LFE")
        .expect("lfe signal flow");
    assert_eq!(lfe.destination, "LFE");
    assert_eq!(lfe.low_pass_hz, Some(80.0));
    assert_eq!(lfe.lfe_gain_db, 10.0);
    assert!(!lfe.redirects_bass);
    assert!(report.advisory.contains("sub_gain_limited_for_headroom"));
    assert!(
        report
            .advisory
            .contains("lfe_gain_reported_not_applied_to_physical_sub_chain")
    );
}

#[test]
fn bass_management_report_preserves_optimization_metadata() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("Sub".to_string(), "Sub".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig::default()),
        }),
        speakers: HashMap::new(),
        crossovers: Some(HashMap::from([(
            "xo".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        )])),
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };
    let optimization = BassManagementOptimizationReport {
        applied: true,
        phase_required: true,
        phase_available: true,
        configured_crossover_hz: Some(80.0),
        optimized_crossover_hz: Some(90.0),
        crossover_range_hz: Some((60.0, 120.0)),
        crossover_type: "LR24".to_string(),
        main_delay_ms: 0.0,
        sub_delay_ms: 1.25,
        relative_sub_delay_ms: 1.25,
        sub_polarity_inverted: true,
        requested_sub_gain_db: 5.0,
        applied_sub_gain_db: 3.0,
        gain_limited: true,
        estimated_bass_bus_peak_gain_db: Some(12.0),
        objective_before: Some(4.0),
        objective_after: Some(2.0),
        group_results: Vec::new(),
        sub_output_results: Vec::new(),
        advisories: vec!["sub_gain_limited_for_headroom".to_string()],
    };

    let report =
        bass_management_report_with_optimization(&config, Some(3.0), true, Some(optimization))
            .expect("report");
    let reported = report.optimization.expect("optimization");
    assert!(reported.applied);
    assert_eq!(reported.optimized_crossover_hz, Some(90.0));
    assert_eq!(reported.sub_delay_ms, 1.25);
    assert!(reported.sub_polarity_inverted);
    assert_eq!(reported.objective_after, Some(2.0));
}

#[test]
fn bass_management_routes_use_group_specific_crossovers() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("R".to_string(), "R".to_string()),
                ("SL".to_string(), "SL".to_string()),
                ("SR".to_string(), "SR".to_string()),
                ("TFL".to_string(), "TFL".to_string()),
                ("TFR".to_string(), "TFR".to_string()),
                ("Sub".to_string(), "Sub".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("lcr_xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig {
                group_crossovers: HashMap::from([
                    ("surround".to_string(), "surround_xo".to_string()),
                    ("height".to_string(), "height_xo".to_string()),
                ]),
                ..Default::default()
            }),
        }),
        speakers: HashMap::new(),
        crossovers: Some(HashMap::from([
            (
                "lcr_xo".to_string(),
                CrossoverConfig {
                    crossover_type: "LR24".to_string(),
                    frequency: Some(80.0),
                    frequencies: None,
                    frequency_range: None,
                },
            ),
            (
                "surround_xo".to_string(),
                CrossoverConfig {
                    crossover_type: "BW12".to_string(),
                    frequency: Some(100.0),
                    frequencies: None,
                    frequency_range: None,
                },
            ),
            (
                "height_xo".to_string(),
                CrossoverConfig {
                    crossover_type: "LR48".to_string(),
                    frequency: Some(140.0),
                    frequencies: None,
                    frequency_range: None,
                },
            ),
        ])),
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let report = bass_management_report(&config, None, false).expect("report");
    let routing = report.routing_graph.as_ref().expect("routing graph");
    let l_route = routing
        .routes
        .iter()
        .find(|route| route.source_channel == "L" && route.route_kind == "main_highpass_to_self")
        .expect("L highpass route");
    assert_eq!(l_route.crossover_type, "LR24");
    assert_eq!(l_route.high_pass_hz, Some(80.0));
    assert_eq!(l_route.pre_chain_channel.as_deref(), Some("L"));
    assert_eq!(l_route.post_chain_channel.as_deref(), Some("L"));
    let sl_route = routing
        .routes
        .iter()
        .find(|route| route.source_channel == "SL" && route.route_kind == "main_highpass_to_self")
        .expect("SL highpass route");
    assert_eq!(sl_route.crossover_type, "BW12");
    assert_eq!(sl_route.high_pass_hz, Some(100.0));
    let height_route = routing
        .routes
        .iter()
        .find(|route| route.source_channel == "TFL" && route.route_kind == "main_highpass_to_self")
        .expect("TFL highpass route");
    assert_eq!(height_route.crossover_type, "LR48");
    assert_eq!(height_route.high_pass_hz, Some(140.0));

    let surround_group = report
        .groups
        .iter()
        .find(|group| group.group_id == "surround")
        .expect("surround group report");
    assert_eq!(surround_group.crossover_type, "BW12");
    assert_eq!(surround_group.selected_crossover_hz, Some(100.0));
    let height_flow = report
        .signal_flow
        .iter()
        .find(|entry| entry.source_channel == "TFL")
        .expect("TFL signal flow");
    assert_eq!(height_flow.high_pass_hz, Some(140.0));
    assert_eq!(height_flow.low_pass_hz, Some(140.0));
}

#[test]
fn bass_headroom_uses_supplied_sample_rate_for_crossover_response() {
    let route = BassManagementRoute {
        group_id: Some("lcr".to_string()),
        source_channel: "L".to_string(),
        source_index: 0,
        destination: "Sub".to_string(),
        destination_index: 1,
        pre_chain_channel: Some("Sub".to_string()),
        post_chain_channel: Some("Sub".to_string()),
        route_kind: "redirected_bass_lowpass_to_sub".to_string(),
        crossover_type: "LR48".to_string(),
        high_pass_hz: None,
        low_pass_hz: Some(160.0),
        gain_db: 0.0,
        gain_linear: 1.0,
        matrix_gain: 1.0,
        delay_ms: 0.0,
        polarity_inverted: false,
    };
    let at_48k = bass_route_complex_gain(&route, 220.0, 48_000.0);
    let at_96k = bass_route_complex_gain(&route, 220.0, 96_000.0);
    assert!(
        (at_48k.norm() - at_96k.norm()).abs() > 1e-7,
        "headroom route response should be generated for the actual sample rate"
    );
}

#[test]
fn bass_headroom_route_gain_includes_crossover_phase_delay_and_polarity() {
    let base = BassManagementRoute {
        group_id: Some("lcr".to_string()),
        source_channel: "L".to_string(),
        source_index: 0,
        destination: "Sub".to_string(),
        destination_index: 1,
        pre_chain_channel: Some("Sub".to_string()),
        post_chain_channel: Some("Sub".to_string()),
        route_kind: "redirected_bass_lowpass_to_sub".to_string(),
        crossover_type: "LR24".to_string(),
        high_pass_hz: None,
        low_pass_hz: Some(80.0),
        gain_db: 0.0,
        gain_linear: 1.0,
        matrix_gain: 1.0,
        delay_ms: 0.0,
        polarity_inverted: false,
    };

    let no_delay = bass_route_complex_gain(&base, 80.0, 48_000.0);
    let delayed = bass_route_complex_gain(
        &BassManagementRoute {
            delay_ms: 6.25,
            ..base.clone()
        },
        80.0,
        48_000.0,
    );
    let inverted = bass_route_complex_gain(
        &BassManagementRoute {
            polarity_inverted: true,
            ..base
        },
        80.0,
        48_000.0,
    );
    let gain_plugin_route = BassManagementRoute {
        gain_db: -6.0,
        gain_linear: 10.0_f64.powf(-6.0 / 20.0),
        matrix_gain: 1.0,
        ..BassManagementRoute {
            group_id: Some("lcr".to_string()),
            source_channel: "L".to_string(),
            source_index: 0,
            destination: "Sub".to_string(),
            destination_index: 1,
            pre_chain_channel: Some("Sub".to_string()),
            post_chain_channel: Some("Sub".to_string()),
            route_kind: "redirected_bass_lowpass_to_sub".to_string(),
            crossover_type: "none".to_string(),
            high_pass_hz: None,
            low_pass_hz: None,
            gain_db: 0.0,
            gain_linear: 1.0,
            matrix_gain: 1.0,
            delay_ms: 0.0,
            polarity_inverted: false,
        }
    };

    assert!((no_delay.norm() - delayed.norm()).abs() < 1e-9);
    assert!(
        (no_delay.arg() - delayed.arg()).abs() > 0.5,
        "delay should rotate route phase"
    );
    assert!(
        (no_delay + inverted).norm() < 1e-9,
        "polarity inversion should flip complex sign"
    );
    assert!(
        (bass_route_complex_gain(&gain_plugin_route, 80.0, 48_000.0).norm()
            - gain_plugin_route.gain_linear)
            .abs()
            < 1e-9,
        "gain-plugin routes should contribute their gain to headroom"
    );
}

#[test]
fn bass_management_routes_expand_to_physical_sub_outputs() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("LFE".to_string(), "subs".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Mso,
                crossover: Some("xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig::default()),
        }),
        speakers: HashMap::new(),
        crossovers: Some(HashMap::from([(
            "xo".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        )])),
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };
    let optimization = BassManagementOptimizationReport {
        applied: true,
        phase_required: true,
        phase_available: true,
        configured_crossover_hz: Some(80.0),
        optimized_crossover_hz: Some(80.0),
        crossover_range_hz: None,
        crossover_type: "LR24".to_string(),
        main_delay_ms: 0.0,
        sub_delay_ms: 1.0,
        relative_sub_delay_ms: 1.0,
        sub_polarity_inverted: false,
        requested_sub_gain_db: 0.0,
        applied_sub_gain_db: 0.0,
        gain_limited: false,
        estimated_bass_bus_peak_gain_db: None,
        objective_before: Some(3.0),
        objective_after: Some(2.0),
        group_results: vec![BassManagementGroupReport {
            group_id: "lcr".to_string(),
            roles: vec!["L".to_string()],
            crossover_type: "LR24".to_string(),
            selected_crossover_hz: Some(90.0),
            configured_crossover_hz: Some(80.0),
            main_delay_ms: 0.5,
            bass_route_delay_ms: 1.0,
            polarity_inverted: true,
            trim_db: -2.0,
            objective_before: Some(3.0),
            objective_after: Some(2.0),
            advisories: vec!["ok".to_string()],
        }],
        sub_output_results: vec![
            BassManagementSubOutputReport {
                output_role: "subs_1".to_string(),
                gain_db: -1.0,
                delay_ms: 2.0,
                polarity_inverted: false,
                strategy_source: "mso".to_string(),
                headroom_contribution_db: -1.0,
            },
            BassManagementSubOutputReport {
                output_role: "subs_2".to_string(),
                gain_db: -3.0,
                delay_ms: 4.0,
                polarity_inverted: true,
                strategy_source: "mso".to_string(),
                headroom_contribution_db: -3.0,
            },
        ],
        advisories: vec!["ok".to_string()],
    };

    let graph = bass_management_routing_graph(&config, Some(&optimization)).expect("routing graph");
    assert!(
        graph.matrix.is_none(),
        "multi-sub routing needs route branches"
    );
    assert!(
        graph
            .advisories
            .contains(&"branch_routing_required_for_multiple_sub_outputs".to_string())
    );
    assert!(graph.output_channels.contains(&"subs_1".to_string()));
    assert!(graph.output_channels.contains(&"subs_2".to_string()));
    let sub1_route = graph
        .routes
        .iter()
        .find(|route| {
            route.route_kind == "redirected_bass_lowpass_to_sub" && route.destination == "subs_1"
        })
        .expect("sub1 route");
    assert_eq!(sub1_route.low_pass_hz, Some(90.0));
    assert_eq!(sub1_route.pre_chain_channel.as_deref(), Some("LFE"));
    assert_eq!(sub1_route.post_chain_channel.as_deref(), Some("subs_1"));
    assert!((sub1_route.gain_db - -3.0).abs() < 1e-9);
    assert!((sub1_route.delay_ms - 3.0).abs() < 1e-9);
    assert!(sub1_route.polarity_inverted);
    let sub2_route = graph
        .routes
        .iter()
        .find(|route| {
            route.route_kind == "redirected_bass_lowpass_to_sub" && route.destination == "subs_2"
        })
        .expect("sub2 route");
    assert!((sub2_route.gain_db - -5.0).abs() < 1e-9);
    assert!((sub2_route.delay_ms - 5.0).abs() < 1e-9);
    assert!(!sub2_route.polarity_inverted);
}

#[test]
fn bass_management_report_warns_when_highpass_is_not_redirected() {
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("Sub".to_string(), "Sub".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig {
                redirect_bass: false,
                ..Default::default()
            }),
        }),
        speakers: HashMap::new(),
        crossovers: Some(HashMap::from([(
            "xo".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        )])),
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    let report = bass_management_report(&config, None, false).expect("report");
    assert_eq!(report.main_high_pass_hz, Some(80.0));
    assert_eq!(report.redirected_bass_channel_count, 0);
    assert!(
        report
            .signal_flow_advisories
            .contains(&"main_highpass_without_redirected_bass".to_string())
    );
    let left = report
        .signal_flow
        .iter()
        .find(|entry| entry.source_channel == "L")
        .expect("left signal flow");
    assert_eq!(left.high_pass_hz, Some(80.0));
    assert!(!left.redirects_bass);
}

#[test]
fn bass_output_role_uses_configured_lfe_channel() {
    let system = SystemConfig {
        model: SystemModel::HomeCinema,
        speakers: HashMap::from([
            ("L".to_string(), "L".to_string()),
            ("R".to_string(), "R".to_string()),
            ("Sub".to_string(), "Sub".to_string()),
        ]),
        subwoofers: Some(SubwooferSystemConfig {
            config: SubwooferStrategy::Single,
            crossover: Some("xo".to_string()),
            mapping: HashMap::new(),
        }),
        bass_management: Some(BassManagementConfig {
            lfe_channel: "Sub".to_string(),
            ..Default::default()
        }),
    };
    let config = RoomConfig {
        version: "test".to_string(),
        system: Some(system.clone()),
        speakers: HashMap::new(),
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    assert_eq!(bass_output_role(&config, &system), "Sub");
}
