// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================

use sotf_host::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use sotf_host::param_specs::{find_by_key as pk, fletcher_munson::PARAMS as FM};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

use math_audio_dsp::fast_math::fast_pow10;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum FletcherMunsonBandType {
    Lowshelf,
    Peak,
    Highshelf,
}

impl FletcherMunsonBandType {
    fn to_biquad_type(self) -> BiquadFilterType {
        match self {
            Self::Lowshelf => BiquadFilterType::Lowshelf,
            Self::Peak => BiquadFilterType::Peak,
            Self::Highshelf => BiquadFilterType::Highshelf,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FletcherMunsonBand {
    pub frequency: f64,
    pub q: f64,
    pub max_gain_db: f64,
    pub slope: f64,
    #[serde(default = "default_band_filter_type")]
    pub filter_type: FletcherMunsonBandType,
}

fn default_band_filter_type() -> FletcherMunsonBandType {
    FletcherMunsonBandType::Peak
}

impl FletcherMunsonBand {
    pub fn new(freq: f64, q: f64, max: f64, slp: f64, filter_type: FletcherMunsonBandType) -> Self {
        Self {
            frequency: freq,
            q,
            max_gain_db: max,
            slope: slp,
            filter_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FletcherMunsonPluginParams {
    pub playback_volume_db: f32,
    pub reference_level_db: f32,
    pub band1: Option<FletcherMunsonBand>,
    pub band2: Option<FletcherMunsonBand>,
    pub band3: Option<FletcherMunsonBand>,
    pub band4: Option<FletcherMunsonBand>,
    pub smoothing_ms: f32,
    pub enabled: bool,
    pub auto_gain_enabled: bool,
}

const NUM_BANDS: usize = 4;

// ============================================================================
// ISO 226:2003 Equal-Loudness Contour Model
// ============================================================================

/// ISO 226:2003 equal-loudness contour model parameters.
/// For a given frequency, the perceived loudness difference between two SPL levels
/// can be approximated as a frequency-dependent gain correction.
///
/// The model uses standardized reference frequencies and polynomial coefficients
/// to compute the loudness level in phons for a given SPL and frequency.
/// We approximate this with a multi-band parametric EQ where each band's gain
/// is derived from the ISO 226 contour difference between the reference and
/// playback SPL levels.
struct Iso226Model;

impl Iso226Model {
    /// ISO 226:2003 reference frequencies and approximate alpha_f (exponent) values.
    /// alpha_f represents how much the loudness contour deviates from flat at each freq.
    /// Higher alpha_f = more compensation needed at that frequency.
    ///
    /// Reference table (simplified from ISO 226:2003 Table 1):
    /// freq_hz, alpha_f, L_U (threshold of hearing in dB SPL), T_f (transfer function exponent)
    const CONTOUR_DATA: &[(f32, f32, f32, f32)] = &[
        (20.0, 0.532, 78.5, 74.3),
        (25.0, 0.506, 68.7, 65.0),
        (31.5, 0.480, 59.5, 56.3),
        (40.0, 0.455, 51.1, 48.4),
        (50.0, 0.434, 44.0, 41.7),
        (63.0, 0.414, 37.5, 35.5),
        (80.0, 0.396, 31.5, 29.8),
        (100.0, 0.380, 26.5, 25.1),
        (125.0, 0.367, 22.1, 20.7),
        (160.0, 0.356, 17.9, 16.8),
        (200.0, 0.349, 14.4, 13.8),
        (250.0, 0.345, 11.4, 11.2),
        (315.0, 0.343, 8.6, 8.5),
        (400.0, 0.343, 6.2, 6.1),
        (500.0, 0.346, 4.4, 4.4),
        (630.0, 0.349, 3.0, 3.0),
        (800.0, 0.354, 2.2, 2.2),
        (1000.0, 0.359, 2.4, 2.4),
        (1250.0, 0.367, 3.5, 3.5),
        (1600.0, 0.371, 1.7, 1.7),
        (2000.0, 0.370, -1.3, -1.3),
        (2500.0, 0.366, -4.2, -4.2),
        (3150.0, 0.359, -6.0, -6.0),
        (4000.0, 0.353, -5.4, -5.4),
        (5000.0, 0.348, -1.5, -1.5),
        (6300.0, 0.342, 6.0, 6.0),
        (8000.0, 0.340, 12.6, 12.6),
        (10000.0, 0.336, 13.9, 13.9),
        (12500.0, 0.337, 12.3, 12.3),
    ];

    /// Compute gain correction at a given frequency for a delta in SPL (dB).
    /// delta_db = reference_spl - playback_spl (positive means playing quieter).
    /// Returns the gain in dB to apply at this frequency to compensate.
    fn gain_at_freq(freq: f32, delta_db: f32) -> f32 {
        if delta_db <= 0.0 {
            return 0.0; // No compensation when playing at or above reference
        }

        // Find surrounding contour data points and interpolate
        let data = Self::CONTOUR_DATA;
        if freq <= data[0].0 {
            return Self::compute_gain(data[0].1, data[0].2, delta_db);
        }
        if freq >= data[data.len() - 1].0 {
            return Self::compute_gain(
                data[data.len() - 1].1,
                data[data.len() - 1].2,
                delta_db,
            );
        }

        // Binary search for interpolation
        let mut lo = 0;
        let mut hi = data.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if data[mid].0 <= freq {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        // Log-frequency interpolation
        let t = (freq.ln() - data[lo].0.ln()) / (data[hi].0.ln() - data[lo].0.ln());
        let alpha = data[lo].1 + t * (data[hi].1 - data[lo].1);
        let lu = data[lo].2 + t * (data[hi].2 - data[lo].2);
        Self::compute_gain(alpha, lu, delta_db)
    }

    /// Compute the gain correction from ISO 226 parameters.
    /// alpha_f: frequency-dependent exponent (higher = more nonlinear)
    /// l_u: threshold of hearing at this frequency
    /// delta_db: SPL difference (positive = quieter playback)
    fn compute_gain(alpha_f: f32, l_u: f32, delta_db: f32) -> f32 {
        // The ISO 226 model says: at lower SPLs, frequencies far from ~3-4kHz
        // need more boost. The alpha_f exponent controls this nonlinearity.
        // Simplified: gain ≈ delta * (alpha_f / 0.36 - 1) * scale
        // where 0.36 is the alpha at ~1kHz (reference), so deviation from flat.
        let deviation = (alpha_f / 0.359 - 1.0).abs(); // deviation from 1kHz behavior
        let base_gain = delta_db * deviation;

        // Apply threshold-of-hearing influence: frequencies with higher L_U
        // (harder to hear) get more compensation
        let threshold_factor = (l_u - 2.4).max(0.0) / 80.0; // normalized, 2.4 dB = L_U at 1kHz
        let total = base_gain + delta_db * threshold_factor * 0.3;

        total.min(20.0) // Clamp to prevent excessive boost
    }

    /// Compute ISO 226-based band gains for the 4-band parametric EQ.
    /// Returns [band1_gain, band2_gain, band3_gain, band4_gain] in dB.
    fn compute_band_gains(delta_db: f32) -> [f32; NUM_BANDS] {
        if delta_db <= 0.0 {
            return [0.0; NUM_BANDS];
        }

        // Evaluate the ISO 226 model at each band center frequency
        let band_freqs = [60.0, 250.0, 3500.0, 12000.0];
        let ref_gain = Self::gain_at_freq(1000.0, delta_db); // reference at 1kHz

        let mut gains = [0.0f32; NUM_BANDS];
        for (i, &freq) in band_freqs.iter().enumerate() {
            // Gain relative to 1kHz (which should have minimal correction)
            gains[i] = (Self::gain_at_freq(freq, delta_db) - ref_gain).max(0.0);
        }
        gains
    }
}

pub struct FletcherMunsonPlugin {
    num_channels: usize,
    sample_rate: u32,
    playback_volume_db: f32,
    reference_level_db: f32,
    bands: [FletcherMunsonBand; NUM_BANDS],
    enabled: bool,
    iso_226: bool,
    filters: Vec<Vec<Biquad>>,
    gain_smoothers: [Smoother; NUM_BANDS],
    compensation_smoother: Smoother,
    auto_gain: Option<AutoGain>,
    auto_gain_input_snapshot: Vec<f32>,
}

impl FletcherMunsonPlugin {
    pub fn new(num_channels: usize) -> Self {
        let sr = 44100;
        // Band 1 (sub-bass ~60 Hz) and Band 2 (mid-bass ~250 Hz) use lowshelf
        // filters for natural roll-off matching the equal-loudness contours.
        // Band 3 (presence ~3.5 kHz) uses a peak filter for targeted boost.
        // Band 4 (air ~12 kHz) uses a highshelf for natural treble compensation.
        let bands = [
            FletcherMunsonBand::new(
                pk(FM, "band1_freq").default_f64(),
                pk(FM, "band1_q").default_f64(),
                pk(FM, "band1_max_gain").default_f64(),
                pk(FM, "band1_slope").default_f64(),
                FletcherMunsonBandType::Lowshelf,
            ),
            FletcherMunsonBand::new(
                pk(FM, "band2_freq").default_f64(),
                pk(FM, "band2_q").default_f64(),
                pk(FM, "band2_max_gain").default_f64(),
                pk(FM, "band2_slope").default_f64(),
                FletcherMunsonBandType::Lowshelf,
            ),
            FletcherMunsonBand::new(
                pk(FM, "band3_freq").default_f64(),
                pk(FM, "band3_q").default_f64(),
                pk(FM, "band3_max_gain").default_f64(),
                pk(FM, "band3_slope").default_f64(),
                FletcherMunsonBandType::Peak,
            ),
            FletcherMunsonBand::new(
                pk(FM, "band4_freq").default_f64(),
                pk(FM, "band4_q").default_f64(),
                pk(FM, "band4_max_gain").default_f64(),
                pk(FM, "band4_slope").default_f64(),
                FletcherMunsonBandType::Highshelf,
            ),
        ];

        let mut p = Self {
            num_channels,
            sample_rate: sr,
            playback_volume_db: pk(FM, "playback_volume_db").default_f64() as f32,
            reference_level_db: pk(FM, "reference_level_db").default_f64() as f32,
            bands,
            enabled: true,
            iso_226: false,
            filters: vec![Vec::with_capacity(NUM_BANDS); num_channels],
            gain_smoothers: [Smoother::new(0.0, 50.0, sr); NUM_BANDS],
            compensation_smoother: Smoother::new(1.0, 50.0, sr),
            auto_gain: None,
            auto_gain_input_snapshot: vec![0.0; 4096 * num_channels],
        };

        p.rebuild_filters();
        p.update_band_targets();
        p
    }

    fn update_band_targets(&mut self) {
        let delta = self.reference_level_db - self.playback_volume_db;
        let mut max_g = 0.0f32;

        if self.iso_226 {
            // ISO 226:2003 model: compute frequency-dependent gains
            let iso_gains = Iso226Model::compute_band_gains(delta);
            for (i, &iso_g) in iso_gains.iter().enumerate() {
                let g = iso_g.min(self.bands[i].max_gain_db as f32);
                self.gain_smoothers[i].set_target(g);
                max_g = max_g.max(g);
            }
        } else {
            // Original 4-band parametric approach
            for i in 0..NUM_BANDS {
                let g = if delta <= 0.0 {
                    0.0
                } else {
                    (self.bands[i].slope as f32 * delta).min(self.bands[i].max_gain_db as f32)
                };
                self.gain_smoothers[i].set_target(g);
                max_g = max_g.max(g);
            }
        }

        self.compensation_smoother
            .set_target(fast_pow10(-max_g / 20.0));
    }

    fn rebuild_filters(&mut self) {
        let sr = self.sample_rate as f64;
        for ch in 0..self.num_channels {
            self.filters[ch].clear();
            for i in 0..NUM_BANDS {
                let b = &self.bands[i];
                self.filters[ch].push(Biquad::new(
                    b.filter_type.to_biquad_type(),
                    b.frequency,
                    sr,
                    b.q,
                    self.gain_smoothers[i].current() as f64,
                ));
            }
        }
    }

    pub fn from_params(
        num_channels: usize,
        params: FletcherMunsonPluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(num_channels);
        p.playback_volume_db = params.playback_volume_db;
        p.reference_level_db = params.reference_level_db;
        p.enabled = params.enabled;

        if params.auto_gain_enabled {
            p.auto_gain = Some(AutoGain::new(
                num_channels,
                p.sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type: AutoGainLoudnessType::Momentary,
                    max_gain_db: pk(FM, "auto_gain_max_db").default_f64() as f32,
                    smoothing_ms: pk(FM, "auto_gain_smoothing_ms").default_f64() as f32,
                },
            )?);
        }
        p.update_band_targets();
        Ok(p)
    }
}

impl InPlacePlugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Fletcher-Munson", "2.0.0", "Sotf")
            .with_description("Loudness-dependent frequency compensation")
    }

    fn channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        let mut params = vec![
            Parameter::new_float(
                "playback_volume_db",
                "Playback Volume",
                self.playback_volume_db,
                pk(FM, "playback_volume_db").min_f64() as f32,
                pk(FM, "playback_volume_db").max_f64() as f32,
            )
            .with_group("Levels")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "reference_level_db",
                "Reference Level",
                self.reference_level_db,
                pk(FM, "reference_level_db").min_f64() as f32,
                pk(FM, "reference_level_db").max_f64() as f32,
            )
            .with_group("Levels"),
            Parameter::new_bool("enabled", "Enabled", self.enabled).with_group("Control"),
            Parameter::new_bool("iso_226", "ISO 226:2003", self.iso_226).with_group("Control"),
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", self.auto_gain.is_some())
                .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_max_db",
                "AG Max",
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.max_gain_db())
                    .unwrap_or(pk(FM, "auto_gain_max_db").default_f32()),
                pk(FM, "auto_gain_max_db").min_f64() as f32,
                pk(FM, "auto_gain_max_db").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "AG Smoothing",
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.smoothing_ms())
                    .unwrap_or(pk(FM, "auto_gain_smoothing_ms").default_f32()),
                pk(FM, "auto_gain_smoothing_ms").min_f64() as f32,
                pk(FM, "auto_gain_smoothing_ms").max_f64() as f32,
            )
            .with_group("Auto Gain"),
        ];
        for (i, band) in self.bands.iter().enumerate() {
            let group = format!("Band {}", i + 1);
            let keys = [
                ("freq", "Freq", band.frequency as f32),
                ("q", "Q", band.q as f32),
                ("max_gain", "Max Gain", band.max_gain_db as f32),
                ("slope", "Slope", band.slope as f32),
            ];
            for (suffix, label, val) in keys {
                let key = format!("band{}_{}", i + 1, suffix);
                params.push(
                    Parameter::new_float(
                        &key,
                        label,
                        val,
                        pk(FM, &key).min_f64() as f32,
                        pk(FM, &key).max_f64() as f32,
                    )
                    .with_group(&group),
                );
            }
        }
        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let name = id.0.as_str();
        if name == "playback_volume_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "playback_volume_db must be a float".to_string())?;
            if v.is_finite() {
                self.playback_volume_db = v;
                self.update_band_targets();
            }
        } else if name == "reference_level_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "reference_level_db must be a float".to_string())?;
            if v.is_finite() {
                self.reference_level_db = v;
                self.update_band_targets();
            }
        } else if name == "enabled" {
            self.enabled = value
                .as_bool()
                .ok_or_else(|| "enabled must be a boolean".to_string())?;
        } else if name == "iso_226" {
            self.iso_226 = value
                .as_bool()
                .ok_or_else(|| "iso_226 must be a boolean".to_string())?;
            self.update_band_targets();
        } else if name == "auto_gain_enabled" {
            let v = value
                .as_bool()
                .ok_or_else(|| "auto_gain_enabled must be a boolean".to_string())?;
            if v && self.auto_gain.is_none() {
                self.auto_gain = Some(AutoGain::new(
                    self.num_channels,
                    self.sample_rate,
                    AutoGainParams {
                        enabled: true,
                        loudness_type: AutoGainLoudnessType::Momentary,
                        max_gain_db: pk(FM, "auto_gain_max_db").default_f32(),
                        smoothing_ms: pk(FM, "auto_gain_smoothing_ms").default_f32(),
                    },
                )?);
            } else if !v {
                self.auto_gain = None;
            }
        } else if name == "auto_gain_max_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_max_db must be a float".to_string())?;
            if v.is_finite()
                && let Some(ag) = &mut self.auto_gain
            {
                ag.set_max_gain_db(v);
            }
        } else if name == "auto_gain_smoothing_ms" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_smoothing_ms must be a float".to_string())?;
            if v.is_finite()
                && let Some(ag) = &mut self.auto_gain
            {
                ag.set_smoothing_ms(v);
            }
        } else if name.starts_with("band") && name.len() > 5 {
            // Parse band parameter: band1_freq, band2_q, etc.
            let v = value
                .as_float()
                .ok_or_else(|| format!("{} must be a float", name))?;
            if v.is_finite() {
                let band_idx = name.as_bytes()[4] - b'1';
                if band_idx < NUM_BANDS as u8 {
                    let field = &name[6..]; // skip "bandN_"
                    let band = &mut self.bands[band_idx as usize];
                    match field {
                        "freq" => {
                            band.frequency = v as f64;
                            self.rebuild_filters();
                        }
                        "q" => {
                            band.q = v as f64;
                            self.rebuild_filters();
                        }
                        "max_gain" => {
                            band.max_gain_db = v as f64;
                            self.update_band_targets();
                        }
                        "slope" => {
                            band.slope = v as f64;
                            self.update_band_targets();
                        }
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                } else {
                    return Err(format!("Band index out of range: {}", name));
                }
            }
        } else {
            return Err(format!("Unknown: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = id.0.as_str();
        if name == "playback_volume_db" {
            Some(ParameterValue::Float(self.playback_volume_db))
        } else if name == "reference_level_db" {
            Some(ParameterValue::Float(self.reference_level_db))
        } else if name == "enabled" {
            Some(ParameterValue::Bool(self.enabled))
        } else if name == "iso_226" {
            Some(ParameterValue::Bool(self.iso_226))
        } else if name == "auto_gain_enabled" {
            Some(ParameterValue::Bool(self.auto_gain.is_some()))
        } else if name == "auto_gain_max_db" {
            Some(ParameterValue::Float(
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.max_gain_db())
                    .unwrap_or(pk(FM, "auto_gain_max_db").default_f32()),
            ))
        } else if name == "auto_gain_smoothing_ms" {
            Some(ParameterValue::Float(
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.smoothing_ms())
                    .unwrap_or(pk(FM, "auto_gain_smoothing_ms").default_f32()),
            ))
        } else if name.starts_with("band") && name.len() > 5 {
            let band_idx = name.as_bytes()[4].wrapping_sub(b'1');
            if band_idx < NUM_BANDS as u8 {
                let field = &name[6..];
                let band = &self.bands[band_idx as usize];
                match field {
                    "freq" => Some(ParameterValue::Float(band.frequency as f32)),
                    "q" => Some(ParameterValue::Float(band.q as f32)),
                    "max_gain" => Some(ParameterValue::Float(band.max_gain_db as f32)),
                    "slope" => Some(ParameterValue::Float(band.slope as f32)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        for s in &mut self.gain_smoothers {
            s.set_time(50.0, sr);
        }
        self.compensation_smoother.set_time(50.0, sr);
        self.rebuild_filters();
        if let Some(ag) = &mut self.auto_gain {
            ag.set_sample_rate(sr).map_err(|e| e.to_string())?;
        }
        self.auto_gain_input_snapshot
            .resize(4096 * self.num_channels, 0.0);
        Ok(())
    }

    fn reset(&mut self) {
        self.rebuild_filters();
        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if !self.enabled {
            return Ok(context.num_frames);
        }
        enable_ftz_daz();
        let nf = context.num_frames;

        if let Some(ag) = &mut self.auto_gain {
            let len = nf * self.num_channels;
            if self.auto_gain_input_snapshot.len() < len {
                self.auto_gain_input_snapshot.resize(len, 0.0);
            }
            self.auto_gain_input_snapshot[..len].copy_from_slice(&buffer[..len]);
            let _ = ag.measure_input(&self.auto_gain_input_snapshot[..len]);
        }

        // Update filters if gain changed significantly
        let mut gains = [0.0f32; NUM_BANDS];
        let mut changed = false;
        for (i, smoother) in self.gain_smoothers.iter_mut().enumerate().take(NUM_BANDS) {
            gains[i] = smoother.advance();
            if (gains[i] - self.filters[0][i].db_gain as f32).abs() > 0.05 {
                changed = true;
            }
        }
        if changed {
            let sr = self.sample_rate as f64;
            for ch in 0..self.num_channels {
                for (filter, (band, &gain)) in self.filters[ch]
                    .iter_mut()
                    .zip(self.bands.iter().zip(gains.iter()))
                    .take(NUM_BANDS)
                {
                    filter.update_params(
                        band.filter_type.to_biquad_type(),
                        band.frequency,
                        sr,
                        band.q,
                        gain as f64,
                    );
                }
            }
        }

        let comp = self.compensation_smoother.next_n(nf);
        for frame in 0..nf {
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let mut s = buffer[idx] as f64;
                for f in &mut self.filters[ch] {
                    s = f.process(s);
                }
                buffer[idx] = (s as f32) * comp;
            }
        }

        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_output(buffer);
            ag.apply_compensation(buffer, nf);
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

impl sotf_host::plugin::Plugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        InPlacePlugin::info(self)
    }
    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        InPlacePlugin::parameters(self)
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        InPlacePlugin::set_parameter(self, id, val)
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        InPlacePlugin::get_parameter(self, id)
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        InPlacePlugin::initialize(self, sr)
    }
    fn reset(&mut self) {
        InPlacePlugin::reset(self)
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        self.process_in_place(output, context)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::plugin::{InPlacePlugin, ProcessContext};
    #[test]
    fn test_fm_basic() {
        let mut p = FletcherMunsonPlugin::new(1);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        let mut b = vec![0.5; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999].is_finite());
    }
}
