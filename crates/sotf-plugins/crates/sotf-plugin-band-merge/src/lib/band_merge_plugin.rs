use super::misc::GAIN_SMOOTH_MS;
use super::misc::MAX_BANDS;
use super::misc::db_to_linear;
use super::types::BandMergePluginParams;
use crate::params::PARAMS;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BandSumPath {
    Bands2,
    Bands3,
    Bands4,
    Bands5,
    Bands6,
    Bands7,
    Bands8,
    Generic(usize),
}

impl BandSumPath {
    #[inline]
    pub(super) fn for_bands(bands: usize) -> Self {
        match bands {
            2 => Self::Bands2,
            3 => Self::Bands3,
            4 => Self::Bands4,
            5 => Self::Bands5,
            6 => Self::Bands6,
            7 => Self::Bands7,
            8 => Self::Bands8,
            count => Self::Generic(count),
        }
    }

    #[cfg(test)]
    pub(super) fn is_unrolled(self) -> bool {
        !matches!(self, Self::Generic(_))
    }
}

#[inline(always)]
pub(super) fn sum_bands(
    input_frame: &[f32],
    output_channels: usize,
    channel: usize,
    gains: &[f32; MAX_BANDS],
    path: BandSumPath,
) -> f32 {
    macro_rules! term {
        ($band:expr) => {
            input_frame[$band * output_channels + channel] * gains[$band]
        };
    }

    match path {
        BandSumPath::Bands2 => term!(0) + term!(1),
        BandSumPath::Bands3 => (term!(0) + term!(1)) + term!(2),
        BandSumPath::Bands4 => ((term!(0) + term!(1)) + term!(2)) + term!(3),
        BandSumPath::Bands5 => (((term!(0) + term!(1)) + term!(2)) + term!(3)) + term!(4),
        BandSumPath::Bands6 => {
            ((((term!(0) + term!(1)) + term!(2)) + term!(3)) + term!(4)) + term!(5)
        }
        BandSumPath::Bands7 => {
            (((((term!(0) + term!(1)) + term!(2)) + term!(3)) + term!(4)) + term!(5)) + term!(6)
        }
        BandSumPath::Bands8 => {
            ((((((term!(0) + term!(1)) + term!(2)) + term!(3)) + term!(4)) + term!(5)) + term!(6))
                + term!(7)
        }
        BandSumPath::Generic(bands) => {
            let mut sum = 0.0;
            for band in 0..bands {
                sum += term!(band);
            }
            sum
        }
    }
}

pub struct BandMergePlugin {
    pub(super) output_channels: usize,
    pub(super) num_bands: usize,
    pub(super) param_bands: ParameterId,
    /// Per-band gain in dB (up to MAX_BANDS).
    pub(super) band_gains_db: [f32; MAX_BANDS],
    /// Per-band linear gain (precomputed from dB).
    pub(super) band_gains_linear: [f32; MAX_BANDS],
    /// Per-band mute toggle.
    pub(super) band_mutes: [bool; MAX_BANDS],
    /// Reconstruction error in dB (diagnostic). Measures how much the output
    /// deviates from perfect reconstruction.
    ///
    /// This value is refreshed when requested by the host.
    pub(super) reconstruction_error_db: f32,
    pub(super) reconstruction_error_requested: Cell<bool>,
    pub(super) cached_parameters: Vec<Parameter>,
    // ---- gain smoothing ----
    /// Per-band one-pole gain smoother to prevent zipper noise during automation.
    pub(super) band_gain_smoothers: [sotf_host::smoothing::Smoother; MAX_BANDS],
    /// Sample rate, needed to reinitialise smoothers on initialize().
    pub(super) sample_rate: u32,
    pub(super) initialized: bool,
}

impl BandMergePlugin {
    pub fn new(output_channels: usize, bands: usize) -> Result<Self, String> {
        if output_channels == 0 {
            return Err("Band Merge requires at least one output channel".into());
        }
        if bands < 2 {
            return Err("Min 2 bands".into());
        }
        if bands > MAX_BANDS {
            return Err(format!("Max {} bands", MAX_BANDS));
        }
        output_channels
            .checked_mul(bands)
            .ok_or_else(|| "Band Merge channel count overflow".to_string())?;
        // Default sample rate before initialize() is called.
        const DEFAULT_SR: u32 = 48000;
        let mut p = Self {
            output_channels,
            num_bands: bands,
            param_bands: ParameterId::from("bands"),
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            band_mutes: [false; MAX_BANDS],
            reconstruction_error_db: 0.0,
            reconstruction_error_requested: Cell::new(false),
            cached_parameters: Vec::new(),
            band_gain_smoothers: std::array::from_fn(|_| {
                sotf_host::smoothing::Smoother::new(1.0, GAIN_SMOOTH_MS, DEFAULT_SR)
            }),
            sample_rate: DEFAULT_SR,
            initialized: false,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(
        output_channels: usize,
        params: &BandMergePluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(output_channels, params.bands)?;
        for (index, &gain) in params.band_gains_db.iter().enumerate() {
            if index >= params.bands {
                break;
            }
            if !gain.is_finite() || !(-60.0..=24.0).contains(&gain) {
                return Err(format!(
                    "band_{index}_gain_db must be finite and in [-60, 24], got {gain}"
                ));
            }
        }
        for (i, &g) in params.band_gains_db.iter().enumerate().take(params.bands) {
            p.band_gains_db[i] = g;
            p.band_gains_linear[i] = db_to_linear(g);
            // Snap smoother to the preset value immediately (no ramp on load).
            p.band_gain_smoothers[i].reset(db_to_linear(g));
        }
        for (i, &m) in params.band_mutes.iter().enumerate().take(params.bands) {
            p.band_mutes[i] = m;
            if m {
                p.band_gain_smoothers[i].reset(0.0);
            }
        }
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_int("bands", "Bands", self.num_bands as i32, 2, MAX_BANDS as i32)
                .with_update_mode(pk(PARAMS, "bands").update_mode),
        ];
        for i in 0..self.num_bands {
            let gain_id = format!("band_{}_gain_db", i);
            let gain_label = format!("Band {} Gain (dB)", i);
            params.push(Parameter::new_float(
                &gain_id,
                &gain_label,
                self.band_gains_db[i],
                -60.0,
                24.0,
            ));
            let mute_id = format!("band_{}_mute", i);
            let mute_label = format!("Band {} Mute", i);
            params.push(Parameter::new_bool(
                &mute_id,
                &mute_label,
                self.band_mutes[i],
            ));
        }
        params.push(
            Parameter::new_float(
                "reconstruction_error_db",
                "Reconstruction Error",
                self.reconstruction_error_db,
                -60.0,
                60.0,
            )
            .with_description(
                "Normalized RMS(output - unity-band sum) in dB (read-only diagnostic)",
            )
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
        );
        self.cached_parameters = params;
    }
}

impl Plugin for BandMergePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandMerge", env!("CARGO_PKG_VERSION"), "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.output_channels
            .checked_mul(self.num_bands)
            .expect("validated Band Merge channel count")
    }
    fn output_channels(&self) -> usize {
        self.output_channels
    }
    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::linear_transform(
            PluginCostClass::Scalar,
            None,
            0,
            true,
            true,
            false,
        );
        metadata.boundary = true;
        metadata
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_bands {
            let v = value.as_int().ok_or("val")? as usize;
            if !(2..=MAX_BANDS).contains(&v) {
                return Err(format!("bands must be 2..={}", MAX_BANDS));
            }
            if v != self.num_bands {
                return Err(format!(
                    "bands is structural; rebuild Band Merge to change {} to {v}",
                    self.num_bands
                ));
            }
            return Ok(());
        }
        // Check per-band parameters using prefix parsing (no heap allocation)
        if let Some(rest) = id.0.strip_prefix("band_") {
            if let Some(idx_str) = rest.strip_suffix("_gain_db") {
                if let Ok(i) = idx_str.parse::<usize>()
                    && i < self.num_bands
                {
                    let v = value
                        .as_float()
                        .ok_or_else(|| format!("band_{}_gain_db must be a float", i))?;
                    if !v.is_finite() || !(-60.0..=24.0).contains(&v) {
                        return Err(format!(
                            "band_{i}_gain_db must be finite and in [-60, 24], got {v}"
                        ));
                    }
                    self.band_gains_db[i] = v;
                    let linear = db_to_linear(v);
                    self.band_gains_linear[i] = linear;
                    self.band_gain_smoothers[i].set_target(if self.band_mutes[i] {
                        0.0
                    } else {
                        linear
                    });
                    if let Some(parameter) = self
                        .cached_parameters
                        .iter_mut()
                        .find(|parameter| parameter.id == id)
                    {
                        parameter.default_value = ParameterValue::Float(v);
                    }
                    return Ok(());
                }
            } else if let Some(idx_str) = rest.strip_suffix("_mute")
                && let Ok(i) = idx_str.parse::<usize>()
                && i < self.num_bands
            {
                let v = value
                    .as_bool()
                    .ok_or_else(|| format!("band_{}_mute must be a bool", i))?;
                self.band_mutes[i] = v;
                self.band_gain_smoothers[i].set_target(if v {
                    0.0
                } else {
                    self.band_gains_linear[i]
                });
                if let Some(parameter) = self
                    .cached_parameters
                    .iter_mut()
                    .find(|parameter| parameter.id == id)
                {
                    parameter.default_value = ParameterValue::Bool(v);
                }
                return Ok(());
            }
        }
        Err(format!("Unknown parameter: {}", id))
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bands {
            return Some(ParameterValue::Int(self.num_bands as i32));
        }
        if id.as_str() == "reconstruction_error_db" {
            self.reconstruction_error_requested.set(true);
            return Some(ParameterValue::Float(self.reconstruction_error_db));
        }
        if let Some(rest) = id.0.strip_prefix("band_") {
            if let Some(idx_str) = rest.strip_suffix("_gain_db") {
                if let Ok(i) = idx_str.parse::<usize>()
                    && i < self.num_bands
                {
                    return Some(ParameterValue::Float(self.band_gains_db[i]));
                }
            } else if let Some(idx_str) = rest.strip_suffix("_mute")
                && let Ok(i) = idx_str.parse::<usize>()
                && i < self.num_bands
            {
                return Some(ParameterValue::Bool(self.band_mutes[i]));
            }
        }
        None
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Band Merge sample rate must be greater than zero".into());
        }
        self.sample_rate = sample_rate;
        for i in 0..MAX_BANDS {
            self.band_gain_smoothers[i].set_time(GAIN_SMOOTH_MS, sample_rate);
        }
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        // Snap all smoothers to their current target so playback resumes
        // without a ramp artefact after a transport reset.
        for i in 0..self.num_bands {
            self.band_gain_smoothers[i].reset(if self.band_mutes[i] {
                0.0
            } else {
                self.band_gains_linear[i]
            });
        }
        self.reconstruction_error_requested.set(false);
        self.reconstruction_error_db = 0.0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if !self.initialized {
            return Err("Band Merge must be initialized before processing".into());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Band Merge context rate {} Hz differs from initialized rate {} Hz",
                context.sample_rate, self.sample_rate
            ));
        }
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let out_ch = self.output_channels;
        let in_ch = out_ch
            .checked_mul(self.num_bands)
            .ok_or_else(|| "Band Merge input channel count overflow".to_string())?;
        let expected_input = num_frames
            .checked_mul(in_ch)
            .ok_or_else(|| "Band Merge input length overflow".to_string())?;
        let expected_output = num_frames
            .checked_mul(out_ch)
            .ok_or_else(|| "Band Merge output length overflow".to_string())?;
        if input.len() != expected_input {
            return Err(format!(
                "Band Merge expected {expected_input} input samples, got {}",
                input.len()
            ));
        }
        if output.len() != expected_output {
            return Err(format!(
                "Band Merge expected {expected_output} output samples, got {}",
                output.len()
            ));
        }

        // Accumulate normalized RMS error against the unity-gain, unmuted sum.
        let measure_reconstruction_error = self.reconstruction_error_requested.replace(false);
        let mut reference_energy = 0.0_f64;
        let mut error_energy = 0.0_f64;

        let mut effective_gains = [0.0f32; MAX_BANDS];
        let band_sum_path = BandSumPath::for_bands(self.num_bands);

        for frame in 0..num_frames {
            for (band, gain) in effective_gains.iter_mut().enumerate().take(self.num_bands) {
                *gain = self.band_gain_smoothers[band].advance();
            }
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            let input_frame = &input[in_off..in_off + in_ch];
            for ch in 0..out_ch {
                let sum = sum_bands(input_frame, out_ch, ch, &effective_gains, band_sum_path);
                output[out_off + ch] = sum;
                if measure_reconstruction_error {
                    let mut ref_sum = 0.0_f32;
                    for band in 0..self.num_bands {
                        ref_sum += input_frame[band * out_ch + ch];
                    }
                    let error = sum - ref_sum;
                    reference_energy += (ref_sum as f64) * (ref_sum as f64);
                    error_energy += (error as f64) * (error as f64);
                }
            }
        }

        // Compute normalized reconstruction error in dB. The floor represents
        // exact reconstruction; the ceiling keeps a zero-reference mismatch finite.
        if measure_reconstruction_error {
            let total_samples = (num_frames * out_ch) as f64;
            if total_samples == 0.0 || error_energy == 0.0 {
                self.reconstruction_error_db = -60.0;
            } else {
                let ref_rms = (reference_energy / total_samples).sqrt();
                let error_rms = (error_energy / total_samples).sqrt();
                let normalized_error = error_rms / ref_rms.max(1e-10);
                self.reconstruction_error_db =
                    (20.0 * normalized_error.log10()).clamp(-60.0, 60.0) as f32;
            }
        }

        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}
