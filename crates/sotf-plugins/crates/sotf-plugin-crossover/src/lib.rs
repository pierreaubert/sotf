// ============================================================================
// Crossover Plugin
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::fir_crossover::{DEFAULT_FIR_CROSSOVER_TAPS, FirCrossover, MultibandFirCrossover};
use sotf_host::lr4_crossover::{Lr4Crossover, MultibandLr4Crossover};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::LogSmoother;

/// Crossover output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrossoverMode {
    Lowpass,
    Highpass,
    Both,
}

impl CrossoverMode {
    fn from_str(s: &str) -> Result<Self, String> {
        if s.eq_ignore_ascii_case("low")
            || s.eq_ignore_ascii_case("lowpass")
            || s.eq_ignore_ascii_case("lp")
        {
            Ok(CrossoverMode::Lowpass)
        } else if s.eq_ignore_ascii_case("high")
            || s.eq_ignore_ascii_case("highpass")
            || s.eq_ignore_ascii_case("hp")
        {
            Ok(CrossoverMode::Highpass)
        } else if s.eq_ignore_ascii_case("both") {
            Ok(CrossoverMode::Both)
        } else {
            Err(format!("Invalid output mode: {}", s))
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            CrossoverMode::Lowpass => "lowpass",
            CrossoverMode::Highpass => "highpass",
            CrossoverMode::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverPluginParams {
    #[serde(rename = "type")]
    pub crossover_type: String,
    pub frequency: f64,
    pub output: String,
    /// Additional crossover frequencies for 3-way or 4-way mode.
    /// When provided, creates a multi-way crossover. The primary `frequency`
    /// becomes the first crossover point.
    #[serde(default)]
    pub extra_frequencies: Vec<f64>,
    /// FIR taps for linear-phase crossover mode. Even values are rounded up.
    #[serde(default)]
    pub fir_taps: Option<usize>,
    /// Per-channel crossover frequencies in Hz. When non-empty, switches the
    /// plugin into per-channel mode (one independent LR24 crossover per
    /// channel) and the scalar `frequency` / `output` fields are ignored.
    #[serde(default)]
    pub channel_frequencies_hz: Vec<f32>,
    /// Per-channel mode for each channel in per-channel mode: "lowpass",
    /// "highpass", or "mute" (channel outputs silence). Must match
    /// `channel_frequencies_hz.len()` when both are non-empty.
    #[serde(default)]
    pub channel_modes: Vec<String>,
}

/// Per-channel operation mode used when the crossover runs in per-channel
/// mode. Separate from the global `CrossoverMode` (which always describes a
/// uniform output across all channels) so the existing global processing
/// paths stay untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerChannelOpMode {
    Lowpass,
    Highpass,
    /// Output silence on this channel.
    Mute,
    /// Output the input unchanged on this channel (no filtering, no
    /// smoothing state). Used by destination-only channels in the RoomEQ
    /// factored graph so signals arriving on a sub channel reach the
    /// post-EQ stage without being filtered out.
    Passthrough,
}

impl PerChannelOpMode {
    fn from_str(s: &str) -> Result<Self, String> {
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "low" | "lowpass" | "lp" => Ok(Self::Lowpass),
            "high" | "highpass" | "hp" => Ok(Self::Highpass),
            "mute" | "off" | "silence" => Ok(Self::Mute),
            "passthrough" | "bypass" | "pass" => Ok(Self::Passthrough),
            other => Err(format!(
                "Invalid per-channel crossover mode: '{other}'. Expected lowpass/highpass/mute/passthrough."
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrossoverKind {
    Lr24,
    LinearPhase,
}

impl CrossoverKind {
    fn parse(crossover_type: &str) -> Result<Self, String> {
        if crossover_type.eq_ignore_ascii_case("lr24") || crossover_type.eq_ignore_ascii_case("lr4")
        {
            Ok(Self::Lr24)
        } else if is_linear_phase_type(crossover_type) {
            Ok(Self::LinearPhase)
        } else {
            Err(format!(
                "Unsupported crossover type: '{}'. Supported: LR24/LR4 and LinearPhase/FIR.",
                crossover_type
            ))
        }
    }
}

fn is_linear_phase_type(crossover_type: &str) -> bool {
    matches!(
        crossover_type.to_ascii_lowercase().as_str(),
        "linearphase" | "linear_phase" | "linear-phase" | "linearphasefir" | "fir" | "lpfir"
    )
}

/// Parse a per-channel frequency parameter id of the form `channel_frequency_{N}`.
/// Returns the channel index, or None if the id does not match.
fn parse_channel_freq_id(id: &str) -> Option<usize> {
    id.strip_prefix("channel_frequency_")
        .and_then(|tail| tail.parse::<usize>().ok())
}

/// Parse a per-channel mode parameter id of the form `channel_mode_{N}`.
fn parse_channel_mode_id(id: &str) -> Option<usize> {
    id.strip_prefix("channel_mode_")
        .and_then(|tail| tail.parse::<usize>().ok())
}

pub struct CrossoverPlugin {
    num_channels: usize,
    sample_rate: u32,
    mode: CrossoverMode,
    kind: CrossoverKind,
    fir_taps: usize,
    cached_parameters: Vec<Parameter>,

    /// Single crossover for 2-way operation
    crossover_2way: Lr4Crossover<f32>,
    fir_crossover_2way: Option<FirCrossover<f32>>,
    freq_smoother: LogSmoother,

    /// Multi-band crossover for 3-way and 4-way operation.
    /// None when in 2-way mode.
    multiband: Option<MultibandLr4Crossover<f32>>,
    fir_multiband: Option<MultibandFirCrossover<f32>>,
    extra_freq_smoothers: Vec<LogSmoother>,

    /// Sorted crossover frequencies for multi-way mode (including primary).
    all_frequencies: Vec<f32>,

    /// Pre-allocated scratch buffers
    low_buf: Vec<f32>,
    high_buf: Vec<f32>,
    /// Flat buffer for multi-way band outputs: [band0_ch0..band0_chN, band1_ch0..band1_chN, ...]
    band_flat: Vec<f32>,

    /// When non-empty, the plugin runs in per-channel mode: each channel is
    /// processed by its own single-channel LR24 crossover and the per-channel
    /// `op_modes` array decides what each channel outputs.
    channel_frequencies_hz: Vec<f32>,
    op_modes: Vec<PerChannelOpMode>,
    per_channel_lr4: Vec<Lr4Crossover<f32>>,
    /// Per-channel scratch buffers for the 1-sample-wide low/high outputs.
    per_channel_low: Vec<f32>,
    per_channel_high: Vec<f32>,
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

    fn new_multiway_with_fir_taps(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
        extra_frequencies: &[f64],
        fir_taps: usize,
    ) -> Result<Self, String> {
        let kind = CrossoverKind::parse(crossover_type)?;
        let mode = CrossoverMode::from_str(output)?;
        let sr = 48000;
        let fir_taps = if fir_taps.is_multiple_of(2) {
            fir_taps + 1
        } else {
            fir_taps
        };

        let mut all_freqs: Vec<f32> = vec![frequency as f32];
        for &f in extra_frequencies {
            all_freqs.push(f as f32);
        }
        all_freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_freqs.dedup();

        let num_bands = all_freqs.len() + 1;

        let (multiband, extra_smoothers) = if all_freqs.len() > 1 {
            let mb = MultibandLr4Crossover::new(&all_freqs, sr as f32, num_channels);
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

        let mut p = Self {
            num_channels,
            sample_rate: sr,
            mode,
            kind,
            fir_taps,
            crossover_2way: Lr4Crossover::new(frequency as f32, sr as f32, num_channels),
            fir_crossover_2way,
            freq_smoother: LogSmoother::new(frequency as f32, 20.0, sr),
            multiband,
            fir_multiband,
            extra_freq_smoothers: extra_smoothers,
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
            mode: CrossoverMode::Lowpass,
            kind,
            fir_taps: DEFAULT_FIR_CROSSOVER_TAPS,
            crossover_2way: Lr4Crossover::new(primary_freq, sr as f32, num_channels),
            fir_crossover_2way: None,
            freq_smoother: LogSmoother::new(primary_freq, 20.0, sr),
            multiband: None,
            fir_multiband: None,
            extra_freq_smoothers: Vec::new(),
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

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_float(
                "frequency",
                "Frequency",
                self.freq_smoother.target(),
                20.0,
                20000.0,
            ),
            Parameter::new_string("mode", "Mode", self.mode.as_str().to_string()),
        ];

        // Add extra frequency parameters for multi-way
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

    fn rebuild_fir_crossovers(&mut self) {
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
        Self::new_multiway(
            num_channels,
            &params.crossover_type,
            params.frequency,
            &params.output,
            &params.extra_frequencies,
        )
        .map(|mut plugin| {
            if let Some(taps) = params.fir_taps {
                plugin.fir_taps = if taps.is_multiple_of(2) {
                    taps + 1
                } else {
                    taps
                };
                plugin.rebuild_fir_crossovers();
                plugin.rebuild_cached_parameters();
            }
            plugin
        })
    }

    /// Number of output bands based on current configuration.
    fn num_bands(&self) -> usize {
        self.all_frequencies.len() + 1
    }

    /// Calculate output channels based on mode and band count.
    fn calc_output_channels(&self) -> usize {
        if self.is_per_channel() {
            return self.num_channels;
        }
        match self.mode {
            CrossoverMode::Lowpass | CrossoverMode::Highpass => self.num_channels,
            CrossoverMode::Both => self.num_channels * self.num_bands(),
        }
    }

    /// Returns true if operating in multi-way (3+ bands) mode.
    fn is_multiway(&self) -> bool {
        self.multiband.is_some()
    }

    /// Parse "frequency_N" into an extra smoother index (0-based).
    /// "frequency_2" -> Some(0), "frequency_3" -> Some(1), etc.
    /// Returns None for indices < 2 to prevent aliasing "frequency_1" onto index 0.
    fn parse_extra_freq_index(s: &str) -> Option<usize> {
        s.strip_prefix("frequency_")
            .and_then(|idx_str| idx_str.parse::<usize>().ok())
            .and_then(|idx| if idx >= 2 { Some(idx - 2) } else { None })
    }
}

impl Plugin for CrossoverPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Crossover", "3.0.0", "SotF").with_description(
            "Linkwitz-Riley and linear-phase FIR crossover with multi-way and dual-output support",
        )
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.calc_output_channels()
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        // In per-channel mode, the global `frequency` and `mode` parameters
        // don't apply — every channel has its own. Reject these writes so
        // they don't silently mutate unused global state.
        if self.is_per_channel() && (id.0 == "frequency" || id.0 == "mode") {
            return Err(format!(
                "crossover '{}' is in per-channel mode; use 'channel_frequency_N' / 'channel_mode_N' instead",
                id.0
            ));
        }

        if id.0 == "frequency" {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() {
                self.freq_smoother.set_target(val);
                // Update first frequency in multi-way list and re-sort to maintain
                // sorted order. MultibandLr4Crossover requires sorted frequencies.
                if !self.all_frequencies.is_empty() {
                    self.all_frequencies[0] = val;
                    self.all_frequencies
                        .sort_by(|a, b| a.partial_cmp(b).unwrap());
                    self.all_frequencies.dedup();
                }
                self.rebuild_fir_crossovers();
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if id.0 == "mode" {
            if let Some(s) = value.as_string() {
                self.mode = CrossoverMode::from_str(s)?;
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() && smoother_idx < self.extra_freq_smoothers.len() {
                self.extra_freq_smoothers[smoother_idx].set_target(val);
                let freq_idx = smoother_idx + 1; // offset: extra smoothers start at freq index 1
                if freq_idx < self.all_frequencies.len() {
                    self.all_frequencies[freq_idx] = val;
                    self.all_frequencies
                        .sort_by(|a, b| a.partial_cmp(b).unwrap());
                    self.all_frequencies.dedup();
                }
                self.rebuild_fir_crossovers();
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if id.0 == "fir_taps" {
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
            if val.is_finite() && val > 0.0 {
                let nyquist_limit = self.sample_rate as f32 * 0.5 * 0.99;
                let clamped = val.min(nyquist_limit);
                self.channel_frequencies_hz[ch] = clamped;
                self.per_channel_lr4[ch] = Lr4Crossover::new(clamped, self.sample_rate as f32, 1);
                self.rebuild_cached_parameters();
            }
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
        if id.0 == "frequency" {
            Some(ParameterValue::Float(self.freq_smoother.target()))
        } else if id.0 == "mode" {
            Some(ParameterValue::String(self.mode.as_str().to_string()))
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            self.extra_freq_smoothers
                .get(smoother_idx)
                .map(|s| ParameterValue::Float(s.target()))
        } else if id.0 == "fir_taps" && self.kind == CrossoverKind::LinearPhase {
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
            return Ok(());
        }

        let clamped_primary = self.freq_smoother.target().min(nyquist_limit);
        self.freq_smoother = LogSmoother::new(clamped_primary, 20.0, sample_rate);
        self.crossover_2way
            .reinit(clamped_primary, sample_rate as f32, self.num_channels);
        self.low_buf.resize(self.num_channels, 0.0);
        self.high_buf.resize(self.num_channels, 0.0);

        if let Some(ref mut mb) = self.multiband {
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
            let clamped_freqs: Vec<f32> = self
                .all_frequencies
                .iter()
                .map(|&f| f.min(nyquist_limit))
                .collect();
            mb.reinit(&clamped_freqs, sample_rate as f32, self.num_channels);
        }
        self.rebuild_fir_crossovers();

        // Resize band flat buffer
        let nb = self.num_bands();
        self.band_flat.resize(nb * self.num_channels, 0.0);

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
        for s in &mut self.extra_freq_smoothers {
            s.reset(s.target());
        }
        if let Some(ref mut mb) = self.multiband {
            mb.reset();
        }
        if let Some(ref mut mb) = self.fir_multiband {
            mb.reset();
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
        debug_assert_eq!(
            input.len(),
            num_frames * in_ch,
            "Input buffer size mismatch"
        );
        debug_assert_eq!(
            output.len(),
            num_frames * out_ch,
            "Output buffer size mismatch"
        );

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
            let mb = self.multiband.as_mut().unwrap();

            // Sub-block size for frequency updates: every 16 samples to avoid
            // zipper noise while keeping CPU cost reasonable.
            const SUBBLOCK: usize = 16;

            for frame in 0..num_frames {
                if frame % SUBBLOCK == 0 {
                    let new_freq0 = self.freq_smoother.next_n(SUBBLOCK.min(num_frames - frame));
                    mb.set_frequency(0, new_freq0);
                    for (i, smoother) in self.extra_freq_smoothers.iter_mut().enumerate() {
                        let f = smoother.next_n(SUBBLOCK.min(num_frames - frame));
                        mb.set_frequency(i + 1, f);
                    }
                }
                let in_off = frame * in_ch;
                let out_off = frame * out_ch;
                let frame_slice = &input[in_off..in_off + in_ch];

                // Build mutable slice references into band_flat using split_at_mut
                // to satisfy the borrow checker without unsafe.
                {
                    let flat = &mut self.band_flat[..num_bands * in_ch];
                    // Use fixed-size array to avoid per-frame heap allocation.
                    // Crossover supports up to 4 bands (3 crossover points).
                    let mut band_slices: [&mut [f32]; 4] = [&mut [], &mut [], &mut [], &mut []];
                    let mut remaining = flat;
                    for slot in band_slices.iter_mut().take(num_bands) {
                        let (chunk, rest) = remaining.split_at_mut(in_ch);
                        *slot = chunk;
                        remaining = rest;
                    }
                    mb.process_frame(frame_slice, &mut band_slices[..num_bands]);
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
                if frame % SUBBLOCK == 0 {
                    let new_freq = self.freq_smoother.next_n(SUBBLOCK.min(num_frames - frame));
                    self.crossover_2way.set_frequency(new_freq);
                }
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

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_crossover_basic() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 1000];
        let mut output = vec![0.0; 1000];
        p.process(&input, &mut output, &ProcessContext::new(48000, 1000))
            .unwrap();
        assert!(output[999].is_finite());
    }

    #[test]
    fn test_crossover_highpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 1000];
        let mut output = vec![0.0; 1000];
        p.process(&input, &mut output, &ProcessContext::new(48000, 1000))
            .unwrap();
        assert!(output[999].is_finite());
    }

    #[test]
    fn test_linear_phase_crossover_reconstructs_delayed_input() {
        let mut p = CrossoverPlugin::new(1, "LinearPhase", 1000.0, "both").unwrap();
        p.set_parameter(ParameterId::from("fir_taps"), ParameterValue::Int(127))
            .unwrap();
        p.initialize(48000).unwrap();
        let latency = p.latency_samples();
        let frames = 512;
        let input: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0; frames * 2];
        p.process(&input, &mut output, &ProcessContext::new(48000, frames))
            .unwrap();

        let mut max_error = 0.0f32;
        for i in (latency + 16)..frames {
            let reconstructed = output[i * 2] + output[i * 2 + 1];
            max_error = max_error.max((reconstructed - input[i - latency]).abs());
        }
        assert!(
            max_error < 0.02,
            "linear-phase bands should reconstruct delayed input, max_error={max_error}"
        );
    }

    #[test]
    fn test_crossover_stereo() {
        let mut p = CrossoverPlugin::new(2, "LR24", 500.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![0.5; 200];
        let mut output = vec![0.0; 200];
        p.process(&input, &mut output, &ProcessContext::new(48000, 100))
            .unwrap();
        assert!(output[0].is_finite());
        assert!(output[199].is_finite());
    }

    #[test]
    fn test_crossover_dc_passes_lowpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 10000];
        let mut output = vec![0.0; 10000];
        p.process(&input, &mut output, &ProcessContext::new(48000, 10000))
            .unwrap();
        assert!(
            output[9999] > 0.9,
            "DC through lowpass should be near 1.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_dc_rejected_highpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 10000];
        let mut output = vec![0.0; 10000];
        p.process(&input, &mut output, &ProcessContext::new(48000, 10000))
            .unwrap();
        assert!(
            output[9999].abs() < 0.1,
            "DC through highpass should be near 0.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_invalid_output() {
        let result = CrossoverPlugin::new(1, "LR24", 1000.0, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_crossover_both_mode_doubles_channels() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 2); // 1 channel * 2 bands

        // Process DC: low band should have the signal, high should be ~0
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames * 2]; // 2 output channels
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // Last frame: output[idx*2] = low, output[idx*2+1] = high
        let last = (num_frames - 1) * 2;
        assert!(
            output[last] > 0.9,
            "DC low band should be near 1.0, got {}",
            output[last]
        );
        assert!(
            output[last + 1].abs() < 0.1,
            "DC high band should be near 0.0, got {}",
            output[last + 1]
        );
    }

    #[test]
    fn test_crossover_both_bands_sum_preserves_energy() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();

        // Feed a signal and verify low + high sum has comparable energy to input.
        // LR4 crossovers sum to flat magnitude but introduce group delay,
        // so per-sample comparison with undelayed input is not valid.
        // Instead, verify RMS energy is preserved.
        let num_frames = 10000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.05).sin()).collect();
        let mut output = vec![0.0f32; num_frames * 2];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // Compare RMS of input vs RMS of (low+high) over the settled region.
        // Use at least 5000 samples for settle to ensure the filter has fully settled.
        let settle = 5000;
        let input_rms: f32 = (input[settle..].iter().map(|s| s * s).sum::<f32>()
            / (num_frames - settle) as f32)
            .sqrt();

        let sum_rms: f32 = ((settle..num_frames)
            .map(|f| {
                let s = output[f * 2] + output[f * 2 + 1];
                s * s
            })
            .sum::<f32>()
            / (num_frames - settle) as f32)
            .sqrt();

        let ratio = sum_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "RMS ratio should be near 1.0 (flat sum), got {}",
            ratio
        );
    }

    #[test]
    fn test_crossover_stereo_both_mode() {
        let mut p = CrossoverPlugin::new(2, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 2);
        assert_eq!(p.output_channels(), 4); // 2 channels * 2 bands

        let num_frames = 100;
        let input = vec![0.5f32; num_frames * 2];
        let mut output = vec![0.0f32; num_frames * 4];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        // All outputs should be finite
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_crossover_3way() {
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 3); // 1 channel * 3 bands

        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0f32; num_frames * 3];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // DC should pass through lowest band only
        let last = (num_frames - 1) * 3;
        assert!(
            output[last] > 0.9,
            "3-way DC band 0 (low) should be near 1.0, got {}",
            output[last]
        );
        assert!(
            output[last + 1].abs() < 0.1,
            "3-way DC band 1 (mid) should be near 0.0, got {}",
            output[last + 1]
        );
        assert!(
            output[last + 2].abs() < 0.1,
            "3-way DC band 2 (high) should be near 0.0, got {}",
            output[last + 2]
        );
    }

    #[test]
    fn test_crossover_4way() {
        let mut p =
            CrossoverPlugin::new_multiway(1, "LR24", 200.0, "both", &[1000.0, 5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 4); // 1 channel * 4 bands

        let num_frames = 1000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0f32; num_frames * 4];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // All outputs should be finite
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_crossover_3way_lowpass_mode() {
        // In lowpass mode, 3-way should output only the lowest band
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "low", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 1); // Only lowest band

        let num_frames = 10000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // DC passes through lowpass
        assert!(
            output[9999] > 0.9,
            "3-way lowpass DC should be near 1.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_output_selection_highpass_rejects_dc() {
        // Highpass mode should reject DC (output near zero)
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0; num_frames];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        assert!(
            output[num_frames - 1].abs() < 0.05,
            "Highpass should reject DC, got {}",
            output[num_frames - 1]
        );
    }

    #[test]
    fn test_crossover_output_selection_lowpass_passes_dc() {
        // Lowpass mode should pass DC (output near 1.0)
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0; num_frames];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        assert!(
            output[num_frames - 1] > 0.95,
            "Lowpass should pass DC, got {}",
            output[num_frames - 1]
        );
    }

    #[test]
    fn test_crossover_mode_parameter() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 1);

        p.set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("both".to_string()),
        )
        .unwrap();
        assert_eq!(p.output_channels(), 2);

        let val = p.get_parameter(&ParameterId::from("mode"));
        assert_eq!(val, Some(ParameterValue::String("both".to_string())));
    }

    /// Changing frequency_2 on a 3-way crossover should not panic and should
    /// continue producing finite output.
    #[test]
    fn test_3way_frequency_update_no_panic() {
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 3); // 3 bands

        let num_frames = 2000;
        let ctx = ProcessContext::new(48000, num_frames);

        // Process a block before parameter change
        let input: Vec<f32> = (0..num_frames)
            .map(|i| 0.3 * (i as f32 * 0.1).sin())
            .collect();
        let mut output = vec![0.0f32; num_frames * 3];
        p.process(&input, &mut output, &ctx).unwrap();

        // Change frequency_2 (the second crossover point)
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(8000.0),
        )
        .unwrap();

        // Verify the parameter was accepted
        let val = p.get_parameter(&ParameterId::from("frequency_2"));
        assert_eq!(val, Some(ParameterValue::Float(8000.0)));

        // Process another block after the change -- must not panic
        let input2: Vec<f32> = (0..num_frames)
            .map(|i| 0.3 * ((num_frames + i) as f32 * 0.1).sin())
            .collect();
        let mut output2 = vec![0.0f32; num_frames * 3];
        p.process(&input2, &mut output2, &ctx).unwrap();

        // All output must be finite
        assert!(
            output2.iter().all(|s| s.is_finite()),
            "All output samples must be finite after frequency_2 change"
        );

        // At least some output should be non-zero
        let has_signal = output2.iter().any(|s| s.abs() > 1e-6);
        assert!(
            has_signal,
            "Output should contain non-zero samples after frequency change"
        );
    }

    // ── New tests for review fixes ─────────────────────────────────────────

    /// §2.1: Setting 'frequency' to a value larger than the second crossover
    /// point must not leave all_frequencies unsorted.
    #[test]
    fn test_all_frequencies_remain_sorted_after_primary_update() {
        // 3-way: [500, 5000]
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();

        // Move primary frequency above the second point — without the fix this
        // would leave all_frequencies = [10000, 5000] (unsorted).
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(10000.0),
        )
        .unwrap();

        // Verify the vector is still in ascending order.
        let freqs = p.all_frequencies.clone();
        let mut sorted = freqs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            freqs, sorted,
            "all_frequencies must remain sorted after primary frequency change; got {:?}",
            freqs
        );

        // Plugin must still produce finite output.
        let num_frames = 1000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let num_bands = p.num_bands();
        let mut output = vec![0.0f32; num_frames * num_bands];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        assert!(output.iter().all(|s| s.is_finite()));
    }

    /// §2.1: Setting 'frequency_2' to a value smaller than 'frequency' must
    /// also maintain sorted order.
    #[test]
    fn test_all_frequencies_remain_sorted_after_extra_freq_update() {
        // 3-way: [500, 5000]
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();

        // Move frequency_2 below the primary — without the fix this would leave
        // all_frequencies = [500, 200] (unsorted).
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(200.0),
        )
        .unwrap();

        let freqs = p.all_frequencies.clone();
        let mut sorted = freqs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            freqs, sorted,
            "all_frequencies must remain sorted after frequency_2 change; got {:?}",
            freqs
        );
    }

    /// §2.2: "frequency_1" must NOT be parsed as a valid extra-freq parameter.
    #[test]
    fn test_parse_extra_freq_index_rejects_idx_less_than_2() {
        // "frequency_1" should return None — it is not a valid parameter.
        assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_1"), None);
        assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_0"), None);
        // "frequency_2" must still map to smoother index 0.
        assert_eq!(
            CrossoverPlugin::parse_extra_freq_index("frequency_2"),
            Some(0)
        );
        // "frequency_3" must map to smoother index 1.
        assert_eq!(
            CrossoverPlugin::parse_extra_freq_index("frequency_3"),
            Some(1)
        );
    }

    /// §4.1: Unsupported crossover type strings must return an error.
    #[test]
    fn test_unsupported_crossover_type_returns_error() {
        let result = CrossoverPlugin::new(1, "LR12", 1000.0, "low");
        assert!(
            result.is_err(),
            "LR12 crossover type must be rejected with an error"
        );
        let result2 = CrossoverPlugin::new(1, "BW18", 1000.0, "low");
        assert!(
            result2.is_err(),
            "BW18 crossover type must be rejected with an error"
        );
        // Case-insensitive acceptance of the supported types.
        assert!(CrossoverPlugin::new(1, "lr24", 1000.0, "low").is_ok());
        assert!(CrossoverPlugin::new(1, "LR4", 1000.0, "low").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "low").is_ok());
    }

    /// §4.2: CrossoverMode::from_str must be case-insensitive (no allocation path).
    #[test]
    fn test_crossover_mode_from_str_is_case_insensitive() {
        assert_eq!(CrossoverMode::from_str("LOW"), Ok(CrossoverMode::Lowpass));
        assert_eq!(
            CrossoverMode::from_str("Lowpass"),
            Ok(CrossoverMode::Lowpass)
        );
        assert_eq!(CrossoverMode::from_str("HP"), Ok(CrossoverMode::Highpass));
        assert_eq!(CrossoverMode::from_str("BOTH"), Ok(CrossoverMode::Both));
    }

    /// §4.3: reset() must snap smoothers to their targets to avoid a
    /// click on the next block when a parameter was mid-transition.
    #[test]
    fn test_reset_snaps_smoothers_to_target() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();

        // Start a slow parameter transition (20 ms @ 48 kHz = ~960 samples to converge).
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(5000.0),
        )
        .unwrap();

        // Process only a few samples so the smoother is mid-transition.
        let input = vec![0.0f32; 16];
        let mut output = vec![0.0f32; 16];
        p.process(&input, &mut output, &ProcessContext::new(48000, 16))
            .unwrap();

        // Reset must snap the smoother current to target.
        p.reset();
        let current = p.freq_smoother.current();
        let target = p.freq_smoother.target();
        assert_eq!(
            current, target,
            "After reset(), smoother current ({}) must equal target ({})",
            current, target
        );
    }

    /// §1.4: initialize() at a low sample rate must not produce NaN/Inf even
    /// when the stored frequency exceeds Nyquist.
    #[test]
    fn test_initialize_clamps_frequency_to_nyquist() {
        // At 32 kHz, Nyquist is 16 kHz. A crossover at 20 kHz exceeds it.
        let mut p = CrossoverPlugin::new(1, "LR24", 20000.0, "low").unwrap();
        p.initialize(32000).unwrap();

        // The effective frequency must be below Nyquist.
        let effective = p.freq_smoother.target();
        assert!(
            effective < 16000.0,
            "Frequency must be clamped below Nyquist (16 kHz) at 32 kHz sample rate, got {}",
            effective
        );

        // Output must be finite.
        let num_frames = 1000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];
        p.process(&input, &mut output, &ProcessContext::new(32000, num_frames))
            .unwrap();
        assert!(
            output.iter().all(|s| s.is_finite()),
            "Output must be finite after initialize at low sample rate"
        );
    }

    // =========================================================================
    // Per-channel mode tests
    // =========================================================================

    #[test]
    fn test_per_channel_construction_and_output_shape() {
        let p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![80.0, 100.0, 120.0],
            vec![
                PerChannelOpMode::Highpass,
                PerChannelOpMode::Lowpass,
                PerChannelOpMode::Mute,
            ],
        )
        .unwrap();
        assert!(p.is_per_channel());
        assert_eq!(p.input_channels(), 3);
        assert_eq!(p.output_channels(), 3);
    }

    #[test]
    fn test_per_channel_mute_outputs_silence() {
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![1000.0, 1000.0],
            vec![PerChannelOpMode::Highpass, PerChannelOpMode::Mute],
        )
        .unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 512;
        let mut input = vec![0.0f32; num_frames * 2];
        for f in 0..num_frames {
            input[f * 2] = 1.0; // ch0: DC
            input[f * 2 + 1] = 1.0; // ch1: DC (muted)
        }
        let mut output = vec![0.0; num_frames * 2];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        // Muted channel must be exactly silence.
        for f in 0..num_frames {
            assert_eq!(output[f * 2 + 1], 0.0, "muted channel must be zero");
        }
    }

    #[test]
    fn test_per_channel_independent_cutoffs() {
        // Two channels with different cutoffs and different modes: ch0 LP@200,
        // ch1 HP@5000. Drive both with white noise; ch0 should preserve LF
        // energy, ch1 should preserve HF energy.
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![200.0, 5000.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
        )
        .unwrap();
        let sr = 48000u32;
        p.initialize(sr).unwrap();

        let num_frames = 8192;
        let mut input = vec![0.0f32; num_frames * 2];
        // 100 Hz tone on ch0 (below LP cutoff, should pass)
        // 100 Hz tone on ch1 (below HP cutoff, should be attenuated)
        for f in 0..num_frames {
            let t = f as f32 / sr as f32;
            let lf = (2.0 * std::f32::consts::PI * 100.0 * t).sin();
            input[f * 2] = lf;
            input[f * 2 + 1] = lf;
        }
        let mut output = vec![0.0f32; num_frames * 2];
        p.process(&input, &mut output, &ProcessContext::new(sr, num_frames))
            .unwrap();

        // Skip transient: measure RMS on the tail half.
        let tail = num_frames / 2;
        let mut rms_ch0 = 0.0;
        let mut rms_ch1 = 0.0;
        for f in tail..num_frames {
            rms_ch0 += output[f * 2] * output[f * 2];
            rms_ch1 += output[f * 2 + 1] * output[f * 2 + 1];
        }
        rms_ch0 = (rms_ch0 / (num_frames - tail) as f32).sqrt();
        rms_ch1 = (rms_ch1 / (num_frames - tail) as f32).sqrt();
        // ch0 LP@200 sees a 100 Hz tone in passband → ~0.707
        assert!(
            rms_ch0 > 0.5,
            "ch0 LP@200 should pass 100Hz tone (rms={rms_ch0})"
        );
        // ch1 HP@5000 sees a 100 Hz tone deep in stopband → ~0
        assert!(
            rms_ch1 < 0.05,
            "ch1 HP@5000 should reject 100Hz tone (rms={rms_ch1})"
        );
    }

    #[test]
    fn test_per_channel_passthrough_preserves_input() {
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![1000.0, 1000.0],
            vec![PerChannelOpMode::Highpass, PerChannelOpMode::Passthrough],
        )
        .unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 256;
        let mut input = vec![0.0f32; num_frames * 2];
        for f in 0..num_frames {
            input[f * 2] = 0.5;
            input[f * 2 + 1] = 0.5;
        }
        let mut output = vec![0.0; num_frames * 2];
        p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();
        for f in 0..num_frames {
            // ch1 (Passthrough) must be exactly the input — bit-for-bit.
            assert_eq!(
                output[f * 2 + 1],
                input[f * 2 + 1],
                "passthrough channel must be bitwise identical to input at frame {f}"
            );
        }
    }

    #[test]
    fn test_per_channel_set_get_frequency_and_mode() {
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![100.0, 200.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
        )
        .unwrap();
        p.initialize(48000).unwrap();
        // Update channel 0 frequency.
        p.set_parameter(
            ParameterId::from("channel_frequency_0"),
            ParameterValue::Float(250.0),
        )
        .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("channel_frequency_0"))
            .unwrap();
        assert_eq!(got, ParameterValue::Float(250.0));
        // Update channel 1 mode to passthrough.
        p.set_parameter(
            ParameterId::from("channel_mode_1"),
            ParameterValue::String("passthrough".to_string()),
        )
        .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("channel_mode_1"))
            .unwrap();
        assert_eq!(got, ParameterValue::String("passthrough".to_string()));
    }

    #[test]
    fn test_per_channel_initialize_clamps_above_nyquist_into_stored_values() {
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![10_000.0, 20_000.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Lowpass],
        )
        .unwrap();
        // Initialize at 32 kHz: Nyquist limit ~15840. Channel 1 (20kHz) clamps.
        p.initialize(32000).unwrap();
        let ch0 = p
            .get_parameter(&ParameterId::from("channel_frequency_0"))
            .unwrap();
        let ch1 = p
            .get_parameter(&ParameterId::from("channel_frequency_1"))
            .unwrap();
        assert_eq!(ch0, ParameterValue::Float(10_000.0));
        // ch1 should reflect the clamped value, not the original 20 kHz.
        match ch1 {
            ParameterValue::Float(v) => {
                assert!(
                    v < 20_000.0 && v > 15_000.0,
                    "ch1 frequency should be clamped to just below Nyquist, got {v}"
                );
            }
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_per_channel_from_params_rejects_mismatched_channels() {
        let params = CrossoverPluginParams {
            crossover_type: "LR24".to_string(),
            frequency: 0.0,
            output: "lowpass".to_string(),
            extra_frequencies: vec![],
            fir_taps: None,
            channel_frequencies_hz: vec![80.0, 100.0],
            channel_modes: vec!["highpass".to_string(), "mute".to_string()],
        };
        // 2 frequencies but channels=3: must error, not silently use 2.
        assert!(CrossoverPlugin::from_params(3, &params).is_err());
    }

    #[test]
    fn test_per_channel_rejects_global_frequency_and_mode_writes() {
        let mut p = CrossoverPlugin::new_per_channel(
            "LR24",
            vec![100.0, 200.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
        )
        .unwrap();
        p.initialize(48000).unwrap();
        // Writing the global `frequency` / `mode` must error in per-channel
        // mode — silently updating unused global state would mask routing bugs.
        assert!(
            p.set_parameter(ParameterId::from("frequency"), ParameterValue::Float(500.0))
                .is_err(),
            "global frequency write must be rejected in per-channel mode"
        );
        assert!(
            p.set_parameter(
                ParameterId::from("mode"),
                ParameterValue::String("highpass".to_string())
            )
            .is_err(),
            "global mode write must be rejected in per-channel mode"
        );
        // Per-channel writes still work.
        assert!(
            p.set_parameter(
                ParameterId::from("channel_frequency_0"),
                ParameterValue::Float(150.0)
            )
            .is_ok()
        );
    }

    #[test]
    fn test_per_channel_from_params() {
        let params = CrossoverPluginParams {
            crossover_type: "LR24".to_string(),
            frequency: 0.0,
            output: "lowpass".to_string(),
            extra_frequencies: vec![],
            fir_taps: None,
            channel_frequencies_hz: vec![80.0, 100.0],
            channel_modes: vec!["highpass".to_string(), "mute".to_string()],
        };
        let p = CrossoverPlugin::from_params(2, &params).unwrap();
        assert!(p.is_per_channel());
        assert_eq!(
            p.op_modes,
            vec![PerChannelOpMode::Highpass, PerChannelOpMode::Mute]
        );
    }
}
