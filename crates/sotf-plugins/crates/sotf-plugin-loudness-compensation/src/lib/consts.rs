/// Number of ISO 226 compensation filters per channel.
pub(super) const ISO_FILTER_COUNT: usize = 7;

/// Center frequencies for the 7 ISO 226 compensation bands.
pub(super) const ISO_BAND_FREQS: [f64; ISO_FILTER_COUNT] =
    [50.0, 150.0, 500.0, 1500.0, 3500.0, 7000.0, 10000.0];

/// Q factors for the 7 ISO 226 compensation bands.
pub(super) const ISO_BAND_QS: [f64; ISO_FILTER_COUNT] = [0.7, 0.8, 1.0, 1.2, 1.5, 1.2, 0.8];
