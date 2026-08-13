/// Crossover frequency for dual-band decoding (Hz).
pub(super) const DUAL_BAND_CROSSOVER_HZ: f32 = 700.0;

/// Maximum number of Ambisonics input channels: (MAX_ORDER+1)² = 16.
pub(super) const MAX_AMBI_CHANNELS: usize =
    (super::spherical_harmonics::MAX_ORDER + 1) * (super::spherical_harmonics::MAX_ORDER + 1);
