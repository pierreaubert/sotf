/// Number of ISO 226 compensation filters per channel.
pub(super) const ISO_FILTER_COUNT: usize = 21;

/// Center frequencies for the jointly fitted ISO 226 compensation bank.
pub(super) const ISO_BAND_FREQS: [f64; ISO_FILTER_COUNT] = [
    20.0, 20.0, 31.5, 50.0, 80.0, 125.0, 200.0, 315.0, 500.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 12500.0,
];

/// Q factors for the 7 ISO 226 compensation bands.
pub(super) const ISO_BAND_QS: [f64; ISO_FILTER_COUNT] = [
    0.8, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.5, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0,
    2.5, 0.8,
];
