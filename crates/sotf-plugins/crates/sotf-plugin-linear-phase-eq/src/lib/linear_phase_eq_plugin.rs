use super::default::default_fir_length_index;
use super::default::default_num_filters;
use super::eq_band::EqBand;
use super::misc::MAG_RESPONSE_POINTS;
use super::misc::filter_type_to_index;
use super::misc::fir_length_from_index;
use super::misc::index_to_filter_type;
use super::misc::parse_filter_type;
use super::types::LinearPhaseEqPluginParams;
use crate::params::{FIR_LENGTH_OPTIONS, PARAMS as LP_PARAMS};
use math_audio_iir_fir::{
    Biquad, BiquadFilterType, FirDesignConfig, FirPhase, WindowType, generate_fir_from_response,
};
use num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
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

pub struct LinearPhaseEqPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) num_filters: usize,
    pub(super) fir_length_index: usize,
    pub(super) auto_gain: bool,
    pub(super) mix_value: f32,

    // EQ band definitions (for magnitude computation only)
    pub(super) bands: Vec<EqBand>,

    // FIR state
    pub(super) fir_coeffs: Vec<f32>,
    pub(super) fir_spectrum: Vec<Complex<f32>>,
    pub(super) fir_dirty: bool,

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
        let fir_length_index = params.fir_length_index.min(FIR_LENGTH_OPTIONS.len() - 1);
        let fir_length = fir_length_from_index(fir_length_index);
        let num_filters = params.num_filters.clamp(1, 10);
        let sr = sample_rate as f64;

        let mut bands = Vec::with_capacity(num_filters);
        for (i, fc) in params.filters.iter().enumerate() {
            if i >= num_filters {
                break;
            }
            let ft = parse_filter_type(&fc.filter_type)?;
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
            params.auto_gain,
            params.mix,
            bands,
        ))
    }

    pub(super) fn build(
        channels: usize,
        sample_rate: u32,
        num_filters: usize,
        fir_length_index: usize,
        fir_length: usize,
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

        let mut plugin = Self {
            channels,
            sample_rate,
            num_filters,
            fir_length_index,
            auto_gain,
            mix_value: mix,
            bands,
            fir_coeffs: vec![0.0; fir_length],
            fir_spectrum: vec![Complex::new(0.0, 0.0); freq_size],
            fir_dirty: true,
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
        self.design_magnitudes_db
            .push(Self::band_contribution_db(&self.bands, 1.0));

        let log_min = 1.0_f64.ln();
        let log_max = nyquist.ln();

        for i in 1..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let freq = (log_min + t * (log_max - log_min)).exp();
            self.design_freqs.push(freq);

            self.design_magnitudes_db
                .push(Self::band_contribution_db(&self.bands, freq));
        }

        // Generate the linear-phase FIR
        let config = FirDesignConfig {
            n_taps: fir_length,
            sample_rate: sr,
            phase: FirPhase::Linear,
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

        // Auto-gain: normalize FIR so that passband gain is unity
        if self.auto_gain {
            let sum: f32 = self.fir_coeffs.iter().sum();
            if sum.abs() > 1e-10 {
                let inv = 1.0 / sum;
                // Apply correction relative to unity
                // The FIR's DC gain is its sum. We want to keep the EQ shape
                // but remove any overall level shift.
                // Actually for auto-gain: we just want to undo the DC offset
                // Compute current DC gain and apply inverse
                for c in &mut self.fir_coeffs {
                    *c *= inv;
                }
            }
        }

        // Pre-compute FFT of the FIR
        self.compute_fir_spectrum();
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
        for (i, band) in self.bands.iter().enumerate() {
            let group = format!("Band {}", i + 1);
            params.push(
                Parameter::new_int(
                    &format!("band_{}_type", i),
                    "Type",
                    filter_type_to_index(band.filter_type) as i32,
                    0,
                    4,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_freq", i),
                    "Freq",
                    band.frequency as f32,
                    20.0,
                    20000.0,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(&format!("band_{}_q", i), "Q", band.q as f32, 0.1, 10.0)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_gain", i),
                    "Gain",
                    band.gain_db as f32,
                    -24.0,
                    24.0,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_active", i), "Active", band.active)
                    .with_group(&group),
            );
        }

        self.cached_parameters = params;
    }

    /// Resize FFT buffers when FIR length changes.
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
    }
}

impl ParametricInPlacePlugin for LinearPhaseEqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Linear-Phase EQ", env!("CARGO_PKG_VERSION"), "SOTF").with_description(
            "Parametric EQ with linear-phase FIR convolution for zero phase distortion",
        )
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Convolution
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        if self.auto_gain {
            return PluginCompileMetadata::boundary(
                PluginCostClass::Convolution,
                self.latency_samples(),
            );
        }
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
            ParameterId::from("auto_gain"),
            ParameterValue::Bool(self.auto_gain),
        );
        values.insert(
            ParameterId::from("mix"),
            ParameterValue::Float(self.mix_value),
        );
        for (i, band) in self.bands.iter().enumerate() {
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
                "num_filters" => {
                    if let ParameterValue::Int(v) = value {
                        let new_count = (v as usize).clamp(1, 10);
                        if new_count != self.num_filters {
                            let sr = self.sample_rate as f64;
                            self.num_filters = new_count;
                            // Grow bands if needed
                            while self.bands.len() < new_count {
                                self.bands.push(EqBand::new(
                                    BiquadFilterType::Peak,
                                    1000.0,
                                    1.0,
                                    0.0,
                                    true,
                                    sr,
                                ));
                            }
                            // Shrink if needed (just adjust num_filters, keep bands allocated)
                            self.fir_dirty = true;
                            self.rebuild_cached_parameters();
                        }
                    }
                }
                "fir_length" => {
                    if let ParameterValue::Int(v) = value {
                        let idx = (v as usize).min(FIR_LENGTH_OPTIONS.len() - 1);
                        if idx != self.fir_length_index {
                            self.fir_length_index = idx;
                            self.fir_coeffs.resize(self.fir_length(), 0.0);
                            self.resize_fft_buffers();
                            self.fir_dirty = true;
                            self.rebuild_cached_parameters();
                        }
                    }
                }
                "auto_gain" => {
                    if let ParameterValue::Bool(v) = value {
                        self.auto_gain = v;
                        self.fir_dirty = true;
                        self.rebuild_cached_parameters();
                    }
                }
                "mix" => {
                    if let ParameterValue::Float(v) = value {
                        self.mix_value = v;
                        self.mix_smoother.set_target(v);
                        self.rebuild_cached_parameters();
                    }
                }
                _ => {
                    // Try per-band parameters: band_{i}_{param}
                    if let Some(rest) = id_str.strip_prefix("band_")
                        && let Some((idx_str, param)) = rest.split_once('_')
                        && let Ok(idx) = idx_str.parse::<usize>()
                        && idx < self.bands.len()
                    {
                        let sr = self.sample_rate as f64;
                        let band = &self.bands[idx];
                        let mut ft = band.filter_type;
                        let mut freq = band.frequency;
                        let mut q = band.q;
                        let mut gain = band.gain_db;
                        let mut active = band.active;

                        match param {
                            "type" => {
                                if let ParameterValue::Int(v) = value {
                                    ft = index_to_filter_type(v as usize);
                                }
                            }
                            "freq" => {
                                if let ParameterValue::Float(v) = value {
                                    freq = v as f64;
                                }
                            }
                            "q" => {
                                if let ParameterValue::Float(v) = value {
                                    q = v as f64;
                                }
                            }
                            "gain" => {
                                if let ParameterValue::Float(v) = value {
                                    gain = v as f64;
                                }
                            }
                            "active" => {
                                if let ParameterValue::Bool(v) = value {
                                    active = v;
                                }
                            }
                            _ => return Err(format!("Unknown band parameter: {param}")),
                        }

                        self.bands[idx].update(ft, freq, q, gain, active, sr);
                        self.fir_dirty = true;
                        self.rebuild_cached_parameters();
                    } else {
                        return Err(format!("Unknown parameter: {}", id));
                    }
                }
            }
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate != self.sample_rate {
            self.sample_rate = sample_rate;
            self.mix_smoother = Smoother::new(self.mix_value, 20.0, sample_rate);
            // Rebuild all biquads at new sample rate
            let sr = sample_rate as f64;
            for band in &mut self.bands {
                band.biquad =
                    Biquad::new(band.filter_type, band.frequency, sr, band.q, band.gain_db);
            }
            self.fir_dirty = true;
        }
        Ok(())
    }

    fn reset(&mut self) {
        // Clear overlap buffers
        for ch_overlap in &mut self.overlap {
            ch_overlap.fill(0.0);
        }
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

        // Guard: FFT convolution is only valid while `nf + fir_len - 1 <= fft_size`.
        // Larger blocks must be chunked before the transform or the FIR tail wraps
        // circularly into the current block.
        let max_chunk_frames = self.fft_size.saturating_sub(self.fir_length() - 1);
        if nf > max_chunk_frames {
            let mut frame = 0;
            while frame < nf {
                let chunk_frames = (nf - frame).min(max_chunk_frames);
                let start = frame * nc;
                let end = start + chunk_frames * nc;
                let chunk_context = ProcessContext::new(context.sample_rate, chunk_frames)
                    .with_sample_position(context.transport.sample_position + frame as u64)
                    .with_transport(context.transport)
                    .with_midi_events(context.midi_events);
                self.process_in_place(&mut buffer[start..end], &chunk_context)?;
                frame += chunk_frames;
            }
            return Ok(nf);
        }

        // Rebuild FIR if parameters changed (outside per-sample loop)
        if self.fir_dirty {
            self.rebuild_fir();
            self.fir_dirty = false;
        }

        let total_samples = nf * nc;

        // Save dry for mix (dry_buf is pre-allocated to fft_size * channels)
        self.dry_buf[..total_samples].copy_from_slice(&buffer[..total_samples]);

        let mix = self.mix_smoother.next_n(nf);

        // Process each channel independently
        for ch in 0..nc {
            // De-interleave: extract this channel from interleaved buffer
            for frame in 0..nf {
                self.input_buf[frame] = buffer[frame * nc + ch];
            }
            // Zero-pad rest of FFT buffer
            self.input_buf[nf..self.fft_size].fill(0.0);

            // FFT the input
            if let Err(e) = self.fft_forward.process_with_scratch(
                &mut self.input_buf,
                &mut self.freq_buf,
                &mut self.fft_scratch_fwd,
            ) {
                return Err(format!("Forward FFT failed: {:?}", e));
            }

            // Multiply with pre-computed FIR spectrum (frequency-domain convolution)
            for (f, h) in self.freq_buf.iter_mut().zip(self.fir_spectrum.iter()) {
                *f *= *h;
            }

            // IFFT
            if let Err(e) = self.fft_inverse.process_with_scratch(
                &mut self.freq_buf,
                &mut self.output_buf,
                &mut self.fft_scratch_inv,
            ) {
                return Err(format!("Inverse FFT failed: {:?}", e));
            }

            // Normalize IFFT output
            let norm = 1.0 / self.fft_size as f32;
            for s in &mut self.output_buf[..self.fft_size] {
                *s *= norm;
            }

            // Overlap-add. The overlap buffer stores exactly `fir_len - 1`
            // pending tail samples, shifted forward by each processed block.
            let fir_len = self.fir_length();
            let tail_len = fir_len.saturating_sub(1);
            let overlap_buf = &mut self.overlap[ch];
            debug_assert_eq!(overlap_buf.len(), tail_len);

            // Add old overlap to output (first nf samples).
            let add_len = nf.min(tail_len);
            for (i, &ov) in overlap_buf.iter().enumerate().take(add_len) {
                self.output_buf[i] += ov;
            }

            // Store the new convolution tail plus any unconsumed old tail.
            for i in 0..tail_len {
                let new_val = self.output_buf[nf + i];
                let old_val = if nf + i < tail_len {
                    overlap_buf[nf + i]
                } else {
                    0.0
                };
                overlap_buf[i] = new_val + old_val;
            }

            // Write back to interleaved buffer with dry/wet mix
            for frame in 0..nf {
                let wet = self.output_buf[frame];
                let dry = self.dry_buf[frame * nc + ch];
                buffer[frame * nc + ch] = dry * (1.0 - mix) + wet * mix;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        // Linear-phase FIR has symmetrical impulse response.
        // Delay = (fir_length - 1) / 2
        (self.fir_length() - 1) / 2
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}
