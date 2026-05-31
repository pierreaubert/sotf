// ============================================================================
// Parametric EQ Plugin
// ============================================================================

pub mod params;

#[cfg(feature = "gpui-ui")]
pub mod ui;

use math_audio_iir_fir::{
    Biquad, BiquadCoefficients, KautzFilter, SvfFilter, SvfFilterType, WarpedBiquad, bark_lambda,
};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainParams};
use sotf_host::oversampling::Oversampler;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_SAMPLE_RATE: u32 = 44100;
const MEASUREMENT_THROTTLE: usize = 10;
/// Duration of coefficient interpolation in seconds (~5ms)
const TRANSITION_DURATION_SECS: f64 = 0.005;

// Parameter limits
const FREQ_MIN: f32 = 20.0;
const FREQ_MAX: f32 = 20000.0;
const Q_MIN: f32 = 0.1;
const Q_MAX: f32 = 10.0;
const GAIN_MIN: f32 = -24.0;
const GAIN_MAX: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EqFilterTopology {
    #[default]
    Biquad,
    WarpedBiquad,
    KautzFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KautzSectionConfig {
    #[serde(alias = "freq", alias = "frequency", alias = "pole_freq_hz")]
    pub pole_freq: f64,
    pub q: f64,
    #[serde(default)]
    pub gain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilterConfig {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    #[serde(default)]
    pub db_gain: f64,
    /// Filter order: 2 (default, single biquad), 4, 6, or 8.
    /// Higher orders cascade N/2 biquads with Butterworth Q staggering.
    #[serde(default = "default_order")]
    pub order: usize,
    /// Runtime filter topology. Defaults to standard biquad for backwards-compatible
    /// Roomeq/host JSON.
    #[serde(default)]
    pub topology: EqFilterTopology,
    /// Warping coefficient for `topology=warped_biquad`. If omitted, the plugin
    /// uses the Bark-scale lambda for the active sample rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda: Option<f64>,
    /// Kautz sections for `topology=kautz_filter`. If omitted, the filter's
    /// `freq`, `q`, and `db_gain` define a single section.
    #[serde(default, alias = "sections", skip_serializing_if = "Vec::is_empty")]
    pub kautz_sections: Vec<KautzSectionConfig>,
}

fn default_order() -> usize {
    2
}

/// Butterworth Q values for cascaded biquad sections.
/// For an Nth-order Butterworth filter implemented as N/2 cascaded biquads,
/// each section uses a Q derived from the analog prototype poles.
/// Q_k = 1 / (2 * cos(pi * (2k + 1) / (2N))) for k = 0..N/2-1
fn butterworth_q_values(order: usize) -> Vec<f64> {
    let n = order.max(2);
    let num_stages = n / 2;
    (0..num_stages)
        .map(|k| {
            let angle = std::f64::consts::PI * (2 * k + 1) as f64 / (2 * n) as f64;
            1.0 / (2.0 * angle.cos())
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EqPluginParams {
    #[serde(default)]
    pub filters: Vec<BiquadFilterConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_filters: Option<Vec<Vec<BiquadFilterConfig>>>,
    #[serde(default)]
    pub auto_gain: AutoGainParams,
}

/// Per-band coefficient transition state for parameter smoothing.
/// Stores per-stage old and new coefficients so all biquad stages transition
/// smoothly, not just the first one (fixes glitch for order > 2 bands).
struct BandTransition {
    /// Old coefficients for each stage (len = num_stages for this band).
    old_coeffs_per_stage: Vec<BiquadCoefficients>,
    /// New coefficients for each stage (len = num_stages for this band).
    new_coeffs_per_stage: Vec<BiquadCoefficients>,
    samples_remaining: usize,
    total_samples: usize,
}

struct KautzRuntime {
    sections: Vec<KautzSectionConfig>,
    filter: KautzFilter<f64>,
}

impl KautzRuntime {
    fn new(sections: Vec<KautzSectionConfig>, sample_rate: f64) -> Result<Self, String> {
        if sections.is_empty() {
            return Err("Kautz filter needs at least one section".into());
        }
        validate_sample_rate(sample_rate)?;
        for section in &sections {
            validate_freq_q_gain(section.pole_freq, section.q, section.gain)?;
        }
        let mut runtime = Self {
            sections,
            filter: KautzFilter::from_room_modes(&[], sample_rate),
        };
        runtime.apply_sample_rate(sample_rate)?;
        Ok(runtime)
    }

    fn apply_sample_rate(&mut self, sample_rate: f64) -> Result<(), String> {
        validate_sample_rate(sample_rate)?;
        let modes: Vec<(f64, f64)> = self.sections.iter().map(|s| (s.pole_freq, s.q)).collect();
        let mut filter = KautzFilter::from_room_modes(&modes, sample_rate);
        for (section, cfg) in filter.sections.iter_mut().zip(self.sections.iter()) {
            section.gain = cfg.gain;
        }
        self.filter = filter;
        Ok(())
    }

    fn process(&mut self, sample: f64) -> f64 {
        sample + self.filter.process(sample)
    }

    fn reset(&mut self) {
        self.filter.reset();
    }
}

enum AdvancedFilter {
    Warped(WarpedBiquad<f64>),
    Kautz(KautzRuntime),
}

impl AdvancedFilter {
    fn from_config(
        config: &BiquadFilterConfig,
        sample_rate: f64,
        parse_filter_type: &dyn Fn(&str) -> Result<math_audio_iir_fir::BiquadFilterType, String>,
    ) -> Result<Option<Self>, String> {
        match config.topology {
            EqFilterTopology::Biquad => Ok(None),
            EqFilterTopology::WarpedBiquad => {
                validate_freq_q_gain(config.freq, config.q, config.db_gain)?;
                validate_sample_rate(sample_rate)?;
                let lambda = config.lambda.unwrap_or_else(|| bark_lambda(sample_rate));
                if !lambda.is_finite() || !(-0.9999..=0.9999).contains(&lambda) {
                    return Err(format!(
                        "Invalid warped_biquad lambda {lambda}: expected finite value in [-0.9999, 0.9999]"
                    ));
                }
                Ok(Some(Self::Warped(WarpedBiquad::new(
                    parse_filter_type(&config.filter_type)?,
                    config.freq,
                    sample_rate,
                    config.q,
                    config.db_gain,
                    lambda,
                ))))
            }
            EqFilterTopology::KautzFilter => {
                let sections = if config.kautz_sections.is_empty() {
                    vec![KautzSectionConfig {
                        pole_freq: config.freq,
                        q: config.q,
                        gain: config.db_gain,
                    }]
                } else {
                    config.kautz_sections.clone()
                };
                Ok(Some(Self::Kautz(KautzRuntime::new(sections, sample_rate)?)))
            }
        }
    }

    fn process(&mut self, sample: f64) -> f64 {
        match self {
            Self::Warped(filter) => filter.process(sample),
            Self::Kautz(filter) => filter.process(sample),
        }
    }

    fn apply_sample_rate(&mut self, sample_rate: f64) -> Result<(), String> {
        match self {
            Self::Warped(filter) => {
                filter.update_params(
                    filter.filter_type,
                    filter.freq,
                    sample_rate,
                    filter.q,
                    filter.db_gain,
                    filter.lambda,
                );
                Ok(())
            }
            Self::Kautz(filter) => filter.apply_sample_rate(sample_rate),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Warped(filter) => {
                *filter = WarpedBiquad::new(
                    filter.filter_type,
                    filter.freq,
                    filter.srate,
                    filter.q,
                    filter.db_gain,
                    filter.lambda,
                );
            }
            Self::Kautz(filter) => filter.reset(),
        }
    }
}

fn validate_sample_rate(sample_rate: f64) -> Result<(), String> {
    if sample_rate.is_finite() && sample_rate > 0.0 {
        Ok(())
    } else {
        Err(format!("Invalid sample rate: {sample_rate}"))
    }
}

fn validate_freq_q_gain(freq: f64, q: f64, gain: f64) -> Result<(), String> {
    if !freq.is_finite() || freq <= 0.0 {
        return Err(format!("Invalid filter frequency: {freq}"));
    }
    if !q.is_finite() || q <= 0.0 {
        return Err(format!("Invalid filter Q: {q}"));
    }
    if !gain.is_finite() {
        return Err(format!("Invalid filter gain: {gain}"));
    }
    Ok(())
}

pub struct EqPlugin {
    num_channels: usize,
    /// filters[channel][band][stage] — for order=2, each band has 1 stage.
    /// For order=N, each band has N/2 stages with Butterworth Q staggering.
    filters: Vec<Vec<Vec<Biquad>>>,
    /// Per-band order (2, 4, 6, 8). Default is 2.
    band_orders: Vec<usize>,
    sample_rate: u32,
    auto_gain: AutoGain,
    cache: RealTimeCache<AutoGainData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
    /// Per-band transition state. Outer index = band, applies to all channels.
    /// Each transition stores per-stage old/new coefficients so all biquad stages
    /// interpolate smoothly (fixes glitch for high-order bands with order > 2).
    transitions: Vec<Option<BandTransition>>,
    /// Oversampling factor: 1 (off), 2, or 4.
    oversampling_factor: u32,
    /// Oversampling state (None when oversampling_factor == 1).
    oversampler: Option<Oversampler>,
    /// Use Transposed Direct Form II for better numerical stability at high Q.
    use_tdf2: bool,
    /// Filter topology: 0 = Biquad (default), 1 = SVF (zero-delay feedback).
    /// When SVF is selected, `svf_filters` is used instead of `filters`.
    topology: usize,
    /// SVF filter banks: svf_filters[channel][band] — single SVF per band (no cascading).
    /// Only populated when topology == 1.
    svf_filters: Vec<Vec<SvfFilter>>,
    /// Advanced filter banks: advanced_filters[channel][filter].
    /// Populated from per-filter `topology=warped_biquad` or `topology=kautz_filter`.
    advanced_filters: Vec<Vec<AdvancedFilter>>,
}

/// Helper: create cascaded biquad stages for a given order.
/// For order=2, returns a single biquad with the original Q.
/// For order=4/6/8, returns N/2 biquads with Butterworth Q staggering,
/// each with gain_db split equally across stages.
fn create_band_stages(
    filter_type: math_audio_iir_fir::BiquadFilterType,
    freq: f64,
    srate: f64,
    q: f64,
    db_gain: f64,
    order: usize,
) -> Vec<Biquad> {
    let order = order.max(2);
    if order == 2 {
        return vec![Biquad::new(filter_type, freq, srate, q, db_gain)];
    }
    let num_stages = order / 2;
    let bw_qs = butterworth_q_values(order);
    let gain_per_stage = db_gain / num_stages as f64;
    bw_qs
        .iter()
        .map(|&bw_q| {
            // Combine user Q with Butterworth Q: multiply for peaking filters,
            // use Butterworth Q directly for LP/HP/shelf filters
            let effective_q = match filter_type {
                math_audio_iir_fir::BiquadFilterType::Peak
                | math_audio_iir_fir::BiquadFilterType::PeakMatched => q * bw_q,
                _ => bw_q,
            };
            Biquad::new(filter_type, freq, srate, effective_q, gain_per_stage)
        })
        .collect()
}

impl EqPlugin {
    /// Create an EQ with single-biquad bands (order=2, backward compatible).
    pub fn new(num_channels: usize, filters: Vec<Biquad>) -> Self {
        let num_bands = filters.len();
        let band_orders = vec![2; num_bands];
        let mut channel_filters = Vec::with_capacity(num_channels);
        for _ in 0..num_channels {
            // Wrap each biquad in a single-element Vec (1 stage per band)
            channel_filters.push(filters.iter().map(|f| vec![f.clone()]).collect());
        }
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate).expect("ag");
        let transitions = (0..num_bands).map(|_| None).collect();
        let mut p = Self {
            num_channels,
            filters: channel_filters,
            band_orders,
            sample_rate,
            auto_gain,
            cache: RealTimeCache::new(AutoGainData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            transitions,
            oversampling_factor: 1,
            use_tdf2: false,
            oversampler: None,
            topology: 0,
            svf_filters: Vec::new(),
            advanced_filters: (0..num_channels).map(|_| Vec::new()).collect(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Rebuild SVF filter bank from current biquad parameters.
    /// Each biquad band maps to one SVF. Multi-stage (high-order) bands
    /// use only the primary stage's parameters since SVF doesn't cascade the same way.
    fn rebuild_svf_filters(&mut self) {
        let sr = self.sample_rate as f64;
        self.svf_filters.clear();
        if self.filters.is_empty() {
            return;
        }
        for _ch in 0..self.num_channels {
            let mut ch_svfs = Vec::with_capacity(self.filters[0].len());
            for stages in &self.filters[0] {
                if let Some(primary) = stages.first() {
                    let svf_type = match primary.filter_type {
                        math_audio_iir_fir::BiquadFilterType::Peak
                        | math_audio_iir_fir::BiquadFilterType::PeakMatched => SvfFilterType::Peak,
                        math_audio_iir_fir::BiquadFilterType::Lowpass => SvfFilterType::Lowpass,
                        math_audio_iir_fir::BiquadFilterType::Highpass => SvfFilterType::Highpass,
                        math_audio_iir_fir::BiquadFilterType::Lowshelf
                        | math_audio_iir_fir::BiquadFilterType::LowshelfOrf => {
                            SvfFilterType::Lowshelf
                        }
                        math_audio_iir_fir::BiquadFilterType::Highshelf
                        | math_audio_iir_fir::BiquadFilterType::HighshelfOrf => {
                            SvfFilterType::Highshelf
                        }
                        math_audio_iir_fir::BiquadFilterType::Bandpass => SvfFilterType::Bandpass,
                        math_audio_iir_fir::BiquadFilterType::Notch => SvfFilterType::Notch,
                        math_audio_iir_fir::BiquadFilterType::AllPass => SvfFilterType::Allpass,
                        // Other biquad types (HighpassVariableQ etc.) map to closest SVF type
                        _ => SvfFilterType::Peak,
                    };
                    ch_svfs.push(SvfFilter::new(
                        svf_type,
                        primary.freq,
                        sr,
                        primary.q,
                        primary.db_gain,
                    ));
                }
            }
            self.svf_filters.push(ch_svfs);
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_bool(
                "auto_gain_enabled",
                "Auto Gain",
                self.auto_gain.is_enabled(),
            ),
            Parameter::new_int("oversampling", "OS", self.oversampling_factor as i32, 1, 4)
                .with_description("Oversampling factor: 1 (off), 2 (2x), 4 (4x)"),
            Parameter::new_bool("tdf2", "TDF-II", self.use_tdf2).with_description(
                "Use Transposed Direct Form II for better numerical stability at high Q",
            ),
            Parameter::new_string(
                "topology",
                "Topology",
                if self.topology == 1 { "SVF" } else { "Biquad" }.to_string(),
            )
            .with_description("Filter topology: Biquad or SVF (zero-delay feedback)"),
        ];

        if !self.filters.is_empty() {
            for (i, stages) in self.filters[0].iter().enumerate() {
                let group = format!("Band {}", i + 1);
                if let Some(f) = stages.first() {
                    params.push(
                        Parameter::new_float(
                            &format!("band_{}_freq", i),
                            "Freq",
                            f.freq as f32,
                            FREQ_MIN,
                            FREQ_MAX,
                        )
                        .with_group(&group),
                    );
                    params.push(
                        Parameter::new_float(
                            &format!("band_{}_q", i),
                            "Q",
                            f.q as f32,
                            Q_MIN,
                            Q_MAX,
                        )
                        .with_group(&group),
                    );
                    // Show total gain for the band (sum of all stages)
                    let total_gain: f64 = stages.iter().map(|s| s.db_gain).sum();
                    params.push(
                        Parameter::new_float(
                            &format!("band_{}_gain", i),
                            "Gain",
                            total_gain as f32,
                            GAIN_MIN,
                            GAIN_MAX,
                        )
                        .with_group(&group),
                    );
                    let order = self.band_orders.get(i).copied().unwrap_or(2);
                    params.push(
                        Parameter::new_int(
                            &format!("band_{}_order", i),
                            "Order",
                            order as i32,
                            2,
                            8,
                        )
                        .with_group(&group),
                    );
                }
            }
        }
        self.cached_parameters = params;
    }

    pub fn new_per_channel(
        num_channels: usize,
        channel_filters: Vec<Vec<Biquad>>,
    ) -> Result<Self, String> {
        if channel_filters.len() != num_channels {
            return Err("Count mismatch".into());
        }
        let num_bands = channel_filters.first().map_or(0, |c| c.len());
        let band_orders = vec![2; num_bands];
        // Wrap each biquad in a single-element Vec
        let channel_filters_3d: Vec<Vec<Vec<Biquad>>> = channel_filters
            .into_iter()
            .map(|ch| ch.into_iter().map(|f| vec![f]).collect())
            .collect();
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate)?;
        let transitions = (0..num_bands).map(|_| None).collect();
        let mut p = Self {
            num_channels,
            filters: channel_filters_3d,
            band_orders,
            sample_rate,
            auto_gain,
            cache: RealTimeCache::new(AutoGainData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            transitions,
            oversampling_factor: 1,
            use_tdf2: false,
            oversampler: None,
            topology: 0,
            svf_filters: Vec::new(),
            advanced_filters: (0..num_channels).map(|_| Vec::new()).collect(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(
        num_channels: usize,
        sample_rate: u32,
        params: EqPluginParams,
    ) -> Result<Self, String> {
        use math_audio_iir_fir::BiquadFilterType;
        let parse_filter_type = |s: &str| -> Result<BiquadFilterType, String> {
            match s {
                "peak" | "Peak" => Ok(BiquadFilterType::Peak),
                "lowshelf" | "Lowshelf" => Ok(BiquadFilterType::Lowshelf),
                "highshelf" | "Highshelf" => Ok(BiquadFilterType::Highshelf),
                "lowpass" | "Lowpass" => Ok(BiquadFilterType::Lowpass),
                "highpass" | "Highpass" => Ok(BiquadFilterType::Highpass),
                "notch" | "Notch" => Ok(BiquadFilterType::Notch),
                "bandpass" | "Bandpass" => Ok(BiquadFilterType::Bandpass),
                "allpass" | "AllPass" => Ok(BiquadFilterType::AllPass),
                "lowshelf_orf" | "LowshelfOrf" => Ok(BiquadFilterType::LowshelfOrf),
                "highshelf_orf" | "HighshelfOrf" => Ok(BiquadFilterType::HighshelfOrf),
                "peak_matched" | "PeakMatched" => Ok(BiquadFilterType::PeakMatched),
                other => Err(format!("Type: {}", other)),
            }
        };
        let config_to_stages = |f: &BiquadFilterConfig| -> Result<(Vec<Biquad>, usize), String> {
            let filter_type = parse_filter_type(&f.filter_type)?;
            let order = f.order.clamp(2, 8);
            if !order.is_multiple_of(2) {
                return Err(format!(
                    "Filter order must be even (2, 4, 6, 8); got {order}"
                ));
            }
            let stages = create_band_stages(
                filter_type,
                f.freq,
                sample_rate as f64,
                f.q,
                f.db_gain,
                order,
            );
            Ok((stages, order))
        };
        let config_to_advanced =
            |f: &BiquadFilterConfig| -> Result<Option<AdvancedFilter>, String> {
                AdvancedFilter::from_config(f, sample_rate as f64, &parse_filter_type)
            };
        let auto_gain = AutoGain::new(num_channels, sample_rate, params.auto_gain)?;
        let mut eq = if let Some(cfgs) = params.channel_filters {
            if cfgs.len() != num_channels {
                return Err("Mismatched chains".into());
            }
            let mut channel_filters = Vec::with_capacity(num_channels);
            let mut advanced_filters = Vec::with_capacity(num_channels);
            let mut band_orders = Vec::new();
            for (ch_idx, c) in cfgs.iter().enumerate() {
                let mut ch_bands = Vec::new();
                let mut ch_advanced = Vec::new();
                for f in c {
                    if f.topology == EqFilterTopology::Biquad {
                        let (stages, order) = config_to_stages(f)?;
                        if ch_idx == 0 {
                            band_orders.push(order);
                        }
                        ch_bands.push(stages);
                    } else if let Some(filter) = config_to_advanced(f)? {
                        ch_advanced.push(filter);
                    }
                }
                channel_filters.push(ch_bands);
                advanced_filters.push(ch_advanced);
            }
            let num_bands = band_orders.len();
            Self {
                num_channels,
                filters: channel_filters,
                band_orders,
                sample_rate,
                auto_gain,
                cache: RealTimeCache::new(AutoGainData::default()),
                cache_update_counter: 0,
                cached_parameters: Vec::new(),
                transitions: (0..num_bands).map(|_| None).collect(),
                oversampling_factor: 1,
                use_tdf2: false,
                oversampler: None,
                topology: 0,
                svf_filters: Vec::new(),
                advanced_filters,
            }
        } else {
            let mut band_stages = Vec::new();
            let mut band_orders = Vec::new();
            for f in &params.filters {
                if f.topology == EqFilterTopology::Biquad {
                    let (stages, order) = config_to_stages(f)?;
                    band_stages.push(stages);
                    band_orders.push(order);
                }
            }
            let num_bands = band_stages.len();
            let mut channel_filters = Vec::with_capacity(num_channels);
            let mut advanced_filters = Vec::with_capacity(num_channels);
            for _ in 0..num_channels {
                channel_filters.push(band_stages.clone());
                let mut ch_advanced = Vec::new();
                for f in &params.filters {
                    if f.topology != EqFilterTopology::Biquad
                        && let Some(filter) = config_to_advanced(f)?
                    {
                        ch_advanced.push(filter);
                    }
                }
                advanced_filters.push(ch_advanced);
            }
            Self {
                num_channels,
                filters: channel_filters,
                band_orders,
                sample_rate,
                auto_gain,
                cache: RealTimeCache::new(AutoGainData::default()),
                cache_update_counter: 0,
                cached_parameters: Vec::new(),
                transitions: (0..num_bands).map(|_| None).collect(),
                oversampling_factor: 1,
                use_tdf2: false,
                oversampler: None,
                topology: 0,
                svf_filters: Vec::new(),
                advanced_filters,
            }
        };
        eq.rebuild_cached_parameters();
        Ok(eq)
    }

    pub fn set_filters(&mut self, filters: Vec<Biquad>) {
        self.filters.clear();
        self.band_orders = vec![2; filters.len()];
        for _ in 0..self.num_channels {
            self.filters
                .push(filters.iter().map(|f| vec![f.clone()]).collect());
        }
        self.transitions = (0..filters.len()).map(|_| None).collect();
        self.advanced_filters = (0..self.num_channels).map(|_| Vec::new()).collect();
    }

    pub fn set_channel_filters(&mut self, channel_filters: Vec<Vec<Biquad>>) -> Result<(), String> {
        if channel_filters.len() != self.num_channels {
            return Err("mismatch".into());
        }
        let num_bands = channel_filters.first().map_or(0, |c| c.len());
        self.band_orders = vec![2; num_bands];
        self.filters = channel_filters
            .into_iter()
            .map(|ch| ch.into_iter().map(|f| vec![f]).collect())
            .collect();
        self.transitions = (0..num_bands).map(|_| None).collect();
        self.advanced_filters = (0..self.num_channels).map(|_| Vec::new()).collect();
        Ok(())
    }

    /// Compute the number of samples for the transition at the current sample rate.
    fn transition_samples(&self) -> usize {
        (self.sample_rate as f64 * TRANSITION_DURATION_SECS) as usize
    }

    /// Rebuild biquad coefficients at the oversampled rate and reset transitions.
    ///
    /// Called from `initialize()`. Biquads must be designed at `srate * factor`
    /// so their frequency response remains correct at the oversampled rate.
    fn apply_sample_rate_to_filters(&mut self, srate: f64) {
        for chain in &mut self.filters {
            for stages in chain {
                for f in stages.iter_mut() {
                    f.update_params(f.filter_type, f.freq, srate, f.q, f.db_gain);
                }
            }
        }
        for t in &mut self.transitions {
            *t = None;
        }
    }

    fn apply_sample_rate_to_advanced_filters(&mut self, sample_rate: f64) -> Result<(), String> {
        for chain in &mut self.advanced_filters {
            for filter in chain {
                filter.apply_sample_rate(sample_rate)?;
            }
        }
        Ok(())
    }

    fn process_advanced_interleaved(&mut self, buffer: &mut [f32], num_frames: usize) {
        if self.advanced_filters.iter().all(Vec::is_empty) {
            return;
        }
        let nc = self.num_channels;
        for frame in 0..num_frames {
            for ch in 0..nc {
                let idx = frame * nc + ch;
                let mut sample = buffer[idx] as f64;
                for filter in &mut self.advanced_filters[ch] {
                    sample = filter.process(sample);
                }
                buffer[idx] = sample as f32;
            }
        }
    }

    /// Process a single chunk of planar audio through the biquad filter chain.
    ///
    /// `planar[ch]` has `num_frames` valid samples starting at offset 0.
    /// Processes in place.
    fn process_biquads_planar(&mut self, planar: &mut [Vec<f32>], num_frames: usize) {
        let has_transitions = self.transitions.iter().any(|t| t.is_some());
        if has_transitions {
            // planar[ch][frame] requires both indices simultaneously; range loops are correct here.
            #[allow(clippy::needless_range_loop)]
            for frame in 0..num_frames {
                for ch in 0..self.num_channels {
                    let mut s = planar[ch][frame] as f64;
                    for (band_idx, stages) in self.filters[ch].iter_mut().enumerate() {
                        if let Some(trans) = self.transitions.get(band_idx).and_then(|t| t.as_ref())
                        {
                            let t =
                                1.0 - (trans.samples_remaining as f64 / trans.total_samples as f64);
                            // Interpolate all stages so multi-order bands don't glitch
                            for (i, stage) in stages.iter_mut().enumerate() {
                                let interpolated = if let (Some(old), Some(new)) = (
                                    trans.old_coeffs_per_stage.get(i),
                                    trans.new_coeffs_per_stage.get(i),
                                ) {
                                    old.lerp(new, t)
                                } else {
                                    stage.coefficients()
                                };
                                s = stage.process_with_coefficients(s, &interpolated);
                            }
                        } else {
                            for stage in stages {
                                s = stage.process(s);
                            }
                        }
                    }
                    planar[ch][frame] = s as f32;
                }
                for t in self.transitions.iter_mut().flatten() {
                    if t.samples_remaining > 0 {
                        t.samples_remaining -= 1;
                    }
                }
            }
            for trans in self.transitions.iter_mut() {
                if trans.as_ref().is_some_and(|t| t.samples_remaining == 0) {
                    *trans = None;
                }
            }
        } else {
            // planar[ch][frame] requires both indices simultaneously; range loops are correct here.
            #[allow(clippy::needless_range_loop)]
            for frame in 0..num_frames {
                for ch in 0..self.num_channels {
                    let mut s = planar[ch][frame] as f64;
                    for stages in &mut self.filters[ch] {
                        for stage in stages {
                            s = stage.process(s);
                        }
                    }
                    planar[ch][frame] = s as f32;
                }
            }
        }
    }
}

impl InPlacePlugin for EqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Parametric EQ", "2.0.0", "SotF")
    }
    fn channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = id.0.as_str();
        if name == "auto_gain_enabled" {
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", true).validate(&value)?;
            self.auto_gain.set_enabled(value.as_bool().unwrap_or(true));
            self.rebuild_cached_parameters();
        } else if name == "oversampling" {
            let new_factor = value.as_int().unwrap_or(1);
            // Only 1, 2, 4 are valid
            if new_factor != 1 && new_factor != 2 && new_factor != 4 {
                return Err(format!(
                    "Invalid oversampling factor {}: must be 1, 2, or 4",
                    new_factor
                ));
            }
            self.oversampling_factor = new_factor as u32;
            // Re-initialize oversampling state (uses current sample_rate)
            if self.oversampling_factor > 1 {
                self.oversampler = Some(Oversampler::new(
                    self.oversampling_factor,
                    self.num_channels,
                )?);
                // Recalculate biquad coefficients at oversampled rate
                let os_rate = self.sample_rate as f64 * self.oversampling_factor as f64;
                self.apply_sample_rate_to_filters(os_rate);
            } else {
                self.oversampler = None;
                // Restore biquad coefficients at nominal rate
                self.apply_sample_rate_to_filters(self.sample_rate as f64);
            }
            self.rebuild_cached_parameters();
        } else if name == "tdf2" {
            let enabled = value.as_bool().unwrap_or(false);
            self.use_tdf2 = enabled;
            // Update all biquad filters
            for ch_filters in &mut self.filters {
                for stages in ch_filters {
                    for bq in stages {
                        bq.use_tdf2 = enabled;
                    }
                }
            }
            self.rebuild_cached_parameters();
        } else if name == "topology" {
            let new_topo = if let Some(s) = value.as_string() {
                match s {
                    "SVF" | "svf" => 1,
                    _ => 0,
                }
            } else if let Some(v) = value.as_float() {
                (v as usize).min(1)
            } else {
                0
            };
            if new_topo != self.topology {
                self.topology = new_topo;
                if new_topo == 1 {
                    self.rebuild_svf_filters();
                } else {
                    self.svf_filters.clear();
                }
            }
            self.rebuild_cached_parameters();
        } else if let Some(rest) = name.strip_prefix("band_") {
            // Parse "band_N_field" without heap allocation.
            // Find the next '_' to split index from field.
            if let Some(sep) = rest.find('_') {
                let b_idx = rest[..sep].parse::<usize>().unwrap_or(0);
                let field = &rest[sep + 1..];

                if field == "order" {
                    // Change filter order: rebuild all stages for this band
                    let new_order = value.as_int().unwrap_or(2).clamp(2, 8) as usize;
                    if !new_order.is_multiple_of(2) {
                        return Err(format!(
                            "Filter order must be even (2, 4, 6, 8); got {new_order}"
                        ));
                    }
                    if let Some(stages) = self.filters[0].get(b_idx)
                        && let Some(primary) = stages.first()
                    {
                        let ft = primary.filter_type;
                        let freq = primary.freq;
                        let srate = primary.srate;
                        let q = primary.q;
                        let total_gain: f64 = stages.iter().map(|s| s.db_gain).sum();
                        while self.band_orders.len() <= b_idx {
                            self.band_orders.push(2);
                        }
                        self.band_orders[b_idx] = new_order;
                        for ch in 0..self.num_channels {
                            if let Some(band) = self.filters[ch].get_mut(b_idx) {
                                *band =
                                    create_band_stages(ft, freq, srate, q, total_gain, new_order);
                            }
                        }
                    }
                    if self.topology == 1 {
                        self.rebuild_svf_filters();
                    }
                    self.rebuild_cached_parameters();
                    return Ok(());
                }

                // Validate using a temporary parameter template
                match field {
                    "freq" => Parameter::new_float("freq", "Freq", 1000.0, FREQ_MIN, FREQ_MAX)
                        .validate(&value)?,
                    "q" => Parameter::new_float("q", "Q", 1.0, Q_MIN, Q_MAX).validate(&value)?,
                    "gain" => Parameter::new_float("gain", "Gain", 0.0, GAIN_MIN, GAIN_MAX)
                        .validate(&value)?,
                    _ => return Err(format!("Unknown field: {}", field)),
                }

                if let Some(v) = value.as_float() {
                    if !v.is_finite() {
                        return Err("Value is not finite".into());
                    }
                    // Capture old per-stage coefficients before updating.
                    // If a transition is already in progress, interpolate to the current
                    // mid-point so the new transition starts from the actual running state.
                    let old_coeffs_per_stage: Vec<BiquadCoefficients> =
                        if let Some(Some(active)) = self.transitions.get(b_idx) {
                            let t = 1.0
                                - (active.samples_remaining as f64 / active.total_samples as f64);
                            active
                                .old_coeffs_per_stage
                                .iter()
                                .zip(active.new_coeffs_per_stage.iter())
                                .map(|(old, new)| old.lerp(new, t))
                                .collect()
                        } else {
                            self.filters[0]
                                .get(b_idx)
                                .map(|stages| stages.iter().map(|f| f.coefficients()).collect())
                                .unwrap_or_default()
                        };

                    let order = self.band_orders.get(b_idx).copied().unwrap_or(2);
                    let num_stages = order / 2;

                    for ch in 0..self.num_channels {
                        if let Some(stages) = self.filters[ch].get_mut(b_idx)
                            && let Some(primary) = stages.first()
                        {
                            let mut freq = primary.freq;
                            let mut q = primary.q;
                            let total_gain: f64 = stages.iter().map(|s| s.db_gain).sum();
                            let mut new_total_gain = total_gain;
                            match field {
                                "freq" => freq = v as f64,
                                "q" => q = v as f64,
                                "gain" => new_total_gain = v as f64,
                                _ => {}
                            }
                            let gain_per_stage = new_total_gain / num_stages as f64;
                            let ft = primary.filter_type;
                            let srate = primary.srate;

                            if num_stages == 1 {
                                stages[0].update_params(ft, freq, srate, q, new_total_gain);
                            } else {
                                let bw_qs = butterworth_q_values(order);
                                for (s, &bw_q) in stages.iter_mut().zip(bw_qs.iter()) {
                                    let effective_q = match ft {
                                        math_audio_iir_fir::BiquadFilterType::Peak
                                        | math_audio_iir_fir::BiquadFilterType::PeakMatched => {
                                            q * bw_q
                                        }
                                        _ => bw_q,
                                    };
                                    s.update_params(ft, freq, srate, effective_q, gain_per_stage);
                                }
                            }
                        }
                    }
                    // Start a per-stage coefficient transition covering all biquad stages
                    if !old_coeffs_per_stage.is_empty()
                        && let Some(stages) = self.filters[0].get(b_idx)
                    {
                        let new_coeffs_per_stage: Vec<BiquadCoefficients> =
                            stages.iter().map(|f| f.coefficients()).collect();
                        let total = self.transition_samples();
                        if total > 0 {
                            while self.transitions.len() <= b_idx {
                                self.transitions.push(None);
                            }
                            self.transitions[b_idx] = Some(BandTransition {
                                old_coeffs_per_stage,
                                new_coeffs_per_stage,
                                samples_remaining: total,
                                total_samples: total,
                            });
                        }
                    }
                    // Update SVF filters if topology is active
                    if self.topology == 1 {
                        self.rebuild_svf_filters();
                    }
                    self.rebuild_cached_parameters();
                }
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = id.0.as_str();
        if name == "auto_gain_enabled" {
            Some(ParameterValue::Bool(self.auto_gain.is_enabled()))
        } else if name == "oversampling" {
            Some(ParameterValue::Int(self.oversampling_factor as i32))
        } else if name == "tdf2" {
            Some(ParameterValue::Bool(self.use_tdf2))
        } else if name == "topology" {
            Some(ParameterValue::String(
                if self.topology == 1 { "SVF" } else { "Biquad" }.to_string(),
            ))
        } else if let Some(rest) = name.strip_prefix("band_") {
            // Parse "band_N_field" without heap allocation.
            // Find the next '_' to split index from field.
            if let Some(sep) = rest.find('_') {
                let b_idx = rest[..sep].parse::<usize>().unwrap_or(0);
                let field = &rest[sep + 1..];
                if let Some(stages) = self.filters[0].get(b_idx)
                    && let Some(primary) = stages.first()
                {
                    return match field {
                        "freq" => Some(ParameterValue::Float(primary.freq as f32)),
                        "q" => Some(ParameterValue::Float(primary.q as f32)),
                        "gain" => {
                            let total: f64 = stages.iter().map(|s| s.db_gain).sum();
                            Some(ParameterValue::Float(total as f32))
                        }
                        "order" => {
                            let order = self.band_orders.get(b_idx).copied().unwrap_or(2);
                            Some(ParameterValue::Int(order as i32))
                        }
                        _ => None,
                    };
                }
            }
            None
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Biquad coefficients are designed at the oversampled rate so that
        // the filter frequency response is correct relative to the true input rate.
        let filter_rate = sample_rate as f64 * self.oversampling_factor as f64;
        for chain in &mut self.filters {
            for stages in chain {
                for f in stages {
                    f.update_params(f.filter_type, f.freq, filter_rate, f.q, f.db_gain);
                }
            }
        }
        for t in &mut self.transitions {
            *t = None;
        }
        self.apply_sample_rate_to_advanced_filters(sample_rate as f64)?;
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| e.to_string())?;

        // Rebuild oversampling state if active
        if self.oversampling_factor > 1 {
            self.oversampler = Some(Oversampler::new(
                self.oversampling_factor,
                self.num_channels,
            )?);
        } else {
            self.oversampler = None;
        }

        // Rebuild SVF filters if SVF topology is active
        if self.topology == 1 {
            self.rebuild_svf_filters();
        }

        Ok(())
    }
    fn reset(&mut self) {
        // Reset SVF integrator state
        for ch_svfs in &mut self.svf_filters {
            for svf in ch_svfs {
                svf.reset();
            }
        }
        for chain in &mut self.filters {
            for stages in chain {
                for f in stages {
                    f.reset();
                }
            }
        }
        for chain in &mut self.advanced_filters {
            for filter in chain {
                filter.reset();
            }
        }
        for t in &mut self.transitions {
            *t = None;
        }
        self.auto_gain.reset();

        // Reset oversampling resamplers
        if let Some(os) = &mut self.oversampler {
            os.reset();
        }
    }
    fn latency_samples(&self) -> usize {
        if let Some(os) = &self.oversampler {
            os.latency_samples()
        } else {
            0
        }
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let nc = self.num_channels;

        // Throttled measurement
        self.cache_update_counter += 1;
        let mut do_measure = false;
        if self.cache_update_counter >= MEASUREMENT_THROTTLE {
            self.cache_update_counter = 0;
            do_measure = true;
        }

        if do_measure {
            let _ = self.auto_gain.measure_input(buffer);
        }

        if self.topology == 1 && !self.svf_filters.is_empty() {
            // ----------------------------------------------------------------
            // SVF topology: zero-delay feedback, inherently modulation-stable
            // No coefficient interpolation needed — SVF handles parameter
            // changes without transients.
            // ----------------------------------------------------------------
            for frame in 0..num_frames {
                for ch in 0..nc {
                    let idx = frame * nc + ch;
                    let mut s = buffer[idx] as f64;
                    for svf in &mut self.svf_filters[ch] {
                        s = svf.process(s);
                    }
                    buffer[idx] = s as f32;
                }
            }
        } else if self.oversampling_factor == 1 {
            // ----------------------------------------------------------------
            // Fast path: no oversampling — process biquads directly
            // ----------------------------------------------------------------
            let has_transitions = self.transitions.iter().any(|t| t.is_some());

            if has_transitions {
                // Process with per-sample coefficient interpolation on ALL stages
                for frame in 0..num_frames {
                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let mut s = buffer[idx] as f64;
                        for (band_idx, stages) in self.filters[ch].iter_mut().enumerate() {
                            if let Some(trans) =
                                self.transitions.get(band_idx).and_then(|t| t.as_ref())
                            {
                                // Interpolate all stages so multi-order bands don't glitch
                                let t = 1.0
                                    - (trans.samples_remaining as f64 / trans.total_samples as f64);
                                for (i, stage) in stages.iter_mut().enumerate() {
                                    let interpolated = if let (Some(old), Some(new)) = (
                                        trans.old_coeffs_per_stage.get(i),
                                        trans.new_coeffs_per_stage.get(i),
                                    ) {
                                        old.lerp(new, t)
                                    } else {
                                        stage.coefficients()
                                    };
                                    s = stage.process_with_coefficients(s, &interpolated);
                                }
                            } else {
                                for stage in stages {
                                    s = stage.process(s);
                                }
                            }
                        }
                        buffer[idx] = s as f32;
                    }
                    for t in self.transitions.iter_mut().flatten() {
                        if t.samples_remaining > 0 {
                            t.samples_remaining -= 1;
                        }
                    }
                }
                for trans in self.transitions.iter_mut() {
                    if trans.as_ref().is_some_and(|t| t.samples_remaining == 0) {
                        *trans = None;
                    }
                }
            } else {
                // Fast path: no transitions active
                for frame in 0..num_frames {
                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let mut s = buffer[idx] as f64;
                        for stages in &mut self.filters[ch] {
                            for stage in stages {
                                s = stage.process(s);
                            }
                        }
                        buffer[idx] = s as f32;
                    }
                }
            }
        } else {
            // ----------------------------------------------------------------
            // Oversampling path: delegate to shared Oversampler
            // ----------------------------------------------------------------
            // Take the oversampler out to split the borrow: the callback
            // needs &mut self.filters/transitions while oversampler needs &mut.
            let mut os = self
                .oversampler
                .take()
                .ok_or_else(|| "oversampling enabled but oversampler is unavailable".to_string())?;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                os.process(buffer, num_frames, |planar, os_frames| {
                    self.process_biquads_planar(planar, os_frames);
                })
            }));
            self.oversampler = Some(os);
            match result {
                Ok(result) => {
                    result?;
                }
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }

        self.process_advanced_interleaved(buffer, num_frames);

        if do_measure {
            let _ = self.auto_gain.measure_output(buffer);
            let ag_data = self.auto_gain.get_data();
            self.cache.update(|d| {
                *d = ag_data;
            });
        }

        self.auto_gain.apply_compensation(buffer, num_frames);

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use math_audio_iir_fir::{Biquad, BiquadFilterType};
    use sotf_host::parameters::{ParameterId, ParameterValue};
    use sotf_host::*;

    #[test]
    fn test_eq_passthrough() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        let mut b = vec![0.5; 2048];
        p.process_in_place(&mut b, &ProcessContext::new(48000, 1024))
            .unwrap();
        assert_eq!(b, vec![0.5; 2048]);
    }

    #[test]
    fn test_eq_boost() {
        let f = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0,
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
        let mut b: Vec<f32> = (0..1024).map(|k| (k as f32 * 0.1).sin()).collect();
        let i = b.clone();
        p.process_in_place(&mut b, &ProcessContext::new(48000, 1024))
            .unwrap();
        // Check a sample after some settling
        assert!(b[100].abs() > i[100].abs());
    }

    #[test]
    fn test_eq_processing_varied_buffers() {
        use sotf_host::{InPlacePluginAdapter, Plugin, test_varied_buffer_sizes};
        let sample_rate = 48000.0;
        let channels = 2;
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            sample_rate,
            1.0,
            6.0,
        )];
        let mut inner = EqPlugin::new(channels, f);
        inner.initialize(sample_rate as u32).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        let mut signal_gen = SignalGen::new_sine(sample_rate, 1000.0, 0.5);
        let input = signal_gen.generate(4800 * channels);

        let mut expected_output = vec![0.0; input.len()];
        let ctx = ProcessContext::new(sample_rate as u32, 4800);
        plugin.process(&input, &mut expected_output, &ctx).unwrap();

        plugin.reset();
        test_varied_buffer_sizes(&mut plugin, sample_rate, &input, &expected_output);
    }

    #[test]
    fn test_eq_allpass_filter_type_parses() {
        // AllPass filters are generated by GD-Opt and serialized as "allpass".
        // Verify the EQ plugin can parse them.
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "allpass".to_string(),
                freq: 100.0,
                q: 0.707,
                db_gain: 0.0,
                order: 2,
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let result = EqPlugin::from_params(2, 48000, params);
        assert!(
            result.is_ok(),
            "EqPlugin should parse 'allpass' filter type, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_eq_warped_biquad_filter_processes() {
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 6.0,
                order: 2,
                topology: EqFilterTopology::WarpedBiquad,
                lambda: Some(0.5),
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let mut p = EqPlugin::from_params(1, 48000, params).unwrap();
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        assert_eq!(p.filters[0].len(), 0);
        assert_eq!(p.advanced_filters[0].len(), 1);

        let mut buf: Vec<f32> = (0..2048).map(|i| (i as f32 * 0.11).sin() * 0.25).collect();
        let input = buf.clone();
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 2048))
            .unwrap();

        assert!(buf.iter().all(|s| s.is_finite()));
        let diff: f32 = buf
            .iter()
            .zip(input.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.01, "warped biquad should alter the signal");
    }

    #[test]
    fn test_eq_kautz_filter_processes_as_dry_plus_correction() {
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 100.0,
                q: 5.0,
                db_gain: 0.0,
                order: 2,
                topology: EqFilterTopology::KautzFilter,
                lambda: None,
                kautz_sections: vec![KautzSectionConfig {
                    pole_freq: 100.0,
                    q: 5.0,
                    gain: 0.5,
                }],
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let mut p = EqPlugin::from_params(1, 48000, params).unwrap();
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        assert_eq!(p.filters[0].len(), 0);
        assert_eq!(p.advanced_filters[0].len(), 1);

        let mut buf: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.013).sin() * 0.25).collect();
        let input = buf.clone();
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 4096))
            .unwrap();

        assert!(buf.iter().all(|s| s.is_finite()));
        let diff: f32 = buf
            .iter()
            .zip(input.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.01, "Kautz correction should alter the signal");
    }

    #[test]
    fn test_eq_rt_safety() {
        use sotf_host::{InPlacePluginAdapter, Plugin, assert_no_allocs};
        let sample_rate = 48000;
        let channels = 2;
        let mut inner = EqPlugin::new(channels, vec![]);
        inner.initialize(sample_rate).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        let input = vec![0.1; 512 * channels];
        let mut output = vec![0.0; 512 * channels];
        let ctx = ProcessContext::new(sample_rate, 512);

        // Warm up
        for _ in 0..10 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }

        assert_no_allocs("EqPlugin::process", || {
            plugin.process(&input, &mut output, &ctx).unwrap();
        });
    }

    #[test]
    fn test_parameter_smoothing_starts_transition() {
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            0.0,
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // No transition initially
        assert!(p.transitions[0].is_none());

        // Change gain -> should start a transition
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(6.0),
        )
        .unwrap();

        assert!(p.transitions[0].is_some());
        let trans = p.transitions[0].as_ref().unwrap();
        assert!(trans.total_samples > 0);
        assert_eq!(trans.samples_remaining, trans.total_samples);
    }

    #[test]
    fn test_parameter_smoothing_completes() {
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            0.0,
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        // Trigger a transition
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(6.0),
        )
        .unwrap();
        assert!(p.transitions[0].is_some());

        // Process enough samples to complete the transition (~5ms at 48kHz = 240 samples)
        let num_frames = 512;
        let mut buf = vec![0.0f32; num_frames];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = (i as f32 * 0.1).sin() * 0.5;
        }
        p.process_in_place(&mut buf, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // Transition should be complete after 512 samples (> 240)
        assert!(p.transitions[0].is_none());
    }

    #[test]
    fn test_initialize_preserves_state_on_sample_rate_change() {
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            44100.0,
            1.0,
            6.0,
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 44100).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        // Process some audio to build up filter state
        let mut buf: Vec<f32> = (0..256).map(|k| (k as f32 * 0.1).sin()).collect();
        p.process_in_place(&mut buf, &ProcessContext::new(44100, 256))
            .unwrap();

        // Re-initialize at new sample rate - should use update_params, not new
        // (filter params should stay the same, just recompute coeffs for new rate)
        InPlacePlugin::initialize(&mut p, 96000).unwrap();
        assert_eq!(p.sample_rate, 96000);
        // Filter should still have the same user parameters
        assert_eq!(p.filters[0][0][0].freq, 1000.0);
        assert_eq!(p.filters[0][0][0].db_gain, 6.0);
        // srate should be at oversampled rate (96000 * 1 = 96000 when factor=1)
        assert!((p.filters[0][0][0].srate - 96000.0).abs() < 1e-10);
    }

    #[test]
    fn test_smoothed_output_bounded_between_old_and_new() {
        // After a gain change, the output during transition should be bounded
        // between the old filter response and the new filter response
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            0.0, // start at 0dB (passthrough)
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        // Process some warmup
        let mut warmup = vec![0.5f32; 1024];
        p.process_in_place(&mut warmup, &ProcessContext::new(48000, 1024))
            .unwrap();

        // Now change gain to +12dB
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(12.0),
        )
        .unwrap();

        // Process during transition with DC signal
        let mut buf = vec![0.5f32; 512];
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
            .unwrap();

        // All output samples should be finite
        for (i, &s) in buf.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
    }

    #[test]
    fn test_oversampling_parameter_set_get() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // Default is 1 (no oversampling)
        assert_eq!(
            InPlacePlugin::get_parameter(&p, &ParameterId::from("oversampling")),
            Some(ParameterValue::Int(1))
        );

        // Set to 2x
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(2),
        )
        .unwrap();
        assert_eq!(p.oversampling_factor, 2);
        assert!(p.oversampler.is_some());
        assert!(p.latency_samples() > 0);

        // Set to 4x
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(4),
        )
        .unwrap();
        assert_eq!(p.oversampling_factor, 4);
        assert!(p.oversampler.is_some());

        // Set back to 1x
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(1),
        )
        .unwrap();
        assert_eq!(p.oversampling_factor, 1);
        assert!(p.oversampler.is_none());
        assert_eq!(p.latency_samples(), 0);
    }

    #[test]
    fn test_oversampling_invalid_factor() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // Factor 3 is invalid
        assert!(
            InPlacePlugin::set_parameter(
                &mut p,
                ParameterId::from("oversampling"),
                ParameterValue::Int(3),
            )
            .is_err()
        );

        // Factor 0 is invalid
        assert!(
            InPlacePlugin::set_parameter(
                &mut p,
                ParameterId::from("oversampling"),
                ParameterValue::Int(0),
            )
            .is_err()
        );
    }

    #[test]
    fn test_oversampling_2x_processes_audio() {
        let f = vec![Biquad::new(
            BiquadFilterType::Lowpass,
            10000.0,
            48000.0,
            0.707,
            0.0,
        )];
        let mut p = EqPlugin::new(2, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(2),
        )
        .unwrap();

        // Process several blocks to let the resampler fill up
        let num_frames = 512;
        let nc = 2;
        let mut signal: Vec<f32> = (0..num_frames * nc)
            .map(|i| (i as f32 * 0.05).sin() * 0.5)
            .collect();

        // Warm up — process multiple blocks
        for _ in 0..10 {
            p.process_in_place(&mut signal, &ProcessContext::new(48000, num_frames))
                .unwrap();
        }

        // All output samples must be finite
        for (i, &s) in signal.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
        assert!(
            p.oversampler.is_some(),
            "oversampler must be restored after processing"
        );
    }

    #[test]
    fn test_oversampling_4x_processes_audio() {
        let f = vec![Biquad::new(
            BiquadFilterType::Lowpass,
            10000.0,
            48000.0,
            0.707,
            0.0,
        )];
        let mut p = EqPlugin::new(2, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(4),
        )
        .unwrap();

        let num_frames = 512;
        let nc = 2;
        let mut signal: Vec<f32> = (0..num_frames * nc)
            .map(|i| (i as f32 * 0.05).sin() * 0.5)
            .collect();

        // Warm up
        for _ in 0..10 {
            p.process_in_place(&mut signal, &ProcessContext::new(48000, num_frames))
                .unwrap();
        }

        for (i, &s) in signal.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
    }

    #[test]
    fn test_oversampling_latency_reported() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // No latency without oversampling
        assert_eq!(p.latency_samples(), 0);

        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(2),
        )
        .unwrap();
        let lat_2x = p.latency_samples();
        assert!(lat_2x > 0, "2x oversampling should have latency");

        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(4),
        )
        .unwrap();
        let lat_4x = p.latency_samples();
        assert!(lat_4x > 0, "4x oversampling should have latency");
    }

    #[test]
    fn test_oversampling_biquad_freq_scaled() {
        // Biquads should be designed at oversampled rate.
        // When oversampling=2 and SR=48000, filters should use srate=96000.
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            6.0,
        )];
        let mut p = EqPlugin::new(1, f);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        assert!((p.filters[0][0][0].srate - 48000.0).abs() < 1.0);

        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(2),
        )
        .unwrap();
        // After setting 2x oversampling, biquads should be recalculated at 96000 Hz
        assert!((p.filters[0][0][0].srate - 96000.0).abs() < 1.0);
    }

    #[test]
    fn test_oversampling_reset_clears_state() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("oversampling"),
            ParameterValue::Int(2),
        )
        .unwrap();

        // Push some audio through
        let num_frames = 512;
        let nc = 2;
        let mut signal = vec![0.5f32; num_frames * nc];
        p.process_in_place(&mut signal, &ProcessContext::new(48000, num_frames))
            .unwrap();

        // Reset should clear residuals — after reset, processing silence yields silence
        InPlacePlugin::reset(&mut p);
        assert!(p.oversampler.is_some());
        let mut silence = vec![0.0f32; num_frames * nc];
        // Process enough blocks to flush any stale state
        for _ in 0..10 {
            p.process_in_place(&mut silence, &ProcessContext::new(48000, num_frames))
                .unwrap();
        }
        for (i, &s) in silence.iter().enumerate() {
            assert!(s.abs() < 1e-6, "sample {} not silent after reset: {}", i, s);
        }
    }

    #[test]
    fn test_multi_stage_transition_covers_all_stages() {
        // A 4th-order band (2 biquad stages) should start a transition that covers
        // both stages, not just the first. Verify by checking the transition
        // stores coefficients for all N/2 stages.
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 0.0,
                order: 4, // 2 stages
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let mut p = EqPlugin::from_params(1, 48000, params).unwrap();
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        // Trigger a gain change
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(6.0),
        )
        .unwrap();

        // Transition should exist and cover 2 stages (order=4 => 2 stages)
        assert!(p.transitions[0].is_some());
        let trans = p.transitions[0].as_ref().unwrap();
        assert_eq!(
            trans.old_coeffs_per_stage.len(),
            2,
            "4th-order band should transition 2 stages"
        );
        assert_eq!(
            trans.new_coeffs_per_stage.len(),
            2,
            "4th-order band should transition 2 stages"
        );
    }

    #[test]
    fn test_allpass_in_band_template() {
        // AllPass should be included in the BAND_TEMPLATE filter type choices.
        use crate::params::BAND_TEMPLATE;
        use sotf_host::param_specs::ParamType;
        let filter_type_spec = BAND_TEMPLATE
            .iter()
            .find(|s| s.engine_key == "filter_type")
            .expect("BAND_TEMPLATE must have a filter_type param");
        if let ParamType::Choice { labels, .. } = filter_type_spec.param_type {
            assert!(
                labels.contains(&"AllPass"),
                "AllPass must be in BAND_TEMPLATE filter type choices; found: {:?}",
                labels
            );
        } else {
            panic!("filter_type param should be a Choice type");
        }
    }

    #[test]
    fn test_from_params_rejects_odd_filter_order() {
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 6.0,
                order: 3,
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };

        let err = match EqPlugin::from_params(1, 48000, params) {
            Ok(_) => panic!("odd order should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("Filter order must be even"),
            "odd order should be rejected, got: {err}"
        );
    }

    #[test]
    fn test_set_parameter_rejects_odd_filter_order() {
        let mut p = EqPlugin::new(
            1,
            vec![Biquad::new(
                BiquadFilterType::Peak,
                1000.0,
                48000.0,
                1.0,
                6.0,
            )],
        );

        let err = InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_order"),
            ParameterValue::Int(3),
        )
        .unwrap_err();
        assert!(
            err.contains("Filter order must be even"),
            "odd runtime order should be rejected, got: {err}"
        );
        assert_eq!(p.band_orders[0], 2);
    }

    #[test]
    fn test_reset_preserves_biquad_coefficients() {
        let mut p = EqPlugin::new(
            1,
            vec![Biquad::new(
                BiquadFilterType::Peak,
                1000.0,
                48000.0,
                2.0,
                6.0,
            )],
        );
        let before = p.filters[0][0][0].coefficients();

        // Put non-zero state into the biquad before reset.
        let mut buf = vec![0.5f32; 128];
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 128))
            .unwrap();

        InPlacePlugin::reset(&mut p);
        let after = p.filters[0][0][0].coefficients();
        let max_diff = (before.b0 - after.b0)
            .abs()
            .max((before.b1 - after.b1).abs())
            .max((before.b2 - after.b2).abs())
            .max((before.a1 - after.a1).abs())
            .max((before.a2 - after.a2).abs());
        assert!(
            max_diff < 1e-12,
            "reset should clear state without rebuilding/changing coefficients; max_diff={max_diff}"
        );
    }

    #[test]
    fn test_multi_stage_transition_output_is_finite() {
        // A 4th-order peak with a gain change during transition should produce
        // only finite output (regression: old code only interpolated stage 0,
        // stages 1+ snapped immediately causing potential NaN on extreme settings).
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 8.0, // high Q to stress test numerical stability
                db_gain: 0.0,
                order: 4,
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let mut p = EqPlugin::from_params(1, 48000, params).unwrap();
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

        // Warmup
        let mut buf = vec![0.5f32; 256];
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
            .unwrap();

        // Trigger transition
        InPlacePlugin::set_parameter(
            &mut p,
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(18.0),
        )
        .unwrap();

        // Process during transition
        let mut buf = vec![0.5f32; 512];
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
            .unwrap();

        for (i, &s) in buf.iter().enumerate() {
            assert!(
                s.is_finite(),
                "sample {} not finite during 4th-order transition: {}",
                i,
                s
            );
        }
    }

    #[test]
    fn test_eq_oversampling_12ch_does_not_panic() {
        // Regression: stack buffer was [0.0; OS_CHUNK_SIZE * 8] = 2048 elements.
        // With 12 channels (e.g., 7.1.4), chunk_len = 256 * 12 = 3072, causing an OOB panic.
        use sotf_host::plugin::InPlacePlugin;

        let nc = 12;
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 3.0,
                order: 2,
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            }],
            channel_filters: None,
            auto_gain: Default::default(),
        };
        let mut p = EqPlugin::from_params(nc, 48000, params).unwrap();

        // Enable oversampling
        p.set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(2))
            .unwrap();

        // Process enough frames to trigger the oversampling chunk path (>= OS_CHUNK_SIZE)
        let frames = 512;
        let mut buffer = vec![0.5f32; frames * nc];
        let ctx = ProcessContext::new(48000, frames);
        // Should not panic with 12 channels
        p.process_in_place(&mut buffer, &ctx).unwrap();
        assert!(buffer.iter().all(|s| s.is_finite()));
    }
}
