#![allow(dead_code)]
use crate::params::AaePluginParams;
use sotf_host::auto_gain::{AutoGainLoudnessType, AutoGainParams};
use sotf_host::multichannel_auto_gain::MultichannelAutoGain;

/// Always allocates a `MultichannelAutoGain` instance so enabling it via
/// `set_parameter` on the audio thread does not trigger a heap allocation.
/// The instance starts enabled or disabled depending on
/// `params.auto_gain_enabled`; callers can flip the flag at runtime.
pub(super) fn create_auto_gain(
    params: &AaePluginParams,
    sample_rate: u32,
) -> Option<MultichannelAutoGain> {
    MultichannelAutoGain::new(
        sample_rate,
        AutoGainParams {
            enabled: params.auto_gain_enabled,
            loudness_type: AutoGainLoudnessType::Momentary,
            max_gain_db: params.auto_gain_max_db,
            smoothing_ms: params.auto_gain_smoothing_ms,
        },
    )
    .map_err(|err| log::warn!("AAE auto-gain initialization failed: {err}"))
    .ok()
}

#[derive(Clone, Copy)]
struct BiquadLowpass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl BiquadLowpass {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let omega = std::f32::consts::TAU * cutoff_hz / sample_rate;
        let (sin, cos) = omega.sin_cos();
        let alpha = sin / std::f32::consts::SQRT_2;
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos) * 0.5) / a0,
            b1: (1.0 - cos) / a0,
            b2: ((1.0 - cos) * 0.5) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// Fourth-order Linkwitz-Riley low-pass used by the synthesized LFE effects send.
pub(super) struct LfeLowpass {
    stages: [BiquadLowpass; 2],
}

impl LfeLowpass {
    pub(super) fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let stage = BiquadLowpass::new(cutoff_hz, sample_rate);
        Self {
            stages: [stage, stage],
        }
    }

    #[inline]
    pub(super) fn process(&mut self, input: f32) -> f32 {
        self.stages
            .iter_mut()
            .fold(input, |sample, stage| stage.process(sample))
    }

    pub(super) fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }
}

pub(super) fn ms_to_samples(time_ms: f32, sample_rate: u32) -> usize {
    (time_ms * 0.001 * sample_rate as f32).round().max(1.0) as usize
}

pub(super) fn signed_rms(_sum: f32, energy: f32, count: usize, _polarity_hint: f32) -> f32 {
    if count == 0 || energy <= 0.0 {
        return 0.0;
    }

    (energy / count as f32).sqrt()
}
