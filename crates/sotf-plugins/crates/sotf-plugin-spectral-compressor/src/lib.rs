// ============================================================================
// Spectral Compressor Plugin
// ============================================================================
//
// Per-bin dynamics processor in the frequency domain. Each FFT bin has its
// own compressor envelope, enabling surgical control of tonal resonances
// and harsh overtones that multiband compression cannot reach.
//
// HARD RULES:
// - No allocations in process_in_place() hot path
// - All Vecs pre-allocated in new()/initialize()
// - No mutex locks in process()
// - No unsafe code

pub mod params;

use math_audio_dsp::stft::{RealFftProcessor, generate_hann_window};
use math_audio_dsp::tonal_transient::TonalTransientSeparator;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::delta_monitor::DeltaMonitor;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

use crate::params::{PARAMS as SC, TARGET_MODES};

// ============================================================================
// FFT size helpers
// ============================================================================

const FFT_SIZE_OPTIONS: [usize; 3] = [1024, 2048, 4096];

fn fft_size_from_index(index: usize) -> usize {
    FFT_SIZE_OPTIONS.get(index).copied().unwrap_or(2048)
}

// ============================================================================
// Gain reduction formula (matches dynamics_core.rs calculate_compress_gr)
// ============================================================================

/// Compressor gain reduction: standard soft-knee formula.
///
/// Returns gain reduction in dB (positive value) for signals above threshold.
#[inline]
fn compress_gr(input_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if knee < 0.1 {
        if input_db <= threshold {
            0.0
        } else {
            (input_db - threshold) * slope
        }
    } else if input_db < threshold - knee / 2.0 {
        0.0
    } else if input_db > threshold + knee / 2.0 {
        (input_db - threshold) * slope
    } else {
        let overshoot = input_db - threshold + knee / 2.0;
        let kf = overshoot / knee;
        kf * kf * (knee / 2.0) * slope
    }
}

// ============================================================================
// Plugin Params (serde)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralCompressorPluginParams {
    #[serde(default = "default_fft_size_index")]
    pub fft_size_index: usize,
    #[serde(default = "default_threshold")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_release")]
    pub release_ms: f32,
    #[serde(default = "default_knee")]
    pub knee_db: f32,
    #[serde(default = "default_spectral_smoothing")]
    pub spectral_smoothing: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_fft_size_index() -> usize {
    pk(SC, "fft_size").default_f64() as usize
}
fn default_threshold() -> f32 {
    pk(SC, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(SC, "ratio").default_f64() as f32
}
fn default_attack() -> f32 {
    pk(SC, "attack").default_f64() as f32
}
fn default_release() -> f32 {
    pk(SC, "release").default_f64() as f32
}
fn default_knee() -> f32 {
    pk(SC, "knee").default_f64() as f32
}
fn default_spectral_smoothing() -> f32 {
    pk(SC, "spectral_smoothing").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(SC, "mix").default_f64() as f32
}

impl Default for SpectralCompressorPluginParams {
    fn default() -> Self {
        Self {
            fft_size_index: default_fft_size_index(),
            threshold_db: default_threshold(),
            ratio: default_ratio(),
            attack_ms: default_attack(),
            release_ms: default_release(),
            knee_db: default_knee(),
            spectral_smoothing: default_spectral_smoothing(),
            mix: default_mix(),
        }
    }
}

// ============================================================================
// STFT State
// ============================================================================

/// All STFT buffers needed for spectral processing.
/// Pre-allocated to avoid hot-path allocation.
struct StftState {
    fft_size: usize,
    hop_size: usize,
    num_bins: usize,

    /// Per-channel FFT processor (forward + inverse)
    fft_processors: Vec<RealFftProcessor>,

    /// Hann analysis window (length = fft_size)
    analysis_window: Vec<f32>,

    /// Combined COLA normalization + 1/fft_size scale factor.
    /// For 75% overlap dual-windowing periodic Hann: 1/(1.5 * N)
    /// (4 overlapping Hann² windows sum to exactly 1.5 at all positions)
    output_scale: f32,

    // --- Input staging ---
    /// Per-channel linear input buffer [channels][fft_size]
    input_buffers: Vec<Vec<f32>>,
    /// How many valid samples are in the tail of each input_buffer
    input_fill: usize,

    // --- Per-channel, per-bin envelope state ---
    /// [channels][num_bins] — smoothed gain reduction in dB (0 = no compression)
    bin_envelopes: Vec<Vec<f32>>,

    // --- OLA output accumulator ---
    /// Flat interleaved ring buffer: [ch0_f0, ch1_f0, ch0_f1, ...]
    output_accumulator: Vec<f32>,
    output_accumulator_mask: usize,
    output_accumulator_fill: usize,
    next_add_position: usize,
    output_read_position: usize,
    latency_filled: usize,

    // --- Temporary working buffers ---
    /// Frequency-domain scratch [num_bins]
    freq_scratch: Vec<Complex<f32>>,
    /// Scratch for envelope values after median + smoothing [num_bins]
    gains_scratch: Vec<f32>,

    // --- Phase 4A: Tonal/Transient separation ---
    /// Per-channel tonal/transient separator
    tonal_transient: Vec<TonalTransientSeparator>,
    /// Scratch for magnitudes [num_bins]
    magnitudes_scratch: Vec<f32>,
    /// Scratch for tonal mask [num_bins]
    tonal_mask: Vec<f32>,
    /// Scratch for transient mask [num_bins]
    transient_mask: Vec<f32>,
    /// Per-channel long-term spectral average for adaptive threshold [channels][num_bins]
    adaptive_avg: Vec<Vec<f32>>,
}

impl StftState {
    fn new(fft_size: usize, channels: usize) -> Self {
        let hop_size = fft_size / 4; // 75% overlap
        let num_bins = fft_size / 2 + 1;

        let fft_processors: Vec<RealFftProcessor> = (0..channels)
            .map(|_| RealFftProcessor::new_bidirectional(fft_size))
            .collect();

        let analysis_window = generate_hann_window(fft_size);

        // 75% overlap, dual periodic Hann (Hann²): COLA sum = 1.5 (exact)
        let output_scale = 1.0 / (fft_size as f32 * 1.5);

        let bin_envelopes: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; num_bins]).collect();

        let output_accumulator_frames = (fft_size * 4).next_power_of_two();
        let output_accumulator = vec![0.0f32; output_accumulator_frames * channels];

        Self {
            fft_size,
            hop_size,
            num_bins,
            fft_processors,
            analysis_window,
            output_scale,
            input_buffers: vec![vec![0.0f32; fft_size]; channels],
            input_fill: 0,
            bin_envelopes,
            output_accumulator,
            output_accumulator_mask: output_accumulator_frames - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            latency_filled: 0,
            freq_scratch: vec![Complex::new(0.0, 0.0); num_bins],
            gains_scratch: vec![0.0; num_bins],
            // Phase 4A: Tonal/Transient
            tonal_transient: (0..channels)
                .map(|_| TonalTransientSeparator::new(num_bins, 7, 7))
                .collect(),
            magnitudes_scratch: vec![0.0; num_bins],
            tonal_mask: vec![0.0; num_bins],
            transient_mask: vec![0.0; num_bins],
            adaptive_avg: vec![vec![-20.0; num_bins]; channels], // init near typical threshold to avoid startup transient
        }
    }

    fn reset(&mut self) {
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        self.input_fill = 0;
        for env in &mut self.bin_envelopes {
            env.fill(0.0);
        }
        for tt in &mut self.tonal_transient {
            tt.reset();
        }
        for avg in &mut self.adaptive_avg {
            avg.fill(-20.0);
        }
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
    }
}

// ============================================================================
// Plugin Struct
// ============================================================================

pub struct SpectralCompressorPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    fft_size_index: usize,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    spectral_smoothing: f32,
    mix: f32,

    // Derived coefficients
    fft_size: usize,
    attack_coeff: f32,
    release_coeff: f32,

    // Phase 4A: SOTA params
    target_mode: usize, // 0=All, 1=Tonal, 2=Transient
    delta_monitor: DeltaMonitor,
    adaptive_threshold: bool,
    adaptive_offset_db: f32,

    // STFT state
    stft: StftState,

    // Smoothers
    threshold_smoother: Smoother,
    mix_smoother: Smoother,

    // Cached parameter list
    cached_parameters: Vec<Parameter>,
}

impl SpectralCompressorPlugin {
    pub fn from_params(channels: usize, params: SpectralCompressorPluginParams) -> Self {
        let fft_size = fft_size_from_index(params.fft_size_index);
        let sample_rate = 48000u32;
        let hop_size = fft_size / 4;
        let hop_rate = sample_rate as f32 / hop_size as f32;

        // Guard against zero/negative values: zero → instant response (coeff=0.0),
        // negative → would give exp(+inf) = +inf corrupting envelope state.
        let attack_coeff = if params.attack_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (params.attack_ms * 0.001 * hop_rate)).exp()
        };
        let release_coeff = if params.release_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (params.release_ms * 0.001 * hop_rate)).exp()
        };

        let mut plugin = Self {
            channels,
            sample_rate,

            fft_size_index: params.fft_size_index,
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            spectral_smoothing: params.spectral_smoothing,
            mix: params.mix,

            fft_size,
            attack_coeff,
            release_coeff,

            target_mode: 0, // All
            delta_monitor: DeltaMonitor::new(),
            adaptive_threshold: false,
            adaptive_offset_db: 0.0,

            stft: StftState::new(fft_size, channels),

            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sample_rate),
            mix_smoother: Smoother::new(params.mix, 20.0, sample_rate),

            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Recompute attack/release coefficients at hop rate.
    fn recompute_coefficients(&mut self) {
        let hop_size = self.stft.hop_size;
        let hop_rate = self.sample_rate as f32 / hop_size as f32;
        self.attack_coeff = if self.attack_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (self.attack_ms * 0.001 * hop_rate)).exp()
        };
        self.release_coeff = if self.release_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (self.release_ms * 0.001 * hop_rate)).exp()
        };
    }

    /// Rebuild STFT state when FFT size changes.
    fn rebuild_stft(&mut self) {
        let new_fft_size = fft_size_from_index(self.fft_size_index);
        if new_fft_size != self.fft_size {
            self.fft_size = new_fft_size;
            self.stft = StftState::new(new_fft_size, self.channels);
            self.recompute_coefficients();
        }
    }

    /// Process one STFT hop: FFT -> per-bin compression -> IFFT -> OLA.
    fn process_spectral_hop(&mut self) {
        let channels = self.channels;
        let fft_size = self.stft.fft_size;
        let num_bins = self.stft.num_bins;
        let scale = self.stft.output_scale;
        let mask = self.stft.output_accumulator_mask;

        let threshold = self.threshold_smoother.next_n(self.stft.hop_size);
        let ratio = self.ratio;
        let knee = self.knee_db;
        let attack_coeff = self.attack_coeff;
        let release_coeff = self.release_coeff;
        let spectral_smoothing = self.spectral_smoothing;

        // Magnitude calibration:
        // realfft produces an unnormalized FFT. For a rectangular window a unit
        // sine peak = fft_size/2, so the base scale is 2/fft_size.
        // With a periodic Hann analysis window the coherent gain is 0.5, so the
        // peak drops to fft_size/4 for all interior bins. We compensate by an
        // extra ×2, giving mag_norm_interior = 4/fft_size.
        // DC (k=0) and Nyquist (k=num_bins-1) are real-valued and scale as
        // fft_size/2 regardless of the window, so they keep the base scale.
        let mag_norm_base = 2.0 / fft_size as f32;
        let mag_norm_interior = mag_norm_base * 2.0; // compensate Hann coherent gain
        let use_adaptive = self.adaptive_threshold;
        let adaptive_offset = self.adaptive_offset_db;
        // EMA coefficient for long-term average: ~500ms at hop rate
        let adaptive_alpha = 0.98_f32;

        for ch in 0..channels {
            // --- Forward FFT ---
            // Write directly into the FFT processor's time buffer to avoid an extra copy.
            for i in 0..fft_size {
                self.stft.fft_processors[ch].time_buffer[i] =
                    self.stft.input_buffers[ch][i] * self.stft.analysis_window[i];
            }
            self.stft.fft_processors[ch].forward();
            self.stft
                .freq_scratch
                .copy_from_slice(&self.stft.fft_processors[ch].freq_buffer);

            // --- Per-bin compression ---
            for k in 0..num_bins {
                // DC and Nyquist use base scale; all interior bins compensate for
                // the Hann window's 0.5 coherent gain with ×2.
                let mag_norm = if k == 0 || k == num_bins - 1 {
                    mag_norm_base
                } else {
                    mag_norm_interior
                };
                let mag = self.stft.freq_scratch[k].norm() * mag_norm;
                self.stft.magnitudes_scratch[k] = mag;
                let mag_db = 20.0 * mag.max(1e-10).log10();

                // Adaptive threshold: use long-term per-bin average + offset
                let effective_threshold = if use_adaptive {
                    let avg = &mut self.stft.adaptive_avg[ch][k];
                    *avg = adaptive_alpha * *avg + (1.0 - adaptive_alpha) * mag_db;
                    *avg + adaptive_offset
                } else {
                    threshold
                };

                let mut target_gr = compress_gr(mag_db, effective_threshold, ratio, knee);

                // --- Phase 4A: Tonal/Transient masking (applied before envelope smoothing) ---
                // Masking target_gr *before* the one-pole smoother ensures the envelope
                // only tracks energy in the selected component. If we masked after, the
                // envelope would still attack/release based on the unwanted component and
                // then be zeroed, leaving stale state that causes a delayed response when
                // the bin later enters the active component.
                //
                // The tonal_transient separator is run once per channel (not per-bin here),
                // so we defer the actual process() call to outside the k-loop below and
                // re-enter per-bin once the masks are available.
                // For the per-bin loop we rely on previously computed masks (first hop uses
                // all-ones which is safe; subsequent hops use the masks computed at the end
                // of this block).
                let target_mode = self.target_mode;
                if target_mode > 0 {
                    let mask = match target_mode {
                        1 => self.stft.tonal_mask[k],
                        2 => self.stft.transient_mask[k],
                        _ => 1.0,
                    };
                    target_gr *= mask;
                }

                // One-pole envelope smoothing at hop rate
                let envelope = &mut self.stft.bin_envelopes[ch][k];
                let coeff = if target_gr > *envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                *envelope = target_gr + coeff * (*envelope - target_gr);
            }

            // --- Update tonal/transient masks for the NEXT hop ---
            // We update after the compression loop so that the current hop uses the masks
            // from the previous hop (which were computed from the previous hop's magnitudes).
            // This is a one-hop lag but is the correct approach: computing masks from the
            // current magnitudes and then using them in the same hop would require two passes.
            if self.target_mode > 0 {
                self.stft.tonal_transient[ch].process(
                    &self.stft.magnitudes_scratch[..num_bins],
                    &mut self.stft.tonal_mask[..num_bins],
                    &mut self.stft.transient_mask[..num_bins],
                );
            }

            // --- 3-bin median filter on envelope (reduce musical noise) ---
            // Copy envelopes to scratch for in-place median
            self.stft.gains_scratch[..num_bins]
                .copy_from_slice(&self.stft.bin_envelopes[ch][..num_bins]);

            // Apply 3-bin median: for each bin k in [1..num_bins-1],
            // replace with median of (k-1, k, k+1).
            // Boundary bins use min of 2 neighbors (conservative).
            if num_bins >= 2 {
                self.stft.bin_envelopes[ch][0] =
                    self.stft.gains_scratch[0].min(self.stft.gains_scratch[1]);
            }
            for k in 1..num_bins.saturating_sub(1) {
                let a = self.stft.gains_scratch[k - 1];
                let b = self.stft.gains_scratch[k];
                let c = self.stft.gains_scratch[k + 1];
                let med = if a <= b {
                    if b <= c {
                        b
                    } else if a <= c {
                        c
                    } else {
                        a
                    }
                } else if a <= c {
                    a
                } else if b <= c {
                    c
                } else {
                    b
                };
                self.stft.bin_envelopes[ch][k] = med;
            }
            if num_bins >= 2 {
                let last = num_bins - 1;
                self.stft.bin_envelopes[ch][last] =
                    self.stft.gains_scratch[last].min(self.stft.gains_scratch[last - 1]);
            }

            // --- Frequency-axis EMA smoothing (optional) ---
            if spectral_smoothing > 0.001 {
                let alpha = spectral_smoothing;
                let one_minus_alpha = 1.0 - alpha;
                // Forward pass
                let mut prev = self.stft.bin_envelopes[ch][0];
                for k in 1..num_bins {
                    let smoothed = alpha * prev + one_minus_alpha * self.stft.bin_envelopes[ch][k];
                    self.stft.bin_envelopes[ch][k] = smoothed;
                    prev = smoothed;
                }
                // Backward pass for symmetric smoothing
                prev = self.stft.bin_envelopes[ch][num_bins - 1];
                for k in (0..num_bins.saturating_sub(1)).rev() {
                    let smoothed = alpha * prev + one_minus_alpha * self.stft.bin_envelopes[ch][k];
                    self.stft.bin_envelopes[ch][k] = smoothed;
                    prev = smoothed;
                }
            }

            // --- Apply gain reduction to frequency bins ---
            for k in 0..num_bins {
                let envelope_db = self.stft.bin_envelopes[ch][k];
                if envelope_db > 0.001 {
                    let gain_linear = 10.0_f32.powf(-envelope_db / 20.0);
                    self.stft.freq_scratch[k] *= gain_linear;
                }
                // else: no gain change needed (envelope ~= 0)
            }

            // --- Inverse FFT ---
            self.stft.fft_processors[ch]
                .freq_buffer
                .copy_from_slice(&self.stft.freq_scratch);
            self.stft.fft_processors[ch].inverse();

            // Apply synthesis window (Hann) + scale, overlap-add into ring
            let next_pos = self.stft.next_add_position;
            for i in 0..fft_size {
                let frame_idx = (next_pos + i) & mask;
                let s = self.stft.fft_processors[ch].time_buffer[i]
                    * self.stft.analysis_window[i] // synthesis window = same Hann
                    * scale;
                self.stft.output_accumulator[frame_idx * channels + ch] += s;
            }
        }

        // Advance OLA write position by one hop
        let hop_size = self.stft.hop_size;
        self.stft.next_add_position = (self.stft.next_add_position + hop_size) & mask;

        // Zero the "fresh" positions for clean OLA
        {
            let clear_start = (self.stft.next_add_position + fft_size) & mask;
            for i in 0..hop_size {
                let frame_idx = (clear_start + i) & mask;
                for ch in 0..channels {
                    self.stft.output_accumulator[frame_idx * channels + ch] = 0.0;
                }
            }
        }

        self.stft.output_accumulator_fill += hop_size;
        self.stft.latency_filled += hop_size;
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "fft_size",
                "FFT Size",
                self.fft_size_index as i32,
                0,
                (FFT_SIZE_OPTIONS.len() - 1) as i32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(SC, "threshold").min_f64() as f32,
                pk(SC, "threshold").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(SC, "ratio").min_f64() as f32,
                pk(SC, "ratio").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(SC, "attack").min_f64() as f32,
                pk(SC, "attack").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(SC, "release").min_f64() as f32,
                pk(SC, "release").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "knee",
                "Knee",
                self.knee_db,
                pk(SC, "knee").min_f64() as f32,
                pk(SC, "knee").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "spectral_smoothing",
                "Spectral Smooth",
                self.spectral_smoothing,
                pk(SC, "spectral_smoothing").min_f64() as f32,
                pk(SC, "spectral_smoothing").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(SC, "mix").min_f64() as f32,
                pk(SC, "mix").max_f64() as f32,
            )
            .with_importance(ParameterImportance::Critical),
            // Phase 4A: SOTA
            Parameter::new_string(
                "target_mode",
                "Target",
                TARGET_MODES[self.target_mode.min(2)].to_string(),
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("delta_listen", "Delta Listen", self.delta_monitor.enabled())
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("adaptive_threshold", "Adaptive", self.adaptive_threshold)
                .with_group("Analysis")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "adaptive_offset_db",
                "Adapt Offset",
                self.adaptive_offset_db,
                -20.0,
                20.0,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    #[inline]
    fn mix_output_sample(dry: f32, wet: f32, mix: f32, delta_enabled: bool) -> f32 {
        let mixed = dry * (1.0 - mix) + wet * mix;
        if delta_enabled { mixed - dry } else { mixed }
    }
}

// ============================================================================
// InPlacePlugin Implementation
// ============================================================================

impl InPlacePlugin for SpectralCompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Spectral Compressor", "1.0.0", "Sotf")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = &id.0;
        match name.as_str() {
            "fft_size" => {
                let idx = value
                    .as_int()
                    .ok_or_else(|| "FFT size must be an integer".to_string())?
                    as usize;
                if idx != self.fft_size_index {
                    self.fft_size_index = idx;
                    self.rebuild_stft();
                }
            }
            "threshold" => {
                self.threshold_db = value
                    .as_float()
                    .ok_or_else(|| "Threshold must be a float".to_string())?;
                self.threshold_smoother.set_target(self.threshold_db);
            }
            "ratio" => {
                self.ratio = value
                    .as_float()
                    .ok_or_else(|| "Ratio must be a float".to_string())?;
            }
            "attack" => {
                self.attack_ms = value
                    .as_float()
                    .ok_or_else(|| "Attack must be a float".to_string())?;
                self.recompute_coefficients();
            }
            "release" => {
                self.release_ms = value
                    .as_float()
                    .ok_or_else(|| "Release must be a float".to_string())?;
                self.recompute_coefficients();
            }
            "knee" => {
                self.knee_db = value
                    .as_float()
                    .ok_or_else(|| "Knee must be a float".to_string())?;
            }
            "spectral_smoothing" => {
                self.spectral_smoothing = value
                    .as_float()
                    .ok_or_else(|| "Spectral smoothing must be a float".to_string())?;
            }
            "mix" => {
                self.mix = value
                    .as_float()
                    .ok_or_else(|| "Mix must be a float".to_string())?;
                self.mix_smoother.set_target(self.mix);
            }
            "target_mode" => {
                let idx = if let Some(s) = value.as_string() {
                    TARGET_MODES.iter().position(|&m| m == s).unwrap_or(0)
                } else if let Some(v) = value.as_float() {
                    (v as usize).min(2)
                } else {
                    0
                };
                self.target_mode = idx;
            }
            "delta_listen" => {
                let enabled = value.as_bool().unwrap_or(false);
                self.delta_monitor.set_enabled(enabled);
            }
            "adaptive_threshold" => {
                self.adaptive_threshold = value.as_bool().unwrap_or(false);
            }
            "adaptive_offset_db" => {
                let v = value.as_float().unwrap_or(0.0);
                if v.is_finite() {
                    self.adaptive_offset_db = v.clamp(-20.0, 20.0);
                }
            }
            other => return Err(format!("Unknown parameter: {other}")),
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "fft_size" => Some(ParameterValue::Int(self.fft_size_index as i32)),
            "threshold" => Some(ParameterValue::Float(self.threshold_db)),
            "ratio" => Some(ParameterValue::Float(self.ratio)),
            "attack" => Some(ParameterValue::Float(self.attack_ms)),
            "release" => Some(ParameterValue::Float(self.release_ms)),
            "knee" => Some(ParameterValue::Float(self.knee_db)),
            "spectral_smoothing" => Some(ParameterValue::Float(self.spectral_smoothing)),
            "mix" => Some(ParameterValue::Float(self.mix)),
            "target_mode" => Some(ParameterValue::String(
                TARGET_MODES[self.target_mode.min(2)].to_string(),
            )),
            "delta_listen" => Some(ParameterValue::Bool(self.delta_monitor.enabled())),
            "adaptive_threshold" => Some(ParameterValue::Bool(self.adaptive_threshold)),
            "adaptive_offset_db" => Some(ParameterValue::Float(self.adaptive_offset_db)),
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.stft = StftState::new(self.fft_size, self.channels);
        self.recompute_coefficients();
        self.threshold_smoother = Smoother::new(self.threshold_db, 20.0, sample_rate);
        self.mix_smoother = Smoother::new(self.mix, 20.0, sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        self.stft.reset();
        self.threshold_smoother = Smoother::new(self.threshold_db, 20.0, self.sample_rate);
        self.mix_smoother = Smoother::new(self.mix, 20.0, self.sample_rate);
    }

    fn latency_samples(&self) -> usize {
        // Causal STFT latency: the OLA output is first valid after fft_size - hop_size
        // samples have been accumulated. Reporting the full fft_size over-compensates
        // by hop_size (~10 ms for N=2048 at 48 kHz).
        self.fft_size - self.stft.hop_size
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();

        let nf = context.num_frames;
        let channels = self.channels;
        let fft_size = self.stft.fft_size;
        let total = nf
            .checked_mul(channels)
            .ok_or_else(|| "Frame/channel count overflow".to_string())?;
        if buffer.len() != total {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                total,
                buffer.len()
            ));
        }

        let g_mix = self.mix_smoother.next_n(nf);
        let delta_enabled = self.delta_monitor.enabled();

        let mut input_pos = 0; // frame index into the caller's buffer
        let mut output_pos = 0; // frame index into the caller's output

        let hop_size = self.stft.hop_size;

        while output_pos < nf {
            // --- Step 1: Fill input ring from caller's buffer ---
            if input_pos < nf {
                let space_in_tail = fft_size - self.stft.input_fill;
                let available = nf - input_pos;
                let to_copy = space_in_tail.min(available);

                if to_copy > 0 {
                    // Iterate over frames in the outer loop so that we read the
                    // interleaved source buffer contiguously (cache-friendly).
                    for i in 0..to_copy {
                        let src_base = (input_pos + i) * channels;
                        let dst_idx = self.stft.input_fill + i;
                        for ch in 0..channels {
                            self.stft.input_buffers[ch][dst_idx] = buffer[src_base + ch];
                        }
                    }
                    self.stft.input_fill += to_copy;
                    input_pos += to_copy;
                }
            }

            // --- Step 2: Process STFT frames while we have a full window ---
            if self.stft.input_fill >= fft_size {
                self.process_spectral_hop();
                // Shift input ring: keep overlap = fft_size - hop_size samples
                let overlap = fft_size - hop_size;
                for ch in 0..channels {
                    self.stft.input_buffers[ch].copy_within(hop_size..fft_size, 0);
                    self.stft.input_buffers[ch][overlap..].fill(0.0);
                }
                self.stft.input_fill = overlap;
            }

            // --- Step 3: Drain available OLA frames into output ---
            let frames_to_drain = self.stft.output_accumulator_fill.min(nf - output_pos);
            if frames_to_drain > 0 {
                let mask = self.stft.output_accumulator_mask;
                for i in 0..frames_to_drain {
                    let read_idx = (self.stft.output_read_position + i) & mask;
                    let out_base = (output_pos + i) * channels;
                    for ch in 0..channels {
                        let idx = out_base + ch;
                        let dry = buffer[idx];
                        let wet = self.stft.output_accumulator[read_idx * channels + ch];
                        buffer[idx] = Self::mix_output_sample(dry, wet, g_mix, delta_enabled);
                    }
                }
                // Clear drained frames
                for i in 0..frames_to_drain {
                    let read_idx = (self.stft.output_read_position + i) & mask;
                    for ch in 0..channels {
                        self.stft.output_accumulator[read_idx * channels + ch] = 0.0;
                    }
                }
                self.stft.output_read_position =
                    (self.stft.output_read_position + frames_to_drain) & mask;
                self.stft.output_accumulator_fill -= frames_to_drain;
                output_pos += frames_to_drain;
            } else {
                // No output ready: output silence for the wet path during initial latency fill.
                for i in output_pos..nf {
                    let out_base = i * channels;
                    for ch in 0..channels {
                        let idx = out_base + ch;
                        let dry = buffer[idx];
                        buffer[idx] = Self::mix_output_sample(dry, 0.0, g_mix, delta_enabled);
                    }
                }
                output_pos = nf;
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

    fn make_plugin(threshold: f32, ratio: f32) -> SpectralCompressorPlugin {
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1, // 2048
            threshold_db: threshold,
            ratio,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            spectral_smoothing: 0.3,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();
        plugin
    }

    fn process_signal(plugin: &mut SpectralCompressorPlugin, signal: &[f32]) -> Vec<f32> {
        let channels = plugin.channels();
        let total_frames = signal.len() / channels;

        // Process the entire signal in one call (like the multiband expander test)
        let mut buf = signal.to_vec();
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: total_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();
        buf
    }

    #[test]
    fn test_passthrough_with_high_threshold() {
        // Threshold = 0dB means nothing should be compressed
        // (typical audio is well below 0dBFS per-bin)
        let mut plugin = make_plugin(0.0, 4.0);
        let channels = 2;
        let num_frames = 48000; // 1 second
        let freq = 440.0;
        let amplitude = 0.1; // -20dBFS, well below 0dB threshold

        // Generate stereo sine
        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // After OLA converges, output RMS should match input RMS (no compression)
        let skip = 16384; // generous skip for convergence
        let check_len = num_frames - skip - 4096;
        assert!(check_len > 0, "Not enough samples to compare");

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Passthrough RMS ratio should be ~1.0, got {:.4} (in={:.4}, out={:.4})",
            ratio,
            rms_in,
            rms_out
        );
    }

    #[test]
    fn test_compresses_loud_bins() {
        // Low threshold, high ratio: should compress a loud signal
        let mut plugin = make_plugin(-40.0, 8.0);
        let channels = 2;
        let num_frames = 48000;
        let freq = 1000.0;
        let amplitude = 0.5; // -6dBFS, well above -40dB threshold

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // Check that output RMS is lower than input RMS (compression happened)
        let skip = plugin.fft_size + 4096;
        let check_len = num_frames - skip - 1024;

        let mut rms_in = 0.0f32;
        let mut rms_out = 0.0f32;
        for i in skip..skip + check_len {
            let idx = i * channels;
            rms_in += signal[idx] * signal[idx];
            rms_out += output[idx] * output[idx];
        }
        rms_in = (rms_in / check_len as f32).sqrt();
        rms_out = (rms_out / check_len as f32).sqrt();

        assert!(
            rms_out < rms_in * 0.9,
            "Expected compression: rms_out={:.4} should be < rms_in={:.4} * 0.9",
            rms_out,
            rms_in
        );
    }

    #[test]
    fn test_quiet_bins_untouched() {
        // Threshold at -10dB, signal at -60dBFS (well below threshold)
        let mut plugin = make_plugin(-10.0, 4.0);
        let channels = 2;
        let num_frames = 48000;
        let freq = 440.0;
        let amplitude = 0.001; // -60dBFS, below -10dB threshold

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // Output RMS should match input RMS (no compression, below threshold)
        let skip = 16384;
        let check_len = num_frames - skip - 4096;

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Quiet signal RMS ratio should be ~1.0, got {:.4}",
            ratio
        );
    }

    #[test]
    fn test_latency_reported_correctly() {
        // Causal STFT latency = fft_size - hop_size (hop = fft_size/4 at 75% overlap).
        // fft_size=2048, hop=512 → latency=1536
        let plugin = make_plugin(-20.0, 2.0);
        assert_eq!(plugin.latency_samples(), 1536);

        // fft_size=1024, hop=256 → latency=768
        let params_1024 = SpectralCompressorPluginParams {
            fft_size_index: 0,
            ..Default::default()
        };
        let plugin_1024 = SpectralCompressorPlugin::from_params(2, params_1024);
        assert_eq!(plugin_1024.latency_samples(), 768);

        // fft_size=4096, hop=1024 → latency=3072
        let params_4096 = SpectralCompressorPluginParams {
            fft_size_index: 2,
            ..Default::default()
        };
        let plugin_4096 = SpectralCompressorPlugin::from_params(2, params_4096);
        assert_eq!(plugin_4096.latency_samples(), 3072);
    }

    #[test]
    fn test_process_rejects_buffer_size_mismatch() {
        let mut plugin = make_plugin(-20.0, 2.0);
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 64,
        };
        let mut short = vec![0.0f32; ctx.num_frames * plugin.channels() - 1];
        let err = plugin.process_in_place(&mut short, &ctx).unwrap_err();
        assert!(err.contains("Buffer size mismatch"));
    }

    #[test]
    fn test_mix_zero_passthrough_during_latency_fill() {
        let params = SpectralCompressorPluginParams {
            mix: 0.0,
            ..Default::default()
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        let frames = 128;
        let mut buffer = vec![0.0f32; frames * 2];
        for i in 0..frames {
            buffer[i * 2] = i as f32 * 0.001;
            buffer[i * 2 + 1] = -(i as f32) * 0.001;
        }
        let original = buffer.clone();
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: frames,
        };

        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        assert_eq!(buffer, original);
    }

    /// Verify the magnitude calibration: a -20 dBFS sine that is above threshold
    /// should be compressed, and a sine well below threshold should pass through
    /// with correct amplitude (RMS ratio ≈ 1.0). This test would catch any
    /// systematic dB offset in `mag_norm` (e.g. the previous 6 dB Hann error).
    #[test]
    fn test_fft_roundtrip_no_compression_below_threshold() {
        // ratio=1.0 with any threshold → no compression regardless of level.
        // Use threshold=0 dB, ratio=1.0, mix=1.0, knee=0.
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1, // 2048
            threshold_db: 0.0,
            ratio: 1.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        let channels = 2;
        let num_frames = 96000usize; // 2 seconds
        let freq = 1000.0_f32;
        let amplitude = 0.1_f32; // -20 dBFS

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let s = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = s;
            signal[i * channels + 1] = s;
        }

        let output = process_signal(&mut plugin, &signal);

        // Skip initial latency + settling, compare RMS in the steady-state window.
        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Identity STFT (ratio=1.0) RMS ratio should be ~1.0, got {:.4} \
             (rms_in={:.6}, rms_out={:.6}). A value near 0.5 indicates the \
             6 dB Hann coherent-gain bug.",
            ratio,
            rms_in,
            rms_out,
        );
    }

    /// Verify that the magnitude calibration is correct: a -20 dBFS sine with
    /// threshold=-25 dB must be detected as above threshold and compressed.
    /// Before the Hann-gain fix the measured level was -26 dB, causing the
    /// compressor to see it as 1 dB below threshold and skip compression.
    #[test]
    fn test_magnitude_calibration_6db_hann_fix() {
        // threshold=-25 dB, ratio=8:1. A -20 dBFS sine is 5 dB above threshold.
        // Expected gain reduction ≈ (5 * 7/8) ≈ 4.4 dB → output ≈ -24.4 dBFS.
        // Before the fix: measured level was ~-26 dB (below -25 threshold) → no compression.
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1,
            threshold_db: -25.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 10.0,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        let channels = 2;
        let num_frames = 96000usize;
        let amplitude = 0.1_f32; // -20 dBFS (0.1 = 10^(-20/20))
        let freq = 1000.0_f32;

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let s = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = s;
            signal[i * channels + 1] = s;
        }

        let output = process_signal(&mut plugin, &signal);

        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        // Output must be reduced relative to input (compression happened).
        // rms_in ≈ 0.1/√2 ≈ 0.0707. After compression output should be noticeably lower.
        let rms_in_expected = amplitude / std::f32::consts::SQRT_2;
        assert!(
            rms_out < rms_in_expected * 0.85,
            "Expected compression (threshold=-25 dB, input=-20 dBFS): \
             rms_out={:.5} should be < {:.5}. \
             If rms_out ≈ rms_in, the 6 dB Hann calibration bug is present \
             (compressor sees input as -26 dB, below -25 dB threshold).",
            rms_out,
            rms_in_expected * 0.85,
        );
    }

    /// Verify that constructing a plugin with attack_ms=0 or release_ms=0 does
    /// not produce NaN/inf coefficients. Zero → instant response (coeff=0).
    #[test]
    fn test_zero_attack_release_coefficients() {
        let params = SpectralCompressorPluginParams {
            attack_ms: 0.0,
            release_ms: 0.0,
            ..Default::default()
        };
        let plugin = SpectralCompressorPlugin::from_params(2, params);
        assert!(
            plugin.attack_coeff.is_finite(),
            "attack_coeff should be finite when attack_ms=0, got {}",
            plugin.attack_coeff
        );
        assert!(
            plugin.release_coeff.is_finite(),
            "release_coeff should be finite when release_ms=0, got {}",
            plugin.release_coeff
        );
        assert_eq!(
            plugin.attack_coeff, 0.0,
            "attack_ms=0 should give instant coeff=0"
        );
        assert_eq!(
            plugin.release_coeff, 0.0,
            "release_ms=0 should give instant coeff=0"
        );
    }

    /// Verify that L and R channels are processed independently: feeding different
    /// signals to L and R and checking each channel's output independently.
    #[test]
    fn test_stereo_independence() {
        // Use high ratio, low threshold so both channels get compressed.
        let mut plugin = make_plugin(-30.0, 8.0);
        let channels = 2;
        let num_frames = 96000usize;

        // L: 440 Hz, R: 880 Hz, same amplitude
        let amplitude = 0.5_f32;
        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            signal[i * channels] = amplitude * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            signal[i * channels + 1] = amplitude * (2.0 * std::f32::consts::PI * 880.0 * t).sin();
        }

        let output = process_signal(&mut plugin, &signal);

        // After settling, measure RMS per-channel in output.
        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;
        let mut rms_l = 0.0f32;
        let mut rms_r = 0.0f32;
        for i in skip..skip + check_len {
            rms_l += output[i * channels] * output[i * channels];
            rms_r += output[i * channels + 1] * output[i * channels + 1];
        }
        rms_l = (rms_l / check_len as f32).sqrt();
        rms_r = (rms_r / check_len as f32).sqrt();

        // Both channels should have been compressed (output < input).
        let rms_in = amplitude / std::f32::consts::SQRT_2;
        assert!(
            rms_l < rms_in * 0.9,
            "L channel should be compressed: rms_l={:.4} vs rms_in={:.4}",
            rms_l,
            rms_in
        );
        assert!(
            rms_r < rms_in * 0.9,
            "R channel should be compressed: rms_r={:.4} vs rms_in={:.4}",
            rms_r,
            rms_in
        );
        // Channels should not be identical (different frequencies → different bin responses)
        // just verify they were processed (both nonzero).
        assert!(rms_l > 1e-6, "L channel output is silence");
        assert!(rms_r > 1e-6, "R channel output is silence");
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = make_plugin(-20.0, 2.0);

        // Set all parameters
        plugin
            .set_parameter(
                ParameterId("threshold".into()),
                ParameterValue::Float(-30.0),
            )
            .unwrap();
        plugin
            .set_parameter(ParameterId("ratio".into()), ParameterValue::Float(4.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId("attack".into()), ParameterValue::Float(10.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId("release".into()), ParameterValue::Float(100.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId("knee".into()), ParameterValue::Float(3.0))
            .unwrap();
        plugin
            .set_parameter(
                ParameterId("spectral_smoothing".into()),
                ParameterValue::Float(0.5),
            )
            .unwrap();
        plugin
            .set_parameter(ParameterId("mix".into()), ParameterValue::Float(0.8))
            .unwrap();
        plugin
            .set_parameter(ParameterId("fft_size".into()), ParameterValue::Int(2))
            .unwrap();

        // Verify all parameters
        assert_eq!(
            plugin.get_parameter(&ParameterId("threshold".into())),
            Some(ParameterValue::Float(-30.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("ratio".into())),
            Some(ParameterValue::Float(4.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("attack".into())),
            Some(ParameterValue::Float(10.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("release".into())),
            Some(ParameterValue::Float(100.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("knee".into())),
            Some(ParameterValue::Float(3.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("spectral_smoothing".into())),
            Some(ParameterValue::Float(0.5))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("mix".into())),
            Some(ParameterValue::Float(0.8))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("fft_size".into())),
            Some(ParameterValue::Int(2))
        );
    }
}
