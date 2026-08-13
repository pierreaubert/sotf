use super::consts::GAIN_SMOOTH_MS;
use super::consts::PRESET_51_REMAP;
use super::consts::PRESET_CHOICES;
use super::consts::PRESET_CUSTOM;
use super::consts::PRESET_MS_DECODE;
use super::consts::PRESET_MS_ENCODE;
use super::consts::PRESET_STEREO_DOWNMIX;
use crate::params::PARAMS as MX;
use sotf_host::param_bridge;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::param_specs::UpdateMode;
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::smoothing::Smoother;
use sotf_plugin_channel_mute_solo::ChannelState;

const MAX_MATRIX_CHANNELS: usize = 128;

/// Matrix mixer plugin that routes N input channels to P output channels
///
/// Gains are linear coefficients. Negative gains are allowed and produce
/// phase inversion. For explicit per-connection phase inversion, use the
/// `phase_invert` flags which multiply the gain by -1 during processing.
pub struct MatrixPlugin {
    pub(super) preset: &'static str,
    pub(super) input_channel_map: Vec<usize>,
    pub(super) output_channel_map: Vec<usize>,
    pub(super) matrix: Vec<f32>,
    /// Per-connection phase inversion flags (parallel to `matrix`).
    /// When set, the effective gain is negated during processing.
    pub(super) phase_invert: Vec<bool>,
    pub(super) gain_smoothers: Vec<Smoother>,
    pub(super) sample_rate: u32,
    pub(super) physical_input_channels: usize,
    pub(super) physical_output_channels: usize,
    pub(super) channel_states: Vec<ChannelState>,
    pub(super) channel_state_smoothers: Vec<Smoother>,
    pub(super) active_connections: Vec<(usize, usize, usize)>,
    /// Resolved (phys_in, phys_out) per active connection, rebuilt
    /// alongside active_connections to avoid per-sample channel-map lookups.
    pub(super) connection_phys: Vec<(usize, usize)>,
    /// Avoids advancing output-state smoothers in the common all-unity case.
    pub(super) channel_state_processing_active: bool,
    pub(super) cached_parameters: Vec<sotf_host::parameters::Parameter>,
    /// Global gain (from PARAMS spec), linear coefficient 0.0–1.0
    pub(super) gain: f64,
    pub(super) gain_smoother: Smoother,
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
        let matrix_len = matrix.len();
        let mut plugin = Self {
            preset: PRESET_CUSTOM,
            input_channel_map: Vec::new(),
            output_channel_map: Vec::new(),
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
            channel_states: vec![ChannelState::default(); output_channels],
            channel_state_smoothers: vec![
                Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate);
                output_channels
            ],
            active_connections: Vec::with_capacity(matrix_len),
            connection_phys: Vec::with_capacity(matrix_len),
            channel_state_processing_active: false,
            cached_parameters: Vec::new(),
            gain: 1.0,
            gain_smoother: Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate),
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
        if input_channels == 0 || output_channels == 0 {
            return Err("Matrix channel counts must be nonzero".into());
        }
        if input_channels > MAX_MATRIX_CHANNELS || output_channels > MAX_MATRIX_CHANNELS {
            return Err(format!(
                "Matrix channel counts must be <= {MAX_MATRIX_CHANNELS}"
            ));
        }
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
        let matrix_len = matrix.len();
        let mut plugin = Self {
            preset: PRESET_CUSTOM,
            input_channel_map: Vec::new(),
            output_channel_map: Vec::new(),
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
            channel_states: vec![ChannelState::default(); output_channels],
            channel_state_smoothers: vec![
                Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate);
                output_channels
            ],
            active_connections: Vec::with_capacity(matrix_len),
            connection_phys: Vec::with_capacity(matrix_len),
            channel_state_processing_active: false,
            cached_parameters: Vec::new(),
            gain: 1.0,
            gain_smoother: Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate),
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
        if input_channel_map.len() > MAX_MATRIX_CHANNELS
            || output_channel_map.len() > MAX_MATRIX_CHANNELS
        {
            return Err(format!(
                "Matrix channel counts must be <= {MAX_MATRIX_CHANNELS}"
            ));
        }
        let has_duplicate = |map: &[usize]| {
            map.iter()
                .enumerate()
                .any(|(i, channel)| map[..i].contains(channel))
        };
        if has_duplicate(&input_channel_map) || has_duplicate(&output_channel_map) {
            return Err("Sparse channel maps must not contain duplicates".into());
        }
        let expected = input_channel_map
            .len()
            .checked_mul(output_channel_map.len())
            .ok_or("Sparse matrix dimensions overflow")?;
        if matrix.len() != expected {
            return Err(format!(
                "Size mismatch: expected {expected} but got {}",
                matrix.len()
            ));
        }
        let physical_input_channels = input_channel_map
            .iter()
            .max()
            .and_then(|&v| v.checked_add(1))
            .ok_or("Input channel index overflow")?;
        let physical_output_channels = output_channel_map
            .iter()
            .max()
            .and_then(|&v| v.checked_add(1))
            .ok_or("Output channel index overflow")?;
        if physical_input_channels > MAX_MATRIX_CHANNELS
            || physical_output_channels > MAX_MATRIX_CHANNELS
        {
            return Err(format!(
                "Physical channel indices must be below {MAX_MATRIX_CHANNELS}"
            ));
        }
        let sample_rate = 48000;
        let gain_smoothers = matrix
            .iter()
            .map(|&v| Smoother::new(v, GAIN_SMOOTH_MS, sample_rate))
            .collect();
        let phase_invert = vec![false; matrix.len()];
        let matrix_len = matrix.len();
        let logical_outputs = output_channel_map.len();
        let mut plugin = Self {
            preset: PRESET_CUSTOM,
            input_channel_map,
            output_channel_map,
            matrix,
            phase_invert,
            gain_smoothers,
            sample_rate,
            physical_input_channels,
            physical_output_channels,
            channel_states: vec![ChannelState::default(); logical_outputs],
            channel_state_smoothers: vec![
                Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate);
                logical_outputs
            ],
            active_connections: Vec::with_capacity(matrix_len),
            connection_phys: Vec::with_capacity(matrix_len),
            channel_state_processing_active: false,
            cached_parameters: Vec::new(),
            gain: 1.0,
            gain_smoother: Smoother::new(1.0, GAIN_SMOOTH_MS, sample_rate),
        };
        plugin.update_active_connections();
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = Vec::new();
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        // Preset selector (index into PRESET_CHOICES)
        let preset_idx = PRESET_CHOICES
            .iter()
            .position(|&p| p == self.preset)
            .unwrap_or(0) as i32;
        params.push(sotf_host::parameters::Parameter::new_int(
            "preset",
            "Preset",
            preset_idx,
            0,
            (PRESET_CHOICES.len() - 1) as i32,
        )
        .with_update_mode(UpdateMode::Structural));

        for out_ch in 0..num_outputs {
            for in_ch in 0..num_inputs {
                let idx = out_ch * num_inputs + in_ch;
                params.push(sotf_host::parameters::Parameter::new_float(
                    &format!("gain_{}_{}", in_ch, out_ch),
                    &format!("Gain In {} Out {}", in_ch, out_ch),
                    self.matrix.get(idx).copied().unwrap_or(0.0),
                    -1.0,
                    1.0,
                ));
                params.push(sotf_host::parameters::Parameter::new_bool(
                    &format!("phase_invert_{}_{}", in_ch, out_ch),
                    &format!("Phase Invert In {} Out {}", in_ch, out_ch),
                    self.phase_invert.get(idx).copied().unwrap_or(false),
                ));
            }
            params.push(sotf_host::parameters::Parameter::new_bool(
                &format!("mute_{}", out_ch),
                &format!("Mute {}", out_ch),
                self.channel_states.get(out_ch).is_some_and(|s| s.muted),
            ));
            params.push(sotf_host::parameters::Parameter::new_bool(
                &format!("dim_{}", out_ch),
                &format!("Dim {}", out_ch),
                self.channel_states.get(out_ch).is_some_and(|s| s.dimmed),
            ));
            params.push(sotf_host::parameters::Parameter::new_bool(
                &format!("solo_{}", out_ch),
                &format!("Solo {}", out_ch),
                self.channel_states.get(out_ch).is_some_and(|s| s.soloed),
            ));
        }

        params.push(sotf_host::parameters::Parameter::new_string(
            "channel_states",
            "Channel States",
            serde_json::to_string(&self.channel_states).unwrap_or_else(|_| "[]".to_string()),
        ));

        // Prepend PARAMS-based parameters (gain) before dynamic ones
        let mut bridge_params = param_bridge::build_parameters(MX, |i| self.param_value(i));
        bridge_params.append(&mut params);
        self.cached_parameters = bridge_params;
    }

    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.gain),
            _ => None,
        }
    }

    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        if index == 0 && value.is_finite() && (0.0..=1.0).contains(&value) {
            self.gain = value;
            self.gain_smoother.set_target(value as f32);
        }
    }

    pub(super) fn update_active_connections(&mut self) {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        self.active_connections.clear();
        self.connection_phys.clear();

        for out_ch in 0..num_outputs {
            for in_ch in 0..num_inputs {
                let idx = out_ch * num_inputs + in_ch;
                let target = self.gain_smoothers[idx].target();
                let current = self.gain_smoothers[idx].current();
                if target.abs() > 1e-4 || (current - target).abs() > 1e-4 {
                    self.active_connections.push((in_ch, out_ch, idx));
                    // Pre-resolve physical channel indices so the
                    // hot process() loop does not branch on channel maps per sample.
                    let phys_in = if self.input_channel_map.is_empty() {
                        in_ch
                    } else {
                        self.input_channel_map[in_ch]
                    };
                    let phys_out = if self.output_channel_map.is_empty() {
                        out_ch
                    } else {
                        self.output_channel_map[out_ch]
                    };
                    self.connection_phys.push((phys_in, phys_out));
                }
            }
        }
    }

    pub(super) fn create_identity_matrix(
        input_channels: usize,
        output_channels: usize,
    ) -> Vec<f32> {
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
        if matrix.iter().any(|gain| !gain.is_finite()) {
            return Err("Matrix gains must be finite".into());
        }
        for (idx, &gain) in matrix.iter().enumerate() {
            self.matrix[idx] = gain;
            let effective_gain = self.effective_gain(idx);
            self.gain_smoothers[idx].set_target(effective_gain);
        }
        self.update_active_connections();
        Ok(())
    }

    pub fn set_gain(&mut self, input_ch: usize, output_ch: usize, gain: f32) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        if input_ch >= num_inputs || output_ch >= self.num_outputs() {
            return Err(format!(
                "Channel index out of range: input={input_ch}, output={output_ch}"
            ));
        }
        let idx = output_ch * num_inputs + input_ch;
        if idx >= self.gain_smoothers.len() {
            return Err("OOB".into());
        }
        if !gain.is_finite() {
            return Err("Gain must be finite".into());
        }
        self.matrix[idx] = gain;
        let effective_gain = self.effective_gain(idx);
        self.gain_smoothers[idx].set_target(effective_gain);
        self.update_active_connections();
        Ok(())
    }

    pub fn get_gain(&self, input_ch: usize, output_ch: usize) -> Option<f32> {
        let num_inputs = self.num_inputs();
        if input_ch >= num_inputs || output_ch >= self.num_outputs() {
            return None;
        }
        self.matrix.get(output_ch * num_inputs + input_ch).copied()
    }

    pub fn set_phase_invert(
        &mut self,
        input_ch: usize,
        output_ch: usize,
        invert: bool,
    ) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        if input_ch >= num_inputs || output_ch >= self.num_outputs() {
            return Err(format!(
                "Channel index out of range: input={input_ch}, output={output_ch}"
            ));
        }
        let idx = output_ch * num_inputs + input_ch;
        if idx >= self.phase_invert.len() {
            return Err("OOB".into());
        }
        self.phase_invert[idx] = invert;
        // Smooth the signed coefficient through zero instead of flipping phase
        // instantaneously at a block boundary.
        let effective_gain = self.effective_gain(idx);
        self.gain_smoothers[idx].set_target(effective_gain);
        self.update_active_connections();
        Ok(())
    }

    pub fn get_phase_invert(&self, input_ch: usize, output_ch: usize) -> Option<bool> {
        let num_inputs = self.num_inputs();
        if input_ch >= num_inputs || output_ch >= self.num_outputs() {
            return None;
        }
        self.phase_invert
            .get(output_ch * num_inputs + input_ch)
            .copied()
    }

    pub fn with_channel_states(
        mut self,
        channel_states: Vec<ChannelState>,
    ) -> Result<Self, String> {
        if channel_states.len() != self.num_outputs() {
            return Err(format!(
                "channel_states requires {} entries",
                self.num_outputs()
            ));
        }
        self.channel_states = channel_states;
        self.reset_channel_state_smoothers();
        Ok(self)
    }

    #[inline]
    fn effective_gain(&self, idx: usize) -> f32 {
        self.matrix[idx] * if self.phase_invert[idx] { -1.0 } else { 1.0 }
    }

    fn channel_state_target(&self, channel: usize, any_soloed: bool) -> f32 {
        let state = &self.channel_states[channel];
        if any_soloed {
            if state.soloed { 1.0 } else { 0.0 }
        } else if state.muted {
            0.0
        } else if state.dimmed {
            0.1
        } else {
            1.0
        }
    }

    pub(super) fn ensure_channel_state_smoothers(&mut self) {
        let num_outputs = self.num_outputs();
        if self.channel_state_smoothers.len() != num_outputs {
            self.channel_state_smoothers =
                vec![Smoother::new(1.0, GAIN_SMOOTH_MS, self.sample_rate); num_outputs];
        }
        let any_soloed = self.channel_states.iter().any(|s| s.soloed);
        for ch in 0..num_outputs {
            let target = self.channel_state_target(ch, any_soloed);
            self.channel_state_smoothers[ch].set_target(target);
        }
        self.channel_state_processing_active = self
            .channel_state_smoothers
            .iter()
            .any(|s| (s.current() - 1.0).abs() > 1e-4 || (s.target() - 1.0).abs() > 1e-4);
    }

    /// Apply a routing preset by setting matrix gains to standard values.
    /// Returns Ok(()) if the preset was applied, Err if it requires different dimensions.
    pub(super) fn apply_preset(&mut self, preset: &str) -> Result<(), String> {
        let ni = self.num_inputs();
        let no = self.num_outputs();
        match preset {
            PRESET_CUSTOM => return Ok(()),
            PRESET_STEREO_DOWNMIX if no != 2 || (ni != 2 && ni != 6) => {
                return Err("stereo_downmix requires 2x2 or 6x2".into());
            }
            PRESET_MS_ENCODE | PRESET_MS_DECODE if ni < 2 || no < 2 => {
                return Err(format!("{preset} requires at least 2x2"));
            }
            PRESET_51_REMAP if ni != 6 || no != 6 => {
                return Err("5.1_remap requires exactly 6x6".into());
            }
            PRESET_STEREO_DOWNMIX | PRESET_MS_ENCODE | PRESET_MS_DECODE | PRESET_51_REMAP => {}
            _ => return Err(format!("Unknown preset: {preset}")),
        }

        // All fallible validation happens above. Mutating the existing matrix
        // after that point is both atomic and allocation-free.
        self.matrix.fill(0.0);
        self.phase_invert.fill(false);
        let set = |matrix: &mut [f32], input: usize, output: usize, gain: f32| {
            matrix[output * ni + input] = gain;
        };
        match preset {
            PRESET_STEREO_DOWNMIX => {
                if ni == 2 {
                    set(&mut self.matrix, 0, 0, 1.0);
                    set(&mut self.matrix, 1, 1, 1.0);
                } else {
                    // SMPTE/WAVE 5.1 input: L, R, C, LFE, Ls, Rs. LFE is omitted.
                    // Normalize the worst-case correlated sum to unity headroom.
                    let norm = 1.0 / (1.0 + 2.0 * std::f32::consts::FRAC_1_SQRT_2);
                    set(&mut self.matrix, 0, 0, norm);
                    set(
                        &mut self.matrix,
                        2,
                        0,
                        norm * std::f32::consts::FRAC_1_SQRT_2,
                    );
                    set(
                        &mut self.matrix,
                        4,
                        0,
                        norm * std::f32::consts::FRAC_1_SQRT_2,
                    );
                    set(&mut self.matrix, 1, 1, norm);
                    set(
                        &mut self.matrix,
                        2,
                        1,
                        norm * std::f32::consts::FRAC_1_SQRT_2,
                    );
                    set(
                        &mut self.matrix,
                        5,
                        1,
                        norm * std::f32::consts::FRAC_1_SQRT_2,
                    );
                }
            }
            PRESET_MS_ENCODE => {
                set(&mut self.matrix, 0, 0, 0.5);
                set(&mut self.matrix, 1, 0, 0.5);
                set(&mut self.matrix, 0, 1, 0.5);
                set(&mut self.matrix, 1, 1, -0.5);
            }
            PRESET_MS_DECODE => {
                set(&mut self.matrix, 0, 0, 1.0);
                set(&mut self.matrix, 1, 0, 1.0);
                set(&mut self.matrix, 0, 1, 1.0);
                set(&mut self.matrix, 1, 1, -1.0);
            }
            PRESET_51_REMAP => {
                // SMPTE/WAVE [L,R,C,LFE,Ls,Rs] -> AAC [C,L,R,Ls,Rs,LFE].
                for (output, input) in [2, 0, 1, 4, 5, 3].into_iter().enumerate() {
                    set(&mut self.matrix, input, output, 1.0);
                }
            }
            _ => unreachable!("preset validated above"),
        }
        for idx in 0..self.gain_smoothers.len() {
            let target = self.effective_gain(idx);
            self.gain_smoothers[idx].set_target(target);
        }
        self.update_active_connections();
        Ok(())
    }

    pub(super) fn reset_channel_state_smoothers(&mut self) {
        let num_outputs = self.num_outputs();
        let any_soloed = self.channel_states.iter().any(|s| s.soloed);
        debug_assert_eq!(self.channel_state_smoothers.len(), num_outputs);
        for ch in 0..num_outputs {
            let target = self.channel_state_target(ch, any_soloed);
            self.channel_state_smoothers[ch] =
                Smoother::new(target, GAIN_SMOOTH_MS, self.sample_rate);
        }
        self.channel_state_processing_active = self
            .channel_state_smoothers
            .iter()
            .any(|s| (s.current() - 1.0).abs() > 1e-4);
    }
}

fn parse_crosspoint_id(id: &str, prefix: &str) -> Result<(usize, usize), String> {
    let rest = id
        .strip_prefix(prefix)
        .ok_or_else(|| format!("Invalid parameter id: {id}"))?;
    let mut fields = rest.split('_');
    let input = fields
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing input channel".to_string())?
        .parse::<usize>()
        .map_err(|_| "Invalid input channel".to_string())?;
    let output = fields
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing output channel".to_string())?
        .parse::<usize>()
        .map_err(|_| "Invalid output channel".to_string())?;
    if fields.next().is_some() {
        return Err(format!("Invalid parameter id: {id}"));
    }
    Ok((input, output))
}

impl Plugin for MatrixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Matrix", env!("CARGO_PKG_VERSION"), "SotF")
    }
    fn input_channels(&self) -> usize {
        self.physical_input_channels
    }
    fn output_channels(&self) -> usize {
        self.physical_output_channels
    }
    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::routing(PluginCostClass::Scalar, None, true);
        metadata.boundary = self.physical_input_channels != self.physical_output_channels;
        metadata
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        let mut parameters = self.cached_parameters.clone();
        for parameter in &mut parameters {
            if let Some(current) = self.get_parameter(&parameter.id) {
                parameter.default_value = current;
            }
        }
        parameters
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let id_str = id.as_str();
        // Avoid asking the generic bridge to reject every dynamic Matrix ID: its
        // diagnostic String would allocate on each realtime control update.
        if id_str == "gain" {
            param_bridge::set_parameter(MX, &id, &value, |i, v| self.set_param_value(i, v))?;
            return Ok(());
        }
        if id_str == "preset" {
            let idx = value
                .as_int()
                .ok_or_else(|| "preset must be an integer".to_string())?;
            if !(0..PRESET_CHOICES.len() as i32).contains(&idx) {
                return Err(format!("preset index out of range: {idx}"));
            }
            let idx = idx as usize;
            let preset_name = PRESET_CHOICES[idx];
            if preset_name != PRESET_CUSTOM {
                self.apply_preset(preset_name)?;
            }
            self.preset = preset_name;
            return Ok(());
        }
        if id_str.starts_with("gain_") {
            let (in_ch, out_ch) = parse_crosspoint_id(id_str, "gain_")?;
            let v = value
                .as_float()
                .ok_or_else(|| format!("{} must be a float", id_str))?;
            if v.is_finite() {
                self.set_gain(in_ch, out_ch, v)?;
                return Ok(());
            }
            return Err(format!("{} must be finite", id_str));
        }
        if let Some(rest) = id_str.strip_prefix("phase_invert_") {
            // Format: phase_invert_{in_ch}_{out_ch}
            let (in_ch, out_ch) = parse_crosspoint_id(rest, "")?;
            let v = value
                .as_bool()
                .ok_or_else(|| format!("{} must be a bool", id_str))?;
            self.set_phase_invert(in_ch, out_ch, v)?;
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
                return Ok(());
            }
        }
        if let Some(rest) = id_str.strip_prefix("solo_") {
            let ch = rest
                .parse::<usize>()
                .map_err(|_| "Invalid channel index".to_string())?;
            if ch < self.num_outputs() {
                self.channel_states[ch].soloed = value
                    .as_bool()
                    .ok_or_else(|| format!("{} must be a boolean", id_str))?;
                self.ensure_channel_state_smoothers();
                return Ok(());
            }
        }
        if id_str == "channel_states" {
            let json_str = value.as_string().ok_or("channel_states must be string")?;
            let states: Vec<ChannelState> =
                serde_json::from_str(json_str).map_err(|e| e.to_string())?;
            if states.len() != self.num_outputs() {
                return Err(format!(
                    "channel_states requires {} entries",
                    self.num_outputs()
                ));
            }
            self.channel_states = states;
            self.ensure_channel_state_smoothers();
            return Ok(());
        }
        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Try PARAMS-based keys first (e.g. "gain")
        if let Some(v) = param_bridge::get_parameter(MX, id, |i| self.param_value(i)) {
            return Some(v);
        }
        let id_str = id.as_str();
        if id_str == "preset" {
            let idx = PRESET_CHOICES
                .iter()
                .position(|&p| p == self.preset)
                .unwrap_or(0) as i32;
            return Some(ParameterValue::Int(idx));
        }
        if id_str.starts_with("gain_") {
            let (in_ch, out_ch) = parse_crosspoint_id(id_str, "gain_").ok()?;
            return self.get_gain(in_ch, out_ch).map(ParameterValue::Float);
        }
        if let Some(rest) = id_str.strip_prefix("phase_invert_") {
            let (in_ch, out_ch) = parse_crosspoint_id(rest, "").ok()?;
            return self
                .get_phase_invert(in_ch, out_ch)
                .map(ParameterValue::Bool);
        }
        if let Some(rest) = id_str.strip_prefix("mute_") {
            let ch = rest.parse::<usize>().ok()?;
            if ch < self.num_outputs() {
                return Some(ParameterValue::Bool(
                    self.channel_states.get(ch).is_some_and(|s| s.muted),
                ));
            }
            return None;
        }
        if let Some(rest) = id_str.strip_prefix("dim_") {
            let ch = rest.parse::<usize>().ok()?;
            if ch < self.num_outputs() {
                return Some(ParameterValue::Bool(
                    self.channel_states.get(ch).is_some_and(|s| s.dimmed),
                ));
            }
            return None;
        }
        if let Some(rest) = id_str.strip_prefix("solo_") {
            let ch = rest.parse::<usize>().ok()?;
            if ch < self.num_outputs() {
                return Some(ParameterValue::Bool(self.channel_states[ch].soloed));
            }
            return None;
        }
        if id_str == "channel_states" {
            return serde_json::to_string(&self.channel_states)
                .ok()
                .map(ParameterValue::String);
        }
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Matrix sample rate must be greater than zero".into());
        }
        self.sample_rate = sample_rate;
        for idx in 0..self.gain_smoothers.len() {
            self.gain_smoothers[idx] =
                Smoother::new(self.effective_gain(idx), GAIN_SMOOTH_MS, sample_rate);
        }
        self.reset_channel_state_smoothers();
        self.gain_smoother = Smoother::new(self.gain as f32, GAIN_SMOOTH_MS, sample_rate);
        self.update_active_connections();
        Ok(())
    }

    fn reset(&mut self) {
        for idx in 0..self.gain_smoothers.len() {
            self.gain_smoothers[idx] =
                Smoother::new(self.effective_gain(idx), GAIN_SMOOTH_MS, self.sample_rate);
        }
        self.gain_smoother = Smoother::new(self.gain as f32, GAIN_SMOOTH_MS, self.sample_rate);
        self.reset_channel_state_smoothers();
        self.update_active_connections();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let in_channels = self.physical_input_channels;
        let out_channels = self.physical_output_channels;
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Sample-rate mismatch: context {} Hz, initialized {} Hz",
                context.sample_rate, self.sample_rate
            ));
        }
        let input_len = num_frames
            .checked_mul(in_channels)
            .ok_or("Input size overflow")?;
        let output_len = num_frames
            .checked_mul(out_channels)
            .ok_or("Output size overflow")?;
        if input.len() != input_len || output.len() != output_len {
            return Err(format!(
                "Buffer size mismatch: input {} (need {}), output {} (need {})",
                input.len(),
                input_len,
                output.len(),
                output_len
            ));
        }
        output.fill(0.0);

        // Frames outer, connections inner: keeps the current frame's input/output
        // samples in L1 cache while iterating the (much smaller) connection list.
        // Previously, connections were outer and frames inner, which caused the input
        // buffer to be scanned once per connection — cache-thrashing for dense matrices.
        //
        // connection_phys holds pre-resolved (phys_in, phys_out) built by
        // update_active_connections(), so no per-sample channel-map branch is needed.
        //
        // NOTE: update_active_connections() is NOT called here. It is only called by
        // parameter mutators (set_gain, set_matrix, set_phase_invert, apply_preset)
        // when the matrix actually changes. Calling it every block was O(N²) wasted work.
        for frame in 0..num_frames {
            let base_out = frame * out_channels;
            let base_in = frame * in_channels;
            let global_gain = self.gain_smoother.advance();
            if self.channel_state_processing_active {
                for smoother in &mut self.channel_state_smoothers {
                    smoother.advance();
                }
            }
            for (&(_logical_in, logical_out, idx), &(phys_in, phys_out)) in self
                .active_connections
                .iter()
                .zip(self.connection_phys.iter())
            {
                // advance() is called once per sample per connection to maintain
                // correct per-sample gain interpolation (5 ms smoother).
                let gain = self.gain_smoothers[idx].advance();
                let ch_gain = self
                    .channel_state_smoothers
                    .get(logical_out)
                    .map_or(1.0, Smoother::current);
                output[base_out + phys_out] +=
                    input[base_in + phys_in] * gain * ch_gain * global_gain;
            }
        }

        // Compact only after a fade reaches silence. Capacities are reserved at
        // construction, so this bounded control-cache maintenance does not allocate.
        if self.active_connections.iter().any(|&(_, _, idx)| {
            self.gain_smoothers[idx].target().abs() <= 1e-4
                && self.gain_smoothers[idx].current().abs() <= 1e-4
        }) {
            self.update_active_connections();
        }
        if self.channel_state_processing_active
            && self
                .channel_state_smoothers
                .iter()
                .all(|s| (s.target() - 1.0).abs() <= 1e-4 && (s.current() - 1.0).abs() <= 1e-4)
        {
            for smoother in &mut self.channel_state_smoothers {
                *smoother = Smoother::new(1.0, GAIN_SMOOTH_MS, self.sample_rate);
            }
            self.channel_state_processing_active = false;
        }

        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let context = ProcessContext::new(48000, 1);

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
        let context = ProcessContext::new(48000, 1);
        plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(output[15], 10.0);
        assert_eq!(output[16], 20.0);
    }

    /// Number of frames to process for smoother convergence in tests
    const CONVERGE_FRAMES: usize = 2048;
    const TOLERANCE: f32 = 0.001;

    /// Helper: process enough frames for smoothers to converge, return last frame
    fn process_converged(plugin: &mut MatrixPlugin, channels: usize) -> Vec<f32> {
        let context = ProcessContext::new(48000, CONVERGE_FRAMES);
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
        let context = ProcessContext::new(48000, num_frames);
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
    fn test_default_channel_controls_are_readable() {
        let plugin = MatrixPlugin::new(2, 2);

        for channel in 0..2 {
            assert_eq!(
                plugin.get_parameter(&ParameterId::from(format!("mute_{channel}"))),
                Some(ParameterValue::Bool(false))
            );
            assert_eq!(
                plugin.get_parameter(&ParameterId::from(format!("dim_{channel}"))),
                Some(ParameterValue::Bool(false))
            );
        }
        assert_eq!(plugin.get_parameter(&ParameterId::from("mute_2")), None);
        assert_eq!(plugin.get_parameter(&ParameterId::from("dim_2")), None);
    }

    #[test]
    fn test_irregular_blocks_need_no_block_sized_scratch() {
        let mut plugin = MatrixPlugin::new(2, 2);
        for frames in [1, 257, 3, 4096, 17] {
            let input = vec![1.0; frames * 2];
            let mut output = vec![0.0; frames * 2];
            let context = ProcessContext::new(48000, frames);
            plugin.process(&input, &mut output, &context).unwrap();
            assert!(output.iter().all(|sample| (*sample - 1.0).abs() < 1e-6));
        }
    }

    #[test]
    fn faded_zero_connections_are_pruned() {
        let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![1.0; 4]).unwrap();
        assert_eq!(plugin.active_connections.len(), 4);
        plugin.set_matrix(vec![0.0; 4]).unwrap();
        let frames = CONVERGE_FRAMES * 2;
        let input = vec![1.0; frames * 2];
        let mut output = vec![0.0; frames * 2];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, frames))
            .unwrap();
        assert!(plugin.active_connections.is_empty());
    }

    #[test]
    fn phase_toggle_is_smoothed_and_partition_invariant() {
        fn render(blocks: &[usize]) -> Vec<f32> {
            let mut plugin = MatrixPlugin::new(1, 1);
            plugin.set_phase_invert(0, 0, true).unwrap();
            let mut rendered = Vec::new();
            for &frames in blocks {
                let input = vec![1.0; frames];
                let mut output = vec![0.0; frames];
                plugin
                    .process(&input, &mut output, &ProcessContext::new(48_000, frames))
                    .unwrap();
                rendered.extend(output);
            }
            rendered
        }
        let whole = render(&[512]);
        let partitioned = render(&[17, 31, 64, 3, 397]);
        assert_eq!(whole, partitioned);
        assert!(
            whole[0] < 1.0 && whole[0] > -1.0,
            "phase must not flip instantly"
        );
        assert!(whole.windows(2).all(|w| (w[1] - w[0]).abs() < 0.02));
        assert!(whole.last().unwrap() < &-0.7);
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

    /// Solo priority: when one output is soloed, all other outputs should
    /// have zero gain regardless of their mute/dim state.
    #[test]
    fn test_solo_priority_multi_output() {
        let mut plugin = MatrixPlugin::new(4, 4);
        // Solo output 2 only
        let states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
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

        let last = process_converged(&mut plugin, 4);
        // Only output 2 (soloed) should have non-zero gain
        assert!(
            last[0].abs() < TOLERANCE,
            "Ch0 (not soloed) should be silent, got {}",
            last[0]
        );
        assert!(
            last[1].abs() < TOLERANCE,
            "Ch1 (not soloed) should be silent, got {}",
            last[1]
        );
        assert!(
            (last[2] - 1.0).abs() < TOLERANCE,
            "Ch2 (soloed) should pass through, got {}",
            last[2]
        );
        assert!(
            last[3].abs() < TOLERANCE,
            "Ch3 (not soloed) should be silent, got {}",
            last[3]
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

    /// process() smoke test with a known mono-downmix matrix.
    #[test]
    fn test_process_known_mono_downmix() {
        let mut plugin = MatrixPlugin::with_matrix(2, 1, vec![0.5, 0.5]).unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 4;
        // Interleaved stereo: [L0, R0, L1, R1, ...]
        let input = vec![1.0f32, 0.0, 0.0, 1.0, 0.5, 0.5, 0.25, 0.75];
        let mut output = vec![0.0f32; num_frames];
        let context = ProcessContext::new(48000, num_frames);

        plugin.process(&input, &mut output, &context).unwrap();

        assert!((output[0] - 0.5).abs() < 1e-5);
        assert!((output[1] - 0.5).abs() < 1e-5);
        assert!((output[2] - 0.5).abs() < 1e-5);
        assert!((output[3] - 0.5).abs() < 1e-5);
    }

    /// process() smoke test with a known stereo-swap matrix.
    #[test]
    fn test_process_known_stereo_swap() {
        let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![0.0, 1.0, 1.0, 0.0]).unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![0.1f32, 0.9, 0.2, 0.8];
        let mut output = vec![0.0f32; 4];
        let context = ProcessContext::new(48000, 2);

        plugin.process(&input, &mut output, &context).unwrap();

        // Frame 0: L_out = R_in = 0.9, R_out = L_in = 0.1
        assert!((output[0] - 0.9).abs() < 1e-5);
        assert!((output[1] - 0.1).abs() < 1e-5);
        // Frame 1: L_out = R_in = 0.8, R_out = L_in = 0.2
        assert!((output[2] - 0.8).abs() < 1e-5);
        assert!((output[3] - 0.2).abs() < 1e-5);
    }

    /// set_parameter smoke test for gain and phase_invert parameters.
    #[test]
    fn test_set_parameter_gain_and_phase_invert() {
        let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();

        plugin
            .set_parameter(ParameterId::from("gain_0_0"), ParameterValue::Float(0.75))
            .unwrap();
        assert!((plugin.get_gain(0, 0).unwrap() - 0.75).abs() < 1e-6);

        plugin
            .set_parameter(ParameterId::from("gain_1_0"), ParameterValue::Float(-0.5))
            .unwrap();
        assert!((plugin.get_gain(1, 0).unwrap() - (-0.5)).abs() < 1e-6);

        plugin
            .set_parameter(
                ParameterId::from("phase_invert_0_0"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        assert!(plugin.get_phase_invert(0, 0).unwrap());

        plugin
            .set_parameter(
                ParameterId::from("phase_invert_0_0"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert!(!plugin.get_phase_invert(0, 0).unwrap());
    }

    /// Non-finite gain values are rejected without modifying the matrix.
    #[test]
    fn test_set_parameter_non_finite_gain_is_ignored() {
        let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();

        let before = plugin.get_gain(0, 0).unwrap();
        assert!(
            plugin
                .set_parameter(
                    ParameterId::from("gain_0_0"),
                    ParameterValue::Float(f32::NAN)
                )
                .is_err()
        );
        assert_eq!(plugin.get_gain(0, 0).unwrap(), before);

        assert!(
            plugin
                .set_parameter(
                    ParameterId::from("gain_0_0"),
                    ParameterValue::Float(f32::INFINITY)
                )
                .is_err()
        );
        assert_eq!(plugin.get_gain(0, 0).unwrap(), before);
    }

    /// process() with zero frames returns 0 and leaves output zeroed.
    #[test]
    fn test_process_zero_frames() {
        let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
        let input = Vec::new();
        let mut output = Vec::new();
        let context = ProcessContext::new(48000, 0);

        let processed = plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(processed, 0);
        assert!(output.is_empty());
    }

    /// apply_preset smoke tests with known presets.
    #[test]
    fn test_apply_preset_smoke() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.apply_preset("ms_encode").unwrap();
        assert!((plugin.get_gain(0, 0).unwrap() - 0.5).abs() < 1e-6);
        assert!((plugin.get_gain(1, 0).unwrap() - 0.5).abs() < 1e-6);
        assert!((plugin.get_gain(0, 1).unwrap() - 0.5).abs() < 1e-6);
        assert!((plugin.get_gain(1, 1).unwrap() - (-0.5)).abs() < 1e-6);

        plugin.apply_preset("ms_decode").unwrap();
        assert!((plugin.get_gain(0, 0).unwrap() - 1.0).abs() < 1e-6);
        assert!((plugin.get_gain(1, 0).unwrap() - 1.0).abs() < 1e-6);
        assert!((plugin.get_gain(0, 1).unwrap() - 1.0).abs() < 1e-6);
        assert!((plugin.get_gain(1, 1).unwrap() - (-1.0)).abs() < 1e-6);
    }

    /// with_matrix must reject a matrix with the wrong number of elements.
    #[test]
    fn test_with_matrix_rejects_wrong_size() {
        assert!(MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0]).is_err());
        assert!(MatrixPlugin::with_matrix(2, 1, vec![1.0, 0.0, 0.0, 1.0]).is_err());
    }

    #[test]
    fn test_set_parameter_global_gain() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin
            .set_parameter(ParameterId::from("gain"), ParameterValue::Float(0.75))
            .unwrap();
        assert!((plugin.gain - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_get_parameter_global_gain() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.gain = 0.5;
        let val = plugin.get_parameter(&ParameterId::from("gain")).unwrap();
        assert_eq!(val, ParameterValue::Float(0.5));
    }

    #[test]
    fn test_set_parameter_preset_non_int_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(
            ParameterId::from("preset"),
            ParameterValue::String("ms_encode".to_string()),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be an integer"));
    }

    #[test]
    fn test_set_parameter_preset_custom_no_change() {
        let mut plugin = MatrixPlugin::new(2, 2);
        // Apply a preset first
        plugin.apply_preset("ms_encode").unwrap();
        assert!((plugin.get_gain(0, 0).unwrap() - 0.5).abs() < 1e-6);

        // Setting preset to Custom should NOT reset the matrix
        plugin
            .set_parameter(ParameterId::from("preset"), ParameterValue::Int(0))
            .unwrap(); // 0 = Custom
        assert!((plugin.get_gain(0, 0).unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_set_parameter_preset_out_of_range_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        // Index beyond range must not silently select another preset.
        assert!(
            plugin
                .set_parameter(ParameterId::from("preset"), ParameterValue::Int(999))
                .is_err()
        );
        assert_eq!(
            plugin
                .get_parameter(&ParameterId::from("preset"))
                .unwrap()
                .as_int(),
            Some(0)
        );
    }

    #[test]
    fn test_set_parameter_phase_invert_non_bool_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(
            ParameterId::from("phase_invert_0_0"),
            ParameterValue::Float(1.0),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a bool"));
    }

    #[test]
    fn test_set_parameter_mute_non_bool_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(ParameterId::from("mute_0"), ParameterValue::Float(1.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a boolean"));
    }

    #[test]
    fn test_set_parameter_dim_non_bool_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(ParameterId::from("dim_0"), ParameterValue::Float(1.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a boolean"));
    }

    #[test]
    fn test_set_parameter_channel_states_invalid_json_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String("not valid json".to_string()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_parameter_channel_states_non_string_errors() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::Float(1.0),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be string"));
    }

    #[test]
    fn test_set_parameter_unknown_returns_error() {
        let mut plugin = MatrixPlugin::new(2, 2);
        let result = plugin.set_parameter(
            ParameterId::from("totally_unknown_param"),
            ParameterValue::Float(1.0),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown parameter"));
    }
}
