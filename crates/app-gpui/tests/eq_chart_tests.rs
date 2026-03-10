//! EQ chart coordinate and response tests (from components/plugins/ui_eq.rs)

use math_audio_iir_fir::BiquadFilterType;
use sotf_audio_player::EQFilter;
use sotf_audio_player_gpui::{
    calculate_band_response, calculate_plot_width, calculate_response_at_freq,
    drag_delta_to_q_change, freq_to_x, gain_to_y, get_filter_type_index, q_to_bar_width,
    x_to_freq, y_to_gain, CHART_BOTTOM_MARGIN, CHART_LEFT_MARGIN, CHART_TOP_MARGIN,
    GPUI_PX_MARGIN_TOP, MAX_FREQ, MIN_FREQ, Q_BAR_MAX_WIDTH, Q_BAR_MIN_WIDTH,
};
use sotf_plugins::param_specs::{eq::BAND_TEMPLATE as EQ, find_by_key as pk};

const TEST_CHART_HEIGHT: f32 = 300.0;
const TEST_PLOT_HEIGHT: f32 = TEST_CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
const TEST_MIN_GAIN_DB: f64 = -24.0;
const TEST_MAX_GAIN_DB: f64 = 24.0;

#[test]
fn test_freq_x_roundtrip() {
    let plot_width = 500.0;
    let test_freqs = [20.0, 100.0, 1000.0, 10000.0, 20000.0];

    for &freq in &test_freqs {
        let x = freq_to_x(freq, plot_width);
        let recovered_freq = x_to_freq(x, plot_width);
        let rel_error = (recovered_freq - freq).abs() / freq;
        assert!(
            rel_error < 0.001,
            "freq_to_x/x_to_freq roundtrip failed for freq={}: got {}, error={}",
            freq, recovered_freq, rel_error
        );
    }
}

#[test]
fn test_gain_y_roundtrip() {
    let test_gains = [-24.0, -12.0, 0.0, 12.0, 24.0];

    for &gain in &test_gains {
        let y = gain_to_y(gain, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        let recovered_gain = y_to_gain(y, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        let abs_error = (recovered_gain - gain).abs();
        assert!(
            abs_error < 0.01,
            "gain_to_y/y_to_gain roundtrip failed for gain={}: got {}, error={}",
            gain, recovered_gain, abs_error
        );
    }
}

#[test]
fn test_freq_to_x_boundaries() {
    let plot_width = 500.0;

    let x_min = freq_to_x(MIN_FREQ, plot_width);
    assert!(
        (x_min - CHART_LEFT_MARGIN).abs() < 0.01,
        "MIN_FREQ should map to left margin: got {} expected {}",
        x_min, CHART_LEFT_MARGIN
    );

    let x_max = freq_to_x(MAX_FREQ, plot_width);
    let expected_max = CHART_LEFT_MARGIN + plot_width;
    assert!(
        (x_max - expected_max).abs() < 0.01,
        "MAX_FREQ should map to right edge: got {} expected {}",
        x_max, expected_max
    );
}

#[test]
fn test_gain_to_y_boundaries() {
    let y_max = gain_to_y(TEST_MAX_GAIN_DB, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
    assert!(
        (y_max - CHART_TOP_MARGIN).abs() < 0.01,
        "MAX_GAIN_DB should map to top margin: got {} expected {}",
        y_max, CHART_TOP_MARGIN
    );

    let y_min = gain_to_y(TEST_MIN_GAIN_DB, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
    let expected_min = CHART_TOP_MARGIN + TEST_PLOT_HEIGHT;
    assert!(
        (y_min - expected_min).abs() < 0.01,
        "MIN_GAIN_DB should map to bottom edge: got {} expected {}",
        y_min, expected_min
    );

    let y_zero = gain_to_y(0.0, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
    let expected_center = CHART_TOP_MARGIN + TEST_PLOT_HEIGHT / 2.0;
    assert!(
        (y_zero - expected_center).abs() < 0.01,
        "0 dB should map to vertical center: got {} expected {}",
        y_zero, expected_center
    );
}

#[test]
fn test_x_to_freq_clamping() {
    let plot_width = 500.0;

    let freq_before = x_to_freq(0.0, plot_width);
    assert!(
        (freq_before - MIN_FREQ).abs() < 0.01,
        "x before margin should clamp to MIN_FREQ: got {}",
        freq_before
    );

    let freq_after = x_to_freq(CHART_LEFT_MARGIN + plot_width + 100.0, plot_width);
    assert!(
        (freq_after - MAX_FREQ).abs() < 0.01,
        "x after right edge should clamp to MAX_FREQ: got {}",
        freq_after
    );
}

#[test]
fn test_y_to_gain_clamping() {
    let gain_above = y_to_gain(0.0, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
    assert!(
        (gain_above - TEST_MAX_GAIN_DB).abs() < 0.01,
        "y above margin should clamp to MAX_GAIN_DB: got {}",
        gain_above
    );

    let gain_below = y_to_gain(
        TEST_CHART_HEIGHT + 100.0,
        TEST_MIN_GAIN_DB,
        TEST_MAX_GAIN_DB,
    );
    assert!(
        (gain_below - TEST_MIN_GAIN_DB).abs() < 0.01,
        "y below bottom should clamp to MIN_GAIN_DB: got {}",
        gain_below
    );
}

#[test]
fn test_q_to_bar_width() {
    let width_at_min_q = q_to_bar_width(pk(EQ, "q").min_f64());
    assert!(
        (width_at_min_q - Q_BAR_MAX_WIDTH).abs() < 0.01,
        "min Q should give max width: got {} expected {}",
        width_at_min_q, Q_BAR_MAX_WIDTH
    );

    let width_at_max_q = q_to_bar_width(pk(EQ, "q").max_f64());
    assert!(
        (width_at_max_q - Q_BAR_MIN_WIDTH).abs() < 0.01,
        "max Q should give min width: got {} expected {}",
        width_at_max_q, Q_BAR_MIN_WIDTH
    );

    let mid_q = (pk(EQ, "q").min_f64() + pk(EQ, "q").max_f64()) / 2.0;
    let mid_width = (Q_BAR_MIN_WIDTH + Q_BAR_MAX_WIDTH) / 2.0;
    let width_at_mid_q = q_to_bar_width(mid_q);
    assert!(
        (width_at_mid_q - mid_width).abs() < 1.0,
        "Mid Q should give mid width: got {} expected ~{}",
        width_at_mid_q, mid_width
    );
}

#[test]
fn test_control_points_within_bounds() {
    let chart_width = 800.0;
    let labels = ["Combined", "#1 - PK @ 1000Hz", "#2 - LS @ 100Hz (muted)"];
    let plot_width = calculate_plot_width(chart_width, labels.iter().copied());

    let test_cases = [
        (MIN_FREQ, TEST_MIN_GAIN_DB),
        (MIN_FREQ, TEST_MAX_GAIN_DB),
        (MAX_FREQ, TEST_MIN_GAIN_DB),
        (MAX_FREQ, TEST_MAX_GAIN_DB),
        (1000.0, 0.0),
        (100.0, -6.0),
        (10000.0, 6.0),
    ];

    for (freq, gain) in test_cases {
        let x = freq_to_x(freq, plot_width);
        let y = gain_to_y(gain, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);

        assert!(
            x >= CHART_LEFT_MARGIN && x <= CHART_LEFT_MARGIN + plot_width,
            "X out of bounds for freq={}: x={}, bounds=[{}, {}]",
            freq, x, CHART_LEFT_MARGIN, CHART_LEFT_MARGIN + plot_width
        );

        assert!(
            y >= CHART_TOP_MARGIN && y <= CHART_TOP_MARGIN + TEST_PLOT_HEIGHT,
            "Y out of bounds for gain={}: y={}, bounds=[{}, {}]",
            gain, y, CHART_TOP_MARGIN, CHART_TOP_MARGIN + TEST_PLOT_HEIGHT
        );
    }
}

#[test]
fn test_calculate_plot_width() {
    let chart_width = 800.0;

    let short_labels = ["A", "B"];
    let plot_width_short = calculate_plot_width(chart_width, short_labels.iter().copied());

    let long_labels = ["#10 - PK @ 20000Hz (muted+solo)", "Combined response curve"];
    let plot_width_long = calculate_plot_width(chart_width, long_labels.iter().copied());

    assert!(
        plot_width_short > plot_width_long,
        "Short labels should give larger plot width: short={} long={}",
        plot_width_short, plot_width_long
    );

    assert!(plot_width_short > 0.0, "Plot width should be positive: {}", plot_width_short);
    assert!(plot_width_long > 0.0, "Plot width should be positive: {}", plot_width_long);
}

#[test]
fn test_freq_logarithmic_scaling() {
    let plot_width = 600.0;

    let x_100 = freq_to_x(100.0, plot_width);
    let x_200 = freq_to_x(200.0, plot_width);
    let x_1000 = freq_to_x(1000.0, plot_width);
    let x_2000 = freq_to_x(2000.0, plot_width);

    let octave_width_low = x_200 - x_100;
    let octave_width_high = x_2000 - x_1000;

    let rel_diff = (octave_width_high - octave_width_low).abs() / octave_width_low;
    assert!(
        rel_diff < 0.01,
        "Octave widths should be equal in log scale: low={} high={} diff={}",
        octave_width_low, octave_width_high, rel_diff
    );
}

#[test]
fn test_drag_delta_to_q_change() {
    let full_range_delta = 60.0;
    let q_change = drag_delta_to_q_change(full_range_delta);
    let expected_change = pk(EQ, "q").max_f64() - pk(EQ, "q").min_f64();

    assert!(
        (q_change - expected_change).abs() < 0.01,
        "60px drag should change Q by full range: got {} expected {}",
        q_change, expected_change
    );

    let negative_change = drag_delta_to_q_change(-30.0);
    assert!(negative_change < 0.0, "Negative drag should decrease Q: got {}", negative_change);
}

#[test]
fn test_calculate_response_at_freq() {
    let empty: Vec<EQFilter> = vec![];
    assert!(
        (calculate_response_at_freq(&empty, 1000.0) - 0.0).abs() < 0.001,
        "Empty filters should give 0 dB response"
    );

    let flat_filter = vec![EQFilter {
        frequency: 1000.0,
        q: 1.0,
        gain_db: 0.0,
        filter_type: BiquadFilterType::Peak,
        muted: false,
        solo: false,
    }];
    let response = calculate_response_at_freq(&flat_filter, 1000.0);
    assert!(response.abs() < 0.1, "0 dB gain filter should give ~0 dB response: got {}", response);

    let muted_filter = vec![EQFilter {
        frequency: 1000.0,
        q: 1.0,
        gain_db: 12.0,
        filter_type: BiquadFilterType::Peak,
        muted: true,
        solo: false,
    }];
    let muted_response = calculate_response_at_freq(&muted_filter, 1000.0);
    assert!(muted_response.abs() < 0.1, "Muted filter should give ~0 dB response: got {}", muted_response);
}

#[test]
fn test_calculate_response_solo() {
    let filters = vec![
        EQFilter {
            frequency: 100.0,
            q: 1.0,
            gain_db: 6.0,
            filter_type: BiquadFilterType::Peak,
            muted: false,
            solo: false,
        },
        EQFilter {
            frequency: 1000.0,
            q: 1.0,
            gain_db: 12.0,
            filter_type: BiquadFilterType::Peak,
            muted: false,
            solo: true,
        },
    ];

    let response_at_solo = calculate_response_at_freq(&filters, 1000.0);
    let solo_filter_only = vec![filters[1].clone()];
    let expected_response = calculate_response_at_freq(&solo_filter_only, 1000.0);

    assert!(
        (response_at_solo - expected_response).abs() < 0.1,
        "Solo filter should be the only contributor: got {} expected {}",
        response_at_solo, expected_response
    );
}

#[test]
fn test_calculate_band_response() {
    let filter = EQFilter {
        frequency: 1000.0,
        q: 1.0,
        gain_db: 6.0,
        filter_type: BiquadFilterType::Peak,
        muted: false,
        solo: false,
    };

    let response = calculate_band_response(&filter, 1000.0);
    assert!(
        (response - 6.0).abs() < 0.5,
        "Peak filter at center freq should show ~gain: got {} expected ~6.0",
        response
    );

    let far_response = calculate_band_response(&filter, 20.0);
    assert!(far_response.abs() < 1.0, "Peak filter far from center should be ~0: got {}", far_response);

    let muted_filter = EQFilter { muted: true, ..filter };
    let muted_response = calculate_band_response(&muted_filter, 1000.0);
    assert!(muted_response.abs() < 0.001, "Muted filter should return 0: got {}", muted_response);
}

#[test]
fn test_filter_type_index() {
    assert_eq!(get_filter_type_index(&BiquadFilterType::Peak), 0);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Lowshelf), 1);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Highshelf), 2);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Lowpass), 3);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Highpass), 4);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Bandpass), 5);
    assert_eq!(get_filter_type_index(&BiquadFilterType::Notch), 6);
    assert_eq!(get_filter_type_index(&BiquadFilterType::HighpassVariableQ), 4);
}
