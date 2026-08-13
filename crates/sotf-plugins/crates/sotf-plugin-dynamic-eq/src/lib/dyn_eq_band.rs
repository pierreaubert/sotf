use super::misc::bandpass_edges;
use crate::params::{default_band_ratio, default_band_threshold};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_host::dynamics_core::DynamicsCore;
use sotf_host::dynamics_core::DynamicsMode;

pub(super) struct DynEqBand {
    // EQ parameters
    pub(super) frequency: f32,
    pub(super) q: f32,
    pub(super) target_gain_db: f32,

    // Per-band dynamics overrides
    pub(super) band_threshold: f32,
    pub(super) band_ratio: f32,
    pub(super) use_band_threshold: bool,
    pub(super) use_band_ratio: bool,

    // Band control
    pub(super) active: bool,
    pub(super) solo: bool,

    // DSP state (pre-allocated for max channels)
    /// Highpass filter per channel (lower bound of sidechain BPF)
    pub(super) sidechain_bp_hp: Vec<Biquad>,
    /// Lowpass filter per channel (upper bound of sidechain BPF)
    pub(super) sidechain_bp_lp: Vec<Biquad>,
    /// The actual EQ biquad per channel — held at target_gain_db (static coefficients).
    /// Gain modulation is applied as a dry/wet blend, not via coefficient updates.
    pub(super) eq_filters: Vec<Biquad>,
    /// One DynamicsCore per channel
    pub(super) cores: Vec<DynamicsCore>,
}

impl DynEqBand {
    pub(super) fn new(
        channels: usize,
        sample_rate: u32,
        frequency: f32,
        q: f32,
        target_gain_db: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Self {
        let (f_low, f_high) = bandpass_edges(frequency, q);

        let sidechain_bp_hp = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highpass,
                    f_low as f64,
                    sample_rate as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect();

        let sidechain_bp_lp = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Lowpass,
                    f_high as f64,
                    sample_rate as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect();

        let eq_filters = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Peak,
                    frequency as f64,
                    sample_rate as f64,
                    q as f64,
                    0.0, // starts at 0 dB (passthrough)
                )
            })
            .collect();

        let mut cores: Vec<DynamicsCore> = (0..channels)
            .map(|_| DynamicsCore::new(DynamicsMode::Compress, 1, sample_rate))
            .collect();
        for core in &mut cores {
            core.set_attack_release(attack_ms, release_ms);
        }

        Self {
            frequency,
            q,
            target_gain_db,
            band_threshold: default_band_threshold(),
            band_ratio: default_band_ratio(),
            use_band_threshold: false,
            use_band_ratio: false,
            active: true,
            solo: false,
            sidechain_bp_hp,
            sidechain_bp_lp,
            eq_filters,
            cores,
        }
    }

    /// Process the sidechain bandpass filter on a sample for a given channel.
    ///
    /// Accepts and returns f64 to avoid unnecessary round-trip conversions; the
    /// internal biquads already operate in f64.
    #[inline]
    pub(super) fn apply_sidechain_bp(&mut self, ch: usize, sample: f64) -> f64 {
        let hp_out = self.sidechain_bp_hp[ch].process(sample);
        self.sidechain_bp_lp[ch].process(hp_out)
    }

    /// Compute the proportion of EQ gain to apply based on gain reduction from the
    /// dynamics core. Returns a value in [0.0, 1.0] representing how much of the
    /// full EQ band shape to blend in.
    #[inline]
    pub(super) fn modulation_proportion(target_gain_db: f32, gain_reduction_db: f32) -> f32 {
        if target_gain_db.abs() < 0.01 {
            return 0.0;
        }
        let applied_db =
            gain_reduction_db.clamp(0.0, target_gain_db.abs()) * target_gain_db.signum();
        let full_amplitude = 10.0f32.powf(target_gain_db / 20.0);
        let desired_amplitude = 10.0f32.powf(applied_db / 20.0);
        ((desired_amplitude - 1.0) / (full_amplitude - 1.0)).clamp(0.0, 1.0)
    }

    fn max_frequency(sample_rate: u32) -> f32 {
        (sample_rate as f32 * 0.475).min(20_000.0)
    }

    pub(super) fn rebuild_sidechain_filters(&mut self, sample_rate: u32) {
        self.frequency = self.frequency.clamp(20.0, Self::max_frequency(sample_rate));
        let (f_low, f_high) = bandpass_edges(self.frequency, self.q);
        let f_high = f_high.min(Self::max_frequency(sample_rate));
        for hp in &mut self.sidechain_bp_hp {
            *hp = Biquad::new(
                BiquadFilterType::Highpass,
                f_low as f64,
                sample_rate as f64,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
            );
        }
        for lp in &mut self.sidechain_bp_lp {
            *lp = Biquad::new(
                BiquadFilterType::Lowpass,
                f_high as f64,
                sample_rate as f64,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
            );
        }
    }

    pub(super) fn rebuild_eq_filters(&mut self, sample_rate: u32) {
        self.frequency = self.frequency.clamp(20.0, Self::max_frequency(sample_rate));
        // EQ filters are held at target_gain_db; modulation is a dry/wet blend
        for eq in &mut self.eq_filters {
            *eq = Biquad::new(
                BiquadFilterType::Peak,
                self.frequency as f64,
                sample_rate as f64,
                self.q as f64,
                self.target_gain_db as f64,
            );
        }
    }

    pub(super) fn reset(&mut self, sample_rate: u32) {
        self.rebuild_sidechain_filters(sample_rate);
        self.rebuild_eq_filters(sample_rate);
        for core in &mut self.cores {
            core.reset();
        }
    }

    pub(super) fn get_effective_threshold(&self, global_threshold: f32) -> f32 {
        if self.use_band_threshold {
            self.band_threshold
        } else {
            global_threshold
        }
    }

    pub(super) fn get_effective_ratio(&self, global_ratio: f32) -> f32 {
        if self.use_band_ratio {
            self.band_ratio
        } else {
            global_ratio
        }
    }
}
