// ============================================================================
// Gain Plugin - Simple gain control with per-channel support
// ============================================================================
//
// This plugin applies gain to audio samples. Supports two modes:
// 1. Single gain applied to all channels (default)
// 2. Per-channel gain with independent values for each channel

use super::param_specs::gain::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

fn default_gain_db() -> f32 {
    GAIN_DB_DEFAULT
}

/// Configuration parameters for GainPlugin
///
/// Supports two modes:
/// 1. Single gain for all channels: Use `gain_db` field
/// 2. Per-channel gain: Use `channel_gains` field
///
/// If `channel_gains` is provided and non-empty, it takes precedence over `gain_db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GainPluginParams {
    /// Single gain applied to all channels (default mode)
    #[serde(default = "default_gain_db")]
    pub gain_db: f32,

    /// Per-channel gains in dB (optional)
    /// If provided, must have exactly one value per channel
    #[serde(default)]
    pub channel_gains: Vec<f32>,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Simple gain plugin that multiplies all samples by a gain factor
///
/// Supports two modes:
/// 1. Single gain applied to all channels (default)
/// 2. Per-channel gain with independent values for each channel
pub struct GainPlugin {
    /// Number of channels
    channels: usize,
    sample_rate: u32,

    /// Global gain in dB (stored for parameter retrieval)
    global_gain_db: f32,

    /// Smoother for global linear gain
    global_gain_smoother: Smoother,

    /// Per-channel gains in dB (empty = use global gain)
    channel_gains_db: Vec<f32>,

    /// Smoothers for per-channel linear gains
    channel_gains_smoothers: Vec<Smoother>,

    /// Parameter ID for global gain
    param_gain_db: ParameterId,
}

impl GainPlugin {
    /// Create a new gain plugin with single gain for all channels
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `gain_db` - Initial gain in dB (0.0 = unity, negative = attenuation, positive = boost)
    pub fn new(channels: usize, gain_db: f32) -> Self {
        let sample_rate = 44100; // Default
        let gain_linear = Self::db_to_linear(gain_db);

        Self {
            channels,
            sample_rate,
            global_gain_db: gain_db,
            global_gain_smoother: Smoother::new(gain_linear, 20.0, sample_rate),
            channel_gains_db: Vec::new(),
            channel_gains_smoothers: Vec::new(),
            param_gain_db: ParameterId::from("gain_db"),
        }
    }

    /// Create a new gain plugin with per-channel gains
    ///
    /// # Arguments
    /// * `channel_gains` - List of gains in dB, one per channel
    ///
    /// # Errors
    /// Returns an error if channel_gains is empty
    pub fn new_per_channel(channel_gains: Vec<f32>) -> Result<Self, String> {
        if channel_gains.is_empty() {
            return Err("channel_gains must not be empty".to_string());
        }

        let channels = channel_gains.len();
        let sample_rate = 44100;

        let channel_gains_smoothers: Vec<Smoother> = channel_gains
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), 20.0, sample_rate))
            .collect();

        Ok(Self {
            channels,
            sample_rate,
            global_gain_db: GAIN_DB_DEFAULT,
            global_gain_smoother: Smoother::new(
                Self::db_to_linear(GAIN_DB_DEFAULT),
                20.0,
                sample_rate,
            ),
            channel_gains_db: channel_gains,
            channel_gains_smoothers,
            param_gain_db: ParameterId::from("gain_db"),
        })
    }

    /// Create a new gain plugin from configuration parameters
    pub fn from_params(channels: usize, params: GainPluginParams) -> Result<Self, String> {
        if params.channel_gains.is_empty() {
            // Global gain mode
            Ok(Self::new(channels, params.gain_db))
        } else {
            // Per-channel mode
            if params.channel_gains.len() != channels {
                return Err(format!(
                    "Channel gains count mismatch: expected {} channels, got {} gains",
                    channels,
                    params.channel_gains.len()
                ));
            }
            Self::new_per_channel(params.channel_gains)
        }
    }

    /// Check if plugin is in per-channel mode
    pub fn is_per_channel(&self) -> bool {
        !self.channel_gains_db.is_empty()
    }

    /// Set global gain in dB (switches to global mode if in per-channel mode)
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.global_gain_db = gain_db;
        let gain_linear = Self::db_to_linear(gain_db);
        self.global_gain_smoother.set_target(gain_linear);

        // Clear per-channel gains to switch to global mode
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }

    /// Set gain as linear multiplier (switches to global mode)
    pub fn set_gain_linear(&mut self, gain: f32) {
        self.global_gain_smoother.set_target(gain);
        self.global_gain_db = Self::linear_to_db(gain);
        // Clear per-channel gains to switch to global mode
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }

    /// Set per-channel gains (switches to per-channel mode)
    ///
    /// # Errors
    /// Returns an error if the number of gains doesn't match the channel count
    pub fn set_channel_gains(&mut self, gains_db: Vec<f32>) -> Result<(), String> {
        if gains_db.len() != self.channels {
            return Err(format!(
                "Channel gains count mismatch: expected {} channels, got {} gains",
                self.channels,
                gains_db.len()
            ));
        }

        self.channel_gains_smoothers = gains_db
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), 20.0, self.sample_rate))
            .collect();
        self.channel_gains_db = gains_db;
        Ok(())
    }

    /// Set gain for a specific channel (must already be in per-channel mode or will initialize it)
    ///
    /// # Errors
    /// Returns an error if channel index is out of bounds
    pub fn set_channel_gain_db(&mut self, channel: usize, gain_db: f32) -> Result<(), String> {
        if channel >= self.channels {
            return Err(format!(
                "Channel index {} out of bounds (max {})",
                channel,
                self.channels - 1
            ));
        }

        // Initialize per-channel mode if not already
        if self.channel_gains_db.is_empty() {
            self.channel_gains_db = vec![self.global_gain_db; self.channels];
            let current_linear = self.global_gain_smoother.current(); // Use current smooth value to prevent jump
            self.channel_gains_smoothers =
                vec![Smoother::new(current_linear, 20.0, self.sample_rate); self.channels];
        }

        self.channel_gains_db[channel] = gain_db;
        self.channel_gains_smoothers[channel].set_target(Self::db_to_linear(gain_db));
        Ok(())
    }

    /// Get current global gain in dB
    pub fn gain_db(&self) -> f32 {
        self.global_gain_db
    }

    /// Get current global gain as linear multiplier
    pub fn gain_linear(&self) -> f32 {
        self.global_gain_smoother.current()
    }

    /// Get gain for a specific channel in dB
    pub fn channel_gain_db(&self, channel: usize) -> Option<f32> {
        if self.is_per_channel() {
            self.channel_gains_db.get(channel).copied()
        } else if channel < self.channels {
            Some(self.global_gain_db)
        } else {
            None
        }
    }

    /// Convert dB to linear gain
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    /// Convert linear gain to dB
    fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }
}

impl InPlacePlugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "1.1.0", "SotF")
            .with_description("Gain/volume control plugin with per-channel support (Smoothed)")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        let mut params = vec![
            Parameter::new_float(
                "gain_db",
                "Gain (dB)",
                GAIN_DB_DEFAULT,
                GAIN_DB_MIN,
                GAIN_DB_MAX,
            )
            .with_description(
                "Global gain in dB. 0dB = unity, negative = attenuation, positive = boost",
            )
            .with_group("General")
            .with_importance(ParameterImportance::Critical),
        ];

        // Add per-channel parameters
        for ch in 0..self.channels {
            params.push(
                Parameter::new_float(
                    &format!("gain_db_{}", ch),
                    &format!("Ch{} Gain (dB)", ch),
                    GAIN_DB_DEFAULT,
                    GAIN_DB_MIN,
                    GAIN_DB_MAX,
                )
                .with_description(&format!(
                    "Channel {} gain in dB. Setting this enables per-channel mode.",
                    ch
                ))
                .with_group("Channels")
                .with_importance(ParameterImportance::FineTuning),
            );
        }

        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let id_str = id.as_str();

        // Check for global gain_db
        if id == self.param_gain_db {
            if let Some(gain_db) = value.as_float() {
                self.set_gain_db(gain_db);
                return Ok(());
            } else {
                return Err("Gain parameter must be a float".to_string());
            }
        }

        // Check for per-channel gain_db_N
        if let Some(suffix) = id_str.strip_prefix("gain_db_")
            && let Ok(channel) = suffix.parse::<usize>() {
                if let Some(gain_db) = value.as_float() {
                    return self.set_channel_gain_db(channel, gain_db);
                } else {
                    return Err("Gain parameter must be a float".to_string());
                }
            }

        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let id_str = id.as_str();

        // Check for global gain_db
        if id == &self.param_gain_db {
            return Some(ParameterValue::Float(self.global_gain_db));
        }

        // Check for per-channel gain_db_N
        if let Some(suffix) = id_str.strip_prefix("gain_db_")
            && let Ok(channel) = suffix.parse::<usize>()
                && let Some(gain) = self.channel_gain_db(channel) {
                    return Some(ParameterValue::Float(gain));
                }

        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.global_gain_smoother.set_time(20.0, sample_rate);
        for s in &mut self.channel_gains_smoothers {
            s.set_time(20.0, sample_rate);
        }
        Ok(())
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        _context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify buffer size matches channel count
        if !buffer.len().is_multiple_of(self.channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                buffer.len(),
                self.channels
            ));
        }

        let num_frames = buffer.len() / self.channels;

        if self.is_per_channel() {
            // Per-channel mode
            for frame in 0..num_frames {
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    // Tick smoother for this channel
                    let gain = self.channel_gains_smoothers[ch].next();
                    buffer[idx] *= gain;
                }
            }
        } else {
            // Global mode
            for frame in 0..num_frames {
                // Tick smoother once per frame (shared across channels for this frame)
                // Note: Gain is applied to all channels equally
                let gain = self.global_gain_smoother.next();
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    buffer[idx] *= gain;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_conversion() {
        assert!((GainPlugin::db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((GainPlugin::db_to_linear(-6.0) - 0.501).abs() < 0.01);
        assert!((GainPlugin::db_to_linear(-12.0) - 0.251).abs() < 0.01);
        assert!((GainPlugin::db_to_linear(6.0) - 1.995).abs() < 0.01);
    }

    #[test]
    fn test_unity_gain() {
        let mut plugin = GainPlugin::new(2, 0.0);
        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Should be unchanged at 0dB
        assert_eq!(buffer, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_attenuation() {
        let mut plugin = GainPlugin::new(2, -6.0);
        let mut buffer = vec![1.0, 2.0, 1.0, 2.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // -6dB ≈ 0.5x
        for &sample in &buffer {
            assert!((sample - 0.5).abs() < 0.01 || (sample - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_parameter_change() {
        let mut plugin = GainPlugin::new(2, 0.0);

        // Set via parameter system
        plugin
            .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-12.0))
            .unwrap();

        assert_eq!(plugin.gain_db(), -12.0);
        // Smoother will gradually reach target, but target should be set
        assert!((plugin.global_gain_smoother.target() - 0.251).abs() < 0.01);

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("gain_db"));
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_float(), Some(-12.0));
    }

    #[test]
    fn test_per_channel_creation() {
        let plugin = GainPlugin::new_per_channel(vec![-3.0, -6.0]).unwrap();

        assert!(plugin.is_per_channel());
        assert_eq!(plugin.channels, 2);
        assert_eq!(plugin.channel_gain_db(0), Some(-3.0));
        assert_eq!(plugin.channel_gain_db(1), Some(-6.0));
    }

    #[test]
    fn test_per_channel_processing() {
        let mut plugin = GainPlugin::new_per_channel(vec![0.0, -6.0]).unwrap();
        // 0dB on ch0, -6dB on ch1
        let mut buffer = vec![1.0, 1.0, 1.0, 1.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Ch0 should be unchanged (0dB), Ch1 should be ~0.5 (-6dB)
        assert!((buffer[0] - 1.0).abs() < 0.001); // Frame 0, Ch 0
        assert!((buffer[1] - 0.501).abs() < 0.01); // Frame 0, Ch 1
        assert!((buffer[2] - 1.0).abs() < 0.001); // Frame 1, Ch 0
        assert!((buffer[3] - 0.501).abs() < 0.01); // Frame 1, Ch 1
    }

    #[test]
    fn test_per_channel_parameter() {
        let mut plugin = GainPlugin::new(2, 0.0);

        // Set per-channel gain via parameter
        plugin
            .set_parameter(ParameterId::from("gain_db_0"), ParameterValue::Float(-3.0))
            .unwrap();

        assert!(plugin.is_per_channel());
        assert_eq!(plugin.channel_gain_db(0), Some(-3.0));
        assert_eq!(plugin.channel_gain_db(1), Some(0.0)); // Should inherit global

        // Set second channel
        plugin
            .set_parameter(ParameterId::from("gain_db_1"), ParameterValue::Float(-6.0))
            .unwrap();

        assert_eq!(plugin.channel_gain_db(1), Some(-6.0));

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("gain_db_0"));
        assert_eq!(value.unwrap().as_float(), Some(-3.0));
    }

    #[test]
    fn test_from_params_global() {
        let params = GainPluginParams {
            gain_db: -6.0,
            channel_gains: vec![],
        };
        let plugin = GainPlugin::from_params(2, params).unwrap();

        assert!(!plugin.is_per_channel());
        assert_eq!(plugin.gain_db(), -6.0);
    }

    #[test]
    fn test_from_params_per_channel() {
        let params = GainPluginParams {
            gain_db: 0.0, // Ignored when channel_gains is set
            channel_gains: vec![-3.0, -6.0],
        };
        let plugin = GainPlugin::from_params(2, params).unwrap();

        assert!(plugin.is_per_channel());
        assert_eq!(plugin.channel_gain_db(0), Some(-3.0));
        assert_eq!(plugin.channel_gain_db(1), Some(-6.0));
    }

    #[test]
    fn test_from_params_mismatch() {
        let params = GainPluginParams {
            gain_db: 0.0,
            channel_gains: vec![-3.0], // Only 1 gain for 2 channels
        };
        let result = GainPlugin::from_params(2, params);

        assert!(result.is_err());
    }

    #[test]
    fn test_switch_modes() {
        let mut plugin = GainPlugin::new(2, 0.0);

        // Switch to per-channel mode
        plugin.set_channel_gain_db(0, -3.0).unwrap();
        assert!(plugin.is_per_channel());

        // Switch back to global mode
        plugin.set_gain_db(-6.0);
        assert!(!plugin.is_per_channel());
        assert_eq!(plugin.gain_db(), -6.0);
    }
}
