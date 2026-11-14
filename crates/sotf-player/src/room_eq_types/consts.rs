/// Default crossover reference key emitted into recordings.json when a
/// bass-managed sub channel is detected and the caller has no explicit
/// crossover choice. Matches the key used in `default_bass_management_crossovers`.
pub const DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY: &str = "lfe_xover";

/// Default FIR length for the linear-phase crossover. Power-of-two friendly,
/// matches the plugin-side `DEFAULT_FIR_CROSSOVER_TAPS` default.
pub const DEFAULT_LINEAR_PHASE_CROSSOVER_TAPS: usize = 4096;

/// Classical sample rates for audio (44.1k and 48k families, up to 8x).
pub const CLASSICAL_SAMPLE_RATES: &[usize] = &[
    44100, 48000, 88200, 96000, 176_400, 192_000, 352_800, 384_000,
];

/// Given a raw sample-rate value, snap to the next classical rate above it.
/// Returns the highest rate if already at or above the top.
#[must_use]
pub fn next_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES {
        if rate > current {
            return rate;
        }
    }
    CLASSICAL_SAMPLE_RATES.last().copied().unwrap_or(current)
}

/// Given a raw sample-rate value, snap to the previous classical rate below it.
/// Returns the lowest rate if already at or below the bottom.
#[must_use]
pub fn prev_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES.iter().rev() {
        if rate < current {
            return rate;
        }
    }
    CLASSICAL_SAMPLE_RATES.first().copied().unwrap_or(current)
}
