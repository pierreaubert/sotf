pub(crate) fn should_use_peak_spread(channel_count: usize) -> bool {
    channel_count > 2
}

pub(crate) fn peak_spread_db(true_peaks_dbtp: &[f64]) -> f64 {
    if true_peaks_dbtp.is_empty() {
        return 0.0;
    }

    let mut min_peak = f64::INFINITY;
    let mut max_peak = f64::NEG_INFINITY;

    for &peak in true_peaks_dbtp {
        let peak = if peak.is_finite() { peak } else { -60.0 };
        let peak = peak.clamp(-60.0, 6.0);
        min_peak = min_peak.min(peak);
        max_peak = max_peak.max(peak);
    }

    if min_peak.is_finite() && max_peak.is_finite() {
        max_peak - min_peak
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::super::{peak_spread_db, should_use_peak_spread};
    use super::*;

    #[test]
    fn peak_spread_is_multichannel_only() {
        assert!(!should_use_peak_spread(0));
        assert!(!should_use_peak_spread(1));
        assert!(!should_use_peak_spread(2));
        assert!(should_use_peak_spread(3));
    }

    #[test]
    fn peak_spread_equal_peaks_is_zero() {
        assert_eq!(peak_spread_db(&[-3.0, -3.0, -3.0, -3.0]), 0.0);
    }

    #[test]
    fn peak_spread_uses_max_minus_min() {
        assert_eq!(peak_spread_db(&[-12.0, -9.5, -3.0, -6.0]), 9.0);
    }

    #[test]
    fn peak_spread_handles_silent_or_unavailable_values() {
        assert_eq!(peak_spread_db(&[]), 0.0);
        assert_eq!(peak_spread_db(&[f64::NEG_INFINITY, f64::NAN]), 0.0);
        assert_eq!(peak_spread_db(&[-12.0, f64::NEG_INFINITY]), 48.0);
    }

    #[test]
    fn peak_spread_clamps_values_before_spread() {
        assert_eq!(peak_spread_db(&[-120.0, 20.0]), 66.0);
    }
}
