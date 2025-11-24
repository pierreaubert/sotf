//! Tests for free filter type functionality

use autoeq::cli::{Args, PeqModel};
use autoeq::param_utils::{self};
use autoeq::x2peq::{peq2x, x2peq};
use clap::Parser;

#[test]
fn test_params_per_filter() {
    assert_eq!(param_utils::params_per_filter(PeqModel::Pk), 3);
    assert_eq!(param_utils::params_per_filter(PeqModel::HpPk), 3);
    assert_eq!(param_utils::params_per_filter(PeqModel::HpPkLp), 3);
    assert_eq!(param_utils::params_per_filter(PeqModel::FreePkFree), 4);
    assert_eq!(param_utils::params_per_filter(PeqModel::Free), 4);
}

#[test]
fn test_num_filters() {
    // Fixed filter types: 3 params per filter
    assert_eq!(param_utils::num_filters(&vec![0.0; 9], PeqModel::Pk), 3);
    assert_eq!(param_utils::num_filters(&vec![0.0; 15], PeqModel::HpPk), 5);

    // Free filter types: 4 params per filter
    assert_eq!(param_utils::num_filters(&vec![0.0; 12], PeqModel::Free), 3);
    assert_eq!(
        param_utils::num_filters(&vec![0.0; 20], PeqModel::FreePkFree),
        5
    );
}

#[test]
fn test_filter_params_extraction() {
    // Test fixed filter type extraction
    let x_fixed = vec![
        2.0, 1.5, 3.0, // Filter 1: freq=100Hz (log10), Q=1.5, gain=3dB
        3.0, 2.0, -6.0, // Filter 2: freq=1000Hz, Q=2.0, gain=-6dB
    ];

    let params1 = param_utils::get_filter_params(&x_fixed, 0, PeqModel::Pk);
    assert_eq!(params1.filter_type, None);
    assert_eq!(params1.freq, 2.0);
    assert_eq!(params1.q, 1.5);
    assert_eq!(params1.gain, 3.0);

    // Test free filter type extraction
    let x_free = vec![
        0.0, 2.0, 1.5, 3.0, // Filter 1: type=Peak, freq=100Hz, Q=1.5, gain=3dB
        1.0, 3.0, 1.0, 0.0, // Filter 2: type=Lowpass, freq=1000Hz, Q=1.0, gain=0dB
    ];

    let params2 = param_utils::get_filter_params(&x_free, 1, PeqModel::Free);
    assert_eq!(params2.filter_type, Some(1.0));
    assert_eq!(params2.freq, 3.0);
    assert_eq!(params2.q, 1.0);
    assert_eq!(params2.gain, 0.0);
}

#[test]
fn test_filter_type_encoding_decoding() {
    use autoeq::iir::BiquadFilterType;

    // Test encoding
    assert_eq!(param_utils::encode_filter_type(BiquadFilterType::Peak), 0.0);
    assert_eq!(
        param_utils::encode_filter_type(BiquadFilterType::Lowpass),
        1.0
    );
    assert_eq!(
        param_utils::encode_filter_type(BiquadFilterType::Highpass),
        2.0
    );
    assert_eq!(
        param_utils::encode_filter_type(BiquadFilterType::Lowshelf),
        3.0
    );

    // Test decoding
    assert_eq!(param_utils::decode_filter_type(0.0), BiquadFilterType::Peak);
    assert_eq!(
        param_utils::decode_filter_type(1.0),
        BiquadFilterType::Lowpass
    );
    assert_eq!(
        param_utils::decode_filter_type(2.0),
        BiquadFilterType::Highpass
    );
    assert_eq!(
        param_utils::decode_filter_type(3.5),
        BiquadFilterType::Lowshelf
    ); // Floors to 3
}

#[test]
fn test_x2peq_with_free_filters() {
    // Test FreePkFree model: first and last filters are free
    let x = vec![
        2.0, 2.0, 1.0, 0.0, // Filter 1: type=Highpass, freq=100Hz, Q=1.0, gain=0dB
        0.0, 3.0, 2.0, 6.0, // Filter 2: type=Peak, freq=1000Hz, Q=2.0, gain=6dB
        1.0, 4.0, 1.0, 0.0, // Filter 3: type=Lowpass, freq=10000Hz, Q=1.0, gain=0dB
    ];

    let peq = x2peq(&x, 48000.0, PeqModel::FreePkFree);
    assert_eq!(peq.len(), 3);

    // Check filter types
    use autoeq::iir::BiquadFilterType;
    assert_eq!(peq[0].1.filter_type, BiquadFilterType::Highpass);
    assert_eq!(peq[1].1.filter_type, BiquadFilterType::Peak);
    assert_eq!(peq[2].1.filter_type, BiquadFilterType::Lowpass);

    // Check frequencies
    assert!((peq[0].1.freq - 100.0).abs() < 0.1);
    assert!((peq[1].1.freq - 1000.0).abs() < 0.1);
    assert!((peq[2].1.freq - 10000.0).abs() < 0.1);
}

#[test]
fn test_peq2x_round_trip() {
    // Create parameter vector with free filters
    let x_original = vec![
        2.0, 2.0, 1.0, 0.0, // Filter 1: type=Highpass
        0.0, 3.0, 2.0, 6.0, // Filter 2: type=Peak
        1.0, 4.0, 1.0, 0.0, // Filter 3: type=Lowpass
    ];

    // Convert to PEQ and back
    let peq = x2peq(&x_original, 48000.0, PeqModel::FreePkFree);
    let x_recovered = peq2x(&peq, PeqModel::FreePkFree);

    // Check that we get the same parameters back
    assert_eq!(x_original.len(), x_recovered.len());
    for i in 0..x_original.len() {
        assert!(
            (x_original[i] - x_recovered[i]).abs() < 1e-6,
            "Parameter {} mismatch: {} vs {}",
            i,
            x_original[i],
            x_recovered[i]
        );
    }
}

#[test]
fn test_bounds_with_free_filters() {
    let mut args = Args::parse_from(["test", "--num-filters", "3", "--peq-model", "free"]);

    args.min_freq = 20.0;
    args.max_freq = 20000.0;
    args.min_q = 0.5;
    args.max_q = 10.0;
    args.min_db = 1.0;
    args.max_db = 12.0;

    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_bounds(&args);

    // Should have 4 params per filter for Free model
    assert_eq!(lower_bounds.len(), 12);
    assert_eq!(upper_bounds.len(), 12);

    // Check filter type bounds (first param of each filter)
    for i in 0..3 {
        let type_idx = i * 4;
        assert_eq!(lower_bounds[type_idx], 0.0);
        assert!(upper_bounds[type_idx] > 7.0 && upper_bounds[type_idx] < 8.0);
    }
}

#[test]
fn test_constraint_violations_with_free_filters() {
    use autoeq::constraints::min_gain::viol_min_gain_from_xs;
    use autoeq::constraints::min_spacing::viol_spacing_from_xs;

    // Test spacing constraint with free filters
    let x = vec![
        0.0, 2.0, 1.0, 3.0, // Filter 1: Peak at 100Hz
        0.0, 2.1, 1.0, -3.0, // Filter 2: Peak at ~125Hz (too close!)
        0.0, 3.0, 1.0, 6.0, // Filter 3: Peak at 1000Hz
    ];

    let spacing_viol = viol_spacing_from_xs(&x, PeqModel::Free, 0.5);
    assert!(spacing_viol > 0.0, "Should have spacing violation");

    // Test min gain constraint with free filters
    let x_low_gain = vec![
        2.0, 2.0, 1.0, 0.0, // Filter 1: Highpass (not checked)
        0.0, 3.0, 1.0, 0.5, // Filter 2: Peak with 0.5dB gain (too low!)
        1.0, 4.0, 1.0, 0.0, // Filter 3: Lowpass (not checked)
    ];

    let gain_viol = viol_min_gain_from_xs(&x_low_gain, PeqModel::FreePkFree, 1.0);
    assert!(gain_viol > 0.0, "Should have min gain violation");
}
