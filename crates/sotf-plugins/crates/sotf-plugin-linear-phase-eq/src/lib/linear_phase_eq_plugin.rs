use super::default::default_fir_length_index;
use super::default::default_num_filters;
use super::default::default_phase_mode_index;
use super::eq_band::EqBand;
use super::misc::MAG_RESPONSE_POINTS;
use super::misc::filter_type_to_index;
use super::misc::fir_length_from_index;
use super::misc::parse_filter_type;
use super::types::LinearPhaseEqPluginParams;
use crate::params::{FIR_LENGTH_OPTIONS, MAX_FILTERS, PARAMS as LP_PARAMS, PHASE_MODE_OPTIONS};
use math_audio_iir_fir::{
    Biquad, BiquadFilterType, FirDesignConfig, FirPhase, WindowType, generate_fir_from_response,
};
use num_complex::Complex;
use plugins_spatial::nupc::NupcEngine;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use sotf_host::param_bridge::apply_spec_update_modes;
use sotf_host::param_specs::UpdateMode;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

#[allow(
    dead_code,
    reason = "legacy OLA buffers retained for state-format compatibility"
)]
pub struct LinearPhaseEqPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) num_filters: usize,
    pub(super) fir_length_index: usize,
    pub(super) phase_mode_index: usize,
    pub(super) auto_gain: bool,
    pub(super) mix_value: f32,

    // EQ band definitions (for magnitude computation only)
    pub(super) bands: Vec<EqBand>,

    // FIR state
    pub(super) fir_coeffs: Vec<f32>,
    pub(super) fir_spectrum: Vec<Complex<f32>>,
    pub(super) fir_dirty: bool,
    /// Non-uniform partitioned convolvers with a bounded 32-sample head.
    pub(super) convolvers: Vec<NupcEngine>,

    // FFT planners (Arc'd, no mutex)
    pub(super) fft_forward: Arc<dyn RealToComplex<f32>>,
    pub(super) fft_inverse: Arc<dyn ComplexToReal<f32>>,
    pub(super) fft_size: usize,

    // Pre-allocated processing buffers (per-channel is handled by reuse)
    pub(super) input_buf: Vec<f32>,
    pub(super) output_buf: Vec<f32>,
    pub(super) freq_buf: Vec<Complex<f32>>,
    pub(super) fft_scratch_fwd: Vec<Complex<f32>>,
    pub(super) fft_scratch_inv: Vec<Complex<f32>>,

    // Per-channel overlap-add tail
    pub(super) overlap: Vec<Vec<f32>>,

    // FIR design scratch, reused across parameter changes.
    pub(super) design_freqs: Vec<f64>,
    pub(super) design_magnitudes_db: Vec<f64>,

    // Dry buffer for mix
    pub(super) dry_buf: Vec<f32>,
    // Per-channel dry delay used to align the dry branch with linear-phase FIR latency.
    pub(super) dry_delay: Vec<Vec<f32>>,
    pub(super) dry_delay_pos: usize,

    // Smoothers
    pub(super) mix_smoother: Smoother,

    pub(super) cached_parameters: Vec<Parameter>,
}

impl LinearPhaseEqPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let fir_length_index = default_fir_length_index();
        let fir_length = fir_length_from_index(fir_length_index);
        let num_filters = default_num_filters();

        Self::build(
            channels,
            sample_rate,
            num_filters,
            fir_length_index,
            fir_length,
            default_phase_mode_index(),
            false,
            1.0,
            Vec::new(),
        )
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: LinearPhaseEqPluginParams,
    ) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }
        if !params.mix.is_finite() || !(0.0..=1.0).contains(&params.mix) {
            return Err(format!(
                "mix must be finite and within [0, 1], got {}",
                params.mix
            ));
        }
        let fir_length_index = params.fir_length_index.min(FIR_LENGTH_OPTIONS.len() - 1);
        let phase_mode_index = params.phase_mode_index.min(PHASE_MODE_OPTIONS.len() - 1);
        let fir_length = fir_length_from_index(fir_length_index);
        let num_filters = params.num_filters.clamp(1, MAX_FILTERS);
        let sr = sample_rate as f64;

        let mut bands = Vec::with_capacity(num_filters);
        for (i, fc) in params.filters.iter().enumerate() {
            if i >= num_filters {
                break;
            }
            let ft = parse_filter_type(&fc.filter_type)?;
            Self::validate_band(fc.frequency, fc.q, fc.gain_db, sr)?;
            bands.push(EqBand::new(
                ft,
                fc.frequency,
                fc.q,
                fc.gain_db,
                fc.active,
                sr,
            ));
        }
        // Fill remaining bands with defaults
        while bands.len() < num_filters {
            bands.push(EqBand::new(
                BiquadFilterType::Peak,
                1000.0,
                1.0,
                0.0,
                true,
                sr,
            ));
        }

        Ok(Self::build(
            channels,
            sample_rate,
            num_filters,
            fir_length_index,
            fir_length,
            phase_mode_index,
            params.auto_gain,
            params.mix,
            bands,
        ))
    }

    fn validate_band(frequency: f64, q: f64, gain_db: f64, sample_rate: f64) -> Result<(), String> {
        let max_frequency = (sample_rate * 0.5 * 0.99).min(20_000.0);
        if !frequency.is_finite() || frequency < 20.0 || frequency > max_frequency {
            return Err(format!(
                "frequency must be finite and within [20, {max_frequency}], got {frequency}"
            ));
        }
        if !q.is_finite() || !(0.1..=10.0).contains(&q) {
            return Err(format!("Q must be finite and within [0.1, 10], got {q}"));
        }
        if !gain_db.is_finite() || !(-24.0..=24.0).contains(&gain_db) {
            return Err(format!(
                "gain must be finite and within [-24, 24], got {gain_db}"
            ));
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "internal builder: each argument maps to a distinct FIR-EQ configuration field"
    )]
    pub(super) fn build(
        channels: usize,
        sample_rate: u32,
        num_filters: usize,
        fir_length_index: usize,
        fir_length: usize,
        phase_mode_index: usize,
        auto_gain: bool,
        mix: f32,
        mut bands: Vec<EqBand>,
    ) -> Self {
        let sr = sample_rate as f64;
        // Fill bands to num_filters
        while bands.len() < num_filters {
            bands.push(EqBand::new(
                BiquadFilterType::Peak,
                1000.0,
                1.0,
                0.0,
                true,
                sr,
            ));
        }

        // FFT size = fir_length + max_frame_size - 1, rounded up to power of 2.
        // We use 2 * fir_length as a safe FFT size (supports frames up to fir_length+1).
        let fft_size = (fir_length * 2).next_power_of_two();
        let freq_size = fft_size / 2 + 1;

        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        let fft_scratch_fwd = vec![Complex::new(0.0, 0.0); fft_forward.get_scratch_len()];
        let fft_scratch_inv = vec![Complex::new(0.0, 0.0); fft_inverse.get_scratch_len()];

        // Max buffer size: generous allocation for typical audio frame sizes
        let max_buf = fft_size * channels;
        // Fixed-capacity dry ring: reserve the largest supported linear-phase
        // latency during construction so phase/FIR changes never allocate in
        // the processing callback.
        let dry_delay_len = FIR_LENGTH_OPTIONS
            .iter()
            .filter_map(|length| length.parse::<usize>().ok())
            .map(|length| length / 2 + 32)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut plugin = Self {
            channels,
            sample_rate,
            num_filters,
            fir_length_index,
            phase_mode_index,
            auto_gain,
            mix_value: mix,
            bands,
            fir_coeffs: vec![0.0; fir_length],
            fir_spectrum: vec![Complex::new(0.0, 0.0); freq_size],
            fir_dirty: true,
            convolvers: Vec::new(),
            fft_forward,
            fft_inverse,
            fft_size,
            input_buf: vec![0.0; fft_size],
            output_buf: vec![0.0; fft_size],
            freq_buf: vec![Complex::new(0.0, 0.0); freq_size],
            fft_scratch_fwd,
            fft_scratch_inv,
            overlap: vec![vec![0.0; fir_length.saturating_sub(1)]; channels],
            design_freqs: Vec::new(),
            design_magnitudes_db: Vec::new(),
            dry_buf: vec![0.0; max_buf],
            dry_delay: vec![vec![0.0; dry_delay_len]; channels],
            dry_delay_pos: 0,
            mix_smoother: Smoother::new(mix, 20.0, sample_rate),
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        // Build the initial FIR
        plugin.rebuild_fir();
        plugin.fir_dirty = false;
        plugin
    }

    pub(super) fn fir_length(&self) -> usize {
        fir_length_from_index(self.fir_length_index)
    }

    pub(super) fn band_contribution_db(bands: &[EqBand], freq: f64) -> f64 {
        let mut combined_db = 0.0;
        for band in bands {
            if !band.active {
                continue;
            }
            match band.filter_type {
                BiquadFilterType::Lowpass | BiquadFilterType::Highpass => {
                    combined_db += band.biquad.log_result(freq);
                }
                _ if band.gain_db.abs() > 1e-6 => {
                    combined_db += band.biquad.log_result(freq);
                }
                _ => {}
            }
        }
        combined_db
    }

    /// Rebuild FIR coefficients from current band settings.
    pub(super) fn rebuild_fir(&mut self) {
        let sr = self.sample_rate as f64;
        let fir_length = self.fir_length();
        let nyquist = sr / 2.0;

        // Scale sampling density with FIR length so narrow peaks are captured.
        // For an N-tap FIR we need at least 2*N frequency samples; round to a
        // power-of-two for consistency and clamp to a minimum of MAG_RESPONSE_POINTS.
        let num_points = MAG_RESPONSE_POINTS.max(fir_length * 2).next_power_of_two();
        self.design_freqs.clear();
        self.design_magnitudes_db.clear();
        self.design_freqs.reserve(num_points);
        self.design_magnitudes_db.reserve(num_points);

        // Include DC (1 Hz to avoid log-space interpolation issues with 0)
        // while still using the real combined response at the low end.
        self.design_freqs.push(1.0);
        let active_bands = &self.bands[..self.num_filters.min(self.bands.len())];
        self.design_magnitudes_db
            .push(Self::band_contribution_db(active_bands, 1.0));

        let log_min = 1.0_f64.ln();
        let log_max = nyquist.ln();

        for i in 1..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let freq = (log_min + t * (log_max - log_min)).exp();
            self.design_freqs.push(freq);

            self.design_magnitudes_db
                .push(Self::band_contribution_db(active_bands, freq));
        }

        let phase = match self.phase_mode_index {
            1 => FirPhase::Minimum,
            _ => FirPhase::Linear,
        };
        let config = FirDesignConfig {
            n_taps: fir_length,
            sample_rate: sr,
            phase,
            window: WindowType::Kaiser,
            ..Default::default()
        };

        let fir_f64 =
            generate_fir_from_response(&self.design_freqs, &self.design_magnitudes_db, &config);

        // Convert to f32 and store
        self.fir_coeffs.resize(fir_f64.len(), 0.0);
        for (dst, src) in self.fir_coeffs.iter_mut().zip(fir_f64.iter()) {
            *dst = *src as f32;
        }

        // Auto-gain normally restores unity gain at DC. DC-null filters (such
        // as a high-pass) use Nyquist when it is a meaningful passband
        // reference. If both endpoints are null, retain the designed scale
        // instead of amplifying numerical residue.
        if self.auto_gain {
            let dc = self.fir_coeffs.iter().sum::<f32>();
            let nyquist = self
                .fir_coeffs
                .iter()
                .enumerate()
                .map(|(index, &coefficient)| {
                    if index % 2 == 0 {
                        coefficient
                    } else {
                        -coefficient
                    }
                })
                .sum::<f32>();
            let reference = if dc.is_finite() && dc.abs() > 1e-4 {
                Some(dc)
            } else if nyquist.is_finite() && nyquist.abs() > 1e-4 {
                Some(nyquist)
            } else {
                None
            };
            if let Some(reference) = reference {
                let inv = 1.0 / reference;
                for c in &mut self.fir_coeffs {
                    *c *= inv;
                }
            }
        }

        // Pre-compute FFT of the FIR
        self.compute_fir_spectrum();
        self.convolvers = (0..self.channels)
            .map(|_| NupcEngine::new(&self.fir_coeffs, 32))
            .collect();
    }

    /// Compute the frequency-domain representation of the FIR.
    pub(super) fn compute_fir_spectrum(&mut self) {
        let fir_len = self.fir_coeffs.len();

        // Zero-pad FIR into input_buf
        self.input_buf[..fir_len].copy_from_slice(&self.fir_coeffs);
        self.input_buf[fir_len..self.fft_size].fill(0.0);

        // FFT the FIR
        self.fft_forward
            .process_with_scratch(
                &mut self.input_buf,
                &mut self.fir_spectrum,
                &mut self.fft_scratch_fwd,
            )
            .expect("FIR FFT failed");
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_int(
                "num_filters",
                "Num Filters",
                self.num_filters as i32,
                pk(LP_PARAMS, "num_filters").min_f64() as i32,
                pk(LP_PARAMS, "num_filters").max_f64() as i32,
            )
            .with_description("Number of EQ bands")
            .with_group("EQ"),
            Parameter::new_int(
                "fir_length",
                "FIR Length",
                self.fir_length_index as i32,
                0,
                (FIR_LENGTH_OPTIONS.len() - 1) as i32,
            )
            .with_description("FIR length in taps")
            .with_group("Quality"),
            Parameter::new_int(
                "phase_mode",
                "Phase Mode",
                self.phase_mode_index as i32,
                0,
                (PHASE_MODE_OPTIONS.len() - 1) as i32,
            )
            .with_description("FIR phase design mode")
            .with_group("Phase"),
            Parameter::new_bool("auto_gain", "Auto Gain", self.auto_gain)
                .with_description("Compensate output level")
                .with_group("Output"),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix_value,
                pk(LP_PARAMS, "mix").min_f64() as f32,
                pk(LP_PARAMS, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix")
            .with_group("Output"),
        ];

        // Per-band parameters
        for (i, band) in self.bands.iter().take(self.num_filters).enumerate() {
            let group = format!("Band {}", i + 1);
            params.push(
                Parameter::new_int(
                    &format!("band_{}_type", i),
                    "Type",
                    filter_type_to_index(band.filter_type) as i32,
                    0,
                    4,
                )
                .with_group(&group)
                .with_update_mode(UpdateMode::Structural),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_freq", i),
                    "Freq",
                    band.frequency as f32,
                    20.0,
                    20000.0,
                )
                .with_group(&group)
                .with_update_mode(UpdateMode::Structural),
            );
            params.push(
                Parameter::new_float(&format!("band_{}_q", i), "Q", band.q as f32, 0.1, 10.0)
                    .with_group(&group)
                    .with_update_mode(UpdateMode::Structural),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_gain", i),
                    "Gain",
                    band.gain_db as f32,
                    -24.0,
                    24.0,
                )
                .with_group(&group)
                .with_update_mode(UpdateMode::Structural),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_active", i), "Active", band.active)
                    .with_group(&group)
                    .with_update_mode(UpdateMode::Structural),
            );
        }

        apply_spec_update_modes(&mut params, LP_PARAMS);
        self.cached_parameters = params;
    }

    /// Resize FFT buffers when FIR length changes.
    #[allow(dead_code, reason = "retained for compatible prepared-state migration")]
    pub(super) fn resize_fft_buffers(&mut self) {
        let fir_length = self.fir_length();
        let fft_size = (fir_length * 2).next_power_of_two();
        let freq_size = fft_size / 2 + 1;

        if fft_size != self.fft_size {
            let mut planner = RealFftPlanner::<f32>::new();
            self.fft_forward = planner.plan_fft_forward(fft_size);
            self.fft_inverse = planner.plan_fft_inverse(fft_size);
            self.fft_size = fft_size;
            self.input_buf.resize(fft_size, 0.0);
            self.output_buf.resize(fft_size, 0.0);
            self.freq_buf.resize(freq_size, Complex::new(0.0, 0.0));
            self.fir_spectrum.resize(freq_size, Complex::new(0.0, 0.0));
            self.fft_scratch_fwd
                .resize(self.fft_forward.get_scratch_len(), Complex::new(0.0, 0.0));
            self.fft_scratch_inv
                .resize(self.fft_inverse.get_scratch_len(), Complex::new(0.0, 0.0));
        }

        let overlap_len = fir_length.saturating_sub(1);
        for ch_overlap in &mut self.overlap {
            ch_overlap.resize(overlap_len, 0.0);
            ch_overlap.fill(0.0);
        }
        // The dry ring has fixed capacity independent of FFT/FIR sizing.
    }
}

impl ParametricInPlacePlugin for LinearPhaseEqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("FIR EQ", env!("CARGO_PKG_VERSION"), "SOTF")
            .with_description("Parametric EQ with linear or minimum-phase FIR convolution")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Convolution
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::linear_transform(
            PluginCostClass::Convolution,
            None,
            self.latency_samples(),
            false,
            true,
            false,
        )
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(
            ParameterId::from("num_filters"),
            ParameterValue::Int(self.num_filters as i32),
        );
        values.insert(
            ParameterId::from("fir_length"),
            ParameterValue::Int(self.fir_length_index as i32),
        );
        values.insert(
            ParameterId::from("phase_mode"),
            ParameterValue::Int(self.phase_mode_index as i32),
        );
        values.insert(
            ParameterId::from("auto_gain"),
            ParameterValue::Bool(self.auto_gain),
        );
        values.insert(
            ParameterId::from("mix"),
            ParameterValue::Float(self.mix_value),
        );
        for (i, band) in self.bands.iter().take(self.num_filters).enumerate() {
            values.insert(
                ParameterId::from(format!("band_{}_type", i).as_str()),
                ParameterValue::Int(filter_type_to_index(band.filter_type) as i32),
            );
            values.insert(
                ParameterId::from(format!("band_{}_freq", i).as_str()),
                ParameterValue::Float(band.frequency as f32),
            );
            values.insert(
                ParameterId::from(format!("band_{}_q", i).as_str()),
                ParameterValue::Float(band.q as f32),
            );
            values.insert(
                ParameterId::from(format!("band_{}_gain", i).as_str()),
                ParameterValue::Float(band.gain_db as f32),
            );
            values.insert(
                ParameterId::from(format!("band_{}_active", i).as_str()),
                ParameterValue::Bool(band.active),
            );
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            let id_str = id.as_str();

            match id_str {
                "num_filters" | "fir_length" | "phase_mode" | "auto_gain" => {
                    return Err(format!(
                        "{id_str} is structural; rebuild the plugin to change it"
                    ));
                }
                "mix" => {
                    if let ParameterValue::Float(v) = value {
                        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                            return Err(format!("mix must be finite and within [0, 1], got {v}"));
                        }
                        self.mix_value = v;
                        self.mix_smoother.set_target(v);
                        self.rebuild_cached_parameters();
                    }
                }
                _ if id_str.starts_with("band_") => {
                    return Err(format!(
                        "{id_str} is structural; rebuild the plugin to change it"
                    ));
                }
                _ => return Err(format!("Unknown parameter: {id}")),
            }
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }
        if sample_rate != self.sample_rate {
            self.sample_rate = sample_rate;
            self.mix_smoother = Smoother::new(self.mix_value, 20.0, sample_rate);
            // Rebuild all biquads at new sample rate
            let sr = sample_rate as f64;
            for band in &mut self.bands {
                band.biquad =
                    Biquad::new(band.filter_type, band.frequency, sr, band.q, band.gain_db);
            }
            self.rebuild_fir();
            self.fir_dirty = false;
        }
        Ok(())
    }

    fn reset(&mut self) {
        // Clear overlap buffers
        for ch_overlap in &mut self.overlap {
            ch_overlap.fill(0.0);
        }
        for convolver in &mut self.convolvers {
            convolver.reset();
        }
        for delay in &mut self.dry_delay {
            delay.fill(0.0);
        }
        self.dry_delay_pos = 0;
        self.mix_smoother = Smoother::new(self.mix_value, 20.0, self.sample_rate);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let nc = self.channels;

        if nf == 0 || nc == 0 {
            return Ok(nf);
        }

        let total_samples = nf
            .checked_mul(nc)
            .ok_or_else(|| "audio buffer size overflow".to_string())?;
        if buffer.len() < total_samples {
            return Err(format!(
                "audio buffer too short: need {total_samples}, got {}",
                buffer.len()
            ));
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "sample-rate mismatch: initialized for {}, got {}",
                self.sample_rate, context.sample_rate
            ));
        }

        if self.fir_dirty {
            return Err(
                "FIR state is dirty; rebuild or initialize the plugin off the audio thread".into(),
            );
        }

        // Save a latency-aligned dry signal for mix. Linear-phase FIR output has
        // group delay; delaying the dry branch avoids comb filtering at partial mix.
        let dry_delay = self.latency_samples();
        let ring_len = self.dry_delay.first().map_or(1, Vec::len);
        for frame in 0..nf {
            let ring_pos = self.dry_delay_pos % ring_len;
            let mix = self.mix_smoother.next_n(1);
            let read_pos = if dry_delay == 0 {
                ring_pos
            } else {
                (ring_pos + ring_len - dry_delay % ring_len) % ring_len
            };
            for ch in 0..nc {
                let index = frame * nc + ch;
                let sample = buffer[index];
                let dry = if dry_delay == 0 {
                    sample
                } else {
                    self.dry_delay[ch][read_pos]
                };
                // Keep history in every phase mode so a later switch to
                // linear phase has a valid dry signal without allocation.
                self.dry_delay[ch][ring_pos] = sample;
                let wet = self.convolvers[ch].process_sample(sample);
                buffer[index] = dry * (1.0 - mix) + wet * mix;
            }
            self.dry_delay_pos = (self.dry_delay_pos + 1) % ring_len;
        }

        flush_denormals_inplace(&mut buffer[..total_samples]);
        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        if self.phase_mode_index == 0 {
            // The even-tap designer centers its impulse at N/2. NUPC adds one
            // 32-sample head partition of streaming latency.
            self.fir_length() / 2 + 32
        } else {
            32
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}
