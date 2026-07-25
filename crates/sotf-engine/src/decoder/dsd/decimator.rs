use super::consts::DSD_TO_PCM_DECIMATION;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

const CIC_ORDER: usize = 5;
const CIC_FACTOR: usize = 8;
const INTERMEDIATE_FACTOR: usize = 4;
const FINAL_FACTOR: usize = 2;
const INTERMEDIATE_TAPS: usize = 64;
const FINAL_TAPS: usize = 128;
/// Reserve 6.94 dB so even a pathological bit pattern matched to the
/// reconstruction impulse response remains inside floating-point full scale.
const DSD_PCM_GAIN: f64 = 0.45;

/// Number of decimated outputs removed to compensate the linear-phase delay.
///
/// The cascaded delay is 2301.5 DSD samples. Output number 35 is observed at
/// DSD sample 2303, leaving only 1.5 DSD samples (0.0234 PCM sample) of residual
/// alignment error.
pub(super) const ALIGNMENT_OUTPUTS: u64 = 35;
/// A seek starts this many PCM frames early so the desired sample has complete
/// FIR history after the decimator state is rebuilt.
pub(super) const SEEK_PREROLL_FRAMES: u64 = 36;

pub(super) struct DsdPcmDecimator {
    channels: Vec<ChannelDecimator>,
    output: Vec<f32>,
}

impl DsdPcmDecimator {
    pub(super) fn new(channels: usize) -> Self {
        Self {
            channels: (0..channels).map(|_| ChannelDecimator::new()).collect(),
            output: vec![0.0; channels],
        }
    }

    pub(super) fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.output.fill(0.0);
    }

    /// Push one DSD sample per channel. Returns a PCM frame every 64 pushes.
    pub(super) fn push(&mut self, input: &[f64]) -> Option<&[f32]> {
        debug_assert_eq!(self.channels.len(), input.len());
        let mut produced = true;
        for (index, (channel, &sample)) in self.channels.iter_mut().zip(input).enumerate() {
            if let Some(value) = channel.push(sample) {
                self.output[index] = (value * DSD_PCM_GAIN) as f32;
            } else {
                produced = false;
            }
        }
        produced.then_some(self.output.as_slice())
    }
}

struct ChannelDecimator {
    cic: FirDecimator,
    intermediate: FirDecimator,
    final_stage: FirDecimator,
}

impl ChannelDecimator {
    fn new() -> Self {
        Self {
            cic: FirDecimator::new(cic_taps(), CIC_FACTOR),
            // At 8 × PCM rate, this stage passes audio and rejects the first
            // image that would alias into the final audio band when /4.
            intermediate: FirDecimator::new(intermediate_taps(), INTERMEDIATE_FACTOR),
            // At 2 × PCM rate, cutoff 0.25 is midway between the 0.45 × PCM
            // passband and 0.55 × PCM stopband.
            final_stage: FirDecimator::new(final_taps(), FINAL_FACTOR),
        }
    }

    fn reset(&mut self) {
        self.cic.reset();
        self.intermediate.reset();
        self.final_stage.reset();
    }

    fn push(&mut self, sample: f64) -> Option<f64> {
        let stage_one = self.cic.push(sample)?;
        let stage_two = self.intermediate.push(stage_one)?;
        self.final_stage.push(stage_two)
    }
}

struct FirDecimator {
    taps: Arc<[f64]>,
    delay: Vec<f64>,
    write: usize,
    phase: usize,
    factor: usize,
}

impl FirDecimator {
    fn new(taps: Arc<[f64]>, factor: usize) -> Self {
        Self {
            delay: vec![0.0; taps.len()],
            taps,
            write: 0,
            phase: 0,
            factor,
        }
    }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.write = 0;
        self.phase = 0;
    }

    #[inline]
    fn push(&mut self, sample: f64) -> Option<f64> {
        self.delay[self.write] = sample;
        self.write += 1;
        if self.write == self.delay.len() {
            self.write = 0;
        }

        self.phase += 1;
        if self.phase != self.factor {
            return None;
        }
        self.phase = 0;

        let mut value = 0.0;
        let mut index = if self.write == 0 {
            self.delay.len() - 1
        } else {
            self.write - 1
        };
        for &tap in self.taps.iter() {
            value += tap * self.delay[index];
            index = if index == 0 {
                self.delay.len() - 1
            } else {
                index - 1
            };
        }
        Some(value)
    }
}

fn cic_taps() -> Arc<[f64]> {
    static TAPS: OnceLock<Arc<[f64]>> = OnceLock::new();
    Arc::clone(TAPS.get_or_init(|| {
        let mut taps = vec![1.0];
        for _ in 0..CIC_ORDER {
            let mut next = vec![0.0; taps.len() + CIC_FACTOR - 1];
            for (index, &value) in taps.iter().enumerate() {
                for offset in 0..CIC_FACTOR {
                    next[index + offset] += value;
                }
            }
            taps = next;
        }
        normalize(&mut taps);
        taps.into()
    }))
}

fn intermediate_taps() -> Arc<[f64]> {
    static TAPS: OnceLock<Arc<[f64]>> = OnceLock::new();
    Arc::clone(TAPS.get_or_init(|| lowpass_taps(INTERMEDIATE_TAPS, 0.125).into()))
}

fn final_taps() -> Arc<[f64]> {
    static TAPS: OnceLock<Arc<[f64]>> = OnceLock::new();
    Arc::clone(TAPS.get_or_init(|| lowpass_taps(FINAL_TAPS, 0.25).into()))
}

fn lowpass_taps(length: usize, cutoff: f64) -> Vec<f64> {
    debug_assert!(length >= 2);
    debug_assert!(cutoff > 0.0 && cutoff < 0.5);
    let center = (length - 1) as f64 * 0.5;
    let mut taps = Vec::with_capacity(length);
    for index in 0..length {
        let offset = index as f64 - center;
        let sinc = if offset.abs() < f64::EPSILON {
            2.0 * cutoff
        } else {
            (2.0 * PI * cutoff * offset).sin() / (PI * offset)
        };
        let phase = 2.0 * PI * index as f64 / (length - 1) as f64;
        let blackman = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
        taps.push(sinc * blackman);
    }
    normalize(&mut taps);
    taps
}

fn normalize(taps: &mut [f64]) {
    let sum: f64 = taps.iter().sum();
    for tap in taps {
        *tap /= sum;
    }
}

const _: () =
    assert!(CIC_FACTOR * INTERMEDIATE_FACTOR * FINAL_FACTOR == DSD_TO_PCM_DECIMATION as usize);

#[cfg(test)]
mod tests {
    use super::*;

    fn render_tone(frequency_in_pcm_rates: f64, output_frames: usize) -> Vec<f32> {
        let mut decimator = DsdPcmDecimator::new(1);
        let input_samples = (output_frames + 100) * DSD_TO_PCM_DECIMATION as usize;
        let mut output = Vec::new();
        for sample in 0..input_samples {
            let phase =
                2.0 * PI * frequency_in_pcm_rates * sample as f64 / DSD_TO_PCM_DECIMATION as f64;
            if let Some(frame) = decimator.push(&[phase.sin()]) {
                output.push(frame[0]);
            }
        }
        output
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples
            .iter()
            .map(|&sample| f64::from(sample).powi(2))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
    }

    #[test]
    fn constant_signal_uses_dsd_reconstruction_headroom() {
        let mut decimator = DsdPcmDecimator::new(1);
        let mut last = 0.0;
        for _ in 0..(128 * DSD_TO_PCM_DECIMATION as usize) {
            if let Some(frame) = decimator.push(&[1.0]) {
                last = frame[0];
            }
        }
        assert!((last - DSD_PCM_GAIN as f32).abs() < 1e-6);
    }

    #[test]
    fn audio_passband_is_flat_within_quarter_db() {
        let output = render_tone(0.4, 4096);
        let measured = rms(&output[256..]);
        let db = 20.0 * (measured / ((0.5f64).sqrt() * DSD_PCM_GAIN)).log10();
        assert!(db > -0.25 && db < 0.05, "passband gain was {db:.3} dB");
    }

    #[test]
    fn first_alias_image_is_rejected() {
        let passband = render_tone(0.2, 4096);
        let alias = render_tone(0.8, 4096);
        let attenuation_db = 20.0 * (rms(&alias[256..]) / rms(&passband[256..])).log10();
        assert!(
            attenuation_db < -70.0,
            "alias attenuation was only {attenuation_db:.1} dB"
        );
    }

    #[test]
    fn impulse_response_l1_norm_keeps_any_bounded_input_below_full_scale() {
        let mut worst_case_gain = 0.0f64;
        let response_samples = 128 * DSD_TO_PCM_DECIMATION as usize;

        // Decimation is periodically time-varying. Sum the absolute impulse
        // responses for all 64 input phases to bound any input in [-1, 1].
        for phase in 0..DSD_TO_PCM_DECIMATION as usize {
            let mut decimator = DsdPcmDecimator::new(1);
            for sample in 0..response_samples {
                let input = if sample == phase { 1.0 } else { 0.0 };
                if let Some(frame) = decimator.push(&[input]) {
                    worst_case_gain += f64::from(frame[0].abs());
                }
            }
        }
        assert!(
            worst_case_gain <= 1.0,
            "worst-case reconstruction gain was {worst_case_gain}"
        );
    }
}
