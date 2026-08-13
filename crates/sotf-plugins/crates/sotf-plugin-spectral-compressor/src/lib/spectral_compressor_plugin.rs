use super::misc::FFT_SIZE_OPTIONS;
use super::misc::adaptive_alpha;
use super::misc::compress_gr;
use super::misc::fft_size_from_index;
use super::misc::smooth_spectral_envelope;
use super::spectral_compressor_plugin_params::SpectralCompressorPluginParams;
use super::stft_state::StftState;
use crate::params::{PARAMS as SC, TARGET_MODES};
use sotf_host::delta_monitor::DeltaMonitor;
use sotf_host::param_specs::{UpdateMode, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

const MAX_BLOCK_FRAMES: usize = 16_384;
const ADAPTIVE_TAU_SECONDS: f32 = 0.5;

pub struct SpectralCompressorPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Parameters
    pub(super) fft_size_index: usize,
    pub(super) threshold_db: f32,
    pub(super) ratio: f32,
    pub(super) attack_ms: f32,
    pub(super) release_ms: f32,
    pub(super) knee_db: f32,
    pub(super) spectral_smoothing: f32,
    pub(super) mix: f32,

    // Derived coefficients
    pub(super) fft_size: usize,
    pub(super) attack_coeff: f32,
    pub(super) release_coeff: f32,

    // Phase 4A: SOTA params
    pub(super) target_mode: usize, // 0=All, 1=Tonal, 2=Transient
    pub(super) delta_monitor: DeltaMonitor,
    pub(super) adaptive_threshold: bool,
    pub(super) adaptive_offset_db: f32,
    pub(super) channel_link: f32,

    // STFT state
    pub(super) stft: StftState,
    /// Stable input copy because in-place output can precede later input reads.
    pub(super) block_input: Vec<f32>,

    // Smoothers
    pub(super) threshold_smoother: Smoother,
    pub(super) mix_smoother: Smoother,

    // Cached parameter list
    pub(super) cached_parameters: Vec<Parameter>,
}

impl SpectralCompressorPlugin {
    pub fn try_from_params(
        channels: usize,
        params: SpectralCompressorPluginParams,
    ) -> PluginResult<Self> {
        if channels == 0 {
            return Err("Spectral compressor requires at least one channel".into());
        }
        let values = [
            ("threshold", params.threshold_db, -60.0, 0.0),
            ("ratio", params.ratio, 1.0, 20.0),
            ("attack", params.attack_ms, 0.1, 100.0),
            ("release", params.release_ms, 10.0, 1000.0),
            ("knee", params.knee_db, 0.0, 20.0),
            ("spectral_smoothing", params.spectral_smoothing, 0.0, 1.0),
            ("mix", params.mix, 0.0, 1.0),
            ("adaptive_offset_db", params.adaptive_offset_db, -20.0, 20.0),
            ("channel_link", params.channel_link, 0.0, 1.0),
        ];
        for (name, value, min, max) in values {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "{name} must be finite and in [{min}, {max}], got {value}"
                ));
            }
        }
        if params.fft_size_index >= FFT_SIZE_OPTIONS.len() {
            return Err(format!(
                "FFT size index {} is out of range",
                params.fft_size_index
            ));
        }
        if params.target_mode >= TARGET_MODES.len() {
            return Err(format!(
                "Target mode index {} is out of range",
                params.target_mode
            ));
        }
        Ok(Self::from_validated_params(channels, params))
    }

    pub fn from_params(channels: usize, params: SpectralCompressorPluginParams) -> Self {
        Self::try_from_params(channels, params).expect("invalid spectral compressor parameters")
    }

    fn from_validated_params(channels: usize, params: SpectralCompressorPluginParams) -> Self {
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

            target_mode: params.target_mode,
            delta_monitor: {
                let mut monitor = DeltaMonitor::new();
                monitor.set_enabled(params.delta_listen);
                monitor
            },
            adaptive_threshold: params.adaptive_threshold,
            adaptive_offset_db: params.adaptive_offset_db,
            channel_link: params.channel_link,

            stft: StftState::new(fft_size, channels),
            block_input: vec![0.0; MAX_BLOCK_FRAMES * channels],

            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sample_rate),
            mix_smoother: Smoother::new(params.mix, 20.0, sample_rate),

            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Recompute attack/release coefficients at hop rate.
    pub(super) fn recompute_coefficients(&mut self) {
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

    /// Process one STFT hop: FFT -> per-bin compression -> IFFT -> OLA.
    pub(super) fn process_spectral_hop(&mut self) {
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
        let mag_norm_base = 2.0 / fft_size as f32;
        let mag_norm_interior = mag_norm_base * 2.0;
        let use_adaptive = self.adaptive_threshold;
        let adaptive_offset = self.adaptive_offset_db;
        let adaptive_coeff =
            adaptive_alpha(self.stft.hop_size, self.sample_rate, ADAPTIVE_TAU_SECONDS);

        // Analysis/detector phase. Keeping each channel's FFT in its own
        // processor permits linking before envelopes and gain application.
        for ch in 0..channels {
            for i in 0..fft_size {
                let history_index = (self.stft.input_write_pos + i) & (fft_size - 1);
                self.stft.fft_processors[ch].time_buffer[i] =
                    self.stft.input_buffers[ch][history_index] * self.stft.analysis_window[i];
            }
            self.stft.fft_processors[ch].forward();

            // First calibrate every bin. Interior bins compensate the periodic
            // Hann coherent gain. The detector below aggregates local energy,
            // making a tone's threshold substantially less sensitive to its
            // fractional-bin alignment.
            for k in 0..num_bins {
                let mag_norm = if k == 0 || k == num_bins - 1 {
                    mag_norm_base
                } else {
                    mag_norm_interior
                };
                self.stft.spectral_magnitudes[ch][k] =
                    self.stft.fft_processors[ch].freq_buffer[k].norm() * mag_norm;
            }

            let was_initialized = self.stft.adaptive_initialized[ch];
            for k in 0..num_bins {
                let start = k.saturating_sub(2);
                let end = (k + 3).min(num_bins);
                let local_power: f32 = self.stft.spectral_magnitudes[ch][start..end]
                    .iter()
                    .map(|magnitude| magnitude * magnitude)
                    .sum();
                // A bin-centred Hann-windowed sinusoid distributes calibrated
                // squared magnitude as 1 + 0.25 + 0.25 across three bins.
                let detector_magnitude = (local_power / 1.5).sqrt();
                let mag_db = 20.0 * detector_magnitude.max(1e-10).log10();
                let effective_threshold = if use_adaptive {
                    let avg = &mut self.stft.adaptive_avg[ch][k];
                    if was_initialized {
                        *avg = adaptive_coeff * *avg + (1.0 - adaptive_coeff) * mag_db;
                    } else {
                        *avg = mag_db;
                    }
                    *avg + adaptive_offset
                } else {
                    threshold
                };

                let mut target_gr = compress_gr(mag_db, effective_threshold, ratio, knee);
                if self.target_mode > 0 {
                    let component_mask = match self.target_mode {
                        1 => self.stft.tonal_mask[ch][k],
                        2 => self.stft.transient_mask[ch][k],
                        _ => 1.0,
                    };
                    target_gr *= component_mask;
                }
                self.stft.detector_gr[ch][k] = target_gr;
            }
            self.stft.adaptive_initialized[ch] |= use_adaptive;

            if self.target_mode > 0 {
                self.stft.tonal_transient[ch].process(
                    &self.stft.spectral_magnitudes[ch][..num_bins],
                    &mut self.stft.tonal_mask[ch][..num_bins],
                    &mut self.stft.transient_mask[ch][..num_bins],
                );
            }
        }

        // Blend independent gain reduction toward the maximum across channels.
        // Linking detector values (rather than audio) preserves phase and layout.
        if self.channel_link > 0.0 && channels > 1 {
            for k in 0..num_bins {
                let linked = self
                    .stft
                    .detector_gr
                    .iter()
                    .map(|channel| channel[k])
                    .fold(0.0_f32, f32::max);
                for ch in 0..channels {
                    let independent = self.stft.detector_gr[ch][k];
                    self.stft.detector_gr[ch][k] =
                        independent + self.channel_link * (linked - independent);
                }
            }
        }

        // Envelope, frequency smoothing, gain, and synthesis phase.
        for ch in 0..channels {
            for k in 0..num_bins {
                let target_gr = self.stft.detector_gr[ch][k];
                let envelope = &mut self.stft.bin_envelopes[ch][k];
                let coeff = if target_gr > *envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                *envelope = target_gr + coeff * (*envelope - target_gr);
            }

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

            if spectral_smoothing > 0.001 {
                smooth_spectral_envelope(
                    &mut self.stft.bin_envelopes[ch],
                    spectral_smoothing,
                    &mut self.stft.smoothing_prefix,
                );
            }

            for k in 0..num_bins {
                let envelope_db = self.stft.bin_envelopes[ch][k];
                if envelope_db > 0.001 {
                    let gain_linear = 10.0_f32.powf(-envelope_db / 20.0);
                    self.stft.fft_processors[ch].freq_buffer[k] *= gain_linear;
                }
            }

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
        self.stft.output_accumulator_fill += hop_size;
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "fft_size",
                "FFT Size",
                self.fft_size_index as i32,
                0,
                (FFT_SIZE_OPTIONS.len() - 1) as i32,
            )
            .with_update_mode(UpdateMode::Structural)
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
            Parameter::new_int(
                "target_mode",
                "Target",
                self.target_mode as i32,
                0,
                (TARGET_MODES.len() - 1) as i32,
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
            Parameter::new_float("channel_link", "Channel Link", self.channel_link, 0.0, 1.0)
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    #[inline]
    pub(super) fn mix_output_sample(dry: f32, wet: f32, mix: f32, delta_enabled: bool) -> f32 {
        let mixed = dry * (1.0 - mix) + wet * mix;
        if delta_enabled { mixed - dry } else { mixed }
    }

    /// Backward-compatible parameter list accessor.
    pub fn parameters(&self) -> Vec<Parameter> {
        self.parameter_schema()
    }

    /// Backward-compatible single-parameter getter.
    pub fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.current_values().get(id).cloned()
    }

    /// Backward-compatible single-parameter setter.
    pub fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.parametric_set_parameter(id, value)
    }

    fn apply_parameter(&mut self, id: &ParameterId, value: ParameterValue) -> PluginResult<()> {
        match id.as_str() {
            "fft_size" => {
                let idx = value
                    .as_int()
                    .ok_or_else(|| "FFT size must be an integer".to_string())?
                    as usize;
                if idx != self.fft_size_index {
                    return Err("fft_size is structural and requires a host rebuild".into());
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
                self.target_mode = value
                    .as_int()
                    .ok_or_else(|| "Target mode must be a choice index".to_string())?
                    as usize;
            }
            "delta_listen" => {
                let enabled = value
                    .as_bool()
                    .ok_or_else(|| "Delta listen must be boolean".to_string())?;
                self.delta_monitor.set_enabled(enabled);
            }
            "adaptive_threshold" => {
                let enabled = value
                    .as_bool()
                    .ok_or_else(|| "Adaptive threshold must be boolean".to_string())?;
                if enabled && !self.adaptive_threshold {
                    self.stft.adaptive_initialized.fill(false);
                }
                self.adaptive_threshold = enabled;
            }
            "adaptive_offset_db" => {
                self.adaptive_offset_db = value
                    .as_float()
                    .ok_or_else(|| "Adaptive offset must be a float".to_string())?;
            }
            "channel_link" => {
                self.channel_link = value
                    .as_float()
                    .ok_or_else(|| "Channel link must be a float".to_string())?;
            }
            other => return Err(format!("Unknown parameter: {other}")),
        }
        Ok(())
    }
}

impl ParametricInPlacePlugin for SpectralCompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Spectral Compressor", env!("CARGO_PKG_VERSION"), "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Fft
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Fft, None, self.latency_samples(), false)
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
            ParameterId::from("fft_size"),
            ParameterValue::Int(self.fft_size_index as i32),
        );
        values.insert(
            ParameterId::from("threshold"),
            ParameterValue::Float(self.threshold_db),
        );
        values.insert(
            ParameterId::from("ratio"),
            ParameterValue::Float(self.ratio),
        );
        values.insert(
            ParameterId::from("attack"),
            ParameterValue::Float(self.attack_ms),
        );
        values.insert(
            ParameterId::from("release"),
            ParameterValue::Float(self.release_ms),
        );
        values.insert(
            ParameterId::from("knee"),
            ParameterValue::Float(self.knee_db),
        );
        values.insert(
            ParameterId::from("spectral_smoothing"),
            ParameterValue::Float(self.spectral_smoothing),
        );
        values.insert(ParameterId::from("mix"), ParameterValue::Float(self.mix));
        values.insert(
            ParameterId::from("target_mode"),
            ParameterValue::Int(self.target_mode as i32),
        );
        values.insert(
            ParameterId::from("delta_listen"),
            ParameterValue::Bool(self.delta_monitor.enabled()),
        );
        values.insert(
            ParameterId::from("adaptive_threshold"),
            ParameterValue::Bool(self.adaptive_threshold),
        );
        values.insert(
            ParameterId::from("adaptive_offset_db"),
            ParameterValue::Float(self.adaptive_offset_db),
        );
        values.insert(
            ParameterId::from("channel_link"),
            ParameterValue::Float(self.channel_link),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            self.apply_parameter(&id, value)?;
        }
        Ok(())
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        self.cached_parameters
            .iter()
            .find(|parameter| &parameter.id == id)
            .ok_or_else(|| format!("Unknown parameter: {id}"))?
            .validate(value)
            .map_err(|error| format!("{id}: {error}"))
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.parametric_validate_parameter(&id, &value)?;
        self.apply_parameter(&id, value)
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
        // The output scheduler emits exactly one FFT frame of leading silence,
        // independent of host block partitioning.
        self.fft_size
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
        if nf > MAX_BLOCK_FRAMES {
            return Err(format!(
                "Spectral compressor block size {nf} exceeds max {MAX_BLOCK_FRAMES} frames"
            ));
        }
        for sample in buffer.iter_mut() {
            if !sample.is_finite() {
                *sample = 0.0;
            }
        }
        self.block_input[..total].copy_from_slice(buffer);

        let delta_enabled = self.delta_monitor.enabled();

        let mut input_pos = 0; // frame index into the caller's buffer
        let mut output_pos = 0; // frame index into the caller's output

        let hop_size = self.stft.hop_size;

        while input_pos < nf || output_pos < nf {
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
                        let dst_idx = self.stft.input_write_pos;
                        for ch in 0..channels {
                            self.stft.input_buffers[ch][dst_idx] = self.block_input[src_base + ch];
                        }
                        self.stft.input_write_pos =
                            (self.stft.input_write_pos + 1) & (fft_size - 1);
                    }
                    self.stft.input_fill += to_copy;
                    input_pos += to_copy;
                }
            }

            // --- Step 2: Process STFT frames while we have a full window ---
            if self.stft.input_fill >= fft_size {
                self.process_spectral_hop();
                // Circular history already retains the overlap; only the
                // logical fill count moves back by one hop.
                let overlap = fft_size - hop_size;
                self.stft.input_fill = overlap;
            }

            // --- Step 3: Emit fixed leading latency, then drain OLA output ---
            let startup_remaining = fft_size.saturating_sub(self.stft.latency_filled);
            let startup_frames = startup_remaining.min(nf - output_pos);
            if startup_frames > 0 {
                for i in 0..startup_frames {
                    let out_base = (output_pos + i) * channels;
                    let g_mix = self.mix_smoother.advance();
                    for ch in 0..channels {
                        let idx = out_base + ch;
                        let dry_pos = self.stft.dry_delay_pos + ch;
                        let dry = self.stft.dry_delay_buf[dry_pos];
                        self.stft.dry_delay_buf[dry_pos] = self.block_input[idx];
                        buffer[idx] = Self::mix_output_sample(dry, 0.0, g_mix, delta_enabled);
                    }
                    self.stft.dry_delay_pos += channels;
                    if self.stft.dry_delay_pos >= self.stft.dry_delay_buf.len() {
                        self.stft.dry_delay_pos = 0;
                    }
                }
                self.stft.latency_filled += startup_frames;
                output_pos += startup_frames;
                if output_pos >= nf {
                    continue;
                }
            }

            let frames_to_drain = self.stft.output_accumulator_fill.min(nf - output_pos);
            if frames_to_drain > 0 {
                let mask = self.stft.output_accumulator_mask;
                for i in 0..frames_to_drain {
                    let read_idx = (self.stft.output_read_position + i) & mask;
                    let out_base = (output_pos + i) * channels;
                    let g_mix = self.mix_smoother.advance();
                    for ch in 0..channels {
                        let idx = out_base + ch;
                        let dry_pos = self.stft.dry_delay_pos + ch;
                        let dry = self.stft.dry_delay_buf[dry_pos];
                        self.stft.dry_delay_buf[dry_pos] = self.block_input[idx];
                        let wet = self.stft.output_accumulator[read_idx * channels + ch];
                        buffer[idx] = Self::mix_output_sample(dry, wet, g_mix, delta_enabled);
                    }
                    self.stft.dry_delay_pos += channels;
                    if self.stft.dry_delay_pos >= self.stft.dry_delay_buf.len() {
                        self.stft.dry_delay_pos = 0;
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
                    let g_mix = self.mix_smoother.advance();
                    for ch in 0..channels {
                        let idx = out_base + ch;
                        let dry_pos = self.stft.dry_delay_pos + ch;
                        let dry = self.stft.dry_delay_buf[dry_pos];
                        self.stft.dry_delay_buf[dry_pos] = self.block_input[idx];
                        buffer[idx] = Self::mix_output_sample(dry, 0.0, g_mix, delta_enabled);
                    }
                    self.stft.dry_delay_pos += channels;
                    if self.stft.dry_delay_pos >= self.stft.dry_delay_buf.len() {
                        self.stft.dry_delay_pos = 0;
                    }
                }
                output_pos = nf;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}
