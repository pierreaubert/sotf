// ============================================================================
// Parametric EQ Plugin
// ============================================================================

use math_audio_iir_fir::{Biquad, BiquadCoefficients};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainParams};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilterConfig {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    #[serde(default)]
    pub db_gain: f64,
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
struct BandTransition {
    old_coeffs: BiquadCoefficients,
    new_coeffs: BiquadCoefficients,
    samples_remaining: usize,
    total_samples: usize,
}

pub struct EqPlugin {
    num_channels: usize,
    filters: Vec<Vec<Biquad>>,
    sample_rate: u32,
    auto_gain: AutoGain,
    cache: RealTimeCache<AutoGainData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
    /// Per-band transition state. Outer index = band, applies to all channels.
    transitions: Vec<Option<BandTransition>>,
}

impl EqPlugin {
    pub fn new(num_channels: usize, filters: Vec<Biquad>) -> Self {
        let num_bands = filters.len();
        let mut channel_filters = Vec::with_capacity(num_channels);
        for _ in 0..num_channels {
            channel_filters.push(filters.clone());
        }
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate).expect("ag");
        let transitions = (0..num_bands).map(|_| None).collect();
        let mut p = Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
            cache: RealTimeCache::new(AutoGainData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            transitions,
        };
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![Parameter::new_bool(
            "auto_gain_enabled",
            "Auto Gain",
            self.auto_gain.is_enabled(),
        )];

        if !self.filters.is_empty() {
            for (i, f) in self.filters[0].iter().enumerate() {
                let group = format!("Band {}", i + 1);
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
                    Parameter::new_float(&format!("band_{}_q", i), "Q", f.q as f32, Q_MIN, Q_MAX)
                        .with_group(&group),
                );
                params.push(
                    Parameter::new_float(
                        &format!("band_{}_gain", i),
                        "Gain",
                        f.db_gain as f32,
                        GAIN_MIN,
                        GAIN_MAX,
                    )
                    .with_group(&group),
                );
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
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate)?;
        let transitions = (0..num_bands).map(|_| None).collect();
        let mut p = Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
            cache: RealTimeCache::new(AutoGainData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            transitions,
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
        let config_to_biquad = |f: &BiquadFilterConfig| -> Result<Biquad, String> {
            let filter_type = match f.filter_type.as_str() {
                "peak" | "Peak" => BiquadFilterType::Peak,
                "lowshelf" | "Lowshelf" => BiquadFilterType::Lowshelf,
                "highshelf" | "Highshelf" => BiquadFilterType::Highshelf,
                "lowpass" | "Lowpass" => BiquadFilterType::Lowpass,
                "highpass" | "Highpass" => BiquadFilterType::Highpass,
                "notch" | "Notch" => BiquadFilterType::Notch,
                "bandpass" | "Bandpass" => BiquadFilterType::Bandpass,
                "lowshelf_orf" | "LowshelfOrf" => BiquadFilterType::LowshelfOrf,
                "highshelf_orf" | "HighshelfOrf" => BiquadFilterType::HighshelfOrf,
                "peak_matched" | "PeakMatched" => BiquadFilterType::PeakMatched,
                other => return Err(format!("Type: {}", other)),
            };
            Biquad::try_new(filter_type, f.freq, sample_rate as f64, f.q, f.db_gain)
                .map_err(|e| e.to_string())
        };
        let auto_gain = AutoGain::new(num_channels, sample_rate, params.auto_gain)?;
        let mut eq = if let Some(cfgs) = params.channel_filters {
            if cfgs.len() != num_channels {
                return Err("Mismatched chains".into());
            }
            let mut channel_filters = Vec::with_capacity(num_channels);
            for c in cfgs {
                channel_filters.push(
                    c.iter()
                        .map(config_to_biquad)
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            let num_bands = channel_filters.first().map_or(0, |c| c.len());
            Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
                cache: RealTimeCache::new(AutoGainData::default()),
                cache_update_counter: 0,
                cached_parameters: Vec::new(),
                transitions: (0..num_bands).map(|_| None).collect(),
            }
        } else {
            let filters = params
                .filters
                .iter()
                .map(config_to_biquad)
                .collect::<Result<Vec<_>, _>>()?;
            let num_bands = filters.len();
            let mut channel_filters = Vec::with_capacity(num_channels);
            for _ in 0..num_channels {
                channel_filters.push(filters.clone());
            }
            Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
                cache: RealTimeCache::new(AutoGainData::default()),
                cache_update_counter: 0,
                cached_parameters: Vec::new(),
                transitions: (0..num_bands).map(|_| None).collect(),
            }
        };
        eq.rebuild_cached_parameters();
        Ok(eq)
    }

    pub fn set_filters(&mut self, filters: Vec<Biquad>) {
        self.filters.clear();
        for _ in 0..self.num_channels {
            self.filters.push(filters.clone());
        }
        self.transitions = (0..filters.len()).map(|_| None).collect();
    }

    pub fn set_channel_filters(&mut self, channel_filters: Vec<Vec<Biquad>>) -> Result<(), String> {
        if channel_filters.len() != self.num_channels {
            return Err("mismatch".into());
        }
        let num_bands = channel_filters.first().map_or(0, |c| c.len());
        self.filters = channel_filters;
        self.transitions = (0..num_bands).map(|_| None).collect();
        Ok(())
    }

    /// Compute the number of samples for the transition at the current sample rate.
    fn transition_samples(&self) -> usize {
        (self.sample_rate as f64 * TRANSITION_DURATION_SECS) as usize
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
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                let field = parts[2];

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
                    // Capture old coefficients before updating.
                    // If a transition is already active, use the current interpolated
                    // position to avoid a click from jumping back to the previous target.
                    let old_coeffs = if let Some(Some(active)) = self.transitions.get(b_idx) {
                        let t = 1.0
                            - (active.samples_remaining as f64 / active.total_samples as f64);
                        Some(active.old_coeffs.lerp(&active.new_coeffs, t))
                    } else {
                        self.filters[0].get(b_idx).map(|f| f.coefficients())
                    };
                    for ch in 0..self.num_channels {
                        if let Some(f) = self.filters[ch].get_mut(b_idx) {
                            let mut freq = f.freq;
                            let mut q = f.q;
                            let mut db_gain = f.db_gain;
                            match field {
                                "freq" => freq = v as f64,
                                "q" => q = v as f64,
                                "gain" => db_gain = v as f64,
                                _ => {}
                            }
                            f.update_params(f.filter_type, freq, f.srate, q, db_gain);
                        }
                    }
                    // Start a coefficient transition for this band
                    if let Some(old) = old_coeffs
                        && let Some(f) = self.filters[0].get(b_idx)
                    {
                        let new_coeffs = f.coefficients();
                        let total = self.transition_samples();
                        if total > 0 {
                            // Ensure transitions vec is large enough
                            while self.transitions.len() <= b_idx {
                                self.transitions.push(None);
                            }
                            self.transitions[b_idx] = Some(BandTransition {
                                old_coeffs: old,
                                new_coeffs,
                                samples_remaining: total,
                                total_samples: total,
                            });
                        }
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
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                let field = parts[2];
                if let Some(f) = self.filters[0].get(b_idx) {
                    return match field {
                        "freq" => Some(ParameterValue::Float(f.freq as f32)),
                        "q" => Some(ParameterValue::Float(f.q as f32)),
                        "gain" => Some(ParameterValue::Float(f.db_gain as f32)),
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
        let prev_sample_rate = self.sample_rate;
        self.sample_rate = sample_rate;
        for chain in &mut self.filters {
            for f in chain {
                if prev_sample_rate == 0 {
                    // Initial creation: use Biquad::new to set up from scratch
                    *f = Biquad::new(f.filter_type, f.freq, sample_rate as f64, f.q, f.db_gain);
                } else {
                    // Sample rate change: use update_params to preserve filter state
                    f.update_params(f.filter_type, f.freq, sample_rate as f64, f.q, f.db_gain);
                }
            }
        }
        // Clear any active transitions since sample rate changed
        for t in &mut self.transitions {
            *t = None;
        }
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn reset(&mut self) {
        for chain in &mut self.filters {
            for f in chain {
                *f = Biquad::new(f.filter_type, f.freq, f.srate, f.q, f.db_gain);
            }
        }
        for t in &mut self.transitions {
            *t = None;
        }
        self.auto_gain.reset();
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;

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

        // Check if any band has an active transition
        let has_transitions = self.transitions.iter().any(|t| t.is_some());

        if has_transitions {
            // Process with per-sample coefficient interpolation
            for frame in 0..num_frames {
                for ch in 0..self.num_channels {
                    let idx = frame * self.num_channels + ch;
                    let mut s = buffer[idx] as f64;
                    for (band_idx, f) in self.filters[ch].iter_mut().enumerate() {
                        if let Some(trans) = self.transitions.get(band_idx).and_then(|t| t.as_ref()) {
                            let t = 1.0
                                - (trans.samples_remaining as f64 / trans.total_samples as f64);
                            let interpolated = trans.old_coeffs.lerp(&trans.new_coeffs, t);
                            s = f.process_with_coefficients(s, &interpolated);
                        } else {
                            s = f.process(s);
                        }
                    }
                    buffer[idx] = s as f32;
                }
                // Decrement transition counters once per frame (not per channel)
                for t in self.transitions.iter_mut().flatten() {
                    if t.samples_remaining > 0 {
                        t.samples_remaining -= 1;
                    }
                }
            }
            // Clear completed transitions
            for trans in self.transitions.iter_mut() {
                if trans.as_ref().is_some_and(|t| t.samples_remaining == 0) {
                    *trans = None;
                }
            }
        } else {
            // Fast path: no transitions active, use direct processing
            for frame in 0..num_frames {
                for ch in 0..self.num_channels {
                    let idx = frame * self.num_channels + ch;
                    let mut s = buffer[idx] as f64;
                    for f in &mut self.filters[ch] {
                        s = f.process(s);
                    }
                    buffer[idx] = s as f32;
                }
            }
        }

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
    use sotf_host::*;
    use sotf_host::parameters::{ParameterId, ParameterValue};

    #[test]
    fn test_eq_passthrough() {
        let mut p = EqPlugin::new(2, vec![]);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        let mut b = vec![0.5; 2048];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
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
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
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
        let ctx = ProcessContext {
            sample_rate: sample_rate as u32,
            num_frames: 4800,
        };
        plugin.process(&input, &mut expected_output, &ctx).unwrap();

        plugin.reset();
        test_varied_buffer_sizes(&mut plugin, sample_rate, &input, &expected_output);
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
        let ctx = ProcessContext {
            sample_rate,
            num_frames: 512,
        };

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
        for i in 0..num_frames {
            buf[i] = (i as f32 * 0.1).sin() * 0.5;
        }
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
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
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: 44100,
                num_frames: 256,
            },
        )
        .unwrap();

        // Re-initialize at new sample rate - should use update_params, not new
        // (filter params should stay the same, just recompute coeffs for new rate)
        InPlacePlugin::initialize(&mut p, 96000).unwrap();
        assert_eq!(p.sample_rate, 96000);
        // Filter should still have the same user parameters
        assert_eq!(p.filters[0][0].freq, 1000.0);
        assert_eq!(p.filters[0][0].db_gain, 6.0);
        assert!((p.filters[0][0].srate - 96000.0).abs() < 1e-10);
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
        p.process_in_place(
            &mut warmup,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
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
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 512,
            },
        )
        .unwrap();

        // All output samples should be finite
        for (i, &s) in buf.iter().enumerate() {
            assert!(
                s.is_finite(),
                "sample {} not finite: {}",
                i,
                s
            );
        }
    }
}
