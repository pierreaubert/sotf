use super::band_expander::BandExpander;
use super::band_expander_params::BandExpanderParams;
use super::misc::MAX_BLOCK_FRAMES;
use super::misc::MAX_LOOKAHEAD_MS;
use super::misc::parse_detection_mode;
use super::multiband_expander_data::MultibandExpanderData;
use super::spectral_bin_state::SpectralBinState;
use super::spectral_state::SpectralState;
use super::types::GateState;
use super::types::MultibandExpanderPluginParams;
use crate::params::{BAND_TEMPLATE as MEB, GLOBAL_PARAMS as ME};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use rustfft::num_complex::Complex;
use sotf_host::LookaheadBuffer;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_makeup::MeasuredMakeup;
use sotf_host::detector::LevelDetector;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};
use std::any::Any;
use std::sync::Arc;

pub struct MultibandExpanderPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) num_bands: usize,
    pub(super) _crossover_preset: i32,
    pub(super) crossover_frequencies: Vec<f32>,
    pub(super) threshold_db: f32,
    pub(super) ratio: f32,
    pub(super) attack_ms: f32,
    pub(super) release_ms: f32,
    pub(super) knee_db: f32,
    pub(super) range_db: f32,
    pub(super) hysteresis_db: f32,
    pub(super) hold_ms: f32,
    pub(super) link_channels: bool,
    pub(super) mix: f32,
    pub(super) detection_mode: String,
    pub(super) lookahead_ms: f32,
    /// Sidechain high-pass frequency (single-band compatibility, not yet applied to DSP)
    pub(super) sidechain_hpf_hz: f32,
    /// Per-band, per-channel lookahead delay buffers.
    pub(super) lookahead_buffers: Vec<Vec<LookaheadBuffer>>,
    /// Per-channel lookahead delay for the dry path (time-domain mode).
    /// When lookahead is active the wet path is delayed; this buffer delays
    /// the dry signal by the same amount to keep them time-aligned.
    pub(super) dry_lookahead_buffers: Vec<LookaheadBuffer>,
    /// Processing mode: "time_domain" or "spectral"
    pub(super) processing_mode: String,
    pub(super) band_params: Vec<BandExpanderParams>,
    pub(super) crossover_points: Vec<Lr4Crossover<f32>>,
    pub(super) band_expanders: Vec<BandExpander>,
    pub(super) band_buffers: Vec<f32>,
    pub(super) band_levels_db: Vec<f32>,
    pub(super) dry_buffer: Vec<f32>,
    pub(super) threshold_smoother: Smoother,
    pub(super) mix_smoother: Smoother,
    pub(super) xover_smoothers: Vec<LogSmoother>,

    /// Per-band measured auto-makeup gain trackers.
    pub(super) measured_makeups: Vec<MeasuredMakeup>,

    /// Per-band, per-channel level detectors for RMS mode.
    pub(super) level_detectors: Vec<Vec<LevelDetector>>,

    // Internal flattened monitoring buffers
    pub(super) attenuation_flattened: Vec<f32>,
    pub(super) is_open_buffer: Vec<bool>,
    pub(super) cache: RealTimeCache<MultibandExpanderData>,
    pub(super) cache_update_counter: usize,
    pub(super) cached_parameters: Vec<Parameter>,

    /// State for spectral processing mode (None when in time_domain mode)
    pub(super) spectral: Option<SpectralState>,
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
        let default_xfs = [200.0f32, 2000.0, 8000.0, 12000.0];
        let mut xfs = params.crossover_frequencies.clone();
        for (i, &d) in default_xfs.iter().enumerate() {
            if xfs.get(i).map_or(true, |&v| v == 0.0) {
                if i < xfs.len() {
                    xfs[i] = d;
                } else {
                    xfs.push(d);
                }
            }
        }
        while xfs.len() < 4 {
            xfs.push(default_xfs[xfs.len()]);
        }
        let ratio = if params.ratio == 0.0 { 2.0 } else { params.ratio };
        let attack_ms = if params.attack_ms == 0.0 { 1.0 } else { params.attack_ms };
        let release_ms = if params.release_ms == 0.0 { 100.0 } else { params.release_ms };
        let mut bexps = Vec::with_capacity(nb);
        for _ in 0..nb {
            bexps.push(BandExpander {
                envelope: vec![0.0; channels],
                peak_env: vec![0.0; channels],
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

        // Apply single-band aliases to band 0
        if let Some(am) = params.auto_makeup
            && let Some(bp) = band_params.first_mut()
        {
            bp.auto_makeup = am;
        }
        if let Some(mam) = params.measured_auto_makeup
            && let Some(bp) = band_params.first_mut()
        {
            bp.measured_auto_makeup = mam;
        }

        let measured_makeups = (0..nb).map(|_| MeasuredMakeup::new(1000.0, sr)).collect();

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

        let mode_str = if params.processing_mode.is_empty() {
            "time_domain"
        } else {
            params.processing_mode.as_str()
        };

        let spectral = if mode_str == "spectral" {
            let fft_size = 1024;
            let mut ss = SpectralState::new(fft_size, channels, sr, &xfs, nb);
            ss.update_band_coefficients(nb, &band_params, attack_ms, release_ms, sr);
            Some(ss)
        } else {
            None
        };

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands: nb,
            _crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(),
            threshold_db: params.threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db: params.knee_db,
            range_db: params.range_db,
            hysteresis_db: params.hysteresis_db,
            hold_ms: params.hold_ms,
            link_channels: params.link_channels,
            mix: params.mix,
            detection_mode: det_mode_str.to_string(),
            lookahead_ms: params.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS),
            sidechain_hpf_hz: params.sidechain_hpf_hz.unwrap_or(80.0),
            lookahead_buffers: (0..nb)
                .map(|_| {
                    (0..channels)
                        .map(|_| LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, sr, 1))
                        .collect()
                })
                .collect(),
            dry_lookahead_buffers: (0..channels)
                .map(|_| LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, sr, 1))
                .collect(),
            processing_mode: mode_str.to_string(),
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
            spectral,
        };
        p.build_crossovers();
        p.update_coefficients();
        p.update_lookahead_delay();
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_bands as f64),                       // num_bands
            1 => Some(self._crossover_preset as f64),               // crossover_preset
            2 => Some(self.crossover_frequencies[0] as f64),        // crossover_freq_1
            3 => Some(self.crossover_frequencies[1] as f64),        // crossover_freq_2
            4 => Some(self.crossover_frequencies[2] as f64),        // crossover_freq_3
            5 => Some(self.crossover_frequencies[3] as f64),        // crossover_freq_4
            6 => Some(self.threshold_db as f64),                    // threshold
            7 => Some(self.ratio as f64),                           // ratio
            8 => Some(self.attack_ms as f64),                       // attack
            9 => Some(self.release_ms as f64),                      // release
            10 => Some(self.range_db as f64),                       // range
            11 => Some(self.knee_db as f64),                        // knee
            12 => Some(self.hysteresis_db as f64),                  // hysteresis
            13 => Some(self.hold_ms as f64),                        // hold
            14 => Some(self.mix as f64),                            // mix
            15 => Some(if self.link_channels { 1.0 } else { 0.0 }), // link_channels
            16 => {
                // detection_mode
                let idx = if self.detection_mode == "rms" { 1 } else { 0 };
                Some(idx as f64)
            }
            17 => Some(self.lookahead_ms as f64), // lookahead_ms
            _ => None,
        }
    }

    /// Set the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_bands = value as usize,              // num_bands
            1 => self._crossover_preset = value as i32,        // crossover_preset
            2 => self.crossover_frequencies[0] = value as f32, // crossover_freq_1
            3 => self.crossover_frequencies[1] = value as f32, // crossover_freq_2
            4 => self.crossover_frequencies[2] = value as f32, // crossover_freq_3
            5 => self.crossover_frequencies[3] = value as f32, // crossover_freq_4
            6 => self.threshold_db = value as f32,             // threshold
            7 => self.ratio = value as f32,                    // ratio
            8 => self.attack_ms = value as f32,                // attack
            9 => self.release_ms = value as f32,               // release
            10 => self.range_db = value as f32,                // range
            11 => self.knee_db = value as f32,                 // knee
            12 => self.hysteresis_db = value as f32,           // hysteresis
            13 => self.hold_ms = value as f32,                 // hold
            14 => self.mix = value as f32,                     // mix
            15 => self.link_channels = value > 0.5,            // link_channels
            16 => {
                // detection_mode
                self.detection_mode = if value as i32 == 1 {
                    "rms".to_string()
                } else {
                    "peak".to_string()
                };
            }
            17 => self.lookahead_ms = (value as f32).clamp(0.0, MAX_LOOKAHEAD_MS), // lookahead_ms
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = param_bridge::build_parameters(ME, |i| self.param_value(i));

        // processing_mode is not in GLOBAL_PARAMS, add manually
        let proc_mode_idx = if self.processing_mode == "spectral" {
            1
        } else {
            0
        };
        params.push(
            Parameter::new_int("processing_mode", "Processing Mode", proc_mode_idx, 0, 1)
                .with_group("General"),
        );

        // Single-band aliases (not in GLOBAL_PARAMS, but needed for Expander PluginSettings)
        let bp0 = self.band_params.first();
        params.push(
            Parameter::new_bool(
                "auto_makeup",
                "Auto Makeup",
                bp0.is_some_and(|bp| bp.auto_makeup),
            )
            .with_group("Output"),
        );
        params.push(
            Parameter::new_bool(
                "measured_auto_makeup",
                "Measured Auto Makeup",
                bp0.is_some_and(|bp| bp.measured_auto_makeup),
            )
            .with_group("Output"),
        );
        params.push(
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                self.sidechain_hpf_hz,
                0.0,
                500.0,
            )
            .with_group("Sidechain"),
        );

        // Per-band dynamics (not covered by GLOBAL_PARAMS)
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

    pub(super) fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points.push(Lr4Crossover::new(
                f,
                self.sample_rate as f32,
                self.channels,
            ));
        }
    }

    pub(super) fn update_coefficients(&mut self) {
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

        // Also update spectral-mode coefficients if active
        if let Some(ss) = &mut self.spectral {
            ss.update_band_coefficients(
                self.num_bands,
                &self.band_params,
                self.attack_ms,
                self.release_ms,
                self.sample_rate,
            );
        }
    }

    pub(super) fn update_lookahead_delay(&mut self) {
        for band_bufs in &mut self.lookahead_buffers {
            for buf in band_bufs {
                buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
            }
        }
        // Keep the dry-path delay in sync with the wet-path lookahead.
        for buf in &mut self.dry_lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
    }

    pub(super) fn calculate_expansion_attenuation(
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

    /// Process one STFT hop for the spectral mode.
    ///
    /// Called after `fft_size` samples have been accumulated in the input ring.
    /// Applies per-bin expansion envelope then IFFT + OLA.
    ///
    /// # Expansion model
    /// Each bin's magnitude (in dB) acts as the "input level" to the expander.
    /// The bin is assigned to a band via `bin_to_band`; the band supplies
    /// threshold / ratio / knee / range / hold / hysteresis parameters.
    /// Attack/release coefficients are at *hop rate* so time constants are
    /// perceptually equivalent to the time-domain mode.
    pub(super) fn process_spectral_hop(&mut self, any_solo: bool) {
        let ss = match &mut self.spectral {
            Some(s) => s,
            None => return,
        };

        let fft_size = ss.fft_size;
        let num_bins = ss.num_bins;
        let scale = ss.output_scale;
        let mask = ss.output_accumulator_mask;
        // Window sum used for correct magnitude normalization (see mag computation below).
        let window_sum: f32 = ss.analysis_window.iter().sum();
        let channels = self.channels;

        // Cache band parameters into a compact form to avoid repeated borrow conflicts.
        // Hold time is converted from milliseconds to hop counts at the hop rate.
        // Uses a Vec sized to num_bands (not a fixed-size array) so num_bands > 5 is safe.
        #[derive(Clone, Copy)]
        struct BandInfo {
            th: f32,
            rat: f32,
            kn: f32,
            rg: f32,
            hys: f32,
            /// Hold duration measured in STFT hops (not samples)
            hs: usize,
            bypass: bool,
            active: bool,
            solo: bool,
        }
        let hop_rate = self.sample_rate as f32 / ss.hop_size as f32;
        let band_info: Vec<BandInfo> = (0..self.num_bands)
            .map(|b| {
                let bp = self.band_params.get(b);
                let hold_ms = bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms);
                BandInfo {
                    th: bp.and_then(|p| p.threshold_db).unwrap_or(self.threshold_db),
                    rat: bp.and_then(|p| p.ratio).unwrap_or(self.ratio),
                    kn: bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db),
                    rg: bp.and_then(|p| p.range_db).unwrap_or(self.range_db),
                    hys: bp
                        .and_then(|p| p.hysteresis_db)
                        .unwrap_or(self.hysteresis_db),
                    // Use .round() to avoid truncating short-but-nonzero hold times to 0.
                    hs: (hold_ms * 0.001 * hop_rate).round() as usize,
                    bypass: bp.map(|p| p.bypass).unwrap_or(false),
                    active: bp.map(|p| p.active).unwrap_or(true),
                    solo: bp.map(|p| p.solo).unwrap_or(false),
                }
            })
            .collect();

        for ch in 0..channels {
            // --- Forward FFT ---
            // Apply Hann window to the linear input buffer
            for i in 0..fft_size {
                ss.windowed_buf[i] = ss.input_buffers[ch][i] * ss.analysis_window[i];
            }
            // forward() reads time_buffer, writes freq_buffer
            ss.fft_processors[ch]
                .time_buffer
                .copy_from_slice(&ss.windowed_buf);
            ss.fft_processors[ch].forward();
            ss.freq_scratch
                .copy_from_slice(&ss.fft_processors[ch].freq_buffer);

            // --- Per-bin expansion ---
            for k in 0..num_bins {
                let b = ss.bin_to_band[k];
                let info = &band_info[b];

                // Muted bands (solo active, this band not solo)
                if any_solo && !info.solo {
                    ss.freq_scratch[k] = Complex::new(0.0, 0.0);
                    continue;
                }

                // Bypassed or inactive bands: no gain change
                if info.bypass || !info.active {
                    continue;
                }

                // Bin magnitude normalized to equivalent time-domain amplitude.
                //
                // The realfft forward transform is unnormalized: a cosine with
                // amplitude A at a bin center and a Hann window of sum W gives
                // |X[k]| = A * W / 2 (for a one-sided positive-frequency bin).
                // Multiplying by 2/W recovers amplitude A, making the threshold
                // (in dBFS) numerically consistent with the time-domain mode.
                // For a standard Hann window, W = fft_size / 2.
                let mag = ss.freq_scratch[k].norm() * (2.0 / window_sum);
                let mag_db = 20.0 * fast_log10(mag.max(1e-10));

                // Update gate state and envelope (at hop rate)
                let state = &mut ss.bin_states[ch][k];
                let th = info.th;
                let hys = info.hys;

                let target_atten = match state.gate_state {
                    GateState::Open => {
                        if mag_db < th {
                            state.gate_state = GateState::Hold;
                            state.hold_counter = info.hs;
                            0.0
                        } else {
                            0.0
                        }
                    }
                    GateState::Hold => {
                        if mag_db >= th {
                            state.gate_state = GateState::Open;
                            0.0
                        } else if state.hold_counter > 0 {
                            state.hold_counter -= 1;
                            0.0
                        } else if mag_db < th - hys {
                            state.gate_state = GateState::Closing;
                            Self::calculate_expansion_attenuation(
                                mag_db, th, info.rat, info.kn, info.rg,
                            )
                        } else {
                            0.0
                        }
                    }
                    GateState::Closing => {
                        if mag_db >= th {
                            state.gate_state = GateState::Open;
                            0.0
                        } else {
                            Self::calculate_expansion_attenuation(
                                mag_db, th, info.rat, info.kn, info.rg,
                            )
                        }
                    }
                };

                // One-pole envelope smoothing at hop rate
                let coeff = if target_atten > state.envelope_db {
                    ss.band_attack_hop[b]
                } else {
                    ss.band_release_hop[b]
                };
                state.envelope_db = target_atten + coeff * (state.envelope_db - target_atten);

                // Apply gain to the complex bin
                let gain = fast_pow10(-state.envelope_db / 20.0);
                ss.freq_scratch[k] *= gain;
            }

            // --- Inverse FFT ---
            ss.fft_processors[ch]
                .freq_buffer
                .copy_from_slice(&ss.freq_scratch);
            ss.fft_processors[ch].inverse();

            // Apply synthesis window (Hann) + scale, overlap-add into ring
            let next_pos = ss.next_add_position;
            for i in 0..fft_size {
                let frame_idx = (next_pos + i) & mask;
                let s = ss.fft_processors[ch].time_buffer[i]
                    * ss.analysis_window[i]  // synthesis window = same Hann
                    * scale;
                ss.output_accumulator[frame_idx * channels + ch] += s;
            }
        }

        // Advance OLA write position by one hop
        ss.next_add_position = (ss.next_add_position + ss.hop_size) & mask;

        // Zero the "fresh" fft_size positions just past the write head.
        // Each IFFT produces fft_size samples; we must clear the full fft_size
        // region that the next write will occupy, not just hop_size, otherwise
        // stale samples from earlier cycles contaminate the overlap-add sum.
        {
            let clear_start = (ss.next_add_position + fft_size) & mask;
            for i in 0..fft_size {
                let frame_idx = (clear_start + i) & mask;
                for ch in 0..channels {
                    ss.output_accumulator[frame_idx * channels + ch] = 0.0;
                }
            }
        }

        ss.output_accumulator_fill += ss.hop_size;
    }

    /// Main in-place processing entry point for spectral mode.
    ///
    /// Feeds interleaved samples into per-channel ring buffers. Each time
    /// `fft_size` samples have accumulated (after the initial fill), a STFT
    /// frame is processed. The OLA output is drained into the caller's buffer.
    pub(super) fn process_spectral_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let channels = self.channels;

        let any_solo =
            (0..self.num_bands).any(|b| self.band_params.get(b).map(|p| p.solo).unwrap_or(false));

        let g_mix = self.mix_smoother.next_n(nf);

        // Ensure dry buffer large enough
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // Zero the output portion of the buffer — we'll drain OLA into it
        buffer[..nf * channels].fill(0.0);

        let mut input_pos = 0; // frame index into the caller's buffer
        let mut output_pos = 0; // frame index into the caller's output

        // Safety: spectral must be Some when this path is called
        let fft_size = self.spectral.as_ref().unwrap().fft_size;
        let hop_size = self.spectral.as_ref().unwrap().hop_size;

        while output_pos < nf {
            // --- Step 1: Fill input ring from caller's buffer ---
            if input_pos < nf {
                let ss = self.spectral.as_mut().unwrap();
                let overlap = fft_size - hop_size;
                let space_in_tail = fft_size - ss.input_fill;
                let available = nf - input_pos;
                let to_copy = space_in_tail.min(available);

                if to_copy > 0 {
                    for ch in 0..channels {
                        for i in 0..to_copy {
                            ss.input_buffers[ch][ss.input_fill + i] =
                                self.dry_buffer[(input_pos + i) * channels + ch];
                        }
                    }
                    ss.input_fill += to_copy;
                    input_pos += to_copy;
                    let _ = overlap; // suppress unused warning
                }
            }

            // --- Step 2: Process STFT frames while we have a full window ---
            {
                let input_fill = self.spectral.as_ref().unwrap().input_fill;
                let hop = self.spectral.as_ref().unwrap().hop_size;
                if input_fill >= fft_size {
                    self.process_spectral_hop(any_solo);
                    // Shift input ring: keep overlap = fft_size - hop_size samples
                    let ss = self.spectral.as_mut().unwrap();
                    let overlap = fft_size - hop;
                    for ch in 0..channels {
                        ss.input_buffers[ch].copy_within(hop..fft_size, 0);
                        ss.input_buffers[ch][overlap..].fill(0.0);
                    }
                    ss.input_fill = overlap;
                }
            }

            // --- Step 3: Drain available OLA frames into output ---
            {
                let ss = self.spectral.as_mut().unwrap();
                let frames_to_drain = ss.output_accumulator_fill.min(nf - output_pos);
                if frames_to_drain > 0 {
                    let mask = ss.output_accumulator_mask;
                    for i in 0..frames_to_drain {
                        let read_idx = (ss.output_read_position + i) & mask;
                        let out_base = (output_pos + i) * channels;
                        for ch in 0..channels {
                            buffer[out_base + ch] +=
                                ss.output_accumulator[read_idx * channels + ch];
                        }
                    }
                    // Clear drained frames
                    for i in 0..frames_to_drain {
                        let read_idx = (ss.output_read_position + i) & mask;
                        for ch in 0..channels {
                            ss.output_accumulator[read_idx * channels + ch] = 0.0;
                        }
                    }
                    ss.output_read_position = (ss.output_read_position + frames_to_drain) & mask;
                    ss.output_accumulator_fill -= frames_to_drain;
                    output_pos += frames_to_drain;
                } else {
                    // No output ready: output silence for this iteration and break
                    // This happens only during initial latency fill
                    output_pos = nf;
                }
            }
        }

        // Apply wet/dry mix with latency-compensated dry signal.
        // The STFT introduces (fft_size - hop_size) samples of algorithmic latency.
        // We delay the dry path by the same amount so dry and wet stay time-aligned,
        // preventing comb-filter notches when mix < 1.0.
        {
            let ss = self.spectral.as_mut().unwrap();
            for i in 0..nf {
                for ch in 0..channels {
                    let idx = i * channels + ch;
                    // Push the raw dry sample; read back the delayed copy.
                    let dry_pos = ss.dry_delay_pos + ch;
                    let delayed_dry = ss.dry_delay_buf[dry_pos];
                    ss.dry_delay_buf[dry_pos] = self.dry_buffer[idx];
                    buffer[idx] = delayed_dry * (1.0 - g_mix) + buffer[idx] * g_mix;
                }
                // Advance the dry delay cursor by one frame (channels floats).
                ss.dry_delay_pos += channels;
                if ss.dry_delay_pos >= ss.dry_delay_len {
                    ss.dry_delay_pos = 0;
                }
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

impl MultibandExpanderPlugin {
    /// Backward-compatible parameter list accessor.
    pub fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    pub fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Handle processing_mode separately (not in GLOBAL_PARAMS)
        if id.as_str() == "processing_mode" {
            let idx = value
                .as_int()
                .ok_or_else(|| "processing_mode must be an integer".to_string())?;
            let mode_str = if idx == 1 { "spectral" } else { "time_domain" };
            if mode_str != self.processing_mode {
                self.processing_mode = mode_str.to_string();
                if mode_str == "spectral" && self.spectral.is_none() {
                    let fft_size = 1024;
                    let mut ss = SpectralState::new(
                        fft_size,
                        self.channels,
                        self.sample_rate,
                        &self.crossover_frequencies,
                        self.num_bands,
                    );
                    ss.update_band_coefficients(
                        self.num_bands,
                        &self.band_params,
                        self.attack_ms,
                        self.release_ms,
                        self.sample_rate,
                    );
                    self.spectral = Some(ss);
                } else if mode_str == "time_domain" {
                    self.spectral = None;
                }
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        // Try global params via param_bridge
        if let Ok(idx) =
            param_bridge::set_parameter(ME, &id, &value, |i, v| self.set_param_value(i, v))
        {
            // Side effects for specific global params
            match idx {
                0 => {
                    // num_bands changed
                    let nb = self.num_bands;
                    self.build_crossovers();
                    while self.band_params.len() < nb {
                        self.band_params.push(BandExpanderParams::default());
                    }
                    while self.band_expanders.len() < nb {
                        self.band_expanders.push(BandExpander {
                            envelope: vec![0.0; self.channels],
                            peak_env: vec![0.0; self.channels],
                            gate_state: vec![GateState::Open; self.channels],
                            hold_counter: vec![0; self.channels],
                            attack_coeff: 0.0,
                            release_coeff: 0.0,
                        });
                    }
                    while self.lookahead_buffers.len() < nb {
                        self.lookahead_buffers.push(
                            (0..self.channels)
                                .map(|_| {
                                    LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, self.sample_rate, 1)
                                })
                                .collect(),
                        );
                    }
                    self.update_lookahead_delay();
                    while self.measured_makeups.len() < nb {
                        self.measured_makeups
                            .push(MeasuredMakeup::new(1000.0, self.sample_rate));
                    }
                    let det_mode = parse_detection_mode(&self.detection_mode);
                    while self.level_detectors.len() < nb {
                        self.level_detectors.push(
                            (0..self.channels)
                                .map(|_| LevelDetector::new(det_mode, self.sample_rate))
                                .collect(),
                        );
                    }
                    self.band_levels_db.resize(nb, -100.0);
                    self.attenuation_flattened.resize(nb * self.channels, 0.0);
                    self.is_open_buffer.resize(nb, false);
                    self.update_coefficients();

                    // Rebuild spectral bin->band mapping
                    if let Some(ss) = &mut self.spectral {
                        ss.update_bin_to_band(self.sample_rate, &self.crossover_frequencies, nb);
                        ss.update_band_coefficients(
                            nb,
                            &self.band_params,
                            self.attack_ms,
                            self.release_ms,
                            self.sample_rate,
                        );
                        for ch_states in &mut ss.bin_states {
                            ch_states.resize_with(ss.num_bins, SpectralBinState::new);
                        }
                    }
                }
                2..=5 => {
                    // crossover_freq_1..4 changed
                    let xover_idx = idx - 2;
                    if xover_idx < self.xover_smoothers.len() {
                        self.xover_smoothers[xover_idx]
                            .set_target(self.crossover_frequencies[xover_idx]);
                    }
                    // Update spectral bin->band mapping
                    if let Some(ss) = &mut self.spectral {
                        ss.update_bin_to_band(
                            self.sample_rate,
                            &self.crossover_frequencies,
                            self.num_bands,
                        );
                    }
                }
                6 => {
                    // threshold changed
                    self.threshold_smoother.set_target(self.threshold_db);
                }
                8 | 9 => {
                    // attack or release changed
                    self.update_coefficients();
                }
                14 => {
                    // mix changed
                    self.mix_smoother.set_target(self.mix);
                }
                16 => {
                    // detection_mode changed
                    let det_mode = parse_detection_mode(&self.detection_mode);
                    for band_dets in &mut self.level_detectors {
                        for det in band_dets {
                            det.set_mode(det_mode);
                        }
                    }
                }
                17 => {
                    // lookahead_ms changed
                    self.update_lookahead_delay();
                }
                _ => {}
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        // Single-band aliases: map unprefixed names to band_params[0]
        let name = id.as_str();
        match name {
            "auto_makeup" => {
                let v = value
                    .as_bool()
                    .ok_or_else(|| "auto_makeup must be a boolean".to_string())?;
                if let Some(bp) = self.band_params.first_mut() {
                    bp.auto_makeup = v;
                }
                self.rebuild_cached_parameters();
                return Ok(());
            }
            "measured_auto_makeup" => {
                let v = value
                    .as_bool()
                    .ok_or_else(|| "measured_auto_makeup must be a boolean".to_string())?;
                if let Some(bp) = self.band_params.first_mut() {
                    bp.measured_auto_makeup = v;
                }
                self.rebuild_cached_parameters();
                return Ok(());
            }
            "sidechain_hpf_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "sidechain_hpf_hz must be a float".to_string())?;
                self.sidechain_hpf_hz = v;
                self.rebuild_cached_parameters();
                return Ok(());
            }
            _ => {}
        }

        // Fall through to band-level param handling
        if name.starts_with("band_") {
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
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    pub fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Handle processing_mode separately (not in GLOBAL_PARAMS)
        if id.as_str() == "processing_mode" {
            let idx = if self.processing_mode == "spectral" {
                1
            } else {
                0
            };
            return Some(ParameterValue::Int(idx));
        }
        // Try global params first
        if let Some(v) = param_bridge::get_parameter(ME, id, |i| self.param_value(i)) {
            return Some(v);
        }
        // Single-band aliases: map unprefixed names to band_params[0]
        let name = id.as_str();
        match name {
            "auto_makeup" => {
                return Some(ParameterValue::Bool(
                    self.band_params.first().is_some_and(|bp| bp.auto_makeup),
                ));
            }
            "measured_auto_makeup" => {
                return Some(ParameterValue::Bool(
                    self.band_params
                        .first()
                        .is_some_and(|bp| bp.measured_auto_makeup),
                ));
            }
            "sidechain_hpf_hz" => {
                return Some(ParameterValue::Float(self.sidechain_hpf_hz));
            }
            _ => {}
        }
        // Fall through to band-level params
        let name = id.as_str();
        if name.starts_with("band_") {
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
            None
        }
    }
}

impl ParametricInPlacePlugin for MultibandExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Expander", "1.2.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            self.set_parameter(id, value)?;
        }
        Ok(())
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        for param in &self.cached_parameters {
            if let Some(value) = self.get_parameter(&param.id) {
                values.insert(param.id.clone(), value);
            }
        }
        values
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

        // Resize lookahead buffers for new sample rate
        let max_la_samples = (MAX_LOOKAHEAD_MS * 0.001 * sr as f32).round() as usize;
        for band_bufs in &mut self.lookahead_buffers {
            for buf in band_bufs {
                buf.resize(max_la_samples, 1);
            }
        }
        // Resize dry lookahead buffers to match band lookahead buffers.
        for buf in &mut self.dry_lookahead_buffers {
            buf.resize(max_la_samples, 1);
        }
        self.update_lookahead_delay();

        // Pre-allocate buffers for real-time safety.
        let max_frames = MAX_BLOCK_FRAMES;
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);

        // (Re-)initialize spectral state if mode is active
        if self.processing_mode == "spectral" {
            let fft_size = 1024;
            let mut ss = SpectralState::new(
                fft_size,
                self.channels,
                sr,
                &self.crossover_frequencies,
                self.num_bands,
            );
            ss.update_band_coefficients(
                self.num_bands,
                &self.band_params,
                self.attack_ms,
                self.release_ms,
                sr,
            );
            self.spectral = Some(ss);
        }

        // Reset all state after (re-)initialization to eliminate transient artifacts.
        self.reset();

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
        for band_bufs in &mut self.lookahead_buffers {
            for buf in band_bufs {
                buf.reset();
            }
        }
        for buf in &mut self.dry_lookahead_buffers {
            buf.reset();
        }
        self.band_buffers.fill(0.0);
        self.dry_buffer.fill(0.0);

        if let Some(ss) = &mut self.spectral {
            ss.reset();
        }
    }

    fn latency_samples(&self) -> usize {
        if let Some(ss) = &self.spectral {
            // Algorithmic latency is fft_size - hop_size (not fft_size).
            // Reporting fft_size would cause the host to over-compensate by hop_size samples.
            ss.fft_size - ss.hop_size
        } else if self.lookahead_ms > 0.0 {
            (self.lookahead_ms * 0.001 * self.sample_rate as f32).round() as usize
        } else {
            0
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Dispatch to spectral mode if active
        if self.processing_mode == "spectral" {
            return self.process_spectral_in_place(buffer, context);
        }

        enable_ftz_daz();
        let nf = context.num_frames;
        if nf > MAX_BLOCK_FRAMES {
            let mut processed = 0;
            while processed < nf {
                let chunk_frames = (nf - processed).min(MAX_BLOCK_FRAMES);
                let sample_start = processed * self.channels;
                let sample_end = sample_start + chunk_frames * self.channels;
                let chunk_ctx = ProcessContext::new(context.sample_rate, chunk_frames);
                self.process_in_place(&mut buffer[sample_start..sample_end], &chunk_ctx)?;
                processed += chunk_frames;
            }
            return Ok(nf);
        }
        let stride = nf * self.channels;
        debug_assert!(self.dry_buffer.len() >= buffer.len());
        debug_assert!(self.band_buffers.len() >= self.num_bands * stride);

        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // 1. Update crossovers
        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next_n(nf);
            self.crossover_points[i].set_frequency(freq);
        }

        // 2. Perform Crossover Splitting
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut rem = buffer[idx];
                for xidx in 0..(self.num_bands - 1) {
                    let (low, high) = self.crossover_points[xidx].process(rem, ch);
                    self.band_buffers[xidx * stride + idx] = low;
                    rem = high;
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

        let use_lookahead = self.lookahead_ms > 0.0;

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
            // Use .round() to avoid short-but-nonzero hold times truncating to 0 samples.
            let hs = (bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms)
                * 0.001
                * self.sample_rate as f32)
                .round() as usize;
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

            // Peak detector fast release: 5 ms, independent of the expander's attack time.
            // Using attack_coeff here caused slow-attack settings to hold the peak envelope
            // high after the signal decays, preventing the gate from closing promptly.
            const PEAK_DETECTOR_RELEASE_MS: f32 = 5.0;
            let peak_release_coeff =
                (-1.0 / (PEAK_DETECTOR_RELEASE_MS * 0.001 * self.sample_rate as f32)).exp();

            for frame in 0..nf {
                // Update per-channel peak envelope followers first.
                // Instant attack (sample-accurate), fast independent release.
                for ch in 0..self.channels {
                    let s = self.band_buffers[off + frame * self.channels + ch].abs();
                    bexp.peak_env[ch] = s.max(peak_release_coeff * bexp.peak_env[ch]);
                }

                let mut det_db = 0.0f32;
                if self.link_channels {
                    if use_rms {
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
                            peak = peak.max(bexp.peak_env[ch]);
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
                        20.0 * fast_log10(bexp.peak_env[ch].max(1e-10))
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
                }

                // Update measured makeup tracker once per frame using the max
                // envelope across all channels.  Updating inside the per-channel
                // loop would interleave L/R values into a single tracker, causing
                // makeup gain to jitter on stereo material.
                if use_measured_makeup {
                    let max_env = (0..self.channels)
                        .map(|ch| bexp.envelope[ch])
                        .fold(0.0f32, f32::max);
                    self.measured_makeups[b].update(max_env);
                }

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let gain_linear = fast_pow10(-bexp.envelope[ch] / 20.0);
                    let makeup = if use_measured_makeup {
                        self.measured_makeups[b].makeup_linear()
                    } else {
                        auto_makeup_gain
                    };
                    let sample = if use_lookahead {
                        self.lookahead_buffers[b][ch].push(self.band_buffers[idx])
                    } else {
                        self.band_buffers[idx]
                    };
                    self.band_buffers[idx] = sample * gain_linear * makeup;
                }
            }
            self.band_levels_db[b] = 20.0 * fast_log10(band_max_abs.max(1e-10));
        }

        // 4. Recombination with latency-compensated dry/wet mix.
        // When lookahead is active the wet path is delayed by lookahead_samples.
        // We push the dry signal through an equal-length delay so dry and wet
        // remain time-aligned, preventing comb-filter notches when mix < 1.0.
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut s = 0.0f32;
                for b in 0..self.num_bands {
                    s += self.band_buffers[b * stride + idx];
                }
                let dry = if use_lookahead {
                    self.dry_lookahead_buffers[ch].push(self.dry_buffer[idx])
                } else {
                    self.dry_buffer[idx]
                };
                buffer[idx] = dry * (1.0 - g_mix) + s * g_mix;
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
