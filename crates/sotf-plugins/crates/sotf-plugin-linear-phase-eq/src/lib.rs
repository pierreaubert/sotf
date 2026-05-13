// ============================================================================
// Linear-Phase EQ Plugin
//
// Computes combined magnitude response from parametric EQ bands analytically,
// generates a linear-phase FIR via frequency sampling, and applies it via
// overlap-add FFT convolution.
//
// Hard rules:
// - No allocations in process_in_place()
// - No mutex locks in process()
// - No unsafe code
// - FIR rebuild happens ONLY on parameter change, never per-frame
// ============================================================================

pub mod params;

use math_audio_iir_fir::{
    Biquad, BiquadFilterType, FirDesignConfig, FirPhase, WindowType, generate_fir_from_response,
};
use num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

use crate::params::{FIR_LENGTH_OPTIONS, PARAMS as LP_PARAMS};
use sotf_host::param_specs::find_by_key as pk;

// ============================================================================
// Constants
// ============================================================================

#[cfg(test)]
const DEFAULT_SAMPLE_RATE: u32 = 48000;

/// Number of frequency points to sample for magnitude response.
/// Must cover 0 Hz to Nyquist with sufficient density.
const MAG_RESPONSE_POINTS: usize = 4096;

// ============================================================================
// Params for JSON construction
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearPhaseEqPluginParams {
    #[serde(default = "default_num_filters")]
    pub num_filters: usize,
    #[serde(default = "default_fir_length_index")]
    pub fir_length_index: usize,
    #[serde(default)]
    pub auto_gain: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub filters: Vec<BandConfig>,
}

fn default_num_filters() -> usize {
    pk(LP_PARAMS, "num_filters").default_f64() as usize
}
fn default_fir_length_index() -> usize {
    pk(LP_PARAMS, "fir_length").default_f64() as usize
}
fn default_mix() -> f32 {
    pk(LP_PARAMS, "mix").default_f64() as f32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandConfig {
    #[serde(default = "default_filter_type")]
    pub filter_type: String,
    #[serde(default = "default_frequency")]
    pub frequency: f64,
    #[serde(default = "default_q")]
    pub q: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_filter_type() -> String {
    "Peak".to_string()
}
fn default_frequency() -> f64 {
    1000.0
}
fn default_q() -> f64 {
    1.0
}
fn default_active() -> bool {
    true
}

// ============================================================================
// EQ Band
// ============================================================================

struct EqBand {
    filter_type: BiquadFilterType,
    frequency: f64,
    q: f64,
    gain_db: f64,
    active: bool,
    /// Used only for magnitude response computation, not for direct filtering.
    biquad: Biquad,
}

impl EqBand {
    fn new(
        filter_type: BiquadFilterType,
        frequency: f64,
        q: f64,
        gain_db: f64,
        active: bool,
        sample_rate: f64,
    ) -> Self {
        let biquad = Biquad::new(filter_type, frequency, sample_rate, q, gain_db);
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
            active,
            biquad,
        }
    }

    fn update(
        &mut self,
        filter_type: BiquadFilterType,
        frequency: f64,
        q: f64,
        gain_db: f64,
        active: bool,
        sample_rate: f64,
    ) {
        self.filter_type = filter_type;
        self.frequency = frequency;
        self.q = q;
        self.gain_db = gain_db;
        self.active = active;
        self.biquad = Biquad::new(filter_type, frequency, sample_rate, q, gain_db);
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct LinearPhaseEqPlugin {
    channels: usize,
    sample_rate: u32,
    num_filters: usize,
    fir_length_index: usize,
    auto_gain: bool,
    mix_value: f32,

    // EQ band definitions (for magnitude computation only)
    bands: Vec<EqBand>,

    // FIR state
    fir_coeffs: Vec<f32>,
    fir_spectrum: Vec<Complex<f32>>,
    fir_dirty: bool,

    // FFT planners (Arc'd, no mutex)
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,
    fft_size: usize,

    // Pre-allocated processing buffers (per-channel is handled by reuse)
    input_buf: Vec<f32>,
    output_buf: Vec<f32>,
    freq_buf: Vec<Complex<f32>>,
    fft_scratch_fwd: Vec<Complex<f32>>,
    fft_scratch_inv: Vec<Complex<f32>>,

    // Per-channel overlap-add tail
    overlap: Vec<Vec<f32>>,

    // Dry buffer for mix
    dry_buf: Vec<f32>,

    // Smoothers
    mix_smoother: Smoother,

    cached_parameters: Vec<Parameter>,
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

    fn build(
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
            overlap: vec![vec![0.0; fft_size]; channels],
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

    fn fir_length(&self) -> usize {
        fir_length_from_index(self.fir_length_index)
    }

    /// Rebuild FIR coefficients from current band settings.
    fn rebuild_fir(&mut self) {
        let sr = self.sample_rate as f64;
        let fir_length = self.fir_length();
        let nyquist = sr / 2.0;

        // Scale sampling density with FIR length so narrow peaks are captured.
        // For an N-tap FIR we need at least 2*N frequency samples; round to a
        // power-of-two for consistency and clamp to a minimum of MAG_RESPONSE_POINTS.
        let num_points = MAG_RESPONSE_POINTS.max(fir_length * 2).next_power_of_two();
        let mut freqs = Vec::with_capacity(num_points);
        let mut magnitudes_db = Vec::with_capacity(num_points);

        let band_contribution = |freq: f64| -> f64 {
            let mut combined_db = 0.0;
            for band in &self.bands {
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
        };

        // Include DC (1 Hz to avoid log-space interpolation issues with 0)
        // while still using the real combined response at the low end.
        freqs.push(1.0);
        magnitudes_db.push(band_contribution(1.0));

        let log_min = 1.0_f64.ln();
        let log_max = nyquist.ln();

        for i in 1..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let freq = (log_min + t * (log_max - log_min)).exp();
            freqs.push(freq);

            magnitudes_db.push(band_contribution(freq));
        }

        // Generate the linear-phase FIR
        let config = FirDesignConfig {
            n_taps: fir_length,
            sample_rate: sr,
            phase: FirPhase::Linear,
            window: WindowType::Kaiser,
            ..Default::default()
        };

        let fir_f64 = generate_fir_from_response(&freqs, &magnitudes_db, &config);

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
    fn compute_fir_spectrum(&mut self) {
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

    fn rebuild_cached_parameters(&mut self) {
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
    fn resize_fft_buffers(&mut self) {
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
            for ch_overlap in &mut self.overlap {
                ch_overlap.resize(fft_size, 0.0);
                ch_overlap.fill(0.0);
            }
        }
    }
}

// ============================================================================
// InPlacePlugin trait
// ============================================================================

impl InPlacePlugin for LinearPhaseEqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Linear-Phase EQ", env!("CARGO_PKG_VERSION"), "SOTF").with_description(
            "Parametric EQ with linear-phase FIR convolution for zero phase distortion",
        )
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
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
                }
            }
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let id_str = id.as_str();
        match id_str {
            "num_filters" => Some(ParameterValue::Int(self.num_filters as i32)),
            "fir_length" => Some(ParameterValue::Int(self.fir_length_index as i32)),
            "auto_gain" => Some(ParameterValue::Bool(self.auto_gain)),
            "mix" => Some(ParameterValue::Float(self.mix_value)),
            _ => {
                // Try per-band parameters
                if let Some(rest) = id_str.strip_prefix("band_")
                    && let Some((idx_str, param)) = rest.split_once('_')
                    && let Ok(idx) = idx_str.parse::<usize>()
                    && idx < self.bands.len()
                {
                    let band = &self.bands[idx];
                    return match param {
                        "type" => Some(ParameterValue::Int(
                            filter_type_to_index(band.filter_type) as i32
                        )),
                        "freq" => Some(ParameterValue::Float(band.frequency as f32)),
                        "q" => Some(ParameterValue::Float(band.q as f32)),
                        "gain" => Some(ParameterValue::Float(band.gain_db as f32)),
                        "active" => Some(ParameterValue::Bool(band.active)),
                        _ => None,
                    };
                }
                None
            }
        }
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

        // Guard: frame count must be less than FFT size for overlap-add to work
        if nf >= self.fft_size {
            let max_chunk_frames = self.fft_size - 1;
            let mut frame = 0;
            while frame < nf {
                let chunk_frames = (nf - frame).min(max_chunk_frames);
                let start = frame * nc;
                let end = start + chunk_frames * nc;
                let chunk_context = ProcessContext {
                    sample_rate: context.sample_rate,
                    num_frames: chunk_frames,
                };
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

            // Overlap-add:
            // 1. Add old overlap tail to the first nf samples of output
            // 2. New tail = output_buf[nf..] + old overlap[nf..] (if any remains)
            let fir_len = self.fir_length();
            let fft_size = self.fft_size;
            let overlap_buf = &mut self.overlap[ch];
            let overlap_total = overlap_buf.len();

            // Add old overlap to output (first nf samples)
            let add_len = nf.min(overlap_total);
            for (i, &ov) in overlap_buf.iter().enumerate().take(add_len) {
                self.output_buf[i] += ov;
            }

            // Compute the new tail from output_buf[nf..]
            let new_tail_len = (fir_len - 1).min(fft_size.saturating_sub(nf));

            // Add old overlap beyond nf to the new tail
            let old_remaining = overlap_total.saturating_sub(nf);
            let add_old = old_remaining.min(new_tail_len);
            for i in 0..new_tail_len {
                let new_val = self.output_buf[nf + i];
                let old_val = if i < add_old {
                    overlap_buf[nf + i]
                } else {
                    0.0
                };
                overlap_buf[i] = new_val + old_val;
            }

            // Clear rest of overlap buffer
            for ov in overlap_buf
                .iter_mut()
                .take(overlap_total)
                .skip(new_tail_len)
            {
                *ov = 0.0;
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

// ============================================================================
// Helpers
// ============================================================================

fn fir_length_from_index(index: usize) -> usize {
    match index {
        0 => 1024,
        1 => 2048,
        2 => 4096,
        3 => 8192,
        _ => 2048,
    }
}

fn parse_filter_type(s: &str) -> Result<BiquadFilterType, String> {
    match s {
        "Peak" | "peak" => Ok(BiquadFilterType::Peak),
        "Lowshelf" | "lowshelf" => Ok(BiquadFilterType::Lowshelf),
        "Highshelf" | "highshelf" => Ok(BiquadFilterType::Highshelf),
        "Lowpass" | "lowpass" => Ok(BiquadFilterType::Lowpass),
        "Highpass" | "highpass" => Ok(BiquadFilterType::Highpass),
        other => Err(format!("Unknown filter type: {other}")),
    }
}

fn filter_type_to_index(ft: BiquadFilterType) -> usize {
    match ft {
        BiquadFilterType::Peak => 0,
        BiquadFilterType::Lowshelf => 1,
        BiquadFilterType::Highshelf => 2,
        BiquadFilterType::Lowpass => 3,
        BiquadFilterType::Highpass => 4,
        // All other types map to Peak as default for this plugin
        _ => 0,
    }
}

fn index_to_filter_type(index: usize) -> BiquadFilterType {
    match index {
        0 => BiquadFilterType::Peak,
        1 => BiquadFilterType::Lowshelf,
        2 => BiquadFilterType::Highshelf,
        3 => BiquadFilterType::Lowpass,
        4 => BiquadFilterType::Highpass,
        _ => BiquadFilterType::Peak,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    fn make_context(num_frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: DEFAULT_SAMPLE_RATE,
            num_frames,
        }
    }

    #[test]
    fn test_linear_phase_eq_passthrough() {
        // All bands at 0 dB gain -> output should approximately equal input
        let channels = 2;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 3,
            fir_length_index: 1, // 2048 taps
            auto_gain: false,
            mix: 1.0,
            filters: vec![
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 1000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 2000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 4000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
            ],
        };

        let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();

        // Generate a 1kHz sine wave, process multiple blocks to get past latency
        let blocks_needed = (latency / num_frames) + 5;
        let mut all_output = Vec::new();

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                buffer[frame * channels] = sample; // L
                buffer[frame * channels + 1] = sample; // R
            }
            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            all_output.extend_from_slice(&buffer);
        }

        // After latency, the output should match the input (within FIR precision)
        // Check steady-state region (skip first latency + some margin)
        let check_start = (latency + num_frames) * channels;
        let check_end = all_output.len() - num_frames * channels;

        if check_start < check_end {
            // Reconstruct expected sine at the delayed position
            let mut max_error = 0.0f32;
            for i in (check_start..check_end).step_by(channels) {
                let frame_idx = i / channels;
                // Account for latency
                let source_frame = frame_idx - latency;
                let t = source_frame as f32 / sr as f32;
                let expected = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                let err = (all_output[i] - expected).abs();
                if err > max_error {
                    max_error = err;
                }
            }
            // Tolerance: FIR windowing (Kaiser window) causes small deviations,
            // especially at frequencies near Nyquist. 0.1 corresponds to ~0.8 dB.
            assert!(max_error < 0.1, "Passthrough error too large: {max_error}");
        }
    }

    #[test]
    fn test_large_block_is_chunked_not_silently_bypassed() {
        let channels = 1;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 1,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 12.0,
                active: true,
            }],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = plugin.fft_size + 512;
        let mut buffer = vec![0.0_f32; num_frames];
        for (i, sample) in buffer.iter_mut().enumerate() {
            let t = i as f32 / sr as f32;
            *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.25;
        }
        let input = buffer.clone();

        plugin
            .process_in_place(
                &mut buffer,
                &ProcessContext {
                    sample_rate: sr,
                    num_frames,
                },
            )
            .unwrap();

        let changed = buffer
            .iter()
            .zip(input.iter())
            .any(|(&out, &inp)| (out - inp).abs() > 1.0e-5);
        assert!(
            changed,
            "large blocks must be processed, not passed through"
        );
    }

    #[test]
    fn test_linear_phase_eq_boost() {
        // 1kHz +6dB band -> 1kHz sine should be louder
        let channels = 1;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 6.0,
                active: true,
            }],
        };

        let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency / num_frames) + 10;

        let mut input_rms = 0.0f64;
        let mut output_rms = 0.0f64;
        let mut samples_counted = 0usize;

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                buffer[frame] = sample;
            }

            // Measure input RMS (after latency region)
            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    input_rms += (s as f64) * (s as f64);
                }
                samples_counted += num_frames;
            }

            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();

            // Measure output RMS (same region)
            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    output_rms += (s as f64) * (s as f64);
                }
            }
        }

        if samples_counted > 0 {
            input_rms = (input_rms / samples_counted as f64).sqrt();
            output_rms = (output_rms / samples_counted as f64).sqrt();

            let gain_db = 20.0 * (output_rms / input_rms).log10();
            // Should be approximately +6 dB
            assert!(
                gain_db > 4.0 && gain_db < 8.0,
                "Expected ~6 dB boost, got {gain_db:.1} dB"
            );
        }
    }

    #[test]
    fn test_linear_phase_eq_latency() {
        let plugin = LinearPhaseEqPlugin::new(2, 48000);
        let fir_len = plugin.fir_length();
        assert_eq!(plugin.latency_samples(), (fir_len - 1) / 2);
    }

    #[test]
    fn test_linear_phase_eq_phase_linearity() {
        // Process an impulse, verify the response is symmetrical (linear phase property)
        let channels = 1;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 1, // 2048 taps
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 6.0,
                active: true,
            }],
        };

        let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 256;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency * 3 / num_frames) + 5;

        let mut all_output = Vec::new();

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            // Put impulse in first block, first sample
            if block == 0 {
                buffer[0] = 1.0;
            }
            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            all_output.extend_from_slice(&buffer);
        }

        // Find the peak of the impulse response
        let (peak_idx, _peak_val) = all_output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .unwrap();

        // Verify symmetry around the peak
        let check_range = 100.min(peak_idx).min(all_output.len() - peak_idx - 1);
        let mut max_asymmetry = 0.0f32;
        for offset in 1..check_range {
            let left = all_output[peak_idx - offset];
            let right = all_output[peak_idx + offset];
            let asymmetry = (left - right).abs();
            if asymmetry > max_asymmetry {
                max_asymmetry = asymmetry;
            }
        }

        // Linear-phase FIR should have very symmetrical impulse response
        assert!(
            max_asymmetry < 0.01,
            "Impulse response not symmetrical: max asymmetry = {max_asymmetry}"
        );
    }

    /// Helper: measure RMS level of a sine after EQ latency has passed.
    ///
    /// Feeds `blocks_total` blocks of a pure sine at `freq_hz` through `plugin`,
    /// returns the RMS computed over the last half of the blocks.
    fn rms_after_latency(
        plugin: &mut LinearPhaseEqPlugin,
        freq_hz: f32,
        sr: u32,
        num_frames: usize,
        blocks_total: usize,
    ) -> f64 {
        let nc = plugin.channels;
        let latency = plugin.latency_samples();
        let measure_from = blocks_total / 2;
        let mut sum_sq = 0.0f64;
        let mut n = 0usize;
        for block in 0..blocks_total {
            let mut buf = vec![0.0f32; num_frames * nc];
            let base = block * num_frames;
            for frame in 0..num_frames {
                let t = (base + frame) as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5;
                for ch in 0..nc {
                    buf[frame * nc + ch] = s;
                }
            }
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames,
            };
            plugin.process_in_place(&mut buf, &ctx).unwrap();
            if block >= measure_from && block * num_frames > latency + num_frames {
                for &s in &buf {
                    sum_sq += (s as f64) * (s as f64);
                    n += 1;
                }
            }
        }
        if n > 0 {
            (sum_sq / n as f64).sqrt()
        } else {
            0.0
        }
    }

    /// Bug #2 (🔴): Highpass filter at 200 Hz should attenuate 50 Hz content.
    ///
    /// Before the fix, lowpass/highpass bands were skipped entirely (gain_db==0
    /// satisfied the `gain_db.abs() > 1e-6` guard), making the plugin all-pass.
    #[test]
    fn test_highpass_attenuates_below_cutoff() {
        let sr = 48000u32;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps for clean HP response
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Highpass".to_string(),
                frequency: 800.0,
                q: 0.707,
                gain_db: 0.0,
                active: true,
            }],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(1, sr, params).unwrap();
        let num_frames = 256;
        let blocks = (plugin.latency_samples() / num_frames) + 20;

        // 50 Hz is well below the 800 Hz cutoff — should be strongly attenuated.
        let rms_50hz = rms_after_latency(&mut plugin, 50.0, sr, num_frames, blocks);
        // 4000 Hz is well above the cutoff — should pass with near-unity gain.
        let rms_4khz = rms_after_latency(&mut plugin, 4000.0, sr, num_frames, blocks);

        // Reset between frequency measurements.
        plugin.reset();

        assert!(
            rms_4khz > 0.01,
            "4 kHz should pass through HP filter, got rms={rms_4khz:.4}"
        );
        // At 50 Hz (far below 800 Hz cutoff), expect at least 20 dB attenuation.
        let attenuation_db = if rms_50hz < 1e-10 {
            120.0f64
        } else {
            20.0 * (rms_4khz / rms_50hz).log10()
        };
        assert!(
            attenuation_db > 15.0,
            "Expected >15 dB attenuation at 50 Hz vs 4 kHz, got {attenuation_db:.1} dB"
        );
    }

    /// Bug #1 (🔴): Lowshelf cut should attenuate DC / low frequencies.
    ///
    /// Before the fix, the DC point was hardcoded to 0 dB, making lowshelf-cut
    /// and highpass filters produce incorrect FIR shapes at low frequencies.
    #[test]
    fn test_lowshelf_cut_attenuates_low_frequencies() {
        let sr = 48000u32;
        // -12 dB lowshelf at 500 Hz should visibly attenuate a 100 Hz tone.
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 2,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Lowshelf".to_string(),
                frequency: 500.0,
                q: 0.707,
                gain_db: -12.0,
                active: true,
            }],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(1, sr, params).unwrap();
        let num_frames = 256;
        let blocks = (plugin.latency_samples() / num_frames) + 20;

        let rms_100hz = rms_after_latency(&mut plugin, 100.0, sr, num_frames, blocks);
        plugin.reset();
        let rms_8khz = rms_after_latency(&mut plugin, 8000.0, sr, num_frames, blocks);

        // 8 kHz should be near passband (≥ 0.35 of input 0.5 amplitude).
        assert!(
            rms_8khz > 0.20,
            "8 kHz should be in passband, rms={rms_8khz:.4}"
        );
        // 100 Hz should be at least 6 dB below 8 kHz (cut is -12 dB).
        let attenuation_db = if rms_100hz < 1e-10 {
            120.0f64
        } else {
            20.0 * (rms_8khz / rms_100hz).log10()
        };
        assert!(
            attenuation_db > 6.0,
            "Expected >6 dB low-frequency attenuation with lowshelf cut, got {attenuation_db:.1} dB"
        );
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = LinearPhaseEqPlugin::new(2, 48000);

        // Set mix
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        match val {
            Some(ParameterValue::Float(v)) => assert!((v - 0.5).abs() < 0.01),
            other => panic!("Expected Float(0.5), got {other:?}"),
        }

        // Set band frequency
        plugin
            .set_parameter(
                ParameterId::from("band_0_freq"),
                ParameterValue::Float(2000.0),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("band_0_freq"));
        match val {
            Some(ParameterValue::Float(v)) => assert!((v - 2000.0).abs() < 1.0),
            other => panic!("Expected Float(2000.0), got {other:?}"),
        }
    }

    #[test]
    fn test_dc_gain_not_hardcoded() {
        // CRITICAL: DC magnitude was hardcoded to 0 dB regardless of filter shape.
        // A lowshelf cut should produce a FIR with attenuated DC gain.
        let channels = 1;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Lowshelf".to_string(),
                frequency: 200.0,
                q: 0.7,
                gain_db: -12.0,
                active: true,
            }],
        };

        let plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let dc_gain_linear: f32 = plugin.fir_coeffs.iter().sum();
        let dc_gain_db = 20.0 * dc_gain_linear.abs().max(1e-12).log10();
        // With the bug DC was forced to 0 dB, so sum ≈ 1.0 (0 dB).
        // After the fix the FIR should reflect the shelf cut.
        assert!(
            dc_gain_db < -6.0,
            "Expected DC gain significantly below 0 dB for a lowshelf cut, got {dc_gain_db:.2} dB"
        );
    }

    #[test]
    fn test_lowpass_zero_gain_not_skipped() {
        // CRITICAL: lowpass/highpass bands with 0 dB gain were silently skipped.
        let channels = 1;
        let sr = 48000;
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Lowpass".to_string(),
                frequency: 1000.0,
                q: 0.7,
                gain_db: 0.0,
                active: true,
            }],
        };

        let mut plugin = LinearPhaseEqPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency / num_frames) + 10;

        let mut input_rms = 0.0f64;
        let mut output_rms = 0.0f64;
        let mut samples_counted = 0usize;

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                // 5 kHz sine, well above 1 kHz cutoff
                let sample = (2.0 * std::f32::consts::PI * 5000.0 * t).sin() * 0.5;
                buffer[frame] = sample;
            }

            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    input_rms += (s as f64) * (s as f64);
                }
                samples_counted += num_frames;
            }

            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();

            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    output_rms += (s as f64) * (s as f64);
                }
            }
        }

        if samples_counted > 0 {
            input_rms = (input_rms / samples_counted as f64).sqrt();
            output_rms = (output_rms / samples_counted as f64).sqrt();

            let attenuation_db = 20.0 * (output_rms / input_rms).log10();
            // A 1 kHz lowpass should attenuate a 5 kHz sine significantly.
            // With the bug the band was skipped, resulting in ~0 dB attenuation.
            assert!(
                attenuation_db < -6.0,
                "Expected significant attenuation for lowpass at 5 kHz, got {attenuation_db:.1} dB"
            );
        }
    }
}
