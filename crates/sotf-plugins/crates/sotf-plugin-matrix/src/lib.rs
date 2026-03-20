// ============================================================================
// Matrix Plugin - Channel mixer with configurable gain matrix
// ============================================================================

use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::smoothing::Smoother;
use sotf_plugin_channel_mute_solo::ChannelState;

/// Smoothing time in ms for gain coefficient transitions (~5ms to avoid clicks)
const GAIN_SMOOTH_MS: f32 = 5.0;

/// Known routing presets
const PRESET_CUSTOM: &str = "custom";
const PRESET_STEREO_DOWNMIX: &str = "stereo_downmix";
const PRESET_MS_ENCODE: &str = "ms_encode";
const PRESET_MS_DECODE: &str = "ms_decode";
const PRESET_51_REMAP: &str = "5.1_remap";

const PRESET_CHOICES: &[&str] = &[
    PRESET_CUSTOM,
    PRESET_STEREO_DOWNMIX,
    PRESET_MS_ENCODE,
    PRESET_MS_DECODE,
    PRESET_51_REMAP,
];

/// Matrix mixer plugin that routes N input channels to P output channels
///
/// Gains are linear coefficients. Negative gains are allowed and produce
/// phase inversion. For explicit per-connection phase inversion, use the
/// `phase_invert` flags which multiply the gain by -1 during processing.
pub struct MatrixPlugin {
    preset: String,
    input_channel_map: Vec<usize>,
    output_channel_map: Vec<usize>,
    matrix: Vec<f32>,
    /// Per-connection phase inversion flags (parallel to `matrix`).
    /// When set, the effective gain is negated during processing.
    phase_invert: Vec<bool>,
    gain_smoothers: Vec<Smoother>,
    sample_rate: u32,
    physical_input_channels: usize,
    physical_output_channels: usize,
    channel_states: Vec<ChannelState>,
    channel_state_smoothers: Vec<Smoother>,
    active_connections: Vec<(usize, usize, usize)>,
    ch_gains_buffer: Vec<f32>,
    cached_parameters: Vec<Parameter>,
}

impl MatrixPlugin {
    pub fn new(input_channels: usize, output_channels: usize) -> Self {
        let matrix = Self::create_identity_matrix(input_channels, output_channels);
        let sample_rate = 48000;
        let gain_smoothers = matrix
            .iter()
            .map(|&v| Smoother::new(v, GAIN_SMOOTH_MS, sample_rate))
            .collect();
        let phase_invert = vec![false; matrix.len()];
        let mut plugin = Self {
            preset: PRESET_CUSTOM.to_string(),
            input_channel_map: Vec::new(),
            output_channel_map: Vec::new(),
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
            channel_states: Vec::new(),
            channel_state_smoothers: Vec::new(),
            active_connections: Vec::new(),
            ch_gains_buffer: Vec::new(),
            cached_parameters: Vec::new(),
        };
        plugin.update_active_connections();
        plugin.rebuild_cached_parameters();
        plugin
    }

    pub fn with_matrix(
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>,
    ) -> Result<Self, String> {
        let expected_size = output_channels * input_channels;
        if matrix.len() != expected_size {
            return Err("Size mismatch".into());
        }
        let sample_rate = 48000;
        let gain_smoothers = matrix
            .iter()
            .map(|&v| Smoother::new(v, GAIN_SMOOTH_MS, sample_rate))
            .collect();
        let phase_invert = vec![false; matrix.len()];
        let mut plugin = Self {
            preset: PRESET_CUSTOM.to_string(),
            input_channel_map: Vec::new(),
            output_channel_map: Vec::new(),
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
            channel_states: Vec::new(),
            channel_state_smoothers: Vec::new(),
            active_connections: Vec::new(),
            ch_gains_buffer: Vec::new(),
            cached_parameters: Vec::new(),
        };
        plugin.update_active_connections();
        plugin.rebuild_cached_parameters();

        let off_diag: Vec<_> = plugin
            .active_connections
            .iter()
            .filter(|(i, o, _)| i != o)
            .collect();
        if !off_diag.is_empty() {
            log::debug!(
                "[MatrixPlugin::with_matrix] {}x{} created with {} active connections ({} off-diagonal): {:?}",
                input_channels,
                output_channels,
                plugin.active_connections.len(),
                off_diag.len(),
                off_diag,
            );
        }

        Ok(plugin)
    }

    pub fn with_sparse_mapping(
        input_channel_map: Vec<usize>,
        output_channel_map: Vec<usize>,
        matrix: Vec<f32>,
    ) -> Result<Self, String> {
        if input_channel_map.is_empty() || output_channel_map.is_empty() {
            return Err("Empty map".into());
        }
        let physical_input_channels = input_channel_map.iter().max().map(|&v| v + 1).unwrap();
        let physical_output_channels = output_channel_map.iter().max().map(|&v| v + 1).unwrap();
        let sample_rate = 48000;
        let gain_smoothers = matrix
            .iter()
            .map(|&v| Smoother::new(v, GAIN_SMOOTH_MS, sample_rate))
            .collect();
        let phase_invert = vec![false; matrix.len()];
        let mut plugin = Self {
            preset: PRESET_CUSTOM.to_string(),
            input_channel_map,
            output_channel_map,
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels,
            physical_output_channels,
            channel_states: Vec::new(),
            channel_state_smoothers: Vec::new(),
            active_connections: Vec::new(),
            ch_gains_buffer: Vec::new(),
            cached_parameters: Vec::new(),
        };
        plugin.update_active_connections();
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = Vec::new();
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        // Preset selector (index into PRESET_CHOICES)
        let preset_idx = PRESET_CHOICES
            .iter()
            .position(|&p| p == self.preset)
            .unwrap_or(0) as i32;
        params.push(Parameter::new_int(
            "preset",
            "Preset",
            preset_idx,
            0,
            (PRESET_CHOICES.len() - 1) as i32,
        ));

        for out_ch in 0..num_outputs {
            for in_ch in 0..num_inputs {
                let idx = out_ch * num_inputs + in_ch;
                params.push(Parameter::new_float(
                    &format!("gain_{}_{}", in_ch, out_ch),
                    &format!("Gain In {} Out {}", in_ch, out_ch),
                    0.0,
                    -144.0,
                    24.0,
                ));
                params.push(Parameter::new_bool(
                    &format!("phase_invert_{}_{}", in_ch, out_ch),
                    &format!("Phase Invert In {} Out {}", in_ch, out_ch),
                    self.phase_invert.get(idx).copied().unwrap_or(false),
                ));
            }
            params.push(Parameter::new_bool(
                &format!("mute_{}", out_ch),
                &format!("Mute {}", out_ch),
                false,
            ));
            params.push(Parameter::new_bool(
                &format!("dim_{}", out_ch),
                &format!("Dim {}", out_ch),
                false,
            ));
        }

        params.push(Parameter::new_string(
            "channel_states",
            "Channel States",
            "[]".to_string(),
        ));

        self.cached_parameters = params;
    }

    fn update_active_connections(&mut self) {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        self.active_connections.clear();

        for out_ch in 0..num_outputs {
            for in_ch in 0..num_inputs {
                let idx = out_ch * num_inputs + in_ch;
                let target = self.gain_smoothers[idx].target();
                let current = self.gain_smoothers[idx].current();
                if target.abs() > 1e-4 || (current - target).abs() > 1e-4 {
                    self.active_connections.push((in_ch, out_ch, idx));
                }
            }
        }
    }

    fn create_identity_matrix(input_channels: usize, output_channels: usize) -> Vec<f32> {
        let mut matrix = vec![0.0; output_channels * input_channels];
        for i in 0..input_channels.min(output_channels) {
            matrix[i * input_channels + i] = 1.0;
        }
        matrix
    }

    pub fn num_inputs(&self) -> usize {
        if self.input_channel_map.is_empty() {
            self.physical_input_channels
        } else {
            self.input_channel_map.len()
        }
    }

    pub fn num_outputs(&self) -> usize {
        if self.output_channel_map.is_empty() {
            self.physical_output_channels
        } else {
            self.output_channel_map.len()
        }
    }

    pub fn set_matrix(&mut self, matrix: Vec<f32>) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        let expected = num_outputs * num_inputs;
        if matrix.len() != expected {
            return Err(format!(
                "Size mismatch: expected {} but got {}",
                expected,
                matrix.len()
            ));
        }
        for (idx, &gain) in matrix.iter().enumerate() {
            self.matrix[idx] = gain;
            self.gain_smoothers[idx].set_target(gain);
        }
        self.update_active_connections();
        Ok(())
    }

    pub fn set_gain(&mut self, input_ch: usize, output_ch: usize, gain: f32) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        let idx = output_ch * num_inputs + input_ch;
        if idx >= self.gain_smoothers.len() {
            return Err("OOB".into());
        }
        self.matrix[idx] = gain;
        self.gain_smoothers[idx].set_target(gain);
        self.update_active_connections();
        Ok(())
    }

    pub fn get_gain(&self, input_ch: usize, output_ch: usize) -> Option<f32> {
        let num_inputs = self.num_inputs();
        self.matrix.get(output_ch * num_inputs + input_ch).copied()
    }

    pub fn set_phase_invert(
        &mut self,
        input_ch: usize,
        output_ch: usize,
        invert: bool,
    ) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        let idx = output_ch * num_inputs + input_ch;
        if idx >= self.phase_invert.len() {
            return Err("OOB".into());
        }
        self.phase_invert[idx] = invert;
        Ok(())
    }

    pub fn get_phase_invert(&self, input_ch: usize, output_ch: usize) -> Option<bool> {
        let num_inputs = self.num_inputs();
        self.phase_invert
            .get(output_ch * num_inputs + input_ch)
            .copied()
    }

    pub fn with_channel_states(mut self, channel_states: Vec<ChannelState>) -> Self {
        self.channel_states = channel_states;
        self.reset_channel_state_smoothers();
        self
    }

    fn ensure_channel_state_smoothers(&mut self) {
        let num_outputs = self.num_outputs();
        if self.channel_state_smoothers.len() != num_outputs {
            self.channel_state_smoothers =
                vec![Smoother::new(1.0, GAIN_SMOOTH_MS, self.sample_rate); num_outputs];
        }
        let any_soloed = self.channel_states.iter().any(|s| s.soloed);
        for ch in 0..num_outputs {
            let target = if let Some(state) = self.channel_states.get(ch) {
                if any_soloed {
                    if state.soloed { 1.0 } else { 0.0 }
                } else if state.muted {
                    0.0
                } else if state.dimmed {
                    0.1
                } else {
                    1.0
                }
            } else {
                1.0
            };
            self.channel_state_smoothers[ch].set_target(target);
        }
    }

    /// Apply a routing preset by setting matrix gains to standard values.
    /// Returns Ok(()) if the preset was applied, Err if it requires different dimensions.
    fn apply_preset(&mut self, preset: &str) -> Result<(), String> {
        let ni = self.num_inputs();
        let no = self.num_outputs();
        match preset {
            PRESET_CUSTOM => { /* no-op, user-defined */ }
            PRESET_STEREO_DOWNMIX => {
                if ni < 2 || no < 2 {
                    return Err("stereo_downmix requires at least 2x2".into());
                }
                // Zero entire matrix, then set downmix coefficients
                for idx in 0..self.matrix.len() {
                    self.matrix[idx] = 0.0;
                    self.gain_smoothers[idx].set_target(0.0);
                }
                // L = L + 0.707*R
                self.set_gain(0, 0, 1.0)?; // L -> L
                self.set_gain(1, 0, std::f32::consts::FRAC_1_SQRT_2)?; // R -> L
                // R = R + 0.707*L
                self.set_gain(1, 1, 1.0)?; // R -> R
                self.set_gain(0, 1, std::f32::consts::FRAC_1_SQRT_2)?; // L -> R
            }
            PRESET_MS_ENCODE => {
                if ni < 2 || no < 2 {
                    return Err("ms_encode requires at least 2x2".into());
                }
                for idx in 0..self.matrix.len() {
                    self.matrix[idx] = 0.0;
                    self.gain_smoothers[idx].set_target(0.0);
                }
                // M = (L+R)*0.5
                self.set_gain(0, 0, 0.5)?; // L -> M
                self.set_gain(1, 0, 0.5)?; // R -> M
                // S = (L-R)*0.5
                self.set_gain(0, 1, 0.5)?;  // L -> S
                self.set_gain(1, 1, -0.5)?; // R -> S (negative)
            }
            PRESET_MS_DECODE => {
                if ni < 2 || no < 2 {
                    return Err("ms_decode requires at least 2x2".into());
                }
                for idx in 0..self.matrix.len() {
                    self.matrix[idx] = 0.0;
                    self.gain_smoothers[idx].set_target(0.0);
                }
                // L = M + S
                self.set_gain(0, 0, 1.0)?; // M -> L
                self.set_gain(1, 0, 1.0)?; // S -> L
                // R = M - S
                self.set_gain(0, 1, 1.0)?;  // M -> R
                self.set_gain(1, 1, -1.0)?; // S -> R (negative)
            }
            PRESET_51_REMAP => {
                // Identity pass-through (works for any dimension)
                for idx in 0..self.matrix.len() {
                    self.matrix[idx] = 0.0;
                    self.gain_smoothers[idx].set_target(0.0);
                }
                for i in 0..ni.min(no) {
                    let _ = self.set_gain(i, i, 1.0);
                }
            }
            _ => return Err(format!("Unknown preset: {}", preset)),
        }
        Ok(())
    }

    fn reset_channel_state_smoothers(&mut self) {
        let num_outputs = self.num_outputs();
        let any_soloed = self.channel_states.iter().any(|s| s.soloed);
        self.channel_state_smoothers = (0..num_outputs)
            .map(|ch| {
                let target = if let Some(state) = self.channel_states.get(ch) {
                    if any_soloed {
                        if state.soloed { 1.0 } else { 0.0 }
                    } else if state.muted {
                        0.0
                    } else if state.dimmed {
                        0.1
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                Smoother::new(target, GAIN_SMOOTH_MS, self.sample_rate)
            })
            .collect();
    }
}

impl Plugin for MatrixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Matrix", "1.1.0", "SotF")
    }
    fn input_channels(&self) -> usize {
        self.physical_input_channels
    }
    fn output_channels(&self) -> usize {
        self.physical_output_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let id_str = id.0.as_str();
        if id_str == "preset" {
            let idx = value
                .as_int()
                .ok_or_else(|| "preset must be an integer".to_string())?;
            let idx = idx.clamp(0, (PRESET_CHOICES.len() - 1) as i32) as usize;
            let preset_name = PRESET_CHOICES[idx].to_string();
            self.preset = preset_name.clone();
            if preset_name != PRESET_CUSTOM {
                self.apply_preset(&preset_name)?;
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id_str.starts_with("gain_") {
            let parts: Vec<&str> = id_str.split('_').collect();
            let in_ch = parts[1]
                .parse::<usize>()
                .map_err(|_| "Invalid input channel".to_string())?;
            let out_ch = parts[2]
                .parse::<usize>()
                .map_err(|_| "Invalid output channel".to_string())?;
            let v = value
                .as_float()
                .ok_or_else(|| format!("{} must be a float", id_str))?;
            if v.is_finite() {
                self.set_gain(in_ch, out_ch, v)?;
                self.rebuild_cached_parameters();
                return Ok(());
            }
            return Ok(());
        }
        if let Some(rest) = id_str.strip_prefix("phase_invert_") {
            // Format: phase_invert_{in_ch}_{out_ch}
            let parts: Vec<&str> = rest.split('_').collect();
            let in_ch = parts[0]
                .parse::<usize>()
                .map_err(|_| "Invalid input channel".to_string())?;
            let out_ch = parts[1]
                .parse::<usize>()
                .map_err(|_| "Invalid output channel".to_string())?;
            let v = value
                .as_bool()
                .ok_or_else(|| format!("{} must be a bool", id_str))?;
            self.set_phase_invert(in_ch, out_ch, v)?;
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if let Some(rest) = id_str.strip_prefix("mute_") {
            let ch = rest
                .parse::<usize>()
                .map_err(|_| "Invalid channel index".to_string())?;
            if ch < self.num_outputs() {
                if self.channel_states.len() <= ch {
                    self.channel_states
                        .resize(self.num_outputs(), ChannelState::default());
                }
                self.channel_states[ch].muted = value
                    .as_bool()
                    .ok_or_else(|| format!("{} must be a boolean", id_str))?;
                self.ensure_channel_state_smoothers();
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }
        if let Some(rest) = id_str.strip_prefix("dim_") {
            let ch = rest
                .parse::<usize>()
                .map_err(|_| "Invalid channel index".to_string())?;
            if ch < self.num_outputs() {
                if self.channel_states.len() <= ch {
                    self.channel_states
                        .resize(self.num_outputs(), ChannelState::default());
                }
                self.channel_states[ch].dimmed = value
                    .as_bool()
                    .ok_or_else(|| format!("{} must be a boolean", id_str))?;
                self.ensure_channel_state_smoothers();
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }
        if id_str == "channel_states" {
            let json_str = value.as_string().ok_or("channel_states must be string")?;
            let states: Vec<ChannelState> =
                serde_json::from_str(json_str).map_err(|e| e.to_string())?;
            self.channel_states = states;
            self.ensure_channel_state_smoothers();
            self.rebuild_cached_parameters();
            return Ok(());
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let id_str = id.0.as_str();
        if id_str == "preset" {
            let idx = PRESET_CHOICES
                .iter()
                .position(|&p| p == self.preset)
                .unwrap_or(0) as i32;
            return Some(ParameterValue::Int(idx));
        }
        if id_str.starts_with("gain_") {
            let parts: Vec<&str> = id_str.split('_').collect();
            let in_ch = parts[1].parse::<usize>().ok()?;
            let out_ch = parts[2].parse::<usize>().ok()?;
            return self.get_gain(in_ch, out_ch).map(ParameterValue::Float);
        }
        if let Some(rest) = id_str.strip_prefix("phase_invert_") {
            let parts: Vec<&str> = rest.split('_').collect();
            let in_ch = parts[0].parse::<usize>().ok()?;
            let out_ch = parts[1].parse::<usize>().ok()?;
            return self.get_phase_invert(in_ch, out_ch).map(ParameterValue::Bool);
        }
        if let Some(rest) = id_str.strip_prefix("mute_") {
            let ch = rest.parse::<usize>().ok()?;
            return self
                .channel_states
                .get(ch)
                .map(|s| ParameterValue::Bool(s.muted));
        }
        if let Some(rest) = id_str.strip_prefix("dim_") {
            let ch = rest.parse::<usize>().ok()?;
            return self
                .channel_states
                .get(ch)
                .map(|s| ParameterValue::Bool(s.dimmed));
        }
        if id_str == "channel_states" {
            return serde_json::to_string(&self.channel_states)
                .ok()
                .map(ParameterValue::String);
        }
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        for s in &mut self.gain_smoothers {
            s.set_time(GAIN_SMOOTH_MS, sample_rate);
        }
        for s in &mut self.channel_state_smoothers {
            s.set_time(GAIN_SMOOTH_MS, sample_rate);
        }
        Ok(())
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.fill(0.0);
        let num_frames = context.num_frames;
        let in_channels = self.physical_input_channels;
        let out_channels = self.physical_output_channels;

        // Tick channel state smoothers once per sample before connection loop
        // We use a temporary buffer to store the current gain for each channel for this block
        let buffer_size = out_channels * num_frames;
        if self.ch_gains_buffer.len() < buffer_size {
            self.ch_gains_buffer.resize(buffer_size, 1.0f32);
        }

        // Fill buffer with 1.0 default (in case some channels have no smoother)
        self.ch_gains_buffer[..buffer_size].fill(1.0);

        for ch in 0..self.channel_state_smoothers.len() {
            if ch < out_channels {
                for frame in 0..num_frames {
                    self.ch_gains_buffer[frame * out_channels + ch] =
                        self.channel_state_smoothers[ch].advance();
                }
            }
        }

        for &(logical_in, logical_out, idx) in &self.active_connections {
            let phys_in = if self.input_channel_map.is_empty() {
                logical_in
            } else {
                self.input_channel_map[logical_in]
            };
            let phys_out = if self.output_channel_map.is_empty() {
                logical_out
            } else {
                self.output_channel_map[logical_out]
            };

            let phase_sign = if self.phase_invert[idx] { -1.0 } else { 1.0 };

            for frame in 0..num_frames {
                let gain = self.gain_smoothers[idx].advance() * phase_sign;
                let ch_gain = self.ch_gains_buffer[frame * out_channels + logical_out];

                output[frame * out_channels + phys_out] +=
                    input[frame * in_channels + phys_in] * gain * ch_gain;
            }
        }

        self.update_active_connections();

        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_identity_matrix_2x2() {
        let plugin = MatrixPlugin::new(2, 2);
        assert_eq!(plugin.get_gain(0, 0), Some(1.0));
        assert_eq!(plugin.get_gain(1, 1), Some(1.0));
    }

    #[test]
    fn test_swap_channels() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_gain(0, 0, 0.0).unwrap();
        plugin.set_gain(1, 1, 0.0).unwrap();
        plugin.set_gain(1, 0, 1.0).unwrap();
        plugin.set_gain(0, 1, 1.0).unwrap();

        let input = vec![1.0, 2.0];
        let mut output = vec![0.0, 0.0];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        for _ in 0..5000 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        assert!((output[0] - 2.0).abs() < 0.01);
        assert!((output[1] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sparse_mapping_basic() {
        let mut plugin =
            MatrixPlugin::with_sparse_mapping(vec![1, 2], vec![15, 16], vec![1.0, 0.0, 0.0, 1.0])
                .unwrap();
        let mut input = vec![0.0; 3];
        input[1] = 10.0;
        input[2] = 20.0;
        let mut output = vec![0.0; 17];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };
        plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(output[15], 10.0);
        assert_eq!(output[16], 20.0);
    }

    /// Number of frames to process for smoother convergence in tests
    const CONVERGE_FRAMES: usize = 2048;
    const TOLERANCE: f32 = 0.001;

    /// Helper: process enough frames for smoothers to converge, return last frame
    fn process_converged(plugin: &mut MatrixPlugin, channels: usize) -> Vec<f32> {
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: CONVERGE_FRAMES,
        };
        let input = vec![1.0; CONVERGE_FRAMES * channels];
        let mut output = vec![0.0; CONVERGE_FRAMES * channels];
        plugin.process(&input, &mut output, &context).unwrap();
        output[output.len() - channels..].to_vec()
    }

    #[test]
    fn test_off_diagonal_6ch_center_to_left() {
        // 6x6 identity matrix, then set center(2)→left(0) = 1.0
        let mut plugin = MatrixPlugin::new(6, 6);
        plugin.set_gain(2, 0, 1.0).unwrap(); // center→left

        // Input: only center channel (index 2) has signal
        let num_frames = CONVERGE_FRAMES;
        let channels = 6;
        let mut input = vec![0.0; num_frames * channels];
        for frame in 0..num_frames {
            input[frame * channels + 2] = 0.8; // center channel
        }
        let mut output = vec![0.0; num_frames * channels];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        plugin.process(&input, &mut output, &context).unwrap();

        // Check last frame: left (0) should have center signal, center (2) should also have it (identity)
        let last_frame_start = (num_frames - 1) * channels;
        let left = output[last_frame_start];
        let center = output[last_frame_start + 2];
        assert!(
            (left - 0.8).abs() < TOLERANCE,
            "Left should receive center signal via off-diagonal, got {}",
            left
        );
        assert!(
            (center - 0.8).abs() < TOLERANCE,
            "Center should still pass through via identity diagonal, got {}",
            center
        );
        // Other channels should be silent
        assert!(
            output[last_frame_start + 1].abs() < TOLERANCE,
            "Right should be silent"
        );
        assert!(
            output[last_frame_start + 3].abs() < TOLERANCE,
            "LS should be silent"
        );
        assert!(
            output[last_frame_start + 4].abs() < TOLERANCE,
            "RS should be silent"
        );
        assert!(
            output[last_frame_start + 5].abs() < TOLERANCE,
            "LFE should be silent"
        );
    }

    #[test]
    fn test_channel_states_mute_via_parameter() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let states = vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
        ];
        let json = serde_json::to_string(&states).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("channel_states"),
                ParameterValue::String(json),
            )
            .unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            last[0].abs() < TOLERANCE,
            "Ch0 should be muted, got {}",
            last[0]
        );
        assert!(
            (last[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should pass through, got {}",
            last[1]
        );
    }

    #[test]
    fn test_channel_states_solo_via_parameter() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let states = vec![
            ChannelState {
                muted: false,
                soloed: true,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
        ];
        let json = serde_json::to_string(&states).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("channel_states"),
                ParameterValue::String(json),
            )
            .unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - 1.0).abs() < TOLERANCE,
            "Ch0 (soloed) should pass through"
        );
        assert!(
            last[1].abs() < TOLERANCE,
            "Ch1 (not soloed) should be silent"
        );
    }

    #[test]
    fn test_channel_states_dim_via_parameter() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: true,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
        ];
        let json = serde_json::to_string(&states).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("channel_states"),
                ParameterValue::String(json),
            )
            .unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - 0.1).abs() < TOLERANCE,
            "Ch0 should be dimmed to 0.1"
        );
        assert!((last[1] - 1.0).abs() < TOLERANCE, "Ch1 should pass through");
    }

    #[test]
    fn test_channel_states_get_parameter() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let states = vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: true,
            },
        ];
        let json = serde_json::to_string(&states).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("channel_states"),
                ParameterValue::String(json),
            )
            .unwrap();

        let got = plugin
            .get_parameter(&ParameterId::from("channel_states"))
            .unwrap();
        let got_str = got.as_string().unwrap();
        let got_states: Vec<ChannelState> = serde_json::from_str(got_str).unwrap();
        assert_eq!(got_states.len(), 2);
        assert!(got_states[0].muted);
        assert!(got_states[1].dimmed);
    }

    #[test]
    fn test_phase_invert_negates_output() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_phase_invert(0, 0, true).unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - (-1.0)).abs() < TOLERANCE,
            "Ch0 should be inverted, got {}",
            last[0]
        );
        assert!(
            (last[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should pass through unaffected, got {}",
            last[1]
        );
    }

    #[test]
    fn test_phase_invert_via_parameter() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin
            .set_parameter(
                ParameterId::from("phase_invert_0_0"),
                ParameterValue::Bool(true),
            )
            .unwrap();

        let got = plugin
            .get_parameter(&ParameterId::from("phase_invert_0_0"))
            .unwrap();
        assert_eq!(got.as_bool(), Some(true));

        let got_other = plugin
            .get_parameter(&ParameterId::from("phase_invert_1_1"))
            .unwrap();
        assert_eq!(got_other.as_bool(), Some(false));

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - (-1.0)).abs() < TOLERANCE,
            "Ch0 should be inverted via parameter, got {}",
            last[0]
        );
    }

    #[test]
    fn test_phase_invert_with_gain() {
        // Phase invert on a connection with gain 0.5 should produce -0.5
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_gain(0, 0, 0.5).unwrap();
        plugin.set_phase_invert(0, 0, true).unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - (-0.5)).abs() < TOLERANCE,
            "Ch0 should be 0.5 * -1 = -0.5, got {}",
            last[0]
        );
    }

    #[test]
    fn test_negative_gain_allowed() {
        // Negative gains should work directly (no clamping)
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_gain(0, 0, -1.0).unwrap();

        let last = process_converged(&mut plugin, 2);
        assert!(
            (last[0] - (-1.0)).abs() < TOLERANCE,
            "Ch0 with gain -1.0 should produce -1.0, got {}",
            last[0]
        );
    }
}
