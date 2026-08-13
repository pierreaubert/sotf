use super::crossover_kind::CrossoverKind;
use super::crossover_mode::CrossoverMode;
use super::parse::parse_channel_freq_id;
use super::parse::parse_channel_mode_id;
use super::per_channel_op_mode::PerChannelOpMode;
use super::types::CrossoverPluginParams;
use sotf_host::fir_crossover::{DEFAULT_FIR_CROSSOVER_TAPS, FirCrossover, MultibandFirCrossover};
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::LogSmoother;

/// Delays the early branches of a cascaded FIR crossover so every emitted
/// band has the same cumulative group delay as the final branch.
struct FirBandAlignment {
    delay_lines: Vec<Vec<f32>>,
    write_positions: Vec<usize>,
    delay_frames: Vec<usize>,
    channels: usize,
}

impl FirBandAlignment {
    fn new(num_bands: usize, channels: usize, split_latency: usize) -> Self {
        let num_splits = num_bands.saturating_sub(1);
        let delay_frames: Vec<usize> = (0..num_bands)
            .map(|band| {
                let intrinsic_splits = (band + 1).min(num_splits);
                (num_splits - intrinsic_splits) * split_latency
            })
            .collect();
        let delay_lines = delay_frames
            .iter()
            .map(|&frames| vec![0.0; frames * channels])
            .collect();
        Self {
            delay_lines,
            write_positions: vec![0; num_bands],
            delay_frames,
            channels,
        }
    }

    fn process_frame(&mut self, bands: &mut [f32]) {
        for band in 0..self.delay_frames.len() {
            let frames = self.delay_frames[band];
            if frames == 0 {
                continue;
            }
            let band_offset = band * self.channels;
            let delay_offset = self.write_positions[band] * self.channels;
            for channel in 0..self.channels {
                std::mem::swap(
                    &mut self.delay_lines[band][delay_offset + channel],
                    &mut bands[band_offset + channel],
                );
            }
            self.write_positions[band] = (self.write_positions[band] + 1) % frames;
        }
    }

    fn reset(&mut self) {
        for delay in &mut self.delay_lines {
            delay.fill(0.0);
        }
        self.write_positions.fill(0);
    }
}

pub struct CrossoverPlugin {
    pub(super) num_channels: usize,
    pub(super) sample_rate: u32,
    /// Set once the host has compiled/initialized this instance. Structural
    /// FIR parameters are configuration-only after this point.
    initialized: bool,
    pub(super) mode: CrossoverMode,
    pub(super) kind: CrossoverKind,
    pub(super) fir_taps: usize,
    pub(super) cached_parameters: Vec<Parameter>,

    /// Single crossover for 2-way operation
    pub(super) crossover_2way: Lr4Crossover<f32>,
    pub(super) fir_crossover_2way: Option<FirCrossover<f32>>,
    pub(super) freq_smoother: LogSmoother,

    /// Multi-band crossover for 3-way and 4-way operation.
    /// None when in 2-way mode.
    /// Phase-coherent multiway bank. Every band traverses every split as
    /// either LP or HP, so recombination is the product of LR all-pass sums.
    pub(super) multiband: Option<Vec<Vec<Lr4Crossover<f32>>>>,
    pub(super) fir_multiband: Option<MultibandFirCrossover<f32>>,
    fir_band_alignment: Option<FirBandAlignment>,
    pub(super) extra_freq_smoothers: Vec<LogSmoother>,
    /// Position in the persistent 16-sample coefficient update cadence.
    /// This must not restart at callback boundaries.
    smoother_subblock_phase: usize,

    /// Sorted crossover frequencies for multi-way mode (including primary).
    pub(super) all_frequencies: Vec<f32>,

    /// Pre-allocated scratch buffers
    pub(super) low_buf: Vec<f32>,
    pub(super) high_buf: Vec<f32>,
    /// Flat buffer for multi-way band outputs: [band0_ch0..band0_chN, band1_ch0..band1_chN, ...]
    pub(super) band_flat: Vec<f32>,

    /// When non-empty, the plugin runs in per-channel mode: each channel is
    /// processed by its own single-channel LR24 crossover and the per-channel
    /// `op_modes` array decides what each channel outputs.
    pub(super) channel_frequencies_hz: Vec<f32>,
    pub(super) op_modes: Vec<PerChannelOpMode>,
    pub(super) per_channel_lr4: Vec<Lr4Crossover<f32>>,
    /// Per-channel scratch buffers for the 1-sample-wide low/high outputs.
    pub(super) per_channel_low: Vec<f32>,
    pub(super) per_channel_high: Vec<f32>,
}

impl CrossoverPlugin {
    pub fn new(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
    ) -> Result<Self, String> {
        Self::new_multiway(num_channels, crossover_type, frequency, output, &[])
    }

    pub fn new_multiway(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
        extra_frequencies: &[f64],
    ) -> Result<Self, String> {
        Self::new_multiway_with_fir_taps(
            num_channels,
            crossover_type,
            frequency,
            output,
            extra_frequencies,
            DEFAULT_FIR_CROSSOVER_TAPS,
        )
    }

    pub(super) fn new_multiway_with_fir_taps(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
        extra_frequencies: &[f64],
        fir_taps: usize,
    ) -> Result<Self, String> {
        if num_channels == 0 {
            return Err("crossover requires at least one channel".into());
        }
        if extra_frequencies.len() > 2 {
            return Err("crossover supports at most four bands (three crossover points)".into());
        }
        if !(31..=16385).contains(&fir_taps) {
            return Err(format!("fir_taps must be in [31, 16385], got {fir_taps}"));
        }
        let kind = CrossoverKind::parse(crossover_type)?;
        let mode = CrossoverMode::from_str(output)?;
        let sr = 48000;
        let fir_taps = if fir_taps.is_multiple_of(2) {
            fir_taps
                .checked_add(1)
                .ok_or_else(|| "fir_taps overflow".to_string())?
        } else {
            fir_taps
        };

        let mut all_freqs: Vec<f32> = vec![frequency as f32];
        for &f in extra_frequencies {
            all_freqs.push(f as f32);
        }
        let nyquist_limit = sr as f32 * 0.5 * 0.99;
        if all_freqs
            .iter()
            .any(|f| !f.is_finite() || *f <= 0.0 || *f >= nyquist_limit)
        {
            return Err(format!(
                "crossover frequencies must be finite and in (0, {nyquist_limit}) Hz"
            ));
        }
        all_freqs.sort_by(|a, b| a.total_cmp(b));
        if all_freqs.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("crossover frequencies must be unique".into());
        }

        let num_bands = all_freqs.len() + 1;

        let (multiband, extra_smoothers) = if all_freqs.len() > 1 {
            let mb = (0..num_bands)
                .map(|_| {
                    all_freqs
                        .iter()
                        .map(|&frequency| Lr4Crossover::new(frequency, sr as f32, num_channels))
                        .collect()
                })
                .collect();
            let smoothers: Vec<LogSmoother> = all_freqs
                .iter()
                .skip(1) // first freq uses the primary smoother
                .map(|&f| LogSmoother::new(f, 20.0, sr))
                .collect();
            (Some(mb), smoothers)
        } else {
            (None, Vec::new())
        };
        let fir_crossover_2way = (kind == CrossoverKind::LinearPhase)
            .then(|| FirCrossover::new(frequency as f32, sr as f32, num_channels, fir_taps));
        let fir_multiband = (kind == CrossoverKind::LinearPhase && all_freqs.len() > 1)
            .then(|| MultibandFirCrossover::new(&all_freqs, sr as f32, num_channels, fir_taps));
        let fir_band_alignment = (kind == CrossoverKind::LinearPhase && all_freqs.len() > 1)
            .then(|| FirBandAlignment::new(num_bands, num_channels, (fir_taps - 1) / 2));

        let mut p = Self {
            num_channels,
            sample_rate: sr,
            initialized: false,
            mode,
            kind,
            fir_taps,
            crossover_2way: Lr4Crossover::new(frequency as f32, sr as f32, num_channels),
            fir_crossover_2way,
            freq_smoother: LogSmoother::new(frequency as f32, 20.0, sr),
            multiband,
            fir_multiband,
            fir_band_alignment,
            extra_freq_smoothers: extra_smoothers,
            smoother_subblock_phase: 0,
            all_frequencies: all_freqs,
            cached_parameters: Vec::new(),
            low_buf: vec![0.0; num_channels],
            high_buf: vec![0.0; num_channels],
            band_flat: vec![0.0; num_bands * num_channels],
            channel_frequencies_hz: Vec::new(),
            op_modes: Vec::new(),
            per_channel_lr4: Vec::new(),
            per_channel_low: Vec::new(),
            per_channel_high: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Build a crossover plugin in per-channel mode: each channel gets its
    /// own LR24 crossover with its own cutoff frequency and operation mode
    /// (lowpass / highpass / mute). The plugin remains 1-input / 1-output
    /// per channel; output channel count equals input channel count.
    ///
    /// Used by the RoomEQ factored graph to encode all per-channel HP or LP
    /// route filters in a single multichannel node.
    pub fn new_per_channel(
        crossover_type: &str,
        channel_frequencies_hz: Vec<f32>,
        channel_modes: Vec<PerChannelOpMode>,
    ) -> Result<Self, String> {
        if channel_frequencies_hz.is_empty() {
            return Err("channel_frequencies_hz must not be empty".into());
        }
        if channel_frequencies_hz.len() != channel_modes.len() {
            return Err(format!(
                "channel_frequencies_hz.len()={} but channel_modes.len()={}",
                channel_frequencies_hz.len(),
                channel_modes.len()
            ));
        }
        let nyquist_limit = 48_000.0 * 0.5 * 0.99;
        if channel_frequencies_hz
            .iter()
            .any(|f| !f.is_finite() || *f <= 0.0 || *f >= nyquist_limit)
        {
            return Err(format!(
                "channel crossover frequencies must be finite and in (0, {nyquist_limit}) Hz"
            ));
        }
        let kind = CrossoverKind::parse(crossover_type)?;
        if kind != CrossoverKind::Lr24 {
            return Err(format!(
                "per-channel crossover currently only supports LR24, got {crossover_type}"
            ));
        }
        let num_channels = channel_frequencies_hz.len();
        let sr = 48000u32;
        let per_channel_lr4: Vec<Lr4Crossover<f32>> = channel_frequencies_hz
            .iter()
            .map(|&f| Lr4Crossover::new(f, sr as f32, 1))
            .collect();
        // The shared/global crossover and smoothers remain populated but
        // unused in per-channel mode; sized minimally to avoid surprises.
        let primary_freq = channel_frequencies_hz[0];
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            initialized: false,
            mode: CrossoverMode::Lowpass,
            kind,
            fir_taps: DEFAULT_FIR_CROSSOVER_TAPS,
            crossover_2way: Lr4Crossover::new(primary_freq, sr as f32, num_channels),
            fir_crossover_2way: None,
            freq_smoother: LogSmoother::new(primary_freq, 20.0, sr),
            multiband: None,
            fir_multiband: None,
            fir_band_alignment: None,
            extra_freq_smoothers: Vec::new(),
            smoother_subblock_phase: 0,
            all_frequencies: vec![primary_freq],
            cached_parameters: Vec::new(),
            low_buf: vec![0.0; num_channels],
            high_buf: vec![0.0; num_channels],
            band_flat: vec![0.0; num_channels],
            channel_frequencies_hz,
            op_modes: channel_modes,
            per_channel_lr4,
            per_channel_low: vec![0.0; 1],
            per_channel_high: vec![0.0; 1],
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// True when the plugin is configured with independent per-channel cutoffs.
    pub fn is_per_channel(&self) -> bool {
        !self.channel_frequencies_hz.is_empty()
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![Parameter::new_string(
            "type",
            "Type",
            self.kind.as_str().to_string(),
        )];
        if !self.is_per_channel() {
            params.extend([
                Parameter::new_float(
                    "frequency",
                    "Frequency",
                    self.freq_smoother.target(),
                    20.0,
                    20000.0,
                ),
                Parameter::new_string("mode", "Mode", self.mode.as_str().to_string()),
            ]);
        }

        // Add extra frequency parameters for multi-way
        if !self.is_per_channel() {
            for (i, smoother) in self.extra_freq_smoothers.iter().enumerate() {
                params.push(Parameter::new_float(
                    &format!("frequency_{}", i + 2),
                    &format!("Frequency {}", i + 2),
                    smoother.target(),
                    20.0,
                    20000.0,
                ));
            }
            if self.kind == CrossoverKind::LinearPhase {
                params.push(Parameter::new_int(
                    "fir_taps",
                    "FIR Taps",
                    self.fir_taps as i32,
                    31,
                    16385,
                ));
            }
        }

        if self.is_per_channel() {
            for (ch, &freq) in self.channel_frequencies_hz.iter().enumerate() {
                let id = format!("channel_frequency_{ch}");
                let name = format!("Frequency Ch{ch}");
                params.push(Parameter::new_float(&id, &name, freq, 20.0, 20000.0).with_unit("Hz"));
                let mode_id = format!("channel_mode_{ch}");
                let mode_name = format!("Mode Ch{ch}");
                let mode_str = match self
                    .op_modes
                    .get(ch)
                    .copied()
                    .unwrap_or(PerChannelOpMode::Mute)
                {
                    PerChannelOpMode::Lowpass => "lowpass",
                    PerChannelOpMode::Highpass => "highpass",
                    PerChannelOpMode::Mute => "mute",
                    PerChannelOpMode::Passthrough => "passthrough",
                };
                params.push(Parameter::new_string(
                    &mode_id,
                    &mode_name,
                    mode_str.to_string(),
                ));
            }
        }

        self.cached_parameters = params;
    }

    pub(super) fn rebuild_fir_crossovers(&mut self) {
        if self.kind != CrossoverKind::LinearPhase {
            return;
        }
        let sr = self.sample_rate as f32;
        let primary = self
            .all_frequencies
            .first()
            .copied()
            .unwrap_or_else(|| self.freq_smoother.target());
        self.fir_crossover_2way = Some(FirCrossover::new(
            primary,
            sr,
            self.num_channels,
            self.fir_taps,
        ));
        self.fir_multiband = (self.all_frequencies.len() > 1).then(|| {
            MultibandFirCrossover::new(&self.all_frequencies, sr, self.num_channels, self.fir_taps)
        });
        self.fir_band_alignment = (self.all_frequencies.len() > 1).then(|| {
            FirBandAlignment::new(
                self.all_frequencies.len() + 1,
                self.num_channels,
                (self.fir_taps - 1) / 2,
            )
        });
    }

    pub fn from_params(
        num_channels: usize,
        params: &CrossoverPluginParams,
    ) -> Result<Self, String> {
        if !params.channel_frequencies_hz.is_empty() {
            // Per-channel mode: parse channel_modes, falling back to the
            // scalar `output` mode (lowpass/highpass) when channel_modes is
            // missing or shorter than channel_frequencies_hz.
            let default_mode = PerChannelOpMode::from_str(&params.output)?;
            let mut modes = Vec::with_capacity(params.channel_frequencies_hz.len());
            for i in 0..params.channel_frequencies_hz.len() {
                if let Some(s) = params.channel_modes.get(i) {
                    modes.push(PerChannelOpMode::from_str(s)?);
                } else {
                    modes.push(default_mode);
                }
            }
            let expected = params.channel_frequencies_hz.len();
            if expected != num_channels {
                return Err(format!(
                    "CrossoverPlugin::from_params: channels arg ({num_channels}) does not match channel_frequencies_hz.len() ({expected})"
                ));
            }
            return Self::new_per_channel(
                &params.crossover_type,
                params.channel_frequencies_hz.clone(),
                modes,
            );
        }
        Self::new_multiway_with_fir_taps(
            num_channels,
            &params.crossover_type,
            params.frequency,
            &params.output,
            &params.extra_frequencies,
            params.fir_taps.unwrap_or(DEFAULT_FIR_CROSSOVER_TAPS),
        )
    }

    /// Number of output bands based on current configuration.
    pub(super) fn num_bands(&self) -> usize {
        self.all_frequencies.len() + 1
    }

    /// Calculate output channels based on mode and band count.
    pub(super) fn calc_output_channels(&self) -> usize {
        if self.is_per_channel() {
            return self.num_channels;
        }
        match self.mode {
            CrossoverMode::Lowpass | CrossoverMode::Highpass => self.num_channels,
            CrossoverMode::Both => self.num_channels * self.num_bands(),
        }
    }

    /// Returns true if operating in multi-way (3+ bands) mode.
    pub(super) fn is_multiway(&self) -> bool {
        self.multiband.is_some()
    }

    /// Parse "frequency_N" into an extra smoother index (0-based).
    /// "frequency_2" -> Some(0), "frequency_3" -> Some(1), etc.
    /// Returns None for indices < 2 to prevent aliasing "frequency_1" onto index 0.
    pub(super) fn parse_extra_freq_index(s: &str) -> Option<usize> {
        s.strip_prefix("frequency_")
            .and_then(|idx_str| idx_str.parse::<usize>().ok())
            .and_then(|idx| if idx >= 2 { Some(idx - 2) } else { None })
    }
}

impl Plugin for CrossoverPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Crossover", env!("CARGO_PKG_VERSION"), "SotF").with_description(
            "Linkwitz-Riley and linear-phase FIR crossover with multi-way and dual-output support",
        )
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.calc_output_channels()
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let latency_samples = self.latency_samples();
        let cost = if self.kind == CrossoverKind::LinearPhase {
            PluginCostClass::Convolution
        } else {
            PluginCostClass::Iir
        };
        let mut metadata = PluginCompileMetadata::linear_transform(
            cost,
            None,
            latency_samples,
            false,
            true,
            false,
        );
        // FIR instances carry explicit latency and large convolution state;
        // keep them as scheduling boundaries. LR instances are ordinary
        // zero-latency linear transforms and can participate in fusion.
        metadata.boundary = self.kind == CrossoverKind::LinearPhase;
        metadata
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // FIR coefficients (and, for multi-way FIR, the complete set of
        // convolution histories) are compile-time state. Rebuilding them from
        // the control path after initialization would allocate, reset audio
        // history, and potentially change the graph's declared latency.
        if self.initialized
            && self.kind == CrossoverKind::LinearPhase
            && (id.as_str() == "frequency"
                || id.as_str() == "fir_taps"
                || Self::parse_extra_freq_index(&id.0).is_some())
        {
            return Err(format!(
                "crossover '{}' is a structural FIR parameter; rebuild the graph to change it",
                id.0
            ));
        }

        // In per-channel mode, the global `frequency` and `mode` parameters
        // don't apply — every channel has its own. Reject these writes so
        // they don't silently mutate unused global state.
        if self.is_per_channel() && (id.as_str() == "frequency" || id.as_str() == "mode") {
            return Err(format!(
                "crossover '{}' is in per-channel mode; use 'channel_frequency_N' / 'channel_mode_N' instead",
                id.0
            ));
        }

        if self.initialized
            && (parse_channel_freq_id(&id.0).is_some() || parse_channel_mode_id(&id.0).is_some())
        {
            return Err(format!(
                "crossover '{}' is a structural per-channel parameter; rebuild the graph to change it",
                id.0
            ));
        }

        self.validate_parameter(&id, &value)?;

        if id.as_str() == "type" {
            Err("crossover type is structural; rebuild the graph to change it".into())
        } else if id.as_str() == "frequency" {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() {
                if self.all_frequencies.get(1).is_some_and(|next| val >= *next) {
                    return Err("frequency must remain below frequency_2".into());
                }
                self.freq_smoother.set_target(val);
                // Update first frequency in multi-way list and re-sort to maintain
                // sorted order. MultibandLr4Crossover requires sorted frequencies.
                if !self.all_frequencies.is_empty() {
                    self.all_frequencies[0] = val;
                    self.rebuild_fir_crossovers();
                }
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if id.as_str() == "mode" {
            if let Some(s) = value.as_string() {
                let new_mode = CrossoverMode::from_str(s)?;
                let changes_layout = matches!(self.mode, CrossoverMode::Both)
                    != matches!(new_mode, CrossoverMode::Both);
                if changes_layout {
                    return Err(
                        "crossover mode changes that alter output channels require graph rebuild"
                            .into(),
                    );
                }
                self.mode = new_mode;
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() && smoother_idx < self.extra_freq_smoothers.len() {
                let freq_idx = smoother_idx + 1; // offset: extra smoothers start at freq index 1
                if freq_idx < self.all_frequencies.len() {
                    let lower = self.all_frequencies[freq_idx - 1];
                    let upper = self.all_frequencies.get(freq_idx + 1).copied();
                    if val <= lower || upper.is_some_and(|upper| val >= upper) {
                        return Err(format!(
                            "frequency_{} must remain between its neighboring crossover points",
                            smoother_idx + 2
                        ));
                    }
                    self.extra_freq_smoothers[smoother_idx].set_target(val);
                    self.all_frequencies[freq_idx] = val;
                    self.rebuild_fir_crossovers();
                }
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if id.as_str() == "fir_taps" {
            let taps = value.as_int().unwrap_or(DEFAULT_FIR_CROSSOVER_TAPS as i32);
            self.fir_taps = (taps.max(31) as usize).min(16385);
            if self.fir_taps.is_multiple_of(2) {
                self.fir_taps += 1;
            }
            self.rebuild_fir_crossovers();
            self.rebuild_cached_parameters();
            Ok(())
        } else if let Some(ch) = parse_channel_freq_id(&id.0) {
            if !self.is_per_channel() || ch >= self.channel_frequencies_hz.len() {
                return Err(format!("invalid per-channel frequency id: {}", id.0));
            }
            let val = value
                .as_float()
                .ok_or_else(|| "channel frequency must be a float".to_string())?;
            let nyquist_limit = self.sample_rate as f32 * 0.5 * 0.99;
            if !val.is_finite() || val <= 0.0 || val >= nyquist_limit {
                return Err(format!(
                    "channel frequency must be finite and in (0, {nyquist_limit}) Hz"
                ));
            }
            self.channel_frequencies_hz[ch] = val;
            self.per_channel_lr4[ch] = Lr4Crossover::new(val, self.sample_rate as f32, 1);
            self.rebuild_cached_parameters();
            Ok(())
        } else if let Some(ch) = parse_channel_mode_id(&id.0) {
            if !self.is_per_channel() || ch >= self.op_modes.len() {
                return Err(format!("invalid per-channel mode id: {}", id.0));
            }
            let s = value
                .as_string()
                .ok_or_else(|| "channel mode must be a string".to_string())?;
            self.op_modes[ch] = PerChannelOpMode::from_str(s)?;
            self.rebuild_cached_parameters();
            Ok(())
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "type" {
            Some(ParameterValue::String(self.kind.as_str().to_string()))
        } else if id.as_str() == "frequency" {
            Some(ParameterValue::Float(self.freq_smoother.target()))
        } else if id.as_str() == "mode" {
            Some(ParameterValue::String(self.mode.as_str().to_string()))
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            self.extra_freq_smoothers
                .get(smoother_idx)
                .map(|s| ParameterValue::Float(s.target()))
        } else if id.as_str() == "fir_taps" && self.kind == CrossoverKind::LinearPhase {
            Some(ParameterValue::Int(self.fir_taps as i32))
        } else if let Some(ch) = parse_channel_freq_id(&id.0) {
            self.channel_frequencies_hz
                .get(ch)
                .copied()
                .map(ParameterValue::Float)
        } else if let Some(ch) = parse_channel_mode_id(&id.0) {
            self.op_modes.get(ch).map(|m| {
                ParameterValue::String(
                    match m {
                        PerChannelOpMode::Lowpass => "lowpass",
                        PerChannelOpMode::Highpass => "highpass",
                        PerChannelOpMode::Mute => "mute",
                        PerChannelOpMode::Passthrough => "passthrough",
                    }
                    .to_string(),
                )
            })
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("crossover sample rate must be greater than zero".into());
        }
        let nyquist_limit = sample_rate as f32 * 0.5 * 0.99;
        if self
            .all_frequencies
            .iter()
            .chain(self.channel_frequencies_hz.iter())
            .any(|f| !f.is_finite() || *f <= 0.0 || *f >= nyquist_limit)
        {
            return Err(format!(
                "crossover frequency exceeds sample-rate limit {nyquist_limit} Hz"
            ));
        }
        self.sample_rate = sample_rate;
        // Clamp all frequencies to just below Nyquist to prevent nonsense biquad
        // coefficients at low sample rates (e.g. 32 kHz with a 20 kHz crossover).
        let nyquist_limit = sample_rate as f32 * 0.5 * 0.99;

        if self.is_per_channel() {
            // Mutate the stored values so `get_parameter` and serialization
            // reflect the clamped reality (otherwise the plugin reports a
            // frequency it isn't actually running at).
            for freq in self.channel_frequencies_hz.iter_mut() {
                *freq = freq.min(nyquist_limit);
            }
            self.per_channel_lr4 = self
                .channel_frequencies_hz
                .iter()
                .map(|&f| {
                    let clamped = f.min(nyquist_limit);
                    Lr4Crossover::new(clamped, sample_rate as f32, 1)
                })
                .collect();
            self.per_channel_low.resize(1, 0.0);
            self.per_channel_high.resize(1, 0.0);
            self.initialized = true;
            return Ok(());
        }

        let clamped_primary = self.freq_smoother.target().min(nyquist_limit);
        self.freq_smoother = LogSmoother::new(clamped_primary, 20.0, sample_rate);
        self.crossover_2way
            .reinit(clamped_primary, sample_rate as f32, self.num_channels);
        self.low_buf.resize(self.num_channels, 0.0);
        self.high_buf.resize(self.num_channels, 0.0);

        if let Some(ref mut banks) = self.multiband {
            // extra_freq_smoothers[i] corresponds to all_frequencies[i+1].
            for (freq, smoother) in self
                .all_frequencies
                .iter_mut()
                .skip(1)
                .zip(self.extra_freq_smoothers.iter_mut())
            {
                let clamped = smoother.target().min(nyquist_limit);
                *freq = clamped;
                *smoother = LogSmoother::new(clamped, 20.0, sample_rate);
            }
            // Clamp all_frequencies[0] (primary, already clamped above).
            if !self.all_frequencies.is_empty() {
                self.all_frequencies[0] = clamped_primary;
            }
            for bank in banks {
                for (split, crossover) in bank.iter_mut().enumerate() {
                    crossover.reinit(
                        self.all_frequencies[split],
                        sample_rate as f32,
                        self.num_channels,
                    );
                }
            }
        }
        self.rebuild_fir_crossovers();

        // Resize band flat buffer
        let nb = self.num_bands();
        self.band_flat.resize(nb * self.num_channels, 0.0);

        self.initialized = true;

        Ok(())
    }

    fn reset(&mut self) {
        if self.is_per_channel() {
            for xo in &mut self.per_channel_lr4 {
                xo.reset();
            }
            return;
        }
        self.crossover_2way.reset();
        if let Some(ref mut xover) = self.fir_crossover_2way {
            xover.reset();
        }
        // Reset smoothers to their targets so that a mid-transition reset does not
        // cause a click from the remaining interpolation step on the next block.
        self.freq_smoother.reset(self.freq_smoother.target());
        self.smoother_subblock_phase = 0;
        for s in &mut self.extra_freq_smoothers {
            s.reset(s.target());
        }
        if let Some(ref mut banks) = self.multiband {
            for bank in banks {
                for crossover in bank {
                    crossover.reset();
                }
            }
        }
        if let Some(ref mut mb) = self.fir_multiband {
            mb.reset();
        }
        if let Some(alignment) = &mut self.fir_band_alignment {
            alignment.reset();
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
        let in_ch = self.num_channels;
        let out_ch = self.output_channels();
        let expected_input = num_frames
            .checked_mul(in_ch)
            .ok_or_else(|| "crossover input size overflow".to_string())?;
        let expected_output = num_frames
            .checked_mul(out_ch)
            .ok_or_else(|| "crossover output size overflow".to_string())?;
        if input.len() != expected_input || output.len() != expected_output {
            return Err(format!(
                "crossover buffer mismatch: input {} (expected {expected_input}), output {} (expected {expected_output})",
                input.len(),
                output.len()
            ));
        }

        if self.is_per_channel() {
            // Per-channel mode: each channel is processed independently by
            // its own single-channel LR24 crossover. Output channel count
            // equals input channel count.
            let mut low_scratch = [0.0f32];
            let mut high_scratch = [0.0f32];
            for frame in 0..num_frames {
                let in_off = frame * in_ch;
                let out_off = frame * in_ch;
                for ch in 0..in_ch {
                    let sample = input[in_off + ch];
                    match self.op_modes[ch] {
                        PerChannelOpMode::Mute => {
                            output[out_off + ch] = 0.0;
                        }
                        PerChannelOpMode::Passthrough => {
                            output[out_off + ch] = sample;
                        }
                        mode => {
                            let sample_arr = [sample];
                            self.per_channel_lr4[ch].process_frame(
                                &sample_arr,
                                &mut low_scratch,
                                &mut high_scratch,
                            );
                            output[out_off + ch] = match mode {
                                PerChannelOpMode::Lowpass => low_scratch[0],
                                PerChannelOpMode::Highpass => high_scratch[0],
                                // Mute and Passthrough handled above; matching
                                // here keeps the match exhaustive.
                                PerChannelOpMode::Mute => 0.0,
                                PerChannelOpMode::Passthrough => sample,
                            };
                        }
                    }
                }
            }
            flush_denormals_inplace(output);
            return Ok(num_frames);
        }

        if self.kind == CrossoverKind::LinearPhase {
            if self.is_multiway() {
                let num_bands = self.num_bands();
                let mb = self.fir_multiband.as_mut().unwrap();

                for frame in 0..num_frames {
                    let in_off = frame * in_ch;
                    let out_off = frame * out_ch;
                    let frame_slice = &input[in_off..in_off + in_ch];

                    {
                        let flat = &mut self.band_flat[..num_bands * in_ch];
                        let mut band_slices: [&mut [f32]; 4] = [&mut [], &mut [], &mut [], &mut []];
                        let mut remaining = flat;
                        for slot in band_slices.iter_mut().take(num_bands) {
                            let (chunk, rest) = remaining.split_at_mut(in_ch);
                            *slot = chunk;
                            remaining = rest;
                        }
                        mb.process_frame(frame_slice, &mut band_slices[..num_bands]);
                    }
                    if let Some(alignment) = &mut self.fir_band_alignment {
                        alignment.process_frame(&mut self.band_flat[..num_bands * in_ch]);
                    }

                    match self.mode {
                        CrossoverMode::Lowpass => {
                            output[out_off..out_off + in_ch]
                                .copy_from_slice(&self.band_flat[..in_ch]);
                        }
                        CrossoverMode::Highpass => {
                            let hi_off = (num_bands - 1) * in_ch;
                            output[out_off..out_off + in_ch]
                                .copy_from_slice(&self.band_flat[hi_off..hi_off + in_ch]);
                        }
                        CrossoverMode::Both => {
                            output[out_off..out_off + out_ch]
                                .copy_from_slice(&self.band_flat[..out_ch]);
                        }
                    }
                }
            } else {
                let xover = self.fir_crossover_2way.as_mut().unwrap();
                for frame in 0..num_frames {
                    let in_off = frame * in_ch;
                    let out_off = frame * out_ch;
                    let frame_slice = &input[in_off..in_off + in_ch];

                    xover.process_frame(frame_slice, &mut self.low_buf, &mut self.high_buf);

                    match self.mode {
                        CrossoverMode::Lowpass => {
                            output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                        }
                        CrossoverMode::Highpass => {
                            output[out_off..out_off + in_ch].copy_from_slice(&self.high_buf);
                        }
                        CrossoverMode::Both => {
                            output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                            output[out_off + in_ch..out_off + 2 * in_ch]
                                .copy_from_slice(&self.high_buf);
                        }
                    }
                }
            }
        } else if self.is_multiway() {
            // Multi-way processing
            let num_bands = self.num_bands();
            let banks = self.multiband.as_mut().unwrap();

            // Sub-block size for frequency updates: every 16 samples to avoid
            // zipper noise while keeping CPU cost reasonable.
            const SUBBLOCK: usize = 16;

            for frame in 0..num_frames {
                if self.smoother_subblock_phase == 0 {
                    let new_freq0 = self.freq_smoother.next_n(SUBBLOCK);
                    for bank in banks.iter_mut() {
                        bank[0].set_frequency(new_freq0);
                    }
                    for (i, smoother) in self.extra_freq_smoothers.iter_mut().enumerate() {
                        let f = smoother.next_n(SUBBLOCK);
                        for bank in banks.iter_mut() {
                            bank[i + 1].set_frequency(f);
                        }
                    }
                }
                self.smoother_subblock_phase = (self.smoother_subblock_phase + 1) % SUBBLOCK;
                let in_off = frame * in_ch;
                let out_off = frame * out_ch;
                let frame_slice = &input[in_off..in_off + in_ch];

                for (band, bank) in banks.iter_mut().enumerate() {
                    let band_offset = band * in_ch;
                    self.band_flat[band_offset..band_offset + in_ch].copy_from_slice(frame_slice);
                    for (split, crossover) in bank.iter_mut().enumerate() {
                        crossover.process_frame(
                            &self.band_flat[band_offset..band_offset + in_ch],
                            &mut self.low_buf,
                            &mut self.high_buf,
                        );
                        let filtered = if band <= split {
                            &self.low_buf
                        } else {
                            &self.high_buf
                        };
                        self.band_flat[band_offset..band_offset + in_ch].copy_from_slice(filtered);
                    }
                }

                match self.mode {
                    CrossoverMode::Lowpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.band_flat[..in_ch]);
                    }
                    CrossoverMode::Highpass => {
                        let hi_off = (num_bands - 1) * in_ch;
                        output[out_off..out_off + in_ch]
                            .copy_from_slice(&self.band_flat[hi_off..hi_off + in_ch]);
                    }
                    CrossoverMode::Both => {
                        output[out_off..out_off + out_ch]
                            .copy_from_slice(&self.band_flat[..out_ch]);
                    }
                }
            }
        } else {
            // 2-way processing
            const SUBBLOCK: usize = 16;

            for frame in 0..num_frames {
                if self.smoother_subblock_phase == 0 {
                    let new_freq = self.freq_smoother.next_n(SUBBLOCK);
                    self.crossover_2way.set_frequency(new_freq);
                }
                self.smoother_subblock_phase = (self.smoother_subblock_phase + 1) % SUBBLOCK;
                let in_off = frame * in_ch;
                let out_off = frame * out_ch;
                let frame_slice = &input[in_off..in_off + in_ch];

                self.crossover_2way.process_frame(
                    frame_slice,
                    &mut self.low_buf,
                    &mut self.high_buf,
                );

                match self.mode {
                    CrossoverMode::Lowpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                    }
                    CrossoverMode::Highpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.high_buf);
                    }
                    CrossoverMode::Both => {
                        // Low band first, then high band
                        output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                        output[out_off + in_ch..out_off + 2 * in_ch]
                            .copy_from_slice(&self.high_buf);
                    }
                }
            }
        }

        flush_denormals_inplace(output);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        if self.kind != CrossoverKind::LinearPhase {
            return 0;
        }
        self.fir_multiband
            .as_ref()
            .map(|mb| mb.latency_samples())
            .or_else(|| {
                self.fir_crossover_2way
                    .as_ref()
                    .map(|xo| xo.latency_samples())
            })
            .unwrap_or(0)
    }
}
