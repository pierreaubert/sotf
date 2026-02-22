// ============================================================================
// Auto Gain Compensation
// ============================================================================

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::simd::enable_ftz_daz;
use crate::smoothing::Smoother;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AutoGainLoudnessType {
    #[default]
    Momentary,
    ShortTerm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainParams {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub loudness_type: AutoGainLoudnessType,
    #[serde(default = "default_max_gain_db")]
    pub max_gain_db: f32,
    #[serde(default = "default_smoothing_ms")]
    pub smoothing_ms: f32,
}

fn default_enabled() -> bool {
    false
}
fn default_max_gain_db() -> f32 {
    6.0
}
fn default_smoothing_ms() -> f32 {
    100.0
}

impl Default for AutoGainParams {
    fn default() -> Self {
        Self {
            enabled: false,
            loudness_type: AutoGainLoudnessType::Momentary,
            max_gain_db: 6.0,
            smoothing_ms: 100.0,
        }
    }
}

pub struct AutoGain {
    num_channels: usize,
    sample_rate: u32,
    input_monitor: LoudnessMonitor,
    output_monitor: LoudnessMonitor,
    gain_smoother: Smoother,
    current_gain_linear: f32,
    last_input_lufs: f64,
    last_output_lufs: f64,
    last_input_peak: f64,
    last_output_peak: f64,
    enabled: bool,
    loudness_type: AutoGainLoudnessType,
    max_gain_db: f32,
    smoothing_ms: f32,
    /// Fast attack coefficient (~20ms) for gain decreases (output too loud)
    attack_coeff: f32,
    /// Slow release coefficient (~300ms) for gain increases (output recovered)
    release_coeff: f32,
}

impl std::fmt::Debug for AutoGain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let current_db = if self.current_gain_linear > 1e-10 {
            20.0 * self.current_gain_linear.log10()
        } else {
            -200.0
        };
        f.debug_struct("AutoGain")
            .field("enabled", &self.enabled)
            .field("gain_db", &current_db)
            .finish_non_exhaustive()
    }
}

impl AutoGain {
    pub fn new(
        num_channels: usize,
        sample_rate: u32,
        params: AutoGainParams,
    ) -> Result<Self, String> {
        Ok(Self {
            num_channels,
            sample_rate,
            input_monitor: LoudnessMonitor::new(num_channels as u32, sample_rate)?,
            output_monitor: LoudnessMonitor::new(num_channels as u32, sample_rate)?,
            gain_smoother: Smoother::new(0.0, params.smoothing_ms, sample_rate),
            current_gain_linear: 1.0,
            last_input_lufs: f64::NEG_INFINITY,
            last_output_lufs: f64::NEG_INFINITY,
            last_input_peak: 0.0,
            last_output_peak: 0.0,
            enabled: params.enabled,
            loudness_type: params.loudness_type,
            max_gain_db: params.max_gain_db,
            smoothing_ms: params.smoothing_ms,
            attack_coeff: (-1.0 / (20.0 * 0.001 * sample_rate as f32)).exp(),
            release_coeff: (-1.0 / (300.0 * 0.001 * sample_rate as f32)).exp(),
        })
    }

    pub fn new_default(num_channels: usize, sample_rate: u32) -> Result<Self, String> {
        Self::new(num_channels, sample_rate, Default::default())
    }

    pub fn set_sample_rate(&mut self, sr: u32) -> Result<(), String> {
        self.sample_rate = sr;
        self.input_monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        self.output_monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        self.gain_smoother.set_time(self.smoothing_ms, sr);
        self.attack_coeff = (-1.0 / (20.0 * 0.001 * sr as f32)).exp();
        self.release_coeff = (-1.0 / (300.0 * 0.001 * sr as f32)).exp();
        Ok(())
    }

    pub fn reset(&mut self) {
        let _ = self.input_monitor.reset();
        let _ = self.output_monitor.reset();
        self.gain_smoother.reset(0.0);
        self.current_gain_linear = 1.0;
        self.last_input_lufs = f64::NEG_INFINITY;
        self.last_output_lufs = f64::NEG_INFINITY;
        self.last_input_peak = 0.0;
        self.last_output_peak = 0.0;
    }

    pub fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
        if !e {
            self.gain_smoother.set_target(0.0);
        }
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_max_gain_db(&mut self, m: f32) {
        self.max_gain_db = m.abs();
    }
    pub fn set_smoothing_ms(&mut self, s: f32) {
        self.smoothing_ms = s;
        self.gain_smoother.set_time(s, self.sample_rate);
    }
    pub fn set_loudness_type(&mut self, t: AutoGainLoudnessType) {
        self.loudness_type = t;
    }

    pub fn measure_input(&mut self, input: &[f32]) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.input_monitor.add_frames(input)?;
        let info = self.input_monitor.get_loudness();
        self.last_input_lufs = if self.loudness_type == AutoGainLoudnessType::Momentary {
            info.momentary_lufs
        } else {
            info.shortterm_lufs
        };
        self.last_input_peak = info.peak;
        Ok(())
    }

    pub fn measure_output(&mut self, output: &[f32]) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.output_monitor.add_frames(output)?;
        let info = self.output_monitor.get_loudness();
        self.last_output_lufs = if self.loudness_type == AutoGainLoudnessType::Momentary {
            info.momentary_lufs
        } else {
            info.shortterm_lufs
        };
        self.last_output_peak = info.peak;
        if self.last_input_lufs.is_finite() && self.last_output_lufs.is_finite() {
            let target = (self.last_input_lufs - self.last_output_lufs) as f32;
            self.gain_smoother
                .set_target(target.clamp(-self.max_gain_db, self.max_gain_db));
        }
        Ok(())
    }

    #[inline]
    pub fn next_gain_linear(&mut self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let target_db = self.gain_smoother.next();
        let target_linear = 10.0_f32.powf(target_db / 20.0);
        
        // Asymmetric smoothing in linear domain
        let coeff = if target_linear < self.current_gain_linear {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.current_gain_linear = target_linear + coeff * (self.current_gain_linear - target_linear);
        self.current_gain_linear
    }

    pub fn current_gain_db(&self) -> f32 {
        if !self.enabled {
            0.0
        } else if self.current_gain_linear > 1e-10 {
            20.0 * self.current_gain_linear.log10()
        } else {
            -200.0
        }
    }
    pub fn last_input_lufs(&self) -> f64 {
        self.last_input_lufs
    }
    pub fn last_output_lufs(&self) -> f64 {
        self.last_output_lufs
    }
    pub fn last_input_peak(&self) -> f64 {
        self.last_input_peak
    }
    pub fn last_output_peak(&self) -> f64 {
        self.last_output_peak
    }

    pub fn apply_compensation(&mut self, output: &mut [f32], num_frames: usize) {
        if !self.enabled {
            return;
        }
        enable_ftz_daz();
        
        // Convert target DB to linear once per block to avoid powf in the loop
        let target_db = self.gain_smoother.target();
        let target_linear = 10.0_f32.powf(target_db / 20.0);

        for frame in 0..num_frames {
            // Asymmetric smoothing in linear domain: fast attack (gain decrease), slow release (gain increase)
            let coeff = if target_linear < self.current_gain_linear {
                self.attack_coeff  // reducing gain: fast (~20ms)
            } else {
                self.release_coeff // increasing gain: slow (~300ms)
            };
            self.current_gain_linear = target_linear + coeff * (self.current_gain_linear - target_linear);
            
            let gain = self.current_gain_linear;
            for ch in 0..self.num_channels {
                output[frame * self.num_channels + ch] *= gain;
            }
        }
    }

    pub fn get_data(&self) -> AutoGainData {
        let current_db = if self.enabled && self.current_gain_linear > 1e-10 {
            20.0 * self.current_gain_linear.log10()
        } else {
            0.0
        };
        AutoGainData {
            enabled: self.enabled,
            gain_db: current_db,
            input_lufs: self.last_input_lufs,
            output_lufs: self.last_output_lufs,
            input_peak: self.last_input_peak,
            output_peak: self.last_output_peak,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainData {
    pub enabled: bool,
    pub gain_db: f32,
    pub input_lufs: f64,
    pub output_lufs: f64,
    pub input_peak: f64,
    pub output_peak: f64,
}
#[cfg(test)]
mod tests {
    use crate::auto_gain::*;

    #[test]
    fn test_autogain_convergence() {
        let sample_rate = 48000;
        let mut ag = AutoGain::new(
            2,
            sample_rate,
            AutoGainParams {
                enabled: true,
                loudness_type: AutoGainLoudnessType::Momentary,
                max_gain_db: 12.0,
                smoothing_ms: 50.0,
            },
        ).unwrap();

        // Target: match input loudness.
        // Input: 0.5 amplitude sine wave.
        // Process: apply a -6dB attenuation (gain = 0.5).
        // AutoGain should compensate with +6dB (gain = 2.0).

        let block_size = 1024;
        let num_blocks = 50; // Enough for convergence at 50ms smoothing
        
        let current_signal_gain = 0.5_f32; // -6dB attenuation in the "effect"

        for block in 0..num_blocks {
            let mut input = vec![0.0_f32; block_size * 2];
            for i in 0..block_size {
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * (block * block_size + i) as f32 / sample_rate as f32;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            ag.measure_input(&input).unwrap();

            // Simulate effect: attenuate by 6dB
            let mut output = input.clone();
            for s in &mut output {
                *s *= current_signal_gain;
            }

            // FEED-FORWARD measurement (on uncompensated output)
            ag.measure_output(&output).unwrap();
            
            // Apply compensation
            ag.apply_compensation(&mut output, block_size);

            if block == num_blocks - 1 {
                let gain_db = ag.current_gain_db();
                // Should be close to +6.0 dB
                assert!((gain_db - 6.0).abs() < 0.5, "AutoGain did not converge to +6dB, got {}dB", gain_db);
            }
        }
    }

    #[test]
    fn test_autogain_no_oscillation() {
        let sample_rate = 48000;
        let mut ag = AutoGain::new(
            2,
            sample_rate,
            AutoGainParams {
                enabled: true,
                loudness_type: AutoGainLoudnessType::Momentary,
                max_gain_db: 12.0,
                smoothing_ms: 20.0, // Reasonably fast smoothing
            },
        ).unwrap();

        let block_size = 1024;
        let num_blocks = 150;
        let mut gains = Vec::new();

        for block in 0..num_blocks {
            let mut input = vec![0.0_f32; block_size * 2];
            for i in 0..block_size {
                let phase = 2.0 * std::f32::consts::PI * 440.0 * (block * block_size + i) as f32 / sample_rate as f32;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }
            ag.measure_input(&input).unwrap();

            // Effect: -3dB attenuation
            let mut output = input.clone();
            for s in &mut output {
                *s *= 0.707;
            }

            ag.measure_output(&output).unwrap();
            ag.apply_compensation(&mut output, block_size);
            
            gains.push(ag.current_gain_db());
        }

        // Check last 30 blocks for stability (no oscillations > 0.2 dB)
        // EBU R128 momentary loudness has a 400ms window, so it needs time to settle.
        let stable_part = &gains[num_blocks-30..];
        let min_gain = stable_part.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_gain = stable_part.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        
        assert!(max_gain - min_gain < 0.2, "AutoGain is oscillating: range {}dB. Last gains: {:?}", max_gain - min_gain, stable_part);
    }
}
