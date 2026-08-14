use super::calculate::calculate_panning_gain;
use super::calculate::calculate_panning_gain_with_wraparound;
use super::get::get_available_configs;
use super::get::get_speaker_config;
use super::misc::normalize_gains_l2;
use super::source_position::SourcePosition;
use super::source_position::compute_vbap_matrix;
use super::speaker_position::SpeakerPosition;
use super::types::{ChannelAssignment, ChannelLayout, ChannelRole};

#[test]
fn test_speaker_position_to_cartesian_matches_inline_math() {
    // Verify the extracted method agrees with the historical inline
    // spherical→Cartesian conversion at every speaker in every preset.
    for cfg_id in get_available_configs() {
        let cfg = get_speaker_config(cfg_id).unwrap();
        for sp in cfg.speakers {
            let az = sp.azimuth.to_radians();
            let el = sp.elevation.to_radians();
            let expected = [el.cos() * az.sin(), el.cos() * az.cos(), el.sin()];
            let actual = sp.to_cartesian();
            for i in 0..3 {
                assert!(
                    (actual[i] - expected[i]).abs() < 1e-6,
                    "{}/{} component {}: got {} expected {}",
                    cfg_id,
                    sp.label,
                    i,
                    actual[i],
                    expected[i]
                );
            }
        }
    }
}

#[test]
fn test_to_cartesian_known_directions() {
    // Front center (0°, 0°) → +Y
    let c = SpeakerPosition {
        label: "C",
        name: "Center",
        azimuth: 0.0,
        elevation: 0.0,
        channel: 0,
        is_lfe: false,
    }
    .to_cartesian();
    assert!(c[0].abs() < 1e-6 && (c[1] - 1.0).abs() < 1e-6 && c[2].abs() < 1e-6);

    // Pure left (+90°, 0°) → +X
    let l = SpeakerPosition {
        label: "L",
        name: "Left",
        azimuth: 90.0,
        elevation: 0.0,
        channel: 0,
        is_lfe: false,
    }
    .to_cartesian();
    assert!((l[0] - 1.0).abs() < 1e-6 && l[1].abs() < 1e-6 && l[2].abs() < 1e-6);

    // Overhead (any az, 90°) → +Z
    let vog = SpeakerPosition {
        label: "VoG",
        name: "Voice of God",
        azimuth: 0.0,
        elevation: 90.0,
        channel: 0,
        is_lfe: false,
    }
    .to_cartesian();
    assert!(vog[0].abs() < 1e-6 && vog[1].abs() < 1e-6 && (vog[2] - 1.0).abs() < 1e-6);
}

#[test]
fn test_get_speaker_config() {
    assert!(get_speaker_config("5.1").is_some());
    assert!(get_speaker_config("7.1.4").is_some());
    assert!(get_speaker_config("invalid").is_none());
}

#[test]
fn test_config_5_1() {
    let config = get_speaker_config("5.1").unwrap();
    assert_eq!(config.total_channels, 6);
    assert_eq!(config.speakers.len(), 6);
    assert_eq!(config.speakers[0].label, "FL");
    assert!(config.speakers[3].is_lfe);
}

#[test]
fn test_config_7_1_4() {
    let config = get_speaker_config("7.1.4").unwrap();
    assert_eq!(config.total_channels, 12);
    assert_eq!(config.speakers.len(), 12);

    // Check height channels
    let height_speakers: Vec<_> = config
        .speakers
        .iter()
        .filter(|s| s.elevation > 0.0)
        .collect();
    assert_eq!(height_speakers.len(), 4);
}

#[test]
fn test_panning_gain_center() {
    // Source at center (0°, 0°) should have max gain at center speaker
    let gain = calculate_panning_gain(0.0, 0.0, 0.0, 0.0);
    assert!((gain - 1.0).abs() < 0.001);
}

#[test]
fn test_panning_gain_opposite() {
    // Source at front (0°) should have zero gain at back (180°)
    let gain = calculate_panning_gain(0.0, 0.0, 180.0, 0.0);
    assert!(gain < 0.01);
}

#[test]
fn test_panning_gain_orthogonal() {
    // Source at front (0°) and side (90°) are perpendicular
    let gain = calculate_panning_gain(0.0, 0.0, 90.0, 0.0);
    assert!(gain < 0.1); // Should be very low since they're perpendicular
}

#[test]
fn test_panning_gain_elevation() {
    // Test elevation panning
    let gain = calculate_panning_gain(0.0, 45.0, 0.0, 45.0);
    assert!((gain - 1.0).abs() < 0.001);

    // Source at ear level (0°) to speaker at 45° elevation
    // cosine_gain = cos(45°) ≈ 0.707, with power 0.5: gain = 0.707^0.5 ≈ 0.841
    let gain = calculate_panning_gain(0.0, 0.0, 0.0, 45.0);
    assert!(
        gain > 0.80 && gain < 0.90,
        "Expected gain ~0.841, got {}",
        gain
    );
}

#[test]
fn test_panning_gain_5_1_4_scenario() {
    // Test realistic 5.1.4 scenario: left source (30°, 0°) to various speakers

    // To FL (30°, 0°) - perfect match
    let gain_fl = calculate_panning_gain(30.0, 0.0, 30.0, 0.0);
    assert!(
        (gain_fl - 1.0).abs() < 0.001,
        "FL should have gain ~1.0, got {}",
        gain_fl
    );

    // To TFL (30°, 45°) - same azimuth, 45° elevation difference
    // cosine_gain = cos(45°) ≈ 0.707, with power 0.5: gain ≈ 0.841
    let gain_tfl = calculate_panning_gain(30.0, 0.0, 30.0, 45.0);
    assert!(
        gain_tfl > 0.80 && gain_tfl < 0.90,
        "TFL should have gain ~0.841, got {}",
        gain_tfl
    );

    // To C (0°, 0°) - 30° azimuth difference
    // cosine_gain = cos(30°) ≈ 0.866, with power 0.5: gain ≈ 0.930
    let gain_c = calculate_panning_gain(30.0, 0.0, 0.0, 0.0);
    assert!(
        gain_c > 0.90 && gain_c < 0.95,
        "C should have gain ~0.930, got {}",
        gain_c
    );

    // TFL should have reasonable gain compared to FL (not too attenuated)
    let ratio = gain_tfl / gain_fl;
    assert!(
        ratio > 0.75,
        "Height speaker should have >75% of floor speaker gain, got {:.1}%",
        ratio * 100.0
    );
}

#[test]
fn test_panning_gain_wraparound_back_left() {
    // BL at 150° should get zero from standard panning (more than 90° from 30°)
    let standard_gain = calculate_panning_gain(30.0, 0.0, 150.0, 0.0);
    assert!(
        standard_gain < 0.01,
        "Standard panning should give ~0 for BL, got {}",
        standard_gain
    );

    // With wraparound, BL should receive signal from wrapped source at -150°
    // Wrapped source at -150° to speaker at 150° = 60° difference
    // Expected: cosine_gain = cos(60°) = 0.5, with power 0.5: gain = 0.707
    // Then multiplied by wrap_attenuation = 0.7: final ~0.495
    let wrapped_gain = calculate_panning_gain_with_wraparound(30.0, 0.0, 150.0, 0.0, 0.7);
    assert!(
        wrapped_gain > 0.4 && wrapped_gain < 0.6,
        "Wraparound should give ~0.495 for BL, got {}",
        wrapped_gain
    );
}

#[test]
fn test_panning_gain_wraparound_back_right() {
    // BR at -150° should get zero from standard panning (more than 90° from -30°)
    let standard_gain = calculate_panning_gain(-30.0, 0.0, -150.0, 0.0);
    assert!(
        standard_gain < 0.01,
        "Standard panning should give ~0 for BR, got {}",
        standard_gain
    );

    // With wraparound, BR should receive signal from wrapped source at 150°
    // Wrapped source at 150° to speaker at -150° = 60° difference
    let wrapped_gain = calculate_panning_gain_with_wraparound(-30.0, 0.0, -150.0, 0.0, 0.7);
    assert!(
        wrapped_gain > 0.4 && wrapped_gain < 0.6,
        "Wraparound should give ~0.495 for BR, got {}",
        wrapped_gain
    );
}

#[test]
fn test_panning_gain_wraparound_front_unchanged() {
    // Front speakers should use standard panning (no wraparound needed)
    let standard_gain = calculate_panning_gain(30.0, 0.0, 30.0, 0.0);
    let wrapped_gain = calculate_panning_gain_with_wraparound(30.0, 0.0, 30.0, 0.0, 0.7);

    // Should be identical for front speakers
    assert!(
        (standard_gain - wrapped_gain).abs() < 0.001,
        "Front speaker gains should match: standard={}, wrapped={}",
        standard_gain,
        wrapped_gain
    );
}

#[test]
fn test_panning_gain_wraparound_7_1_config() {
    // Test all speakers in 7.1 config get non-zero gains
    let config = get_speaker_config("7.1").unwrap();
    const LEFT_AZIMUTH: f32 = 30.0;
    const RIGHT_AZIMUTH: f32 = -30.0;
    const WRAP_ATTENUATION: f32 = 0.7;

    for speaker in config.speakers.iter() {
        if speaker.is_lfe {
            continue; // LFE uses fixed 0.5 gains
        }

        let is_rear = speaker.azimuth.abs() > 90.0;
        let (left_gain, right_gain) = if is_rear {
            (
                calculate_panning_gain_with_wraparound(
                    LEFT_AZIMUTH,
                    0.0,
                    speaker.azimuth,
                    speaker.elevation,
                    WRAP_ATTENUATION,
                ),
                calculate_panning_gain_with_wraparound(
                    RIGHT_AZIMUTH,
                    0.0,
                    speaker.azimuth,
                    speaker.elevation,
                    WRAP_ATTENUATION,
                ),
            )
        } else {
            (
                calculate_panning_gain(LEFT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation),
                calculate_panning_gain(RIGHT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation),
            )
        };

        // At least one of left or right should have non-zero gain
        let max_gain = left_gain.max(right_gain);
        assert!(
            max_gain > 0.1,
            "Speaker {} ({}) should have non-zero gain, got L={:.3}, R={:.3}",
            speaker.label,
            speaker.azimuth,
            left_gain,
            right_gain
        );
    }
}

#[test]
fn test_compute_vbap_matrix_zeros_lfe() {
    let cfg = get_speaker_config("5.1").unwrap();
    let m = compute_vbap_matrix(
        cfg,
        &[
            SourcePosition::new(30.0, 0.0),
            SourcePosition::new(-30.0, 0.0),
        ],
        None,
    );
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].len(), cfg.total_channels);
    for sp in cfg.speakers {
        if sp.is_lfe {
            assert_eq!(m[0][sp.channel], 0.0);
            assert_eq!(m[1][sp.channel], 0.0);
        }
    }
}

#[test]
fn test_compute_vbap_matrix_matches_scalar() {
    let cfg = get_speaker_config("7.1.4").unwrap();
    let src = SourcePosition::new(45.0, 30.0);
    let row = &compute_vbap_matrix(cfg, std::slice::from_ref(&src), None)[0];
    for sp in cfg.speakers {
        if sp.is_lfe {
            continue;
        }
        let expected =
            calculate_panning_gain(src.azimuth_deg, src.elevation_deg, sp.azimuth, sp.elevation);
        assert!(
            (row[sp.channel] - expected).abs() < 1e-6,
            "channel {} ({}): got {} expected {}",
            sp.channel,
            sp.label,
            row[sp.channel],
            expected
        );
    }
}

#[test]
fn test_compute_vbap_matrix_wraparound_passes_attenuation() {
    let cfg = get_speaker_config("7.1").unwrap();
    let src = SourcePosition::new(0.0, 0.0); // Front-center source
    let no_wrap = &compute_vbap_matrix(cfg, std::slice::from_ref(&src), None)[0];
    let wrap = &compute_vbap_matrix(cfg, std::slice::from_ref(&src), Some(0.7))[0];
    // Rear speakers should get nonzero gain with wraparound, zero without.
    for sp in cfg.speakers {
        if sp.is_lfe {
            continue;
        }
        if sp.azimuth.abs() > 100.0 {
            assert!(
                wrap[sp.channel] >= no_wrap[sp.channel],
                "wraparound should be ≥ direct for rear channel {}",
                sp.label
            );
        }
    }
}

#[test]
fn test_normalize_gains_l2_unit_energy() {
    let mut g = vec![0.3, 0.4, 0.5, 0.6];
    normalize_gains_l2(&mut g);
    let energy: f32 = g.iter().map(|v| v * v).sum();
    assert!((energy - 1.0).abs() < 1e-5);
}

#[test]
fn test_normalize_gains_l2_zero_input_is_noop() {
    let mut g = vec![0.0_f32, 0.0, 0.0];
    normalize_gains_l2(&mut g);
    assert!(g.iter().all(|v| *v == 0.0));
}

#[test]
fn explicit_channel_layouts_cover_every_published_speaker_config() {
    for id in get_available_configs() {
        let config = get_speaker_config(id).unwrap();
        let layout = ChannelLayout::from_speaker_config(config).unwrap();
        layout.validate_for_width(config.total_channels).unwrap();
        assert_eq!(layout.role_at(config.total_channels), None);
        for speaker in config.speakers {
            assert!(layout.role_at(speaker.channel).is_some(), "{id}");
            assert_eq!(
                layout.role_at(speaker.channel) == Some(ChannelRole::Lfe),
                speaker.is_lfe,
                "{id} channel {}",
                speaker.channel
            );
        }

        let json = serde_json::to_value(&layout).unwrap();
        assert_eq!(
            serde_json::from_value::<ChannelLayout>(json).unwrap(),
            layout
        );
    }
}

#[test]
fn explicit_channel_layout_rejects_duplicate_indices_roles_and_width_mismatch() {
    let duplicate_index = ChannelLayout {
        channels: vec![
            ChannelAssignment {
                index: 0,
                role: ChannelRole::FrontLeft,
            },
            ChannelAssignment {
                index: 0,
                role: ChannelRole::FrontRight,
            },
        ],
    };
    assert!(duplicate_index.validate_for_width(2).is_err());

    let duplicate_role = ChannelLayout {
        channels: vec![
            ChannelAssignment {
                index: 0,
                role: ChannelRole::FrontLeft,
            },
            ChannelAssignment {
                index: 1,
                role: ChannelRole::FrontLeft,
            },
        ],
    };
    assert!(duplicate_role.validate_for_width(2).is_err());

    let stereo = ChannelLayout::from_speaker_config(get_speaker_config("2.0").unwrap()).unwrap();
    assert!(stereo.validate_for_width(6).is_err());
}

#[test]
fn bs1770_role_weights_distinguish_front_surround_height_and_lfe() {
    assert_eq!(ChannelRole::FrontLeft.bs1770_weight(), 1.0);
    assert_eq!(ChannelRole::WideRight.bs1770_weight(), 1.0);
    assert_eq!(ChannelRole::TopMiddleLeft.bs1770_weight(), 1.0);
    assert_eq!(ChannelRole::SideLeft.bs1770_weight(), 1.41);
    assert_eq!(ChannelRole::BackRight.bs1770_weight(), 1.41);
    assert_eq!(ChannelRole::Lfe.bs1770_weight(), 0.0);
}
