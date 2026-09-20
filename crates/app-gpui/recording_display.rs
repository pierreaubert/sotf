// A missing delay is not evidence of zero alignment correction.
pub(crate) fn probe_alignment_text(delay_ms: Option<f64>) -> String {
    delay_ms
        .filter(|value| value.is_finite())
        .map_or_else(|| "—".into(), |value| format!("{value:.2}"))
}

#[cfg(test)]
mod alignment_display_tests {
    use super::probe_alignment_text;

    #[test]
    fn missing_probe_alignment_is_distinct_from_measured_zero() {
        assert_eq!(probe_alignment_text(None), "—");
        assert_eq!(probe_alignment_text(Some(0.0)), "0.00");
        assert_eq!(probe_alignment_text(Some(1.25)), "1.25");
        assert_eq!(probe_alignment_text(Some(f64::NAN)), "—");
        assert_eq!(probe_alignment_text(Some(f64::INFINITY)), "—");
    }
}
