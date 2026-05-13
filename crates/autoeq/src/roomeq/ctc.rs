//! Cross-talk cancellation / binaural transfer-matrix support.

use crate::error::{AutoeqError, Result};
use crate::roomeq::types::{
    ChannelDspChain, CtcConfig, CtcMeasurementConfig, CtcWindowConfig, PluginConfigWrapper,
    SystemConfig,
};
use hound::{SampleFormat, WavReader};
use math_audio_dsp::{
    TransferMatrixBin, align_ir_to_reference_peak,
    biquad_complex_response as dsp_biquad_complex_response, deconvolve_sweep_to_ir,
    direct_peak_sample, direct_peak_windowed_half_spectrum, fdw_complex_half_spectrum,
    fir_complex_response, half_spectrum_to_fir, lr4_crossover_response, position_errors,
    solve_minimax_regularized_inverse_bin, solve_regularized_inverse_bin,
    suppress_log_sweep_harmonic_residues,
};
use num_complex::Complex64;
use rustfft::FftPlanner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sotf_host::sofa::{SofaFile, SourcePosition};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

use math_audio_iir_fir::{Biquad, BiquadFilterType};

const CTC_ARTIFACT_VERSION: &str = "ctc-recommended-v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcReport {
    pub enabled: bool,
    pub source: String,
    pub artifact: String,
    pub speakers: Vec<String>,
    pub ears: Vec<String>,
    pub head_positions: usize,
    pub fir_taps: usize,
    pub latency_samples: usize,
    pub latency_ms: f64,
    pub max_filter_gain_db: f64,
    pub max_condition_number: f64,
    pub mean_reconstruction_error: f64,
    pub worst_position_error: f64,
    pub mean_crosstalk_residual_db: f64,
    pub max_electrical_sum_gain_db: f64,
    pub driver_headroom_limited: bool,
    pub room_eq_correction_applied: bool,
    pub room_eq_correction_channels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_response: Option<CtcDeliveredResponseMetrics>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CtcDeliveredResponseMetrics {
    pub mean_target_error: f64,
    pub worst_target_error: f64,
    pub mean_crosstalk_db: f64,
    pub worst_crosstalk_db: f64,
    pub mean_channel_balance_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CtcArtifact {
    version: String,
    source: String,
    sample_rate: u32,
    speakers: Vec<String>,
    ears: Vec<String>,
    fir_taps: usize,
    latency_samples: usize,
    latency_ms: f64,
    max_filter_gain_db: f64,
    max_condition_number: f64,
    mean_reconstruction_error: f64,
    worst_position_error: f64,
    mean_crosstalk_residual_db: f64,
    max_electrical_sum_gain_db: f64,
    driver_headroom_limited: bool,
    room_eq_correction_applied: bool,
    room_eq_correction_channels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delivered_response: Option<CtcDeliveredResponseMetrics>,
    filters: Vec<CtcFirFilterArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CtcFirFilterArtifact {
    speaker: String,
    target_ear: String,
    taps: Vec<f64>,
}

#[derive(Debug)]
struct MatrixSpectrum {
    source: String,
    speakers: Vec<String>,
    ears: Vec<String>,
    positions: Vec<String>,
    bins: Vec<Vec<TransferMatrixBin>>,
}

pub fn maybe_generate_recommended_xtc(
    config: &CtcConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    output_dir: &Path,
    channels: Option<&HashMap<String, ChannelDspChain>>,
) -> Result<Option<CtcReport>> {
    if !config.enabled {
        return Ok(None);
    }
    if config.fir_taps < 16 || !config.fir_taps.is_power_of_two() {
        return Err(AutoeqError::InvalidConfiguration {
            message: "ctc.fir_taps must be a power of two >= 16".to_string(),
        });
    }

    let sample_rate_u32 = checked_sample_rate(sample_rate)?;
    let fft_size = config.fir_taps;
    let mut spectrum = match config.matrix_source.as_str() {
        "measured" => {
            let measurements =
                config
                    .measurements
                    .as_ref()
                    .ok_or(AutoeqError::InvalidConfiguration {
                        message: "ctc.matrix_source='measured' requires ctc.measurements"
                            .to_string(),
                    })?;
            load_measured_spectrum(measurements, &config.window, sample_rate_u32, fft_size)?
        }
        "raw_sweep" => {
            let measurements =
                config
                    .measurements
                    .as_ref()
                    .ok_or(AutoeqError::InvalidConfiguration {
                        message: "ctc.matrix_source='raw_sweep' requires ctc.measurements"
                            .to_string(),
                    })?;
            load_raw_sweep_spectrum(measurements, config, sample_rate_u32, fft_size)?
        }
        "hrtf_database" | "hrtf" => {
            let hrtf = config
                .hrtf
                .as_ref()
                .ok_or(AutoeqError::InvalidConfiguration {
                    message: "ctc.matrix_source='hrtf_database' requires ctc.hrtf".to_string(),
                })?;
            load_hrtf_spectrum(hrtf, sample_rate_u32, fft_size)?
        }
        other => {
            return Err(AutoeqError::InvalidConfiguration {
                message: format!(
                    "unsupported ctc.matrix_source '{}'; expected 'measured', 'raw_sweep', or 'hrtf_database'",
                    other
                ),
            });
        }
    };

    for speaker in &spectrum.speakers {
        if !sys.speakers.contains_key(speaker) {
            return Err(AutoeqError::InvalidConfiguration {
                message: format!(
                    "ctc speaker '{}' is not present in system.speakers",
                    speaker
                ),
            });
        }
    }
    if spectrum.speakers.len() < 2 {
        return Err(AutoeqError::InvalidConfiguration {
            message: "ctc requires at least two speaker roles".to_string(),
        });
    }
    let room_eq_correction_channels = if config.include_room_eq_dsp {
        if let Some(channels) = channels {
            apply_room_eq_dsp_to_spectrum(&mut spectrum, sys, channels, sample_rate)?;
            spectrum
                .speakers
                .iter()
                .filter_map(|speaker| {
                    let channel_name = sys.speakers.get(speaker)?;
                    channels
                        .get(channel_name)
                        .is_some_and(|chain| !chain.plugins.is_empty())
                        .then(|| channel_name.clone())
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let room_eq_correction_applied = !room_eq_correction_channels.is_empty();

    let target = vec![
        Complex64::new(1.0, 0.0),
        Complex64::new(0.0, 0.0),
        Complex64::new(0.0, 0.0),
        Complex64::new(1.0, 0.0),
    ];
    let num_bins = fft_size / 2 + 1;
    let mut solved_bins = Vec::with_capacity(num_bins);
    let mut max_condition = 0.0_f64;
    let mut total_error = 0.0_f64;
    let mut worst_position_error = 0.0_f64;
    let mut headroom_was_limited = false;

    for bin in 0..num_bins {
        let freq = bin as f64 * sample_rate / fft_size as f64;
        let beta = beta_for_frequency(config, freq);
        let solved = if config.robustness == "minimax" {
            solve_minimax_regularized_inverse_bin(
                &spectrum.bins[bin],
                &target,
                beta,
                Some(config.regularization.max_gain_db),
                config.minimax_iterations,
            )
        } else {
            solve_regularized_inverse_bin(
                &spectrum.bins[bin],
                &target,
                beta,
                Some(config.regularization.max_gain_db),
            )
        }
        .map_err(|message| AutoeqError::OptimizationFailed {
            message: format!("ctc inverse failed at bin {}: {}", bin, message),
        })?;
        max_condition = max_condition.max(solved.condition_number);
        let mut values = solved.values;
        headroom_was_limited |= enforce_electrical_sum_headroom(
            &mut values,
            spectrum.speakers.len(),
            2,
            config.regularization.max_gain_db,
        );
        let errors = position_errors(&spectrum.bins[bin], &values, &target).map_err(|message| {
            AutoeqError::OptimizationFailed {
                message: format!(
                    "ctc reconstruction scoring failed at bin {}: {}",
                    bin, message
                ),
            }
        })?;
        total_error += errors.iter().sum::<f64>() / errors.len().max(1) as f64;
        worst_position_error = worst_position_error.max(errors.iter().copied().fold(0.0, f64::max));
        solved_bins.push(values);
    }

    let latency_samples = config.fir_taps / 2;
    let latency_ms = latency_samples as f64 * 1000.0 / sample_rate;
    let max_condition_json = if max_condition.is_finite() {
        max_condition
    } else {
        f64::MAX
    };
    let mean_reconstruction_error = total_error / num_bins as f64;
    let mut filters = Vec::new();
    let mut max_filter_gain_db = f64::NEG_INFINITY;
    let mut max_electrical_sum_gain_db = f64::NEG_INFINITY;

    for speaker_idx in 0..spectrum.speakers.len() {
        for ear_idx in 0..2 {
            let half_spectrum: Vec<Complex64> = solved_bins
                .iter()
                .map(|matrix| matrix[speaker_idx * 2 + ear_idx])
                .collect();
            let max_mag = half_spectrum.iter().map(|v| v.norm()).fold(0.0, f64::max);
            if max_mag > 0.0 {
                max_filter_gain_db = max_filter_gain_db.max(20.0 * max_mag.log10());
            }
            let taps =
                half_spectrum_to_fir(&half_spectrum, config.fir_taps, latency_samples as f64)
                    .map_err(|message| AutoeqError::OptimizationFailed {
                        message: format!("ctc FIR synthesis failed: {}", message),
                    })?;
            filters.push(CtcFirFilterArtifact {
                speaker: spectrum.speakers[speaker_idx].clone(),
                target_ear: spectrum.ears[ear_idx].clone(),
                taps,
            });
        }
        let max_sum_gain = solved_bins
            .iter()
            .map(|matrix| {
                let row_start = speaker_idx * 2;
                (matrix[row_start].norm_sqr() + matrix[row_start + 1].norm_sqr()).sqrt()
            })
            .fold(0.0, f64::max);
        if max_sum_gain > 0.0 {
            max_electrical_sum_gain_db =
                max_electrical_sum_gain_db.max(20.0 * max_sum_gain.log10());
        }
    }

    if !max_filter_gain_db.is_finite() {
        max_filter_gain_db = 0.0;
    }
    if !max_electrical_sum_gain_db.is_finite() {
        max_electrical_sum_gain_db = 0.0;
    }
    let mean_crosstalk_residual_db = reconstruction_error_to_db(mean_reconstruction_error);
    let driver_headroom_limited = headroom_was_limited
        || max_electrical_sum_gain_db >= config.regularization.max_gain_db - 0.25;
    let delivered_response =
        compute_delivered_response_metrics(&spectrum, &filters, config.fir_taps, latency_samples)?;

    std::fs::create_dir_all(output_dir)?;
    let artifact_path = output_dir.join("recommended_xtc_matrix.json");
    let artifact = CtcArtifact {
        version: CTC_ARTIFACT_VERSION.to_string(),
        source: spectrum.source.clone(),
        sample_rate: sample_rate_u32,
        speakers: spectrum.speakers.clone(),
        ears: spectrum.ears.clone(),
        fir_taps: config.fir_taps,
        latency_samples,
        latency_ms,
        max_filter_gain_db,
        max_condition_number: max_condition_json,
        mean_reconstruction_error,
        worst_position_error,
        mean_crosstalk_residual_db,
        max_electrical_sum_gain_db,
        driver_headroom_limited,
        room_eq_correction_applied,
        room_eq_correction_channels: room_eq_correction_channels.clone(),
        delivered_response: Some(delivered_response.clone()),
        filters,
    };
    let json = serde_json::to_vec_pretty(&artifact)?;
    std::fs::write(&artifact_path, json)?;

    Ok(Some(CtcReport {
        enabled: true,
        source: spectrum.source,
        artifact: artifact_path.to_string_lossy().to_string(),
        speakers: spectrum.speakers,
        ears: spectrum.ears,
        head_positions: spectrum.positions.len(),
        fir_taps: config.fir_taps,
        latency_samples,
        latency_ms,
        max_filter_gain_db,
        max_condition_number: max_condition_json,
        mean_reconstruction_error,
        worst_position_error,
        mean_crosstalk_residual_db,
        max_electrical_sum_gain_db,
        driver_headroom_limited,
        room_eq_correction_applied,
        room_eq_correction_channels,
        delivered_response: Some(delivered_response),
    }))
}

fn compute_delivered_response_metrics(
    spectrum: &MatrixSpectrum,
    filters: &[CtcFirFilterArtifact],
    fft_size: usize,
    latency_samples: usize,
) -> Result<CtcDeliveredResponseMetrics> {
    let num_bins = fft_size / 2 + 1;
    let speakers = spectrum.speakers.len();
    let mut filter_spectra = Vec::with_capacity(speakers * 2);
    for speaker in &spectrum.speakers {
        for ear in &spectrum.ears {
            let filter = filters
                .iter()
                .find(|filter| filter.speaker == *speaker && filter.target_ear == *ear)
                .ok_or_else(|| AutoeqError::OptimizationFailed {
                    message: format!(
                        "ctc delivered-response scoring missing filter speaker='{}', target_ear='{}'",
                        speaker, ear
                    ),
                })?;
            filter_spectra.push(fft_real_to_half_spectrum_f64(&filter.taps, fft_size));
        }
    }

    let mut target_error_sum_sq = 0.0_f64;
    let mut target_count = 0usize;
    let mut worst_target_error = 0.0_f64;
    let mut crosstalk_sum_sq = 0.0_f64;
    let mut crosstalk_count = 0usize;
    let mut worst_crosstalk = 0.0_f64;
    let mut balance_sum_db = 0.0_f64;
    let mut balance_count = 0usize;

    #[allow(clippy::needless_range_loop)]
    for bin in 0..num_bins {
        let latency_phase = 2.0 * PI * bin as f64 * latency_samples as f64 / fft_size as f64;
        let undo_latency = Complex64::from_polar(1.0, latency_phase);

        for position in &spectrum.bins[bin] {
            let mut delivered = [Complex64::new(0.0, 0.0); 4];
            for ear_idx in 0..2 {
                for target_ear_idx in 0..2 {
                    let mut sum = Complex64::new(0.0, 0.0);
                    for speaker_idx in 0..speakers {
                        let h = position.values[ear_idx * speakers + speaker_idx];
                        let f = filter_spectra[speaker_idx * 2 + target_ear_idx][bin];
                        sum += h * f;
                    }
                    delivered[ear_idx * 2 + target_ear_idx] = sum * undo_latency;
                }
            }

            for ear_idx in 0..2 {
                let target = delivered[ear_idx * 2 + ear_idx];
                let error = (target - Complex64::new(1.0, 0.0)).norm();
                target_error_sum_sq += error * error;
                worst_target_error = worst_target_error.max(error);
                target_count += 1;
            }

            let left_mag = delivered[0].norm();
            let right_mag = delivered[3].norm();
            balance_sum_db += (amplitude_to_db(left_mag) - amplitude_to_db(right_mag)).abs();
            balance_count += 1;

            for (ear_idx, target_ear_idx) in [(0, 1), (1, 0)] {
                let crosstalk = delivered[ear_idx * 2 + target_ear_idx].norm();
                crosstalk_sum_sq += crosstalk * crosstalk;
                worst_crosstalk = worst_crosstalk.max(crosstalk);
                crosstalk_count += 1;
            }
        }
    }

    let mean_target_error = if target_count == 0 {
        0.0
    } else {
        (target_error_sum_sq / target_count as f64).sqrt()
    };
    let mean_crosstalk = if crosstalk_count == 0 {
        0.0
    } else {
        (crosstalk_sum_sq / crosstalk_count as f64).sqrt()
    };
    let mean_channel_balance_db = if balance_count == 0 {
        0.0
    } else {
        balance_sum_db / balance_count as f64
    };

    Ok(CtcDeliveredResponseMetrics {
        mean_target_error,
        worst_target_error,
        mean_crosstalk_db: amplitude_to_db(mean_crosstalk),
        worst_crosstalk_db: amplitude_to_db(worst_crosstalk),
        mean_channel_balance_db,
    })
}

fn apply_room_eq_dsp_to_spectrum(
    spectrum: &mut MatrixSpectrum,
    sys: &SystemConfig,
    channels: &HashMap<String, ChannelDspChain>,
    sample_rate: f64,
) -> Result<()> {
    let speakers = spectrum.speakers.len();
    let fft_size = (spectrum.bins.len() - 1) * 2;
    let mut cache = DspResponseCache::new(checked_sample_rate(sample_rate)?);
    let mut responses_by_speaker = Vec::with_capacity(speakers);
    for speaker in &spectrum.speakers {
        let Some(channel_name) = sys.speakers.get(speaker) else {
            responses_by_speaker.push(vec![Complex64::new(1.0, 0.0); spectrum.bins.len()]);
            continue;
        };
        let Some(chain) = channels.get(channel_name) else {
            responses_by_speaker.push(vec![Complex64::new(1.0, 0.0); spectrum.bins.len()]);
            continue;
        };
        let mut responses = Vec::with_capacity(spectrum.bins.len());
        for bin in 0..spectrum.bins.len() {
            let freq = bin as f64 * sample_rate / fft_size as f64;
            responses.push(channel_chain_response(
                chain,
                freq,
                sample_rate,
                &mut cache,
            )?);
        }
        responses_by_speaker.push(responses);
    }

    #[allow(clippy::needless_range_loop)]
    for bin in 0..spectrum.bins.len() {
        for position in &mut spectrum.bins[bin] {
            for speaker_idx in 0..speakers {
                let correction = responses_by_speaker[speaker_idx][bin];
                for ear_idx in 0..2 {
                    position.values[ear_idx * speakers + speaker_idx] *= correction;
                }
            }
        }
    }
    Ok(())
}

struct DspResponseCache {
    sample_rate: u32,
    convolution_ir: HashMap<PathBuf, Vec<f64>>,
}

impl DspResponseCache {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            convolution_ir: HashMap::new(),
        }
    }

    fn convolution_taps(&mut self, path: &Path) -> Result<&[f64]> {
        if !self.convolution_ir.contains_key(path) {
            let channels =
                read_wav_channels_f64(path, self.sample_rate, "RoomEQ convolution IR WAV")?;
            let Some(first_channel) = channels.into_iter().next() else {
                return Err(AutoeqError::InvalidMeasurement {
                    message: format!("convolution IR '{}' has no channels", path.display()),
                });
            };
            self.convolution_ir
                .insert(path.to_path_buf(), first_channel);
        }
        Ok(self
            .convolution_ir
            .get(path)
            .map(Vec::as_slice)
            .expect("cached convolution IR"))
    }
}

fn channel_chain_response(
    chain: &ChannelDspChain,
    freq: f64,
    sample_rate: f64,
    cache: &mut DspResponseCache,
) -> Result<Complex64> {
    let branch_response = if let Some(drivers) = chain.drivers.as_ref() {
        if drivers.is_empty() {
            Complex64::new(1.0, 0.0)
        } else {
            let mut sum = Complex64::new(0.0, 0.0);
            for driver in drivers {
                sum += plugin_chain_response(&driver.plugins, freq, sample_rate, cache)?;
            }
            sum
        }
    } else {
        Complex64::new(1.0, 0.0)
    };

    Ok(branch_response * plugin_chain_response(&chain.plugins, freq, sample_rate, cache)?)
}

fn plugin_chain_response(
    plugins: &[PluginConfigWrapper],
    freq: f64,
    sample_rate: f64,
    cache: &mut DspResponseCache,
) -> Result<Complex64> {
    let mut response = Complex64::new(1.0, 0.0);
    let mut idx = 0usize;
    while idx < plugins.len() {
        let plugin = &plugins[idx];
        if plugin.plugin_type == "band_split"
            && let Some(merge_offset) = plugins[idx + 1..]
                .iter()
                .position(|candidate| candidate.plugin_type == "band_merge")
        {
            let merge_idx = idx + 1 + merge_offset;
            response *= mixed_band_response(
                plugin,
                &plugins[idx + 1..merge_idx],
                freq,
                sample_rate,
                cache,
            )?;
            idx = merge_idx + 1;
            continue;
        }
        response *= plugin_response(plugin, freq, sample_rate, cache)?;
        idx += 1;
    }
    Ok(response)
}

fn mixed_band_response(
    split: &PluginConfigWrapper,
    plugins: &[PluginConfigWrapper],
    freq: f64,
    sample_rate: f64,
    cache: &mut DspResponseCache,
) -> Result<Complex64> {
    let frequency = split
        .parameters
        .get("frequency")
        .and_then(|value| value.as_f64())
        .unwrap_or(1_000.0);
    let mut low =
        lr4_crossover_response("low", frequency, freq, sample_rate).map_err(|message| {
            AutoeqError::InvalidConfiguration {
                message: format!("unsupported RoomEQ band_split in CTC joint path: {message}"),
            }
        })?;
    let mut high =
        lr4_crossover_response("high", frequency, freq, sample_rate).map_err(|message| {
            AutoeqError::InvalidConfiguration {
                message: format!("unsupported RoomEQ band_split in CTC joint path: {message}"),
            }
        })?;

    for plugin in plugins {
        let plugin_response = plugin_response(plugin, freq, sample_rate, cache)?;
        if plugin_affects_mixed_band(plugin, true) {
            low *= plugin_response;
        }
        if plugin_affects_mixed_band(plugin, false) {
            high *= plugin_response;
        }
    }

    Ok(low + high)
}

fn plugin_affects_mixed_band(plugin: &PluginConfigWrapper, low_band: bool) -> bool {
    let Some(channels) = plugin
        .parameters
        .get("channels")
        .and_then(|value| value.as_array())
    else {
        return true;
    };
    channels
        .iter()
        .filter_map(|value| value.as_u64())
        .any(|ch| {
            if low_band {
                ch == 0 || ch == 1
            } else {
                ch == 2 || ch == 3
            }
        })
}

fn plugin_response(
    plugin: &PluginConfigWrapper,
    freq: f64,
    sample_rate: f64,
    cache: &mut DspResponseCache,
) -> Result<Complex64> {
    match plugin.plugin_type.as_str() {
        "gain" => {
            let gain_db = plugin
                .parameters
                .get("gain_db")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let invert = plugin
                .parameters
                .get("invert")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let sign = if invert { -1.0 } else { 1.0 };
            Ok(Complex64::new(sign * 10.0_f64.powf(gain_db / 20.0), 0.0))
        }
        "convolution" => convolution_response(plugin, freq, sample_rate, cache),
        "crossover" => {
            let frequency = plugin
                .parameters
                .get("frequency")
                .and_then(|value| value.as_f64())
                .unwrap_or(1_000.0);
            let output = plugin
                .parameters
                .get("output")
                .and_then(|value| value.as_str())
                .unwrap_or("both");
            lr4_crossover_response(output, frequency, freq, sample_rate).map_err(|message| {
                AutoeqError::InvalidConfiguration {
                    message: format!("unsupported RoomEQ crossover in CTC joint path: {message}"),
                }
            })
        }
        "delay" => {
            let delay_ms = plugin
                .parameters
                .get("delay_ms")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let phase = -2.0 * PI * freq * delay_ms / 1000.0;
            Ok(Complex64::from_polar(1.0, phase))
        }
        "eq" => {
            let mut response = Complex64::new(1.0, 0.0);
            if let Some(filters) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                for filter in filters {
                    response *= biquad_filter_response(filter, freq, sample_rate)?;
                }
            }
            Ok(response)
        }
        _ => Ok(Complex64::new(1.0, 0.0)),
    }
}

fn convolution_response(
    plugin: &PluginConfigWrapper,
    freq: f64,
    sample_rate: f64,
    cache: &mut DspResponseCache,
) -> Result<Complex64> {
    let Some(ir_file) = plugin
        .parameters
        .get("ir_file")
        .and_then(|value| value.as_str())
    else {
        return Ok(Complex64::new(1.0, 0.0));
    };
    let taps = cache.convolution_taps(Path::new(ir_file))?;
    let wet = fir_complex_response(taps, freq, sample_rate);
    let mix = plugin
        .parameters
        .get("mix")
        .and_then(|value| value.as_f64())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let gain = plugin
        .parameters
        .get("gain_db")
        .and_then(|value| value.as_f64())
        .map(|gain_db| 10.0_f64.powf(gain_db / 20.0))
        .unwrap_or(1.0);
    Ok(Complex64::new(1.0 - mix, 0.0) + wet * (mix * gain))
}

fn biquad_filter_response(
    filter: &serde_json::Value,
    freq: f64,
    sample_rate: f64,
) -> Result<Complex64> {
    let filter_type = filter
        .get("filter_type")
        .and_then(|value| value.as_str())
        .and_then(parse_biquad_filter_type)
        .ok_or_else(|| AutoeqError::InvalidConfiguration {
            message: format!(
                "unsupported RoomEQ biquad filter type in CTC joint path: {}",
                filter
            ),
        })?;
    let freq_hz = filter
        .get("freq")
        .and_then(|value| value.as_f64())
        .unwrap_or(1000.0);
    let q = filter
        .get("q")
        .and_then(|value| value.as_f64())
        .unwrap_or(1.0);
    let db_gain = filter
        .get("db_gain")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    Ok(dsp_biquad_complex_response(
        &Biquad::new(filter_type, freq_hz, sample_rate, q, db_gain),
        freq,
    ))
}

fn parse_biquad_filter_type(value: &str) -> Option<BiquadFilterType> {
    match value {
        "lowpass" => Some(BiquadFilterType::Lowpass),
        "highpass" => Some(BiquadFilterType::Highpass),
        "highpassvariableq" => Some(BiquadFilterType::HighpassVariableQ),
        "bandpass" => Some(BiquadFilterType::Bandpass),
        "peak" => Some(BiquadFilterType::Peak),
        "notch" => Some(BiquadFilterType::Notch),
        "lowshelf" => Some(BiquadFilterType::Lowshelf),
        "highshelf" => Some(BiquadFilterType::Highshelf),
        "allpass" => Some(BiquadFilterType::AllPass),
        "lowshelforf" => Some(BiquadFilterType::LowshelfOrf),
        "highshelforf" => Some(BiquadFilterType::HighshelfOrf),
        "peakmatched" => Some(BiquadFilterType::PeakMatched),
        _ => None,
    }
}

fn enforce_electrical_sum_headroom(
    values: &mut [Complex64],
    speakers: usize,
    ears: usize,
    max_gain_db: f64,
) -> bool {
    let max_gain = 10.0_f64.powf(max_gain_db / 20.0);
    let mut limited = false;
    for speaker_idx in 0..speakers {
        let row_start = speaker_idx * ears;
        let row_end = row_start + ears;
        let row_norm = values[row_start..row_end]
            .iter()
            .map(|value| value.norm_sqr())
            .sum::<f64>()
            .sqrt();
        if row_norm > max_gain && row_norm > 0.0 {
            let scale = max_gain / row_norm;
            for value in &mut values[row_start..row_end] {
                *value *= scale;
            }
            limited = true;
        }
    }
    limited
}

fn checked_sample_rate(sample_rate: f64) -> Result<u32> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 || sample_rate > u32::MAX as f64 {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!("invalid sample rate for CTC: {}", sample_rate),
        });
    }
    Ok(sample_rate.round() as u32)
}

fn beta_for_frequency(config: &CtcConfig, freq_hz: f64) -> f64 {
    let beta_db = if freq_hz < 150.0 {
        config.regularization.beta_lf_db
    } else if freq_hz > 6000.0 {
        config.regularization.beta_hf_db
    } else {
        config.regularization.beta_db
    };
    let robustness_scale = if config.robustness == "minimax" {
        2.0
    } else {
        1.0
    };
    10.0_f64.powf(beta_db / 20.0) * robustness_scale
}

fn load_measured_spectrum(
    measurements: &CtcMeasurementConfig,
    window: &CtcWindowConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<MatrixSpectrum> {
    if window.window_type != "ctc_direct" && window.window_type != "fdw" {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "unsupported ctc.window.window_type '{}'; expected 'ctc_direct' or 'fdw'",
                window.window_type
            ),
        });
    }
    let speakers = measurements.speakers.clone();
    let ears = if measurements.mics.is_empty() {
        vec!["left_ear".to_string(), "right_ear".to_string()]
    } else {
        measurements.mics.clone()
    };
    if ears.len() != 2 {
        return Err(AutoeqError::InvalidConfiguration {
            message: "ctc.measurements.mics must contain exactly two ears".to_string(),
        });
    }
    let positions: Vec<String> = if measurements.head_positions.is_empty() {
        vec!["primary".to_string()]
    } else {
        measurements
            .head_positions
            .iter()
            .map(|position| position.id.clone())
            .collect()
    };

    let mut file_map: HashMap<(String, String), Option<std::path::PathBuf>> = HashMap::new();
    for file in &measurements.files {
        file_map.insert(
            (file.head_position.clone(), file.speaker.clone()),
            file.ir.clone(),
        );
    }

    let mut spectra_by_position = Vec::new();
    for position in &positions {
        let mut speaker_spectra = Vec::new();
        for speaker in &speakers {
            let path = file_map
                .get(&(position.clone(), speaker.clone()))
                .ok_or_else(|| AutoeqError::InvalidConfiguration {
                    message: format!(
                        "missing CTC IR file for head_position='{}', speaker='{}'",
                        position, speaker
                    ),
                })?;
            let path = path.as_ref().ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: format!(
                    "ctc.matrix_source='measured' requires ir for head_position='{}', speaker='{}'",
                    position, speaker
                ),
            })?;
            speaker_spectra.push(load_two_channel_ir_spectrum(
                path,
                window,
                sample_rate,
                fft_size,
            )?);
        }
        spectra_by_position.push(speaker_spectra);
    }

    Ok(build_matrix_spectrum(
        "measured".to_string(),
        speakers,
        ears,
        positions,
        spectra_by_position,
        fft_size / 2 + 1,
    ))
}

fn load_raw_sweep_spectrum(
    measurements: &CtcMeasurementConfig,
    config: &CtcConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<MatrixSpectrum> {
    if config.window.window_type != "ctc_direct" && config.window.window_type != "fdw" {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "unsupported ctc.window.window_type '{}'; expected 'ctc_direct' or 'fdw'",
                config.window.window_type
            ),
        });
    }
    let reference_sweep =
        config
            .reference_sweep
            .as_ref()
            .ok_or(AutoeqError::InvalidConfiguration {
                message: "ctc.matrix_source='raw_sweep' requires ctc.reference_sweep".to_string(),
            })?;
    let reference_channels =
        read_wav_channels_f64(reference_sweep, sample_rate, "CTC reference sweep")?;
    let reference = reference_channels
        .first()
        .ok_or_else(|| AutoeqError::InvalidMeasurement {
            message: format!(
                "CTC reference sweep '{}' has no channels",
                reference_sweep.display()
            ),
        })?;

    let speakers = measurements.speakers.clone();
    let ears = if measurements.mics.is_empty() {
        vec!["left_ear".to_string(), "right_ear".to_string()]
    } else {
        measurements.mics.clone()
    };
    if ears.len() != 2 {
        return Err(AutoeqError::InvalidConfiguration {
            message: "ctc.measurements.mics must contain exactly two ears".to_string(),
        });
    }
    let positions: Vec<String> = if measurements.head_positions.is_empty() {
        vec!["primary".to_string()]
    } else {
        measurements
            .head_positions
            .iter()
            .map(|position| position.id.clone())
            .collect()
    };

    let mut file_map = HashMap::new();
    for file in &measurements.files {
        file_map.insert(
            (file.head_position.clone(), file.speaker.clone()),
            (file.raw_sweep.clone(), file.loopback.clone()),
        );
    }

    let mut spectra_by_position = Vec::new();
    for position in &positions {
        let mut speaker_spectra = Vec::new();
        for speaker in &speakers {
            let (raw_sweep, loopback) = file_map
                .get(&(position.clone(), speaker.clone()))
                .ok_or_else(|| AutoeqError::InvalidConfiguration {
                    message: format!(
                        "missing CTC raw sweep file for head_position='{}', speaker='{}'",
                        position, speaker
                    ),
                })?;
            let raw_sweep = raw_sweep.as_ref().ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: format!(
                    "ctc.matrix_source='raw_sweep' requires raw_sweep for head_position='{}', speaker='{}'",
                    position, speaker
                ),
            })?;
            let loopback = loopback.as_ref().ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: format!(
                    "ctc.matrix_source='raw_sweep' requires loopback for head_position='{}', speaker='{}'",
                    position, speaker
                ),
            })?;
            speaker_spectra.push(load_two_channel_raw_sweep_spectrum(
                raw_sweep,
                loopback,
                reference,
                config,
                sample_rate,
                fft_size,
            )?);
        }
        spectra_by_position.push(speaker_spectra);
    }

    Ok(build_matrix_spectrum(
        "raw_sweep".to_string(),
        speakers,
        ears,
        positions,
        spectra_by_position,
        fft_size / 2 + 1,
    ))
}

fn load_hrtf_spectrum(
    hrtf: &crate::roomeq::types::CtcHrtfConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<MatrixSpectrum> {
    let sofa =
        SofaFile::load(&hrtf.hrtf_file).map_err(|message| AutoeqError::InvalidMeasurement {
            message: format!(
                "failed to load CTC HRTF '{}': {}",
                hrtf.hrtf_file.display(),
                message
            ),
        })?;
    if let Some(sofa_sr) = sofa.data_sample_rate
        && (sofa_sr - sample_rate as f32).abs() > 1.0
    {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "SOFA sample rate {} Hz differs from roomEQ sample rate {} Hz",
                sofa_sr, sample_rate
            ),
        });
    }

    let mut speaker_spectra = Vec::new();
    let mut speakers = Vec::new();
    for speaker in &hrtf.speakers {
        let position = SourcePosition::new(
            speaker.azimuth_deg as f32,
            speaker.elevation_deg as f32,
            speaker.distance_m as f32,
        );
        let data = sofa.get_hrtf_at_position(&position).ok_or_else(|| {
            AutoeqError::InvalidMeasurement {
                message: format!(
                    "no HRTF measurement found for speaker '{}'",
                    speaker.speaker
                ),
            }
        })?;
        speakers.push(speaker.speaker.clone());
        speaker_spectra.push([
            fft_real_to_half_spectrum(&data.ir_left, fft_size),
            fft_real_to_half_spectrum(&data.ir_right, fft_size),
        ]);
    }

    Ok(build_matrix_spectrum(
        "hrtf_database".to_string(),
        speakers,
        vec!["left_ear".to_string(), "right_ear".to_string()],
        vec!["primary".to_string()],
        vec![speaker_spectra],
        fft_size / 2 + 1,
    ))
}

fn build_matrix_spectrum(
    source: String,
    speakers: Vec<String>,
    ears: Vec<String>,
    positions: Vec<String>,
    spectra_by_position: Vec<Vec<[Vec<Complex64>; 2]>>,
    num_bins: usize,
) -> MatrixSpectrum {
    let mut bins = Vec::with_capacity(num_bins);
    for bin in 0..num_bins {
        let mut position_bins = Vec::with_capacity(positions.len());
        for speaker_spectra in &spectra_by_position {
            let mut values = vec![Complex64::new(0.0, 0.0); 2 * speakers.len()];
            for (speaker_idx, ear_spectra) in speaker_spectra.iter().enumerate() {
                values[speaker_idx] = ear_spectra[0][bin];
                values[speakers.len() + speaker_idx] = ear_spectra[1][bin];
            }
            position_bins.push(TransferMatrixBin::new(2, speakers.len(), values));
        }
        bins.push(position_bins);
    }

    MatrixSpectrum {
        source,
        speakers,
        ears,
        positions,
        bins,
    }
}

fn load_two_channel_ir_spectrum(
    path: &Path,
    window: &CtcWindowConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<[Vec<Complex64>; 2]> {
    let channels = read_wav_channels_f64(path, sample_rate, "CTC IR WAV")?;
    if channels.len() != 2 {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!(
                "CTC IR WAV '{}' must have exactly two channels, got {}",
                path.display(),
                channels.len()
            ),
        });
    }
    two_channel_ir_spectrum(&channels[0], &channels[1], window, sample_rate, fft_size)
}

fn load_two_channel_raw_sweep_spectrum(
    raw_sweep: &Path,
    loopback: &Path,
    reference: &[f64],
    config: &CtcConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<[Vec<Complex64>; 2]> {
    let raw_channels = read_wav_channels_f64(raw_sweep, sample_rate, "CTC raw sweep WAV")?;
    if raw_channels.len() != 2 {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!(
                "CTC raw sweep WAV '{}' must have exactly two channels, got {}",
                raw_sweep.display(),
                raw_channels.len()
            ),
        });
    }
    let loopback_channels = read_wav_channels_f64(loopback, sample_rate, "CTC loopback WAV")?;
    let loopback_signal =
        loopback_channels
            .first()
            .ok_or_else(|| AutoeqError::InvalidMeasurement {
                message: format!("CTC loopback WAV '{}' has no channels", loopback.display()),
            })?;
    let deconvolve_fft_size = raw_channels[0]
        .len()
        .max(raw_channels[1].len())
        .max(loopback_signal.len())
        .max(reference.len())
        .next_power_of_two()
        .max(fft_size);
    let loopback_ir = deconvolve_sweep_to_ir(loopback_signal, reference, deconvolve_fft_size)
        .map_err(|message| AutoeqError::InvalidMeasurement {
            message: format!(
                "failed deconvolving CTC loopback '{}': {}",
                loopback.display(),
                message
            ),
        })?;
    let alignment_peak = direct_peak_sample(&loopback_ir);
    let mut ear_irs = [Vec::new(), Vec::new()];
    for ear_idx in 0..2 {
        let ir = deconvolve_sweep_to_ir(&raw_channels[ear_idx], reference, deconvolve_fft_size)
            .map_err(|message| AutoeqError::InvalidMeasurement {
                message: format!(
                    "failed deconvolving CTC raw sweep '{}' channel {}: {}",
                    raw_sweep.display(),
                    ear_idx + 1,
                    message
                ),
            })?;
        let mut aligned = align_ir_to_reference_peak(&ir, alignment_peak);
        if let (Some(duration), Some(start_hz), Some(end_hz)) = (
            config.sweep_duration_s,
            config.sweep_start_hz,
            config.sweep_end_hz,
        ) {
            suppress_log_sweep_harmonic_residues(
                &mut aligned,
                sample_rate as f64,
                duration,
                start_hz,
                end_hz,
                config.harmonic_suppression_harmonics,
                config.harmonic_suppression_window_ms,
            );
        }
        ear_irs[ear_idx] = aligned;
    }
    two_channel_ir_spectrum(
        &ear_irs[0],
        &ear_irs[1],
        &config.window,
        sample_rate,
        fft_size,
    )
}

fn two_channel_ir_spectrum(
    left: &[f64],
    right: &[f64],
    window: &CtcWindowConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<[Vec<Complex64>; 2]> {
    Ok([
        ir_to_half_spectrum(left, window, sample_rate, fft_size)?,
        ir_to_half_spectrum(right, window, sample_rate, fft_size)?,
    ])
}

fn ir_to_half_spectrum(
    ir: &[f64],
    window: &CtcWindowConfig,
    sample_rate: u32,
    fft_size: usize,
) -> Result<Vec<Complex64>> {
    match window.window_type.as_str() {
        "ctc_direct" => direct_peak_windowed_half_spectrum(
            ir,
            sample_rate as f64,
            fft_size,
            window.start_ms,
            window.length_ms,
            window.fade_ms,
        )
        .map_err(|message| AutoeqError::InvalidMeasurement {
            message: format!("failed direct-windowing CTC IR: {}", message),
        }),
        "fdw" => fdw_complex_half_spectrum(
            ir,
            sample_rate as f64,
            fft_size,
            direct_peak_sample(ir),
            window.fdw_cycles,
            window.fdw_min_ms,
            window.fdw_max_ms,
        )
        .map_err(|message| AutoeqError::InvalidMeasurement {
            message: format!("failed FDW-windowing CTC IR: {}", message),
        }),
        other => Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "unsupported ctc.window.window_type '{}'; expected 'ctc_direct' or 'fdw'",
                other
            ),
        }),
    }
}

fn read_wav_channels_f64(path: &Path, sample_rate: u32, label: &str) -> Result<Vec<Vec<f64>>> {
    let mut reader = WavReader::open(path).map_err(|err| AutoeqError::InvalidMeasurement {
        message: format!("failed to open {} '{}': {}", label, path.display(), err),
    })?;
    let spec = reader.spec();
    if spec.sample_rate != sample_rate {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!(
                "{} '{}' sample rate {} differs from roomEQ sample rate {}",
                label,
                path.display(),
                spec.sample_rate,
                sample_rate
            ),
        });
    }
    if spec.channels == 0 {
        return Err(AutoeqError::InvalidMeasurement {
            message: format!("{} '{}' has no channels", label, path.display()),
        });
    }
    let mut channels = vec![Vec::new(); spec.channels as usize];
    match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            for (idx, sample) in reader.samples::<f32>().enumerate() {
                let value = sample.map_err(|err| AutoeqError::InvalidMeasurement {
                    message: format!("failed reading '{}': {}", path.display(), err),
                })? as f64;
                channels[idx % spec.channels as usize].push(value);
            }
        }
        (SampleFormat::Int, bits) if bits <= 16 => {
            let scale = (1_i64 << (bits - 1)) as f64;
            for (idx, sample) in reader.samples::<i16>().enumerate() {
                let value = sample.map_err(|err| AutoeqError::InvalidMeasurement {
                    message: format!("failed reading '{}': {}", path.display(), err),
                })? as f64
                    / scale;
                channels[idx % spec.channels as usize].push(value);
            }
        }
        (SampleFormat::Int, bits) => {
            let scale = (1_i64 << (bits - 1)) as f64;
            for (idx, sample) in reader.samples::<i32>().enumerate() {
                let value = sample.map_err(|err| AutoeqError::InvalidMeasurement {
                    message: format!("failed reading '{}': {}", path.display(), err),
                })? as f64
                    / scale;
                channels[idx % spec.channels as usize].push(value);
            }
        }
        other => {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "unsupported {} format {:?} in '{}'",
                    label,
                    other,
                    path.display()
                ),
            });
        }
    }
    Ok(channels)
}

fn fft_real_to_half_spectrum(input: &[f32], fft_size: usize) -> Vec<Complex64> {
    let input_f64: Vec<f64> = input.iter().map(|v| *v as f64).collect();
    fft_real_to_half_spectrum_f64(&input_f64, fft_size)
}

fn fft_real_to_half_spectrum_f64(input: &[f64], fft_size: usize) -> Vec<Complex64> {
    let mut buffer = vec![Complex64::new(0.0, 0.0); fft_size];
    let copy_len = input.len().min(fft_size);
    for idx in 0..copy_len {
        buffer[idx] = Complex64::new(input[idx], 0.0);
    }
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut buffer);
    buffer.truncate(fft_size / 2 + 1);
    buffer
}

fn reconstruction_error_to_db(error: f64) -> f64 {
    10.0 * error.max(1e-24).log10()
}

fn amplitude_to_db(value: f64) -> f64 {
    20.0 * value.max(1e-12).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roomeq::types::{
        CtcConfig, CtcMeasurementConfig, CtcMeasurementFileConfig, CtcRegularizationConfig,
        CtcWindowConfig, SystemConfig, SystemModel,
    };
    use std::collections::HashMap;
    use std::f64::consts::PI;
    use tempfile::tempdir;

    #[test]
    fn beta_uses_lf_mid_hf_bands() {
        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "measured".to_string(),
            measurements: None,
            hrtf: None,
            window: CtcWindowConfig::default(),
            regularization: CtcRegularizationConfig {
                beta_db: -30.0,
                beta_lf_db: -20.0,
                beta_hf_db: -40.0,
                max_gain_db: 12.0,
            },
            robustness: "average".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 1024,
            reference_sweep: None,
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 8,
        };
        assert!((beta_for_frequency(&cfg, 80.0) - 0.1).abs() < 1e-12);
        assert!((beta_for_frequency(&cfg, 1000.0) - 10.0_f64.powf(-30.0 / 20.0)).abs() < 1e-12);
        assert!((beta_for_frequency(&cfg, 8000.0) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn electrical_sum_headroom_scales_complete_speaker_rows() {
        let mut values = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.25, 0.0),
            Complex64::new(0.5, 0.0),
        ];

        assert!(enforce_electrical_sum_headroom(&mut values, 2, 2, 0.0));

        let first_row_norm = (values[0].norm_sqr() + values[1].norm_sqr()).sqrt();
        let second_row_norm = (values[2].norm_sqr() + values[3].norm_sqr()).sqrt();
        assert!((first_row_norm - 1.0).abs() < 1e-12);
        assert!((second_row_norm - 0.559016994).abs() < 1e-9);
        assert!((values[0].re - values[1].re).abs() < 1e-12);
    }

    #[test]
    fn joint_room_eq_path_models_convolution_ir_phase() {
        let dir = tempdir().unwrap();
        let ir = dir.path().join("delay_one.wav");
        write_mono_float_wav(&ir, &[0.0, 1.0, 0.0, 0.0]);
        let chain = test_channel_chain(
            vec![PluginConfigWrapper {
                plugin_type: "convolution".to_string(),
                parameters: serde_json::json!({
                    "ir_file": ir,
                    "mix": 1.0,
                    "gain_db": 0.0
                }),
            }],
            None,
        );
        let mut cache = DspResponseCache::new(48_000);
        let response = channel_chain_response(&chain, 12_000.0, 48_000.0, &mut cache).unwrap();
        assert!(response.re.abs() < 1e-9);
        assert!((response.im + 1.0).abs() < 1e-6);
    }

    #[test]
    fn joint_room_eq_path_models_mixed_fir_iir_band_split() {
        let chain = test_channel_chain(
            vec![
                PluginConfigWrapper {
                    plugin_type: "band_split".to_string(),
                    parameters: serde_json::json!({
                        "frequency": 1_000.0,
                        "type": "LR24"
                    }),
                },
                PluginConfigWrapper {
                    plugin_type: "gain".to_string(),
                    parameters: serde_json::json!({
                        "gain_db": -12.0,
                        "channels": [0, 1]
                    }),
                },
                PluginConfigWrapper {
                    plugin_type: "band_merge".to_string(),
                    parameters: serde_json::json!({
                        "bands": 2
                    }),
                },
            ],
            None,
        );
        let mut cache = DspResponseCache::new(48_000);
        let low = channel_chain_response(&chain, 100.0, 48_000.0, &mut cache)
            .unwrap()
            .norm();
        let high = channel_chain_response(&chain, 10_000.0, 48_000.0, &mut cache)
            .unwrap()
            .norm();
        assert!(low < 0.35, "low-band gain should attenuate LF, got {low}");
        assert!(high > 0.8, "high band should pass through, got {high}");
    }

    #[test]
    fn joint_room_eq_path_models_driver_crossover_branches() {
        let low_driver = crate::roomeq::types::DriverDspChain {
            name: "woofer".to_string(),
            index: 0,
            plugins: vec![PluginConfigWrapper {
                plugin_type: "crossover".to_string(),
                parameters: serde_json::json!({
                    "type": "LR24",
                    "frequency": 1_000.0,
                    "output": "low"
                }),
            }],
            initial_curve: None,
        };
        let chain = test_channel_chain(Vec::new(), Some(vec![low_driver]));
        let mut cache = DspResponseCache::new(48_000);
        let low = channel_chain_response(&chain, 100.0, 48_000.0, &mut cache)
            .unwrap()
            .norm();
        let high = channel_chain_response(&chain, 10_000.0, 48_000.0, &mut cache)
            .unwrap()
            .norm();
        assert!(low > 0.9, "lowpass driver should pass LF, got {low}");
        assert!(high < 0.05, "lowpass driver should reject HF, got {high}");
    }

    #[test]
    fn measured_wav_requires_two_channels() {
        let dir = tempdir().unwrap();
        let wav = dir.path().join("mono.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        writer.write_sample::<i16>(0).unwrap();
        writer.finalize().unwrap();

        let err = load_two_channel_ir_spectrum(&wav, &CtcWindowConfig::default(), 48_000, 1024)
            .unwrap_err();
        assert!(err.to_string().contains("exactly two channels"));
    }

    #[test]
    fn measured_config_reports_missing_position_speaker_file() {
        let dir = tempdir().unwrap();
        let wav = dir.path().join("left.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        for _ in 0..32 {
            writer.write_sample::<i16>(0).unwrap();
            writer.write_sample::<i16>(0).unwrap();
        }
        writer.finalize().unwrap();

        let cfg = CtcMeasurementConfig {
            speakers: vec!["L".to_string(), "R".to_string()],
            mics: vec!["left_ear".to_string(), "right_ear".to_string()],
            head_positions: vec![],
            files: vec![CtcMeasurementFileConfig {
                head_position: "primary".to_string(),
                speaker: "L".to_string(),
                ir: Some(wav),
                raw_sweep: None,
                loopback: None,
            }],
        };

        let err =
            load_measured_spectrum(&cfg, &CtcWindowConfig::default(), 48_000, 1024).unwrap_err();
        assert!(err.to_string().contains("speaker='R'"));
    }

    #[test]
    fn measured_ctc_writes_recommended_artifact() {
        let dir = tempdir().unwrap();
        let left_wav = dir.path().join("left.wav");
        let right_wav = dir.path().join("right.wav");
        write_stereo_impulse(&left_wav, 30_000, 6_000);
        write_stereo_impulse(&right_wav, 6_000, 30_000);

        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "measured".to_string(),
            measurements: Some(CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: vec![],
                files: vec![
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "L".to_string(),
                        ir: Some(left_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "R".to_string(),
                        ir: Some(right_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                ],
            }),
            hrtf: None,
            window: CtcWindowConfig::default(),
            regularization: CtcRegularizationConfig {
                beta_db: -60.0,
                beta_lf_db: -60.0,
                beta_hf_db: -60.0,
                max_gain_db: 12.0,
            },
            robustness: "average".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 64,
            reference_sweep: None,
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 8,
        };
        let sys = SystemConfig {
            model: SystemModel::Stereo,
            speakers: HashMap::from([
                ("L".to_string(), "left".to_string()),
                ("R".to_string(), "right".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };

        let report = maybe_generate_recommended_xtc(&cfg, &sys, 48_000.0, dir.path(), None)
            .unwrap()
            .expect("ctc report");
        assert_eq!(report.speakers, vec!["L", "R"]);
        assert!(report.max_electrical_sum_gain_db <= cfg.regularization.max_gain_db + 1e-9);
        assert!(Path::new(&report.artifact).exists());

        let artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&report.artifact).unwrap()).unwrap();
        assert_eq!(artifact["version"], CTC_ARTIFACT_VERSION);
        assert_eq!(artifact["filters"].as_array().unwrap().len(), 4);
        assert!(artifact["mean_crosstalk_residual_db"].is_number());
        assert!(artifact["delivered_response"]["mean_target_error"].is_number());
        assert!(report.delivered_response.is_some());
    }

    #[test]
    fn joint_room_eq_path_folds_channel_gain_into_ctc_solve() {
        let dir = tempdir().unwrap();
        let left_wav = dir.path().join("left.wav");
        let right_wav = dir.path().join("right.wav");
        write_stereo_impulse(&left_wav, 30_000, 6_000);
        write_stereo_impulse(&right_wav, 6_000, 30_000);

        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "measured".to_string(),
            measurements: Some(CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: vec![],
                files: vec![
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "L".to_string(),
                        ir: Some(left_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "R".to_string(),
                        ir: Some(right_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                ],
            }),
            hrtf: None,
            window: CtcWindowConfig::default(),
            regularization: CtcRegularizationConfig {
                beta_db: -60.0,
                beta_lf_db: -60.0,
                beta_hf_db: -60.0,
                max_gain_db: 12.0,
            },
            robustness: "average".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 64,
            reference_sweep: None,
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 8,
        };
        let sys = SystemConfig {
            model: SystemModel::Stereo,
            speakers: HashMap::from([
                ("L".to_string(), "left".to_string()),
                ("R".to_string(), "right".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };
        let channels = HashMap::from([
            (
                "left".to_string(),
                crate::roomeq::output::build_channel_dsp_chain("left", Some(6.0), Vec::new(), &[]),
            ),
            (
                "right".to_string(),
                crate::roomeq::output::build_channel_dsp_chain("right", Some(6.0), Vec::new(), &[]),
            ),
        ]);

        let plain =
            maybe_generate_recommended_xtc(&cfg, &sys, 48_000.0, &dir.path().join("plain"), None)
                .unwrap()
                .expect("plain ctc report");
        let joint = maybe_generate_recommended_xtc(
            &cfg,
            &sys,
            48_000.0,
            &dir.path().join("joint"),
            Some(&channels),
        )
        .unwrap()
        .expect("joint ctc report");

        assert!(joint.room_eq_correction_applied);
        assert_eq!(joint.room_eq_correction_channels, vec!["left", "right"]);
        assert!(
            joint.max_filter_gain_db < plain.max_filter_gain_db - 4.0,
            "joint max gain {} should reflect downstream +6 dB channel gain, plain {}",
            joint.max_filter_gain_db,
            plain.max_filter_gain_db
        );

        let artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&joint.artifact).unwrap()).unwrap();
        assert_eq!(artifact["room_eq_correction_applied"], true);
        assert_eq!(artifact["room_eq_correction_channels"][0], "left");
    }

    #[test]
    fn raw_sweep_ctc_writes_recommended_artifact() {
        let dir = tempdir().unwrap();
        let reference = dir.path().join("reference.wav");
        let loopback = dir.path().join("loopback.wav");
        let left_wav = dir.path().join("left_raw.wav");
        let right_wav = dir.path().join("right_raw.wav");
        write_mono_impulse(&reference, 30_000);
        write_mono_impulse(&loopback, 30_000);
        write_stereo_impulse(&left_wav, 30_000, 6_000);
        write_stereo_impulse(&right_wav, 6_000, 30_000);

        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "raw_sweep".to_string(),
            measurements: Some(CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: vec![],
                files: vec![
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "L".to_string(),
                        ir: None,
                        raw_sweep: Some(left_wav),
                        loopback: Some(loopback.clone()),
                    },
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "R".to_string(),
                        ir: None,
                        raw_sweep: Some(right_wav),
                        loopback: Some(loopback),
                    },
                ],
            }),
            hrtf: None,
            window: CtcWindowConfig::default(),
            regularization: CtcRegularizationConfig {
                beta_db: -60.0,
                beta_lf_db: -60.0,
                beta_hf_db: -60.0,
                max_gain_db: 12.0,
            },
            robustness: "minimax".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 64,
            reference_sweep: Some(reference),
            sweep_duration_s: Some(1.0),
            sweep_start_hz: Some(20.0),
            sweep_end_hz: Some(20_000.0),
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 3,
        };
        let sys = SystemConfig {
            model: SystemModel::Stereo,
            speakers: HashMap::from([
                ("L".to_string(), "left".to_string()),
                ("R".to_string(), "right".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };

        let report = maybe_generate_recommended_xtc(&cfg, &sys, 48_000.0, dir.path(), None)
            .unwrap()
            .expect("ctc report");
        assert_eq!(report.source, "raw_sweep");
        assert!(report.worst_position_error.is_finite());

        let artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&report.artifact).unwrap()).unwrap();
        assert_eq!(artifact["source"], "raw_sweep");
        assert_eq!(artifact["filters"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn raw_sweep_direct_window_tracks_delayed_acoustic_arrival() {
        let dir = tempdir().unwrap();
        let reference = dir.path().join("reference.wav");
        let loopback = dir.path().join("loopback.wav");
        let left_wav = dir.path().join("left_raw_delayed.wav");
        let right_wav = dir.path().join("right_raw_delayed.wav");
        write_mono_impulse(&reference, 30_000);
        write_mono_impulse(&loopback, 30_000);
        write_stereo_delayed_impulse(&left_wav, 48, 30_000, 6_000);
        write_stereo_delayed_impulse(&right_wav, 48, 6_000, 30_000);

        let window = CtcWindowConfig {
            length_ms: 0.5,
            fade_ms: 0.0,
            ..Default::default()
        };

        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "raw_sweep".to_string(),
            measurements: Some(CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: vec![],
                files: vec![
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "L".to_string(),
                        ir: None,
                        raw_sweep: Some(left_wav),
                        loopback: Some(loopback.clone()),
                    },
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "R".to_string(),
                        ir: None,
                        raw_sweep: Some(right_wav),
                        loopback: Some(loopback),
                    },
                ],
            }),
            hrtf: None,
            window,
            regularization: CtcRegularizationConfig {
                beta_db: -60.0,
                beta_lf_db: -60.0,
                beta_hf_db: -60.0,
                max_gain_db: 12.0,
            },
            robustness: "average".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 128,
            reference_sweep: Some(reference),
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 8,
        };
        let sys = SystemConfig {
            model: SystemModel::Stereo,
            speakers: HashMap::from([
                ("L".to_string(), "left".to_string()),
                ("R".to_string(), "right".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };

        let report = maybe_generate_recommended_xtc(&cfg, &sys, 48_000.0, dir.path(), None)
            .unwrap()
            .expect("ctc report");
        let artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&report.artifact).unwrap()).unwrap();
        let max_tap = artifact["filters"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|filter| filter["taps"].as_array().unwrap())
            .map(|tap| tap.as_f64().unwrap().abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_tap > 0.01,
            "delayed direct arrival should not be clipped by a short direct window"
        );
    }

    #[test]
    fn synthetic_measured_ctc_reconstructs_binaural_identity() {
        let dir = tempdir().unwrap();
        let left_wav = dir.path().join("left_matrix.wav");
        let right_wav = dir.path().join("right_matrix.wav");
        write_stereo_split_impulse(&left_wav, 8, 30_000, 13, 7_000);
        write_stereo_split_impulse(&right_wav, 12, 6_500, 9, 30_000);

        let window = CtcWindowConfig {
            length_ms: 1.0,
            fade_ms: 0.0,
            ..Default::default()
        };
        let cfg = CtcConfig {
            enabled: true,
            matrix_source: "measured".to_string(),
            measurements: Some(CtcMeasurementConfig {
                speakers: vec!["L".to_string(), "R".to_string()],
                mics: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: vec![],
                files: vec![
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "L".to_string(),
                        ir: Some(left_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                    CtcMeasurementFileConfig {
                        head_position: "primary".to_string(),
                        speaker: "R".to_string(),
                        ir: Some(right_wav),
                        raw_sweep: None,
                        loopback: None,
                    },
                ],
            }),
            hrtf: None,
            window,
            regularization: CtcRegularizationConfig {
                beta_db: -80.0,
                beta_lf_db: -80.0,
                beta_hf_db: -80.0,
                max_gain_db: 24.0,
            },
            robustness: "average".to_string(),
            include_room_eq_dsp: true,
            fir_taps: 256,
            reference_sweep: None,
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: 5,
            harmonic_suppression_window_ms: 2.0,
            minimax_iterations: 8,
        };
        let sys = SystemConfig {
            model: SystemModel::Stereo,
            speakers: HashMap::from([
                ("L".to_string(), "left".to_string()),
                ("R".to_string(), "right".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };

        let report = maybe_generate_recommended_xtc(&cfg, &sys, 48_000.0, dir.path(), None)
            .unwrap()
            .expect("ctc report");
        assert!(report.mean_reconstruction_error < 0.02);
        assert!(report.worst_position_error < 0.02);
        assert!(report.mean_crosstalk_residual_db < -15.0);
        let delivered = report.delivered_response.as_ref().unwrap();
        assert!(delivered.mean_target_error < 0.05);
        assert!(delivered.worst_target_error < 0.1);
        assert!(delivered.mean_crosstalk_db < -20.0);
        assert!(delivered.worst_crosstalk_db < -15.0);
        assert!(delivered.mean_channel_balance_db < 1.0);

        let artifact: CtcArtifact =
            serde_json::from_slice(&std::fs::read(&report.artifact).unwrap()).unwrap();
        assert!(artifact.delivered_response.is_some());
        let spectrum = load_measured_spectrum(
            cfg.measurements.as_ref().unwrap(),
            &cfg.window,
            48_000,
            cfg.fir_taps,
        )
        .unwrap();
        let f_ll = artifact_filter_spectrum(&artifact, "L", "left_ear");
        let f_lr = artifact_filter_spectrum(&artifact, "L", "right_ear");
        let f_rl = artifact_filter_spectrum(&artifact, "R", "left_ear");
        let f_rr = artifact_filter_spectrum(&artifact, "R", "right_ear");
        let latency = cfg.fir_taps / 2;
        let mut max_cross = 0.0_f64;
        let mut diag_error_sum = 0.0_f64;
        let mut checked = 0usize;

        for bin in 1..(cfg.fir_taps / 2) {
            let h = &spectrum.bins[bin][0].values;
            let y_ll = h[0] * f_ll[bin] + h[1] * f_rl[bin];
            let y_lr = h[0] * f_lr[bin] + h[1] * f_rr[bin];
            let y_rl = h[2] * f_ll[bin] + h[3] * f_rl[bin];
            let y_rr = h[2] * f_lr[bin] + h[3] * f_rr[bin];
            let phase = 2.0 * PI * bin as f64 * latency as f64 / cfg.fir_taps as f64;
            let undo_latency = Complex64::from_polar(1.0, phase);
            diag_error_sum += (y_ll * undo_latency - Complex64::new(1.0, 0.0)).norm();
            diag_error_sum += (y_rr * undo_latency - Complex64::new(1.0, 0.0)).norm();
            max_cross = max_cross.max((y_lr * undo_latency).norm());
            max_cross = max_cross.max((y_rl * undo_latency).norm());
            checked += 2;
        }

        let mean_diag_error = diag_error_sum / checked as f64;
        assert!(mean_diag_error < 0.05, "mean_diag_error={mean_diag_error}");
        assert!(max_cross < 0.08, "max_cross={max_cross}");
    }

    fn write_mono_impulse(path: &Path, value: i16) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        writer.write_sample::<i16>(value).unwrap();
        for _ in 1..64 {
            writer.write_sample::<i16>(0).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn write_mono_float_wav(path: &Path, samples: &[f32]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample::<f32>(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn test_channel_chain(
        plugins: Vec<PluginConfigWrapper>,
        drivers: Option<Vec<crate::roomeq::types::DriverDspChain>>,
    ) -> ChannelDspChain {
        ChannelDspChain {
            channel: "left".to_string(),
            plugins,
            drivers,
            initial_curve: None,
            final_curve: None,
            eq_response: None,
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        }
    }

    fn artifact_filter_spectrum(
        artifact: &CtcArtifact,
        speaker: &str,
        target_ear: &str,
    ) -> Vec<Complex64> {
        let filter = artifact
            .filters
            .iter()
            .find(|filter| filter.speaker == speaker && filter.target_ear == target_ear)
            .unwrap_or_else(|| {
                panic!("missing filter speaker='{speaker}', target_ear='{target_ear}'")
            });
        fft_real_to_half_spectrum_f64(&filter.taps, artifact.fir_taps)
    }

    fn write_stereo_split_impulse(
        path: &Path,
        left_delay: usize,
        left: i16,
        right_delay: usize,
        right: i16,
    ) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for idx in 0..256 {
            writer
                .write_sample::<i16>(if idx == left_delay { left } else { 0 })
                .unwrap();
            writer
                .write_sample::<i16>(if idx == right_delay { right } else { 0 })
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    fn write_stereo_delayed_impulse(path: &Path, delay: usize, left: i16, right: i16) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for idx in 0..128 {
            writer
                .write_sample::<i16>(if idx == delay { left } else { 0 })
                .unwrap();
            writer
                .write_sample::<i16>(if idx == delay { right } else { 0 })
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    fn write_stereo_impulse(path: &Path, left: i16, right: i16) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        writer.write_sample::<i16>(left).unwrap();
        writer.write_sample::<i16>(right).unwrap();
        for _ in 1..64 {
            writer.write_sample::<i16>(0).unwrap();
            writer.write_sample::<i16>(0).unwrap();
        }
        writer.finalize().unwrap();
    }
}
