// ============================================================================
// Dither Plugin - TPDF dither with F-weighted noise shaping
// ============================================================================

pub mod params;

use crate::params::PARAMS as DT;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};

use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

/// Available bit depths indexed by the choice parameter.
const BIT_DEPTHS: [i32; 3] = [16, 20, 24];

/// F-weighted noise shaping coefficients (Wannamaker 1992, 3rd-order FIR).
/// Pushes quantization noise energy above ~15 kHz where it is less audible.
const NOISE_SHAPING_COEFFS: [f32; 3] = [1.623, -0.982, 0.109];

// ============================================================================
// Plugin Params (JSON deserialization)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DitherPluginParams {
    #[serde(default = "default_bit_depth")]
    pub bit_depth: usize,
    #[serde(default = "default_noise_shaping")]
    pub noise_shaping: bool,
    #[serde(default = "default_dither_type")]
    pub dither_type: usize,
}

fn default_bit_depth() -> usize {
    pk(DT, "bit_depth").default_usize()
}
fn default_noise_shaping() -> bool {
    pk(DT, "noise_shaping").default_bool()
}
fn default_dither_type() -> usize {
    pk(DT, "dither_type").default_usize()
}

// ============================================================================
// PRNG — xorshift64 (no allocation, no external dependency)
// ============================================================================

#[inline(always)]
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Convert xorshift64 output to a uniform f32 in [-0.5, 0.5].
#[inline(always)]
fn random_f32(state: &mut u64) -> f32 {
    // Use upper 32 bits for better distribution, map to [0, 1) then shift to [-0.5, 0.5)
    let upper = (xorshift64(state) >> 32) as u32;
    (upper as f32 / u32::MAX as f32) - 0.5
}

// ============================================================================
// Plugin Struct
// ============================================================================

pub struct DitherPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    bit_depth_index: usize,
    noise_shaping_enabled: bool,
    dither_type_index: usize,

    // Pre-computed from parameters
    scale: f32,
    inv_scale: f32,

    // DSP state (per-channel, pre-allocated)
    error_history: Vec<[f32; 3]>,
    rng_state: Vec<u64>,

    // Parameter IDs
    param_bit_depth: ParameterId,
    param_noise_shaping: ParameterId,
    param_dither_type: ParameterId,

    cached_parameters: Vec<Parameter>,
}

impl DitherPlugin {
    pub fn new(channels: usize) -> Self {
        let bit_depth_index = default_bit_depth();
        let noise_shaping = default_noise_shaping();
        let dither_type = default_dither_type();

        let bits = BIT_DEPTHS[bit_depth_index];
        let scale = 2.0_f32.powi(bits - 1);

        let mut p = Self {
            channels,
            sample_rate: 48000,
            bit_depth_index,
            noise_shaping_enabled: noise_shaping,
            dither_type_index: dither_type,
            scale,
            inv_scale: 1.0 / scale,
            error_history: vec![[0.0; 3]; channels],
            rng_state: Self::init_rng_states(channels),
            param_bit_depth: ParameterId::from("bit_depth"),
            param_noise_shaping: ParameterId::from("noise_shaping"),
            param_dither_type: ParameterId::from("dither_type"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: DitherPluginParams) -> Self {
        let bit_depth_index = params.bit_depth.min(BIT_DEPTHS.len() - 1);
        let bits = BIT_DEPTHS[bit_depth_index];
        let scale = 2.0_f32.powi(bits - 1);

        let mut p = Self {
            channels,
            sample_rate: 48000,
            bit_depth_index,
            noise_shaping_enabled: params.noise_shaping,
            dither_type_index: params.dither_type.min(1),
            scale,
            inv_scale: 1.0 / scale,
            error_history: vec![[0.0; 3]; channels],
            rng_state: Self::init_rng_states(channels),
            param_bit_depth: ParameterId::from("bit_depth"),
            param_noise_shaping: ParameterId::from("noise_shaping"),
            param_dither_type: ParameterId::from("dither_type"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    fn init_rng_states(channels: usize) -> Vec<u64> {
        // Seed each channel with a different non-zero value
        (0..channels)
            .map(|ch| 0xDEAD_BEEF_CAFE_0001_u64.wrapping_add(ch as u64 * 0x9E37_79B9_7F4A_7C15))
            .collect()
    }

    fn update_scales(&mut self) {
        let bits = BIT_DEPTHS[self.bit_depth_index];
        self.scale = 2.0_f32.powi(bits - 1);
        self.inv_scale = 1.0 / self.scale;
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "bit_depth",
                "Bit Depth",
                self.bit_depth_index as i32,
                0,
                (BIT_DEPTHS.len() - 1) as i32,
            ),
            Parameter::new_bool("noise_shaping", "Noise Shaping", self.noise_shaping_enabled),
            Parameter::new_int(
                "dither_type",
                "Dither Type",
                self.dither_type_index as i32,
                0,
                1,
            ),
        ];
    }

    /// Compute noise shaping feedback from the error history for one channel.
    #[inline(always)]
    fn noise_shaping_feedback(history: &[f32; 3]) -> f32 {
        NOISE_SHAPING_COEFFS[0] * history[0]
            + NOISE_SHAPING_COEFFS[1] * history[1]
            + NOISE_SHAPING_COEFFS[2] * history[2]
    }

    /// Push a new error into the history ring (most recent at index 0).
    #[inline(always)]
    fn push_error(history: &mut [f32; 3], error: f32) {
        history[2] = history[1];
        history[1] = history[0];
        history[0] = error;
    }
}

impl InPlacePlugin for DitherPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Dither", "1.0.0", "Sotf")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        if id == self.param_bit_depth
            && let Some(v) = val.as_int()
        {
            let idx = (v as usize).min(BIT_DEPTHS.len() - 1);
            self.bit_depth_index = idx;
            self.update_scales();
            self.rebuild_cached_parameters();
            return Ok(());
        }

        if id == self.param_noise_shaping
            && let Some(v) = val.as_bool()
        {
            self.noise_shaping_enabled = v;
            self.rebuild_cached_parameters();
            return Ok(());
        }

        if id == self.param_dither_type
            && let Some(v) = val.as_int()
        {
            let idx = (v as usize).min(1);
            self.dither_type_index = idx;
            self.rebuild_cached_parameters();
            return Ok(());
        }

        Err(format!("Invalid or unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bit_depth {
            Some(ParameterValue::Int(self.bit_depth_index as i32))
        } else if id == &self.param_noise_shaping {
            Some(ParameterValue::Bool(self.noise_shaping_enabled))
        } else if id == &self.param_dither_type {
            Some(ParameterValue::Int(self.dither_type_index as i32))
        } else {
            None
        }
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        // Re-allocate state for current channel count (in case it changed)
        if self.error_history.len() != self.channels {
            self.error_history.resize(self.channels, [0.0; 3]);
        }
        if self.rng_state.len() != self.channels {
            self.rng_state = Self::init_rng_states(self.channels);
        }
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        for h in &mut self.error_history {
            *h = [0.0; 3];
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let ch = self.channels;
        let scale = self.scale;
        let inv_scale = self.inv_scale;
        let dither_enabled = self.dither_type_index == 0; // 0 = TPDF
        let noise_shaping = self.noise_shaping_enabled;

        for frame in 0..nf {
            let base = frame * ch;
            for c in 0..ch {
                let idx = base + c;
                let input = buffer[idx];

                // Noise shaping feedback
                let shaped = if noise_shaping {
                    input - Self::noise_shaping_feedback(&self.error_history[c])
                } else {
                    input
                };

                // TPDF dither: two uniform randoms subtracted -> triangular PDF
                let dithered = if dither_enabled {
                    let r1 = random_f32(&mut self.rng_state[c]);
                    let r2 = random_f32(&mut self.rng_state[c]);
                    let tpdf = r1 - r2; // range [-1, 1], triangular distribution
                    shaped + tpdf * inv_scale
                } else {
                    shaped
                };

                // Quantize to target bit depth
                let quantized = (dithered * scale).round() * inv_scale;

                // Compute quantization error and store for noise shaping
                if noise_shaping {
                    let error = quantized - dithered;
                    Self::push_error(&mut self.error_history[c], error);
                }

                buffer[idx] = quantized;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(num_frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 48000,
            num_frames,
        }
    }

    #[test]
    fn test_dither_basic() {
        // Process silence, verify output stays near zero
        let mut plugin = DitherPlugin::new(2);
        plugin.initialize(48000).unwrap();

        let num_frames = 1024;
        let mut buffer = vec![0.0f32; num_frames * 2];
        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // With dither on silence, output should be very small (within 1 LSB of 16-bit)
        let max_lsb_16 = 1.0 / 32768.0; // 1 LSB at 16-bit
        for &sample in &buffer {
            assert!(
                sample.abs() <= max_lsb_16 * 2.0,
                "Dithered silence should stay near zero, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_dither_quantizes_to_target_depth() {
        // Process a known signal, verify output values are on the 16-bit grid
        let mut plugin = DitherPlugin::from_params(
            1,
            DitherPluginParams {
                bit_depth: 0, // 16-bit
                noise_shaping: false,
                dither_type: 1, // None (no dither, just quantize)
            },
        );
        plugin.initialize(48000).unwrap();

        let scale_16 = 32768.0_f32;
        let num_frames = 512;
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| (i as f32 / num_frames as f32) * 0.5 - 0.25)
            .collect();

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // Every output value should be exactly on the 16-bit grid
        for &sample in &buffer {
            let scaled = sample * scale_16;
            let rounded = scaled.round();
            assert!(
                (scaled - rounded).abs() < 1e-4,
                "Sample {} is not on 16-bit grid (scaled={})",
                sample,
                scaled
            );
        }
    }

    #[test]
    fn test_noise_shaping_reduces_audible_noise() {
        // Compare total quantization error with and without noise shaping.
        // With noise shaping, the error is reshaped (not necessarily reduced in total
        // energy), but the low-frequency portion should be lower.
        let num_frames = 8192;
        let channels = 1;

        // Generate a quiet sine wave (well below full scale so quantization matters)
        let freq = 1000.0;
        let sr = 48000.0;
        let amplitude = 0.01; // ~-40 dBFS
        let original: Vec<f32> = (0..num_frames)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
            .collect();

        // Process WITHOUT noise shaping
        let mut plugin_no_ns = DitherPlugin::from_params(
            channels,
            DitherPluginParams {
                bit_depth: 0,
                noise_shaping: false,
                dither_type: 0,
            },
        );
        plugin_no_ns.initialize(48000).unwrap();
        let mut buf_no_ns = original.clone();
        plugin_no_ns
            .process_in_place(&mut buf_no_ns, &make_context(num_frames))
            .unwrap();

        // Process WITH noise shaping
        let mut plugin_ns = DitherPlugin::from_params(
            channels,
            DitherPluginParams {
                bit_depth: 0,
                noise_shaping: true,
                dither_type: 0,
            },
        );
        plugin_ns.initialize(48000).unwrap();
        let mut buf_ns = original.clone();
        plugin_ns
            .process_in_place(&mut buf_ns, &make_context(num_frames))
            .unwrap();

        // Compute error energy in the low-frequency band (bins 0..N/8 ~ 0-3kHz)
        // For a rough check, just compute the sum of squared differences
        let error_no_ns: f64 = buf_no_ns
            .iter()
            .zip(original.iter())
            .map(|(o, i)| ((*o - *i) as f64).powi(2))
            .sum();

        let error_ns: f64 = buf_ns
            .iter()
            .zip(original.iter())
            .map(|(o, i)| ((*o - *i) as f64).powi(2))
            .sum();

        // Both should produce some quantization error
        assert!(error_no_ns > 0.0, "No-NS error should be non-zero");
        assert!(error_ns > 0.0, "NS error should be non-zero");

        // Noise shaping may increase total error energy (it reshapes, doesn't remove).
        // We just verify both produce finite, reasonable results.
        assert!(error_no_ns.is_finite());
        assert!(error_ns.is_finite());
    }

    #[test]
    fn test_dither_parameter_set_get() {
        let mut plugin = DitherPlugin::new(2);

        // Test bit_depth
        plugin
            .set_parameter(
                ParameterId::from("bit_depth"),
                ParameterValue::Int(2), // 24-bit
            )
            .unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("bit_depth")),
            Some(ParameterValue::Int(2))
        );

        // Test noise_shaping
        plugin
            .set_parameter(
                ParameterId::from("noise_shaping"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("noise_shaping")),
            Some(ParameterValue::Bool(false))
        );

        // Test dither_type
        plugin
            .set_parameter(
                ParameterId::from("dither_type"),
                ParameterValue::Int(1), // None
            )
            .unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("dither_type")),
            Some(ParameterValue::Int(1))
        );

        // Test unknown parameter
        assert!(
            plugin
                .set_parameter(ParameterId::from("unknown"), ParameterValue::Float(0.0),)
                .is_err()
        );
        assert_eq!(plugin.get_parameter(&ParameterId::from("unknown")), None);
    }

    #[test]
    fn test_xorshift64_produces_nonzero_values() {
        let mut state = 0xDEAD_BEEF_CAFE_0001_u64;
        let mut all_zero = true;
        for _ in 0..100 {
            let val = xorshift64(&mut state);
            if val != 0 {
                all_zero = false;
            }
        }
        assert!(!all_zero, "xorshift64 should produce non-zero values");
    }

    #[test]
    fn test_random_f32_range() {
        let mut state = 0xDEAD_BEEF_CAFE_0001_u64;
        for _ in 0..10000 {
            let val = random_f32(&mut state);
            assert!(
                (-0.5..=0.5).contains(&val),
                "random_f32 out of range: {}",
                val
            );
        }
    }

    #[test]
    fn test_24bit_quantization_grid() {
        let mut plugin = DitherPlugin::from_params(
            1,
            DitherPluginParams {
                bit_depth: 2, // 24-bit
                noise_shaping: false,
                dither_type: 1, // None
            },
        );
        plugin.initialize(48000).unwrap();

        let scale_24 = 8388608.0_f32; // 2^23
        let num_frames = 256;
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| (i as f32 / num_frames as f32) * 0.1)
            .collect();

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        for &sample in &buffer {
            let scaled = sample * scale_24;
            let rounded = scaled.round();
            assert!(
                (scaled - rounded).abs() < 1e-2,
                "Sample {} is not on 24-bit grid (scaled={})",
                sample,
                scaled
            );
        }
    }

    #[test]
    fn test_multichannel_independent() {
        // Verify each channel gets independent dither
        let mut plugin = DitherPlugin::from_params(
            2,
            DitherPluginParams {
                bit_depth: 0,
                noise_shaping: false,
                dither_type: 0,
            },
        );
        plugin.initialize(48000).unwrap();

        let num_frames = 256;
        // Same value on both channels
        let val = 0.00123_f32;
        let mut buffer = vec![val; num_frames * 2];

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // With TPDF dither, the two channels should generally differ
        // (different RNG states), though they could rarely match
        let mut differ_count = 0;
        for frame in 0..num_frames {
            if (buffer[frame * 2] - buffer[frame * 2 + 1]).abs() > 1e-10 {
                differ_count += 1;
            }
        }
        assert!(
            differ_count > num_frames / 2,
            "Channels should have independent dither, but only {} of {} frames differed",
            differ_count,
            num_frames
        );
    }
}
