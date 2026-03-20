// ============================================================================
// Multiband Expander Plugin
// ============================================================================

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_makeup::MeasuredMakeup;
use sotf_host::detector::{DetectionMode, LevelDetector};
use sotf_host::param_specs::{
    find_by_key as pk,
    multiband_expander::{BAND_TEMPLATE as MEB, GLOBAL_PARAMS as ME},
};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};
use std::any::Any;
use std::sync::Arc;

pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),
    (100.0, 3000.0, 8000.0, 12000.0),
    (250.0, 4000.0, 10000.0, 14000.0),
];

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandExpanderParams {
    pub threshold_db: Option<f32>,
    pub ratio: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub knee_db: Option<f32>,
    pub range_db: Option<f32>,
    pub hysteresis_db: Option<f32>,
    pub hold_ms: Option<f32>,
    #[serde(default)]
    pub auto_makeup: bool,
    #[serde(default)]
    pub measured_auto_makeup: bool,
    #[serde(default = "default_true")]
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
}

impl Default for BandExpanderParams {
    fn default() -> Self {
        Self {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            range_db: None,
            hysteresis_db: None,
            hold_ms: None,
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: false,
            bypass: false,
        }
    }
}

fn default_detection_mode() -> String {
    "peak".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultibandExpanderPluginParams {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_frequencies: Vec<f32>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub range_db: f32,
    pub hysteresis_db: f32,
    pub hold_ms: f32,
    pub link_channels: bool,
    pub mix: f32,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    pub bands: Vec<BandExpanderParams>,
}

#[derive(Debug, Clone)]
pub struct MultibandExpanderData {
    /// Attenuation per band and per channel (flattened)
    pub attenuation_db: Arc<Vec<f32>>,
    pub is_open: Arc<Vec<bool>>,
    pub band_levels_db: Arc<Vec<f32>>,
    pub crossover_frequencies: Arc<Vec<f32>>,
}

impl Default for MultibandExpanderData {
    fn default() -> Self {
        Self {
            attenuation_db: Arc::new(Vec::new()),
            is_open: Arc::new(Vec::new()),
            band_levels_db: Arc::new(Vec::new()),
            crossover_frequencies: Arc::new(Vec::new()),
        }
    }
}

impl MultibandExpanderData {
    pub fn new(num_bands: usize, channels: usize) -> Self {
        Self {
            attenuation_db: Arc::new(vec![0.0; num_bands * channels]),
            is_open: Arc::new(vec![false; num_bands]),
            band_levels_db: Arc::new(vec![-120.0; num_bands]),
            crossover_frequencies: Arc::new(vec![0.0; num_bands.saturating_sub(1)]),
        }
    }

    pub fn update(&mut self, atten: &[f32], open: &[bool], levels: &[f32], xovers: &[f32]) {
        if let Some(mut_atten) = Arc::get_mut(&mut self.attenuation_db)
            && mut_atten.len() == atten.len()
        {
            mut_atten.copy_from_slice(atten);
        }
        if let Some(mut_open) = Arc::get_mut(&mut self.is_open)
            && mut_open.len() == open.len()
        {
            mut_open.copy_from_slice(open);
        }
        if let Some(mut_levels) = Arc::get_mut(&mut self.band_levels_db)
            && mut_levels.len() == levels.len()
        {
            mut_levels.copy_from_slice(levels);
        }
        if let Some(mut_xovers) = Arc::get_mut(&mut self.crossover_frequencies)
            && mut_xovers.len() == xovers.len()
        {
            mut_xovers.copy_from_slice(xovers);
        }
    }
}

struct CrossoverPoint {
    lowpass: Vec<Vec<Biquad>>,
    highpass: Vec<Vec<Biquad>>,
    freq: f32,
}

impl CrossoverPoint {
    fn new(channels: usize, freq: f32, sr: u32) -> Self {
        let q = 1.0 / std::f64::consts::SQRT_2;
        let mut lp = Vec::with_capacity(channels);
        let mut hp = Vec::with_capacity(channels);
        for _ in 0..channels {
            lp.push(vec![
                Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0),
                Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0),
            ]);
            hp.push(vec![
                Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0),
                Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0),
            ]);
        }
        Self {
            lowpass: lp,
            highpass: hp,
            freq,
        }
    }
    fn process_lowpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.lowpass[ch] {
            s = f.process(s as f64) as f32;
        }
        s
    }
    fn process_highpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.highpass[ch] {
            s = f.process(s as f64) as f32;
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Hold,
    Closing,
}

struct BandExpander {
    envelope: Vec<f32>,
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
}

impl BandExpander {
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
    }
}

pub struct MultibandExpanderPlugin {
    channels: usize,
    sample_rate: u32,
    num_bands: usize,
    _crossover_preset: i32,
    crossover_frequencies: Vec<f32>,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    range_db: f32,
    hysteresis_db: f32,
    hold_ms: f32,
    link_channels: bool,
    mix: f32,
    detection_mode: String,
    band_params: Vec<BandExpanderParams>,
    crossover_points: Vec<CrossoverPoint>,
    band_expanders: Vec<BandExpander>,
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,
    dry_buffer: Vec<f32>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,

    /// Per-band measured auto-makeup gain trackers.
    measured_makeups: Vec<MeasuredMakeup>,

    /// Per-band, per-channel level detectors for RMS mode.
    level_detectors: Vec<Vec<LevelDetector>>,

    // Internal flattened monitoring buffers
    attenuation_flattened: Vec<f32>,
    is_open_buffer: Vec<bool>,
    cache: RealTimeCache<MultibandExpanderData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

fn parse_detection_mode(s: &str) -> DetectionMode {
    match s {
        "rms" => DetectionMode::Rms { window_ms: 10.0 },
        _ => DetectionMode::Peak,
    }
}

impl MultibandExpanderPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, Default::default())
    }
    pub fn with_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        let nb = params.num_bands.clamp(
            pk(ME, "num_bands").min_f64() as usize,
            pk(ME, "num_bands").max_f64() as usize,
        );
        let sr = 44100;
        let mut xfs = params.crossover_frequencies.clone();
        while xfs.len() < 4 {
            xfs.push(1000.0);
        }
        let mut bexps = Vec::with_capacity(nb);
        for _ in 0..nb {
            bexps.push(BandExpander {
                envelope: vec![0.0; channels],
                gate_state: vec![GateState::Open; channels],
                hold_counter: vec![0; channels],
                attack_coeff: 0.0,
                release_coeff: 0.0,
            });
        }

        let mut band_params = params.bands;
        while band_params.len() < nb {
            band_params.push(BandExpanderParams::default());
        }

        let measured_makeups = (0..nb)
            .map(|_| MeasuredMakeup::new(1000.0, sr))
            .collect();

        let det_mode_str = if params.detection_mode.is_empty() {
            "peak"
        } else {
            &params.detection_mode
        };
        let det_mode = parse_detection_mode(det_mode_str);
        let level_detectors = (0..nb)
            .map(|_| {
                (0..channels)
                    .map(|_| LevelDetector::new(det_mode, sr))
                    .collect()
            })
            .collect();

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands: nb,
            _crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(),
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            range_db: params.range_db,
            hysteresis_db: params.hysteresis_db,
            hold_ms: params.hold_ms,
            link_channels: params.link_channels,
            mix: params.mix,
            detection_mode: det_mode_str.to_string(),
            band_params,
            crossover_points: Vec::new(),
            band_expanders: bexps,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb],
            dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
            measured_makeups,
            level_detectors,
            attenuation_flattened: vec![0.0; nb * channels],
            is_open_buffer: vec![false; nb],
            cache: RealTimeCache::new(MultibandExpanderData::new(nb, channels)),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.build_crossovers();
        p.update_coefficients();
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        let det_mode_idx = if self.detection_mode == "rms" { 1 } else { 0 };
        let mut params = vec![
            Parameter::new_int(
                "num_bands",
                "Bands",
                self.num_bands as i32,
                pk(ME, "num_bands").min_f64() as i32,
                pk(ME, "num_bands").max_f64() as i32,
            )
            .with_group("General")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(ME, "mix").min_f64() as f32,
                pk(ME, "mix").max_f64() as f32,
            )
            .with_group("General")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_int("detection_mode", "Detection Mode", det_mode_idx, 0, 1)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
        ];

        // Crossover frequencies
        for i in 0..(self.num_bands - 1) {
            let name = format!("crossover_freq_{}", i + 1);
            let label = format!("X-Over {}", i + 1);
            params.push(
                Parameter::new_float(&name, &label, self.crossover_frequencies[i], 20.0, 20000.0)
                    .with_group("Crossover"),
            );
        }

        // Global dynamics (defaults for bands)
        params.extend(vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(ME, "threshold").min_f64() as f32,
                pk(ME, "threshold").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(ME, "ratio").min_f64() as f32,
                pk(ME, "ratio").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(ME, "attack").min_f64() as f32,
                pk(ME, "attack").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(ME, "release").min_f64() as f32,
                pk(ME, "release").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
        ]);

        // Per-band dynamics
        for i in 0..self.num_bands {
            let group = format!("Band {}", i + 1);
            let bp = &self.band_params[i];

            params.push(
                Parameter::new_float(
                    &format!("band_{}_threshold", i),
                    "Threshold",
                    bp.threshold_db.unwrap_or(self.threshold_db),
                    pk(MEB, "threshold").min_f64() as f32,
                    pk(MEB, "threshold").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_ratio", i),
                    "Ratio",
                    bp.ratio.unwrap_or(self.ratio),
                    pk(MEB, "ratio").min_f64() as f32,
                    pk(MEB, "ratio").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_attack", i),
                    "Attack",
                    bp.attack_ms.unwrap_or(self.attack_ms),
                    pk(MEB, "attack").min_f64() as f32,
                    pk(MEB, "attack").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_release", i),
                    "Release",
                    bp.release_ms.unwrap_or(self.release_ms),
                    pk(MEB, "release").min_f64() as f32,
                    pk(MEB, "release").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_knee", i),
                    "Knee",
                    bp.knee_db.unwrap_or(self.knee_db),
                    pk(MEB, "knee").min_f64() as f32,
                    pk(MEB, "knee").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_range", i),
                    "Range",
                    bp.range_db.unwrap_or(self.range_db),
                    pk(MEB, "range").min_f64() as f32,
                    pk(MEB, "range").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_hysteresis", i),
                    "Hysteresis",
                    bp.hysteresis_db.unwrap_or(self.hysteresis_db),
                    pk(MEB, "hysteresis").min_f64() as f32,
                    pk(MEB, "hysteresis").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_hold", i),
                    "Hold",
                    bp.hold_ms.unwrap_or(self.hold_ms),
                    pk(MEB, "hold").min_f64() as f32,
                    pk(MEB, "hold").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_auto_makeup", i),
                    "Auto Makeup",
                    bp.auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_measured_auto_makeup", i),
                    "Measured Auto Makeup",
                    bp.measured_auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_active", i), "Active", bp.active)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_solo", i), "Solo", bp.solo)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_bypass", i), "Bypass", bp.bypass)
                    .with_group(&group),
            );
        }

        self.cached_parameters = params;
    }
    pub fn from_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points
                .push(CrossoverPoint::new(self.channels, f, self.sample_rate));
        }
    }

    fn update_coefficients(&mut self) {
        for (i, b) in self.band_expanders.iter_mut().enumerate() {
            let a = self
                .band_params
                .get(i)
                .and_then(|p| p.attack_ms)
                .unwrap_or(self.attack_ms);
            let r = self
                .band_params
                .get(i)
                .and_then(|p| p.release_ms)
                .unwrap_or(self.release_ms);
            b.attack_coeff = (-1.0 / (a * 0.001 * self.sample_rate as f32)).exp();
            b.release_coeff = (-1.0 / (r * 0.001 * self.sample_rate as f32)).exp();
        }
    }

    fn calculate_expansion_attenuation(
        idb: f32,
        th: f32,
        ratio: f32,
        knee: f32,
        range: f32,
    ) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        let atten = if knee < 0.1 {
            if idb >= th { 0.0 } else { (th - idb) * slope }
        } else if idb > th + knee / 2.0 {
            0.0
        } else if idb < th - knee / 2.0 {
            (th - idb) * slope
        } else {
            let b = th + knee / 2.0 - idb;
            let kf = b / knee;
            kf * kf * (knee / 2.0) * slope
        };
        atten.min(range)
    }
}

impl InPlacePlugin for MultibandExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Expander", "1.1.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = &id.0;

        if name == "num_bands" {
            let nb = value
                .as_int()
                .ok_or_else(|| "Bands must be an integer".to_string())?
                as usize;
            let nb = nb.clamp(
                pk(ME, "num_bands").min_f64() as usize,
                pk(ME, "num_bands").max_f64() as usize,
            );
            if nb != self.num_bands {
                self.num_bands = nb;
                self.build_crossovers();
                while self.band_params.len() < self.num_bands {
                    self.band_params.push(BandExpanderParams::default());
                }
                while self.band_expanders.len() < self.num_bands {
                    self.band_expanders.push(BandExpander {
                        envelope: vec![0.0; self.channels],
                        gate_state: vec![GateState::Open; self.channels],
                        hold_counter: vec![0; self.channels],
                        attack_coeff: 0.0,
                        release_coeff: 0.0,
                    });
                }
                while self.measured_makeups.len() < self.num_bands {
                    self.measured_makeups
                        .push(MeasuredMakeup::new(1000.0, self.sample_rate));
                }
                let det_mode = parse_detection_mode(&self.detection_mode);
                while self.level_detectors.len() < self.num_bands {
                    self.level_detectors.push(
                        (0..self.channels)
                            .map(|_| LevelDetector::new(det_mode, self.sample_rate))
                            .collect(),
                    );
                }
                self.band_levels_db.resize(self.num_bands, -100.0);
                self.attenuation_flattened
                    .resize(self.num_bands * self.channels, 0.0);
                self.is_open_buffer.resize(self.num_bands, false);
                self.update_coefficients();
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        self.validate_parameter(&id, &value)?;

        if name == "link_channels" {
            self.link_channels = value
                .as_bool()
                .ok_or_else(|| "link_channels must be a boolean".to_string())?;
        } else if name == "mix" {
            let v = value
                .as_float()
                .ok_or_else(|| "mix must be a float".to_string())?;
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if name == "detection_mode" {
            let idx = value
                .as_int()
                .ok_or_else(|| "detection_mode must be an integer".to_string())?;
            let mode_str = if idx == 1 { "rms" } else { "peak" };
            if mode_str != self.detection_mode {
                self.detection_mode = mode_str.to_string();
                let det_mode = parse_detection_mode(mode_str);
                for band_dets in &mut self.level_detectors {
                    for det in band_dets {
                        det.set_mode(det_mode);
                    }
                }
            }
        } else if name.starts_with("crossover_freq_") {
            let idx = name
                .replace("crossover_freq_", "")
                .parse::<usize>()
                .map_err(|e| format!("Invalid crossover index: {}", e))?
                .checked_sub(1)
                .ok_or_else(|| "Crossover index must be at least 1".to_string())?;

            if idx < self.xover_smoothers.len() {
                let f = value
                    .as_float()
                    .ok_or_else(|| format!("{} must be a float", name))?;
                if f.is_finite() {
                    self.crossover_frequencies[idx] = f;
                    self.xover_smoothers[idx].set_target(f);
                }
            } else {
                return Err(format!("Crossover index {} out of range", idx + 1));
            }
        } else if name == "threshold" {
            let v = value
                .as_float()
                .ok_or_else(|| "threshold must be a float".to_string())?;
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if name == "ratio" {
            let v = value
                .as_float()
                .ok_or_else(|| "ratio must be a float".to_string())?;
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if name == "attack" {
            let v = value
                .as_float()
                .ok_or_else(|| "attack must be a float".to_string())?;
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if name == "release" {
            let v = value
                .as_float()
                .ok_or_else(|| "release must be a float".to_string())?;
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1]
                    .parse::<usize>()
                    .map_err(|e| format!("Invalid band index: {}", e))?;
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &mut self.band_params[b_idx];
                    match field {
                        "threshold" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.threshold_db = Some(v);
                            }
                        }
                        "ratio" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.ratio = Some(v);
                            }
                        }
                        "attack" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.attack_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "release" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.release_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "knee" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.knee_db = Some(v);
                            }
                        }
                        "range" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.range_db = Some(v);
                            }
                        }
                        "hysteresis" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.hysteresis_db = Some(v);
                            }
                        }
                        "hold" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.hold_ms = Some(v);
                            }
                        }
                        "auto" => {
                            bp.auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "measured" => {
                            bp.measured_auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "active" => {
                            bp.active = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "solo" => {
                            bp.solo = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "bypass" => {
                            bp.bypass = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                } else {
                    return Err(format!("Band index {} out of range", b_idx));
                }
            }
        } else {
            // Support legacy names or other fields
            match name.as_str() {
                "knee" => {
                    let v = value
                        .as_float()
                        .ok_or_else(|| "knee must be a float".to_string())?;
                    if v.is_finite() {
                        self.knee_db = v;
                    }
                }
                "range" => {
                    let v = value
                        .as_float()
                        .ok_or_else(|| "range must be a float".to_string())?;
                    if v.is_finite() {
                        self.range_db = v;
                    }
                }
                "hysteresis" => {
                    let v = value
                        .as_float()
                        .ok_or_else(|| "hysteresis must be a float".to_string())?;
                    if v.is_finite() {
                        self.hysteresis_db = v;
                    }
                }
                "hold" => {
                    let v = value
                        .as_float()
                        .ok_or_else(|| "hold must be a float".to_string())?;
                    if v.is_finite() {
                        self.hold_ms = v;
                    }
                }
                _ => return Err(format!("Unknown parameter: {}", id)),
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = &id.0;
        if name == "num_bands" {
            Some(ParameterValue::Int(self.num_bands as i32))
        } else if name == "link_channels" {
            Some(ParameterValue::Bool(self.link_channels))
        } else if name == "mix" {
            Some(ParameterValue::Float(self.mix))
        } else if name == "detection_mode" {
            let idx = if self.detection_mode == "rms" { 1 } else { 0 };
            Some(ParameterValue::Int(idx))
        } else if name == "threshold" {
            Some(ParameterValue::Float(self.threshold_db))
        } else if name == "ratio" {
            Some(ParameterValue::Float(self.ratio))
        } else if name == "attack" {
            Some(ParameterValue::Float(self.attack_ms))
        } else if name == "release" {
            Some(ParameterValue::Float(self.release_ms))
        } else if name.starts_with("crossover_freq_") {
            let idx = name
                .replace("crossover_freq_", "")
                .parse::<usize>()
                .map(|i| i - 1)
                .unwrap_or(0);
            self.crossover_frequencies
                .get(idx)
                .map(|&f| ParameterValue::Float(f))
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &self.band_params[b_idx];
                    match field {
                        "threshold" => Some(ParameterValue::Float(
                            bp.threshold_db.unwrap_or(self.threshold_db),
                        )),
                        "ratio" => Some(ParameterValue::Float(bp.ratio.unwrap_or(self.ratio))),
                        "attack" => Some(ParameterValue::Float(
                            bp.attack_ms.unwrap_or(self.attack_ms),
                        )),
                        "release" => Some(ParameterValue::Float(
                            bp.release_ms.unwrap_or(self.release_ms),
                        )),
                        "knee" => Some(ParameterValue::Float(bp.knee_db.unwrap_or(self.knee_db))),
                        "range" => {
                            Some(ParameterValue::Float(bp.range_db.unwrap_or(self.range_db)))
                        }
                        "hysteresis" => Some(ParameterValue::Float(
                            bp.hysteresis_db.unwrap_or(self.hysteresis_db),
                        )),
                        "hold" => Some(ParameterValue::Float(bp.hold_ms.unwrap_or(self.hold_ms))),
                        "auto" => Some(ParameterValue::Bool(bp.auto_makeup)),
                        "measured" => Some(ParameterValue::Bool(bp.measured_auto_makeup)),
                        "active" => Some(ParameterValue::Bool(bp.active)),
                        "solo" => Some(ParameterValue::Bool(bp.solo)),
                        "bypass" => Some(ParameterValue::Bool(bp.bypass)),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            match name.as_str() {
                "knee" => Some(ParameterValue::Float(self.knee_db)),
                "range" => Some(ParameterValue::Float(self.range_db)),
                "hysteresis" => Some(ParameterValue::Float(self.hysteresis_db)),
                "hold" => Some(ParameterValue::Float(self.hold_ms)),
                _ => None,
            }
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.build_crossovers();
        self.update_coefficients();
        self.threshold_smoother.set_time(20.0, sr);
        self.mix_smoother.set_time(20.0, sr);
        for s in &mut self.xover_smoothers {
            *s = LogSmoother::new(s.target(), 50.0, sr);
        }

        // Reinitialize measured makeup smoothing for new sample rate
        for mm in &mut self.measured_makeups {
            mm.set_smoothing(1000.0, sr);
        }

        // Reinitialize level detectors for new sample rate
        let det_mode = parse_detection_mode(&self.detection_mode);
        for band_dets in &mut self.level_detectors {
            for det in band_dets {
                *det = LevelDetector::new(det_mode, sr);
            }
        }

        // Pre-allocate buffers for real-time safety
        let max_frames = 4096; // Standard max block size
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);

        Ok(())
    }
    fn reset(&mut self) {
        for b in &mut self.band_expanders {
            b.reset();
        }
        for mm in &mut self.measured_makeups {
            mm.reset();
        }
        for band_dets in &mut self.level_detectors {
            for det in band_dets {
                det.reset();
            }
        }
        self.band_buffers.fill(0.0);
        self.dry_buffer.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let stride = nf * self.channels;

        // Ensure buffers are large enough (usually a no-op due to initialize)
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        if self.band_buffers.len() < self.num_bands * stride {
            self.band_buffers.resize(self.num_bands * stride, 0.0);
        }

        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // 1. Update crossovers
        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next_n(nf);
            if (freq - self.crossover_points[i].freq).abs() > 1.0 {
                self.crossover_points[i].freq = freq;
                let q = 1.0 / std::f64::consts::SQRT_2;
                let sr = self.sample_rate as f64;
                for ch in 0..self.channels {
                    for f in &mut self.crossover_points[i].lowpass[ch] {
                        *f = Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr, q, 0.0);
                    }
                    for f in &mut self.crossover_points[i].highpass[ch] {
                        *f = Biquad::new(BiquadFilterType::Highpass, freq as f64, sr, q, 0.0);
                    }
                }
            }
        }

        // 2. Perform Crossover Splitting
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut rem = buffer[idx];
                for xidx in 0..(self.num_bands - 1) {
                    let low = self.crossover_points[xidx].process_lowpass(ch, rem);
                    self.band_buffers[xidx * stride + idx] = low;
                    rem = self.crossover_points[xidx].process_highpass(ch, rem);
                }
                self.band_buffers[(self.num_bands - 1) * stride + idx] = rem;
            }
        }

        let g_th = self.threshold_smoother.next_n(nf);
        let g_mix = self.mix_smoother.next_n(nf);

        let mut any_solo = false;
        for b in 0..self.num_bands {
            if let Some(p) = self.band_params.get(b)
                && p.solo
            {
                any_solo = true;
                break;
            }
        }

        // 3. Dynamic Processing per Band
        for b in 0..self.num_bands {
            let bp = self.band_params.get(b);
            let is_bypassed = bp.map(|p| p.bypass).unwrap_or(false);
            let is_passive = !bp.map(|p| p.active).unwrap_or(true);
            let is_muted = any_solo && !bp.map(|p| p.solo).unwrap_or(false);

            if is_muted {
                let off = b * stride;
                self.band_buffers[off..off + stride].fill(0.0);
                self.band_levels_db[b] = -100.0;
                continue;
            }

            if is_bypassed || is_passive {
                // Keep band signal as is, but still track levels
                let off = b * stride;
                let mut max_abs = 0.0f32;
                for i in 0..stride {
                    max_abs = max_abs.max(self.band_buffers[off + i].abs());
                }
                self.band_levels_db[b] = 20.0 * fast_log10(max_abs.max(1e-10));
                continue;
            }

            let th = bp.and_then(|p| p.threshold_db).unwrap_or(g_th);
            let rat = bp.and_then(|p| p.ratio).unwrap_or(self.ratio);
            let kn = bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db);
            let rg = bp.and_then(|p| p.range_db).unwrap_or(self.range_db);
            let hys = bp
                .and_then(|p| p.hysteresis_db)
                .unwrap_or(self.hysteresis_db);
            let hs = (bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms)
                * 0.001
                * self.sample_rate as f32) as usize;
            let use_measured_makeup = bp.map(|p| p.measured_auto_makeup).unwrap_or(false);
            let auto_makeup_gain = if use_measured_makeup {
                // Measured makeup: will be computed per-frame below
                1.0
            } else if bp.map(|p| p.auto_makeup).unwrap_or(false) {
                let slope = 1.0 - 1.0 / rat.max(1.0);
                let avg_atten = rg.max(0.0) * slope * 0.5;
                fast_pow10(avg_atten / 20.0)
            } else {
                1.0
            };

            let use_rms = self.detection_mode == "rms";
            let bexp = &mut self.band_expanders[b];
            let off = b * stride;
            let mut band_max_abs = 0.0f32;

            for frame in 0..nf {
                let mut det_db = 0.0f32;
                if self.link_channels {
                    if use_rms {
                        // For linked RMS: compute max RMS across channels
                        let mut max_rms_db = -120.0f32;
                        for ch in 0..self.channels {
                            let s = self.band_buffers[off + frame * self.channels + ch];
                            let ch_db = self.level_detectors[b][ch].process(s);
                            max_rms_db = max_rms_db.max(ch_db);
                        }
                        det_db = max_rms_db;
                    } else {
                        let mut peak = 0.0f32;
                        for ch in 0..self.channels {
                            peak = peak
                                .max(self.band_buffers[off + frame * self.channels + ch].abs());
                        }
                        det_db = 20.0 * fast_log10(peak.max(1e-10));
                    }
                }

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let sample_abs = self.band_buffers[idx].abs();
                    band_max_abs = band_max_abs.max(sample_abs);

                    let db = if self.link_channels {
                        det_db
                    } else if use_rms {
                        self.level_detectors[b][ch].process(self.band_buffers[idx])
                    } else {
                        20.0 * fast_log10(sample_abs.max(1e-10))
                    };

                    let target = match bexp.gate_state[ch] {
                        GateState::Open => {
                            if db < th {
                                bexp.gate_state[ch] = GateState::Hold;
                                bexp.hold_counter[ch] = hs;
                                0.0
                            } else {
                                0.0
                            }
                        }
                        GateState::Hold => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else if bexp.hold_counter[ch] > 0 {
                                bexp.hold_counter[ch] -= 1;
                                0.0
                            } else if db < th - hys {
                                bexp.gate_state[ch] = GateState::Closing;
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            } else {
                                0.0
                            }
                        }
                        GateState::Closing => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else {
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            }
                        }
                    };

                    let c = if target > bexp.envelope[ch] {
                        bexp.attack_coeff
                    } else {
                        bexp.release_coeff
                    };
                    bexp.envelope[ch] = target + c * (bexp.envelope[ch] - target);

                    // Update measured makeup tracker if enabled
                    if use_measured_makeup {
                        self.measured_makeups[b].update(bexp.envelope[ch]);
                    }

                    let gain_linear = fast_pow10(-bexp.envelope[ch] / 20.0);
                    let makeup = if use_measured_makeup {
                        self.measured_makeups[b].makeup_linear()
                    } else {
                        auto_makeup_gain
                    };
                    self.band_buffers[idx] *= gain_linear * makeup;
                }
            }
            self.band_levels_db[b] = 20.0 * fast_log10(band_max_abs.max(1e-10));
        }

        // 4. Recombination
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut s = 0.0f32;
                for b in 0..self.num_bands {
                    s += self.band_buffers[b * stride + idx];
                }
                buffer[idx] = self.dry_buffer[idx] * (1.0 - g_mix) + s * g_mix;
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            for b in 0..self.num_bands {
                self.is_open_buffer[b] = self.band_expanders[b]
                    .gate_state
                    .iter()
                    .any(|&s| s != GateState::Closing);
                for ch in 0..self.channels {
                    self.attenuation_flattened[b * self.channels + ch] =
                        self.band_expanders[b].envelope[ch];
                }
            }
            let levels = &self.band_levels_db;
            let xovers = &self.crossover_frequencies;
            let open = &self.is_open_buffer;
            self.cache.update(|d| {
                d.update(&self.attenuation_flattened, open, levels, xovers);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn test_mb_exp_basic() {
        let mut p = MultibandExpanderPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.1; 1000];
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

    /// Verify that low-frequency content triggers expansion in the lowest band
    /// even with default detection settings (no sidechain HPF blocking bass).
    #[test]
    fn test_low_frequency_triggers_expansion() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 3,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            range_db: 40.0,
            mix: 1.0,
            ..Default::default()
        };
        params.bands = vec![
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(4.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(40.0),
                ..Default::default()
            },
            BandExpanderParams::default(),
            BandExpanderParams::default(),
        ];
        let mut p = MultibandExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Feed a loud 50 Hz signal (above threshold) to open the gate
        let nf = 9600;
        let mut loud: Vec<f32> = (0..nf)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
            .collect();
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: nf,
        };
        p.process_in_place(&mut loud, &ctx).unwrap();

        // Verify the loud signal passed through with reasonable level
        let rms_loud: f32 =
            (loud[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        assert!(
            rms_loud > 0.05,
            "Loud 50 Hz signal should pass through expander (gate open), RMS={rms_loud:.6}"
        );

        // Now feed a very quiet 50 Hz signal (below threshold)
        let quiet_amp = 0.001;
        let mut quiet: Vec<f32> = (0..nf)
            .map(|i| {
                quiet_amp
                    * (2.0 * std::f32::consts::PI * 50.0 * (nf + i) as f32 / 48000.0).sin()
            })
            .collect();
        p.process_in_place(&mut quiet, &ctx).unwrap();

        // The quiet signal should be attenuated (gate closing)
        let rms_quiet: f32 =
            (quiet[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let input_rms = quiet_amp / std::f32::consts::SQRT_2;
        assert!(
            rms_quiet < input_rms,
            "Quiet 50 Hz signal should be attenuated by expander, \
             but rms_out={rms_quiet:.8} >= input_rms={input_rms:.8}"
        );
    }

    /// Regression: attack/release coefficients were swapped in per-band processing.
    /// With fast attack and slow release, quiet signals below threshold should be
    /// attenuated quickly (gate closes fast).
    #[test]
    fn test_mb_expander_attack_release_not_swapped() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 2,
            mix: 1.0,       // wet-only to observe expansion effect
            range_db: 60.0, // allow up to 60 dB of expansion attenuation
            ..Default::default()
        };
        params.bands = vec![
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(10.0),
                attack_ms: Some(1.0),
                release_ms: Some(200.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(60.0),
                ..Default::default()
            },
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(10.0),
                attack_ms: Some(1.0),
                release_ms: Some(200.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(60.0),
                ..Default::default()
            },
        ];
        let mut p = MultibandExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Feed loud broadband signal to open gates
        let mut loud = Vec::with_capacity(9600);
        for i in 0..9600 {
            loud.push(0.5 * (i as f32 * 0.3).sin());
        }
        p.process_in_place(
            &mut loud,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 9600,
            },
        )
        .unwrap();

        // Feed quiet broadband signal — gates should close fast with 1ms attack
        let quiet_peak = 0.001f32;
        let mut quiet = Vec::with_capacity(2400);
        for i in 0..2400 {
            quiet.push(quiet_peak * (i as f32 * 0.3).sin());
        }
        let quiet_rms_in: f32 =
            (quiet.iter().map(|s| s * s).sum::<f32>() / quiet.len() as f32).sqrt();
        p.process_in_place(
            &mut quiet,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2400,
            },
        )
        .unwrap();

        // After 50ms with 1ms attack (and 0ms hold), the signal should be attenuated.
        let quiet_rms_out: f32 =
            (quiet[1200..].iter().map(|s| s * s).sum::<f32>() / (quiet.len() - 1200) as f32).sqrt();
        assert!(
            quiet_rms_out < quiet_rms_in * 0.8,
            "Multiband expander gate should close fast with 1ms attack, \
             but RMS out {quiet_rms_out:.6} is too close to RMS in {quiet_rms_in:.6}. \
             Attack/release coefficients may be swapped."
        );
    }

    /// Unity passthrough: with threshold at minimum and ratio 1:1,
    /// the expander should not alter the signal significantly.
    #[test]
    fn test_mb_expander_unity_passthrough() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 3,
            ..Default::default()
        };
        for band in &mut params.bands {
            band.ratio = Some(1.0); // no expansion
        }
        let mut p = MultibandExpanderPlugin::with_params(2, params);
        p.initialize(48000).unwrap();

        // Generate test signal
        let mut input = vec![0.0f32; 4800 * 2];
        for i in 0..4800 {
            let val = 0.3 * (i as f32 * 0.05).sin();
            input[i * 2] = val;
            input[i * 2 + 1] = val;
        }
        let mut output = input.clone();
        p.process_in_place(
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 4800,
            },
        )
        .unwrap();

        // After settling (crossover filter delay), output should be close to input.
        // Allow for crossover phase shift but RMS should be similar.
        let rms_in: f32 =
            (input[2400..].iter().map(|s| s * s).sum::<f32>() / (input.len() - 2400) as f32).sqrt();
        let rms_out: f32 = (output[2400..].iter().map(|s| s * s).sum::<f32>()
            / (output.len() - 2400) as f32)
            .sqrt();
        let ratio = rms_out / rms_in;
        assert!(
            (0.7..1.3).contains(&ratio),
            "Unity ratio (1:1) should pass through, but RMS ratio is {ratio:.3}"
        );
    }
}
