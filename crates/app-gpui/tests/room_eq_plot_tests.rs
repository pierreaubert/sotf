use sotf_audio_player_gpui::{
    calculate_room_eq_log_trend, room_eq_passband_trend_fit_domain, room_eq_trend_fit_domain,
    sum_room_eq_responses_db,
};

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
