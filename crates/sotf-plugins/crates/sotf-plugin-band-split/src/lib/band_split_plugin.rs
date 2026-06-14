use super::crossover_mode::CrossoverMode;
use super::misc::MAX_BANDS;
use super::misc::parse_crossover_type_index;
use super::types::BandSplitPluginParams;
use crate::params::{CROSSOVER_TYPES, PARAMS as BS};
use sotf_host::param_bridge;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LinearSmoother, LogSmoother};

pub struct BandSplitPlugin {
    pub(super) input_channels: usize,
    pub(super) sample_rate: u32,
    pub(super) num_bands: usize,
    pub(super) crossover: CrossoverMode,
    pub(super) freq_smoothers: Vec<LogSmoother>,
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
}

impl BandSplitPlugin {
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
        let sr = 48000;
        let freq_f32: Vec<f32> = frequencies.iter().map(|&f| f as f32).collect();

        let smoothers = frequencies
            .iter()
            .map(|&f| LogSmoother::new(f as f32, 20.0, sr))
            .collect();

        let num_bands = frequencies.len() + 1;

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
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            band_gain_smoothers: gain_smoothers,
            crossover_type_index,
            cached_parameters: Vec::new(),
            dynamic_param_keys,
            band_gain_param_keys,
            band_flat: vec![0.0f32; num_bands * input_channels],
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

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
}

impl Plugin for BandSplitPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandSplit", "2.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.input_channels
    }
    fn output_channels(&self) -> usize {
        self.input_channels * self.num_bands
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = &id.0;
        let previous_crossover_type = self.crossover_type_index;

        // Try static PARAMS first (frequency at index 0, crossover_type at index 1)
        if let Ok(idx) =
            param_bridge::set_parameter(BS, &id, &value, |i, v| self.set_param_value(i, v))
        {
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
            self.rebuild_cached_parameters();
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
            if v.is_finite() {
                self.band_gains_db[band_idx] = v.clamp(-24.0, 24.0);
                let linear = 10.0f32.powf(self.band_gains_db[band_idx] / 20.0);
                self.band_gains_linear[band_idx] = linear;
                // Schedule a smooth ramp to the new gain — prevents zipper noise.
                self.band_gain_smoothers[band_idx].set_target(linear);
                self.rebuild_cached_parameters();
            }
            return Ok(());
        }

        // Match dynamic "frequency_N" (index N-1, for multiband splits)
        if let Some(suffix) = name.strip_prefix("frequency_")
            && let Ok(n) = suffix.parse::<usize>()
        {
            let i = n - 1;
            if i < self.freq_smoothers.len() {
                let v = value
                    .as_float()
                    .ok_or_else(|| "frequency must be a float".to_string())?;
                if v.is_finite() {
                    self.freq_smoothers[i].set_target(v);
                    self.rebuild_cached_parameters();
                }
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
            let i = n - 1;
            if i < self.freq_smoothers.len() {
                return Some(ParameterValue::Float(self.freq_smoothers[i].target()));
            }
        }

        None
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
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
        Ok(())
    }
    fn reset(&mut self) {
        self.crossover.reset();
        for (i, s) in self.band_gain_smoothers.iter_mut().enumerate() {
            s.reset(self.band_gains_linear[i]);
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let in_ch = self.input_channels;
        let out_ch = in_ch * self.num_bands;
        let nb = self.num_bands;

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            let frame_input = &input[in_off..in_off + in_ch];

            // Per-sample frequency smoothing: advance each smoother by one sample and
            // update the crossover coefficients. This prevents step discontinuities
            // (clicks/warbling) when the crossover frequency is automated.
            for (i, smoother) in self.freq_smoothers.iter_mut().enumerate() {
                let freq = smoother.advance();
                self.crossover.set_frequency(i, freq);
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
