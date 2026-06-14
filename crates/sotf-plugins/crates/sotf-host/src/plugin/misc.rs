pub(super) fn samples_to_ppq(sample_position: u64, sample_rate: u32, bpm: f64) -> f64 {
    if sample_rate == 0 || !bpm.is_finite() || bpm <= 0.0 {
        return 0.0;
    }
    sample_position as f64 / sample_rate as f64 * bpm / 60.0
}
