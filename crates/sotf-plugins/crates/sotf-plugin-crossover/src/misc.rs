pub(super) fn is_linear_phase_type(crossover_type: &str) -> bool {
    matches!(
        crossover_type.to_ascii_lowercase().as_str(),
        "linearphase" | "linear_phase" | "linear-phase" | "linearphasefir" | "fir" | "lpfir"
    )
}
