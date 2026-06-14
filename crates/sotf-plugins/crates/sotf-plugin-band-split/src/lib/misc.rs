use crate::params::CROSSOVER_TYPES;

pub(super) fn parse_crossover_type_index(input: &str) -> usize {
    CROSSOVER_TYPES
        .iter()
        .position(|t| t.eq_ignore_ascii_case(input))
        .unwrap_or(0)
}

/// Maximum number of bands supported.
pub(super) const MAX_BANDS: usize = 4;
