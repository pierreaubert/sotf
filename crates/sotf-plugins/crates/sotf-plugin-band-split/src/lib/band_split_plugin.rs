use super::crossover_mode::CrossoverMode;
use super::misc::MAX_BANDS;
use super::misc::parse_crossover_type_index;
use super::types::BandSplitPluginParams;
use crate::params::{CROSSOVER_TYPES, PARAMS as BS};
use sotf_host::param_bridge;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LinearSmoother, LogSmoother};

/// Redesign IIR coefficients at 6 kHz rather than at audio rate. Frequency
/// targets still follow the per-sample logarithmic smoother; the crossover
/// receives stable, bounded-rate snapshots whose phase persists across host
/// callback boundaries.
pub(super) const COEFFICIENT_UPDATE_INTERVAL: usize = 8;

pub struct BandSplitPlugin {
    pub(super) input_channels: usize,
    pub(super) sample_rate: u32,
    pub(super) num_bands: usize,
    pub(super) crossover: CrossoverMode,
    pub(super) freq_smoothers: Vec<LogSmoother>,
    pub(super) applied_frequencies: Vec<f32>,
    pub(super) coefficient_update_countdown: usize,
    #[cfg(test)]
    pub(super) coefficient_update_count: usize,
    /// Per-band gain in dB (one per band, up to MAX_BANDS). Default 0.0 dB.
    pub(super) band_gains_db: [f32; MAX_BANDS],
    /// Pre-computed linear multipliers from band_gains_db (target, for reference).
    pub(super) band_gains_linear: [f32; MAX_BANDS],
    /// One-pole smoothers for per-band linear gains. Prevents zipper noise when gains change.
    pub(super) band_gain_smoothers: [LinearSmoother; MAX_BANDS],
    /// Crossover type string for param_bridge (Choice index <-> string)
    pub(super) crossover_type_index: usize,
    pub(super) cached_parameters: Vec<sotf_host::parameters::Parameter>,
    /// Pre-built parameter IDs and display names for the dynamic frequency and
    /// per-band gain parameters, so `rebuild_cached_parameters` does not
    /// re-format them on every call.
    pub(super) dynamic_param_keys: Vec<(ParameterId, String)>,
    pub(super) band_gain_param_keys: Vec<(ParameterId, String)>,
    /// Pre-allocated flat scratch buffer: [num_bands * input_channels] for per-frame band output.
    pub(super) band_flat: Vec<f32>,
    pub(super) initialized: bool,
}

impl BandSplitPlugin {
    pub(super) fn checked_output_channels(
        input_channels: usize,
        num_bands: usize,
    ) -> Result<usize, String> {
        input_channels
            .checked_mul(num_bands)
            .ok_or_else(|| "BandSplit channel count overflow".to_string())
    }

    pub fn new(
        input_channels: usize,
        frequency: f64,
        crossover_type: &str,
    ) -> Result<Self, String> {
        Self::new_multiband(input_channels, &[frequency], crossover_type)
    }

    pub fn new_multiband(
        input_channels: usize,
        frequencies: &[f64],
        crossover_type: &str,
    ) -> Result<Self, String> {
        if frequencies.is_empty() {
            return Err("At least one crossover frequency is required".to_string());
        }
        if frequencies.len() + 1 > MAX_BANDS {
            return Err(format!(
                "Too many bands: {} (max {})",
                frequencies.len() + 1,
                MAX_BANDS
            ));
        }
        if input_channels == 0 {
            return Err("input_channels must be greater than zero".to_string());
        }
        if !CROSSOVER_TYPES
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case(crossover_type))
        {
            return Err(format!(
                "unsupported crossover type {crossover_type:?}; expected LR24 or LR48"
            ));
        }
        Self::validate_frequencies(frequencies, 48_000)?;
        let sr = 48000;
        let freq_f32: Vec<f32> = frequencies.iter().map(|&f| f as f32).collect();

        let smoothers = frequencies
            .iter()
            .map(|&f| LogSmoother::new(f as f32, 20.0, sr))
            .collect();

        let num_bands = frequencies.len() + 1;
        let output_channels = Self::checked_output_channels(input_channels, num_bands)?;

        // Per-band gain smoothers at unity (0 dB → linear 1.0), 20 ms smoothing.
        let gain_smoothers = [
            LinearSmoother::new(1.0, 20.0, sr),
            LinearSmoother::new(1.0, 20.0, sr),
            LinearSmoother::new(1.0, 20.0, sr),
            LinearSmoother::new(1.0, 20.0, sr),
        ];

        let crossover_type_index = parse_crossover_type_index(crossover_type);
        let (dynamic_param_keys, band_gain_param_keys) =
            Self::build_param_keys(num_bands, frequencies.len());

        let mut p = Self {
            input_channels,
            sample_rate: sr,
            num_bands,
            crossover: CrossoverMode::new(&freq_f32, sr, input_channels, crossover_type_index),
            freq_smoothers: smoothers,
            applied_frequencies: freq_f32,
            coefficient_update_countdown: 0,
            #[cfg(test)]
            coefficient_update_count: 0,
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            band_gain_smoothers: gain_smoothers,
            crossover_type_index,
            cached_parameters: Vec::new(),
            dynamic_param_keys,
            band_gain_param_keys,
            band_flat: vec![0.0f32; output_channels],
            initialized: false,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn validate_frequencies(frequencies: &[f64], sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("sample rate must be greater than zero".to_string());
        }
        let max_frequency = (sample_rate as f64 * 0.49).min(20_000.0);
        let mut previous = 0.0;
        for (index, &frequency) in frequencies.iter().enumerate() {
            if !frequency.is_finite() || frequency < 20.0 || frequency > max_frequency {
                return Err(format!(
                    "frequency {} must be finite and within 20..={max_frequency} Hz",
                    index + 1
                ));
            }
            if index > 0 && frequency <= previous {
                return Err("crossover frequencies must be strictly ascending".to_string());
            }
            previous = frequency;
        }
        Ok(())
    }

    fn validate_frequency_target(&self, index: usize, frequency: f32) -> PluginResult<()> {
        let max_frequency = (self.sample_rate as f32 * 0.49).min(20_000.0);
        if !frequency.is_finite() || frequency < 20.0 || frequency > max_frequency {
            return Err(format!(
                "frequency must be finite and within 20..={max_frequency} Hz"
            ));
        }
        if index > 0 && frequency <= self.freq_smoothers[index - 1].target() {
            return Err("frequency must be above the previous crossover".to_string());
        }
        if index + 1 < self.freq_smoothers.len()
            && frequency >= self.freq_smoothers[index + 1].target()
        {
            return Err("frequency must be below the next crossover".to_string());
        }
        Ok(())
    }

    #[allow(
        clippy::type_complexity,
        reason = "parameter key tuple is the natural representation for this helper"
    )]
    fn build_param_keys(
        num_bands: usize,
        num_frequencies: usize,
    ) -> (Vec<(ParameterId, String)>, Vec<(ParameterId, String)>) {
        let dynamic_param_keys: Vec<_> = (1..num_frequencies)
            .map(|i| {
                let id = format!("frequency_{}", i + 1);
                let name = format!("Frequency {}", i + 1);
                (ParameterId::from(id.as_str()), name)
            })
            .collect();
        let band_gain_param_keys: Vec<_> = (0..num_bands)
            .map(|i| {
                let id = format!("band_{}_gain_db", i);
                let name = format!("Band {} Gain (dB)", i + 1);
                (ParameterId::from(id.as_str()), name)
            })
            .collect();
        (dynamic_param_keys, band_gain_param_keys)
    }

    pub fn from_params(
        input_channels: usize,
        params: &BandSplitPluginParams,
    ) -> Result<Self, String> {
        let freqs = if !params.frequencies.is_empty() {
            params.frequencies.clone()
        } else {
            // Use num_bands to determine the number of crossover frequencies.
            match params.num_bands {
                2 => vec![params.frequency],
                3 => {
                    // Default 3-band: split at frequency and two octaves up.
                    let f1 = params.frequency;
                    let f2 = (f1 * 4.0).min(20000.0);
                    vec![f1, f2]
                }
                4 => {
                    // Default 4-band: two octaves per split.
                    let f1 = params.frequency;
                    let f2 = (f1 * 4.0).min(20000.0);
                    let f3 = (f2 * 4.0).min(20000.0);
                    vec![f1, f2, f3]
                }
                n => return Err(format!("Unsupported num_bands: {} (must be 2-4)", n)),
            }
        };
        Self::new_multiband(input_channels, &freqs, &params.crossover_type)
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(
                self.freq_smoothers
                    .first()
                    .map(|s| s.target() as f64)
                    .unwrap_or(300.0),
            ),
            1 => Some(self.crossover_type_index as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {
                if let Some(s) = self.freq_smoothers.first_mut() {
                    s.set_target(value as f32);
                }
            }
            1 => {
                self.crossover_type_index = (value as usize).min(CROSSOVER_TYPES.len() - 1);
            }
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        // Start with the static PARAMS entries (frequency, crossover_type)
        let mut params = param_bridge::build_parameters(BS, |i| self.param_value(i));
        // Add dynamic frequency parameters (frequency_2, frequency_3, ...)
        for ((_i, smoother), (id, name)) in self
            .freq_smoothers
            .iter()
            .enumerate()
            .skip(1)
            .zip(self.dynamic_param_keys.iter())
        {
            params.push(sotf_host::parameters::Parameter::new_float(
                &id.0,
                name,
                smoother.target(),
                20.0,
                20000.0,
            ));
        }
        // Add dynamic per-band gain parameters
        for (i, (id, name)) in self.band_gain_param_keys.iter().enumerate() {
            params.push(
                sotf_host::parameters::Parameter::new_float(
                    &id.0,
                    name,
                    self.band_gains_db[i],
                    -24.0,
                    24.0,
                )
                .with_group("Band Gains"),
            );
        }
        self.cached_parameters = params;
    }

    fn update_cached_parameter(&mut self, id: &ParameterId, value: ParameterValue) {
        if let Some(parameter) = self
            .cached_parameters
            .iter_mut()
            .find(|parameter| parameter.id == *id)
        {
            parameter.default_value = value;
        }
    }
}

impl Plugin for BandSplitPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandSplit", env!("CARGO_PKG_VERSION"), "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.input_channels
    }
    fn output_channels(&self) -> usize {
        Self::checked_output_channels(self.input_channels, self.num_bands)
            .expect("validated BandSplit channel count")
    }
    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::linear_transform(
            PluginCostClass::Iir,
            None,
            0,
            false,
            true,
            false,
        );
        metadata.boundary = true;
        metadata
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = &id.0;
        let previous_crossover_type = self.crossover_type_index;

        if id.as_str() == "crossover_type" && self.initialized {
            let requested = value
                .as_int()
                .ok_or_else(|| "crossover_type must be a choice index".to_string())?;
            if !(0..CROSSOVER_TYPES.len() as i32).contains(&requested) {
                return Err("crossover_type choice index is out of range".to_string());
            }
            if requested as usize == self.crossover_type_index {
                return Ok(());
            }
            return Err("crossover_type is structural; rebuild the plugin".to_string());
        }
        if id.as_str() == "crossover_type"
            && !matches!(value, ParameterValue::Int(index) if (0..CROSSOVER_TYPES.len() as i32).contains(&index))
        {
            return Err("crossover_type must be an LR24/LR48 choice index (0 or 1)".to_string());
        }
        if id.as_str() == "frequency" {
            let frequency = value
                .as_float()
                .ok_or_else(|| "frequency must be a float".to_string())?;
            self.validate_frequency_target(0, frequency)?;
        }

        // Try static PARAMS first (frequency at index 0, crossover_type at index 1)
        if BS.iter().any(|spec| spec.engine_key == id.as_str()) {
            let idx =
                param_bridge::set_parameter(BS, &id, &value, |i, v| self.set_param_value(i, v))?;
            // Side effect: frequency change needs to propagate to crossover
            if idx == 0 {
                // frequency was already set via set_param_value -> smoother.set_target
            } else if idx == 1 && self.crossover_type_index != previous_crossover_type {
                let freqs: Vec<f32> = self.freq_smoothers.iter().map(|s| s.target()).collect();
                self.crossover.reinit(
                    &freqs,
                    self.sample_rate,
                    self.input_channels,
                    self.crossover_type_index,
                );
            }
            self.update_cached_parameter(&id, value);
            return Ok(());
        }

        // Match dynamic "band_N_gain_db"
        if let Some(rest) = name.strip_prefix("band_")
            && let Some(idx_str) = rest.strip_suffix("_gain_db")
            && let Ok(band_idx) = idx_str.parse::<usize>()
            && band_idx < self.num_bands
        {
            let v = value
                .as_float()
                .ok_or_else(|| "band gain must be a float".to_string())?;
            if !v.is_finite() || !(-24.0..=24.0).contains(&v) {
                return Err("band gain must be finite and within -24..=24 dB".to_string());
            }
            self.band_gains_db[band_idx] = v;
            let linear = 10.0f32.powf(v / 20.0);
            self.band_gains_linear[band_idx] = linear;
            self.band_gain_smoothers[band_idx].set_target(linear);
            self.update_cached_parameter(&id, ParameterValue::Float(v));
            return Ok(());
        }

        // Match dynamic "frequency_N" (index N-1, for multiband splits)
        if let Some(suffix) = name.strip_prefix("frequency_")
            && let Ok(n) = suffix.parse::<usize>()
        {
            let Some(i) = n.checked_sub(1) else {
                return Err(format!("Unknown parameter: {id}"));
            };
            if i < self.freq_smoothers.len() {
                let v = value
                    .as_float()
                    .ok_or_else(|| "frequency must be a float".to_string())?;
                self.validate_frequency_target(i, v)?;
                self.freq_smoothers[i].set_target(v);
                self.update_cached_parameter(&id, ParameterValue::Float(v));
                return Ok(());
            }
        }

        Err(format!("Unknown parameter: {}", id))
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Try static PARAMS first
        if let Some(val) = param_bridge::get_parameter(BS, id, |i| self.param_value(i)) {
            return Some(val);
        }

        let name = &id.0;

        // Match dynamic "band_N_gain_db"
        if let Some(rest) = name.strip_prefix("band_")
            && let Some(idx_str) = rest.strip_suffix("_gain_db")
            && let Ok(band_idx) = idx_str.parse::<usize>()
            && band_idx < self.num_bands
        {
            return Some(ParameterValue::Float(self.band_gains_db[band_idx]));
        }

        // Match dynamic "frequency_N"
        if let Some(suffix) = name.strip_prefix("frequency_")
            && let Ok(n) = suffix.parse::<usize>()
        {
            let i = n.checked_sub(1)?;
            if i < self.freq_smoothers.len() {
                return Some(ParameterValue::Float(self.freq_smoothers[i].target()));
            }
        }

        None
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        let frequencies: Vec<f64> = self
            .freq_smoothers
            .iter()
            .map(|s| s.target() as f64)
            .collect();
        Self::validate_frequencies(&frequencies, sample_rate)?;
        self.sample_rate = sample_rate;
        let freqs: Vec<f32> = self.freq_smoothers.iter().map(|s| s.target()).collect();
        for s in &mut self.freq_smoothers {
            *s = LogSmoother::new(s.target(), 20.0, sample_rate);
        }
        for (i, s) in self.band_gain_smoothers.iter_mut().enumerate() {
            *s = LinearSmoother::new(self.band_gains_linear[i], 20.0, sample_rate);
        }
        self.crossover.reinit(
            &freqs,
            sample_rate,
            self.input_channels,
            self.crossover_type_index,
        );
        self.applied_frequencies.copy_from_slice(&freqs);
        self.coefficient_update_countdown = 0;
        #[cfg(test)]
        {
            self.coefficient_update_count = 0;
        }
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        self.crossover.reset();
        for (i, s) in self.band_gain_smoothers.iter_mut().enumerate() {
            s.reset(self.band_gains_linear[i]);
        }
        for (i, smoother) in self.freq_smoothers.iter_mut().enumerate() {
            let target = smoother.target();
            smoother.reset(target);
            self.crossover.set_frequency(i, target);
            self.applied_frequencies[i] = target;
        }
        self.coefficient_update_countdown = 0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if !self.initialized {
            return Err("BandSplit must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "BandSplit context rate {} Hz differs from initialized rate {} Hz",
                context.sample_rate, self.sample_rate
            ));
        }
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let in_ch = self.input_channels;
        let out_ch = Self::checked_output_channels(in_ch, self.num_bands)?;
        let nb = self.num_bands;
        let expected_input = num_frames
            .checked_mul(in_ch)
            .ok_or_else(|| "BandSplit input buffer size overflow".to_string())?;
        let expected_output = num_frames
            .checked_mul(out_ch)
            .ok_or_else(|| "BandSplit output buffer size overflow".to_string())?;
        if input.len() != expected_input {
            return Err(format!(
                "BandSplit input length mismatch: expected {expected_input}, got {}",
                input.len()
            ));
        }
        if output.len() != expected_output {
            return Err(format!(
                "BandSplit output length mismatch: expected {expected_output}, got {}",
                output.len()
            ));
        }

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            let frame_input = &input[in_off..in_off + in_ch];

            // Advance parameter smoothing at audio rate, but redesign IIR
            // coefficients only at the bounded control rate. The countdown is
            // persistent, so the trajectory is independent of host partitioning.
            for smoother in &mut self.freq_smoothers {
                smoother.advance();
            }
            if self.coefficient_update_countdown == 0 {
                for i in 0..self.freq_smoothers.len() {
                    let frequency = self.freq_smoothers[i].current();
                    if frequency != self.applied_frequencies[i] {
                        self.crossover.set_frequency(i, frequency);
                        self.applied_frequencies[i] = frequency;
                        #[cfg(test)]
                        {
                            self.coefficient_update_count += 1;
                        }
                    }
                }
                self.coefficient_update_countdown = COEFFICIENT_UPDATE_INTERVAL - 1;
            } else {
                self.coefficient_update_countdown -= 1;
            }

            // Build mutable slice refs from pre-allocated flat buffer using split_at_mut
            // (no heap allocation). Fixed-size array since MAX_BANDS=4.
            {
                let flat = &mut self.band_flat[..nb * in_ch];
                let mut band_slices: [&mut [f32]; MAX_BANDS] = [&mut [], &mut [], &mut [], &mut []];
                let mut chunks = flat.chunks_exact_mut(in_ch);
                for band in band_slices.iter_mut().take(nb) {
                    *band = chunks.next().unwrap();
                }
                self.crossover
                    .process_frame(frame_input, &mut band_slices[..nb]);
            }

            // Interleave bands into output: [band0_ch0, band0_ch1, band1_ch0, band1_ch1, ...]
            // Per-sample gain smoothing: advance each smoother by one sample to
            // prevent zipper noise when band gains are automated.
            for band_idx in 0..nb {
                let gain = self.band_gain_smoothers[band_idx].advance();
                let band_off = band_idx * in_ch;
                for ch in 0..in_ch {
                    output[out_off + band_idx * in_ch + ch] = self.band_flat[band_off + ch] * gain;
                }
            }
        }

        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}
