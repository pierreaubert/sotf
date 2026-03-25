//! Output generation for room EQ DSP chains

use super::types::{
    ChannelDspChain, DriverDspChain, DspChainOutput, MixedModeConfig, OptimizationMetadata,
    PluginConfigWrapper,
};
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;

/// Standard display frequency range: 20 Hz
const DISPLAY_MIN_FREQ: f64 = 20.0;
/// Standard display frequency range: 20 kHz
const DISPLAY_MAX_FREQ: f64 = 20000.0;

/// Extend a curve's frequency range to cover 20 Hz – 20 kHz for display.
///
/// Points outside the measurement range are extrapolated with the nearest
/// boundary SPL value. The original measurement data points are preserved.
pub fn extend_curve_to_full_range(curve: &crate::Curve) -> crate::Curve {
    if curve.freq.is_empty() {
        return curve.clone();
    }

    let meas_min = curve.freq[0];
    let meas_max = *curve.freq.last().unwrap();

    // If curve already approximately covers 20 Hz – 20 kHz, return as-is
    if meas_min <= DISPLAY_MIN_FREQ * 1.05 && meas_max >= DISPLAY_MAX_FREQ * 0.95 {
        return curve.clone();
    }

    let first_spl = curve.spl[0];
    let last_spl = *curve.spl.last().unwrap();
    let points_per_decade = 50;

    let mut freq_vec = Vec::new();
    let mut spl_vec = Vec::new();

    // Prepend log-spaced points from 20 Hz to first measurement frequency
    if meas_min > DISPLAY_MIN_FREQ * 1.05 {
        let log_start = DISPLAY_MIN_FREQ.log10();
        let log_end = meas_min.log10();
        let decades = log_end - log_start;
        let n_points = ((decades * points_per_decade as f64).ceil() as usize).max(1);
        for i in 0..n_points {
            let t = i as f64 / n_points as f64;
            let f = 10f64.powf(log_start + t * (log_end - log_start));
            freq_vec.push(f);
            spl_vec.push(first_spl);
        }
    }

    // Copy original data
    freq_vec.extend(curve.freq.iter());
    spl_vec.extend(curve.spl.iter());

    // Append log-spaced points from last measurement frequency to 20 kHz
    if meas_max < DISPLAY_MAX_FREQ * 0.95 {
        let log_start = meas_max.log10();
        let log_end = DISPLAY_MAX_FREQ.log10();
        let decades = log_end - log_start;
        let n_points = ((decades * points_per_decade as f64).ceil() as usize).max(1);
        for i in 1..=n_points {
            let t = i as f64 / n_points as f64;
            let f = 10f64
                .powf(log_start + t * (log_end - log_start))
                .min(DISPLAY_MAX_FREQ);
            freq_vec.push(f);
            spl_vec.push(last_spl);
        }
    }

    crate::Curve {
        freq: Array1::from(freq_vec),
        spl: Array1::from(spl_vec),
        phase: None,
    }
}

/// Convert Biquad filter to JSON configuration
fn biquad_to_json(biquad: &Biquad) -> serde_json::Value {
    json!({
        "filter_type": biquad.filter_type.long_name().to_lowercase(),
        "freq": biquad.freq,
        "q": biquad.q,
        "db_gain": biquad.db_gain,
    })
}

/// Create a gain plugin configuration
pub fn create_gain_plugin(gain_db: f64) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": gain_db
        }),
    }
}

/// Create a gain plugin configuration with polarity inversion
pub fn create_gain_plugin_with_invert(gain_db: f64, invert: bool) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": gain_db,
            "invert": invert
        }),
    }
}

/// Create an EQ plugin configuration from Biquad filters
pub fn create_eq_plugin(filters: &[Biquad]) -> PluginConfigWrapper {
    let filter_configs: Vec<serde_json::Value> = filters.iter().map(biquad_to_json).collect();

    PluginConfigWrapper {
        plugin_type: "eq".to_string(),
        parameters: json!({
            "filters": filter_configs
        }),
    }
}

/// Create a labeled EQ plugin configuration from Biquad filters.
///
/// Adds a `label` field to the parameters JSON to identify which pass
/// of the 3-pass pipeline this EQ belongs to. The audio engine ignores
/// unknown keys, so this is backward-compatible.
pub fn create_labeled_eq_plugin(filters: &[Biquad], label: &str) -> PluginConfigWrapper {
    let filter_configs: Vec<serde_json::Value> = filters.iter().map(biquad_to_json).collect();

    PluginConfigWrapper {
        plugin_type: "eq".to_string(),
        parameters: json!({
            "label": label,
            "filters": filter_configs
        }),
    }
}

/// Create a crossover plugin configuration
pub fn create_crossover_plugin(
    crossover_type: &str,
    frequency: f64,
    output: &str, // "low" or "high"
) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "crossover".to_string(),
        parameters: json!({
            "type": crossover_type,
            "frequency": frequency,
            "output": output
        }),
    }
}

/// Get a descriptive name for a driver based on its index and total count
fn get_driver_name(index: usize, n_drivers: usize) -> String {
    match (n_drivers, index) {
        (2, 0) => "woofer",
        (2, 1) => "tweeter",
        (3, 0) => "woofer",
        (3, 1) => "midrange",
        (3, 2) => "tweeter",
        (4, 0) => "woofer",
        (4, 1) => "lower_midrange",
        (4, 2) => "upper_midrange",
        (4, 3) => "tweeter",
        _ => return format!("driver_{}", index),
    }
    .to_string()
}

/// Build a DSP chain for a single channel
pub fn build_channel_dsp_chain(
    channel_name: &str,
    gain_db: Option<f64>,
    crossovers: Vec<PluginConfigWrapper>,
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    build_channel_dsp_chain_with_curves(channel_name, gain_db, crossovers, eq_filters, None, None)
}

/// Build a DSP chain for a single channel with optional curves
pub fn build_channel_dsp_chain_with_curves(
    channel_name: &str,
    gain_db: Option<f64>,
    crossovers: Vec<PluginConfigWrapper>,
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
) -> ChannelDspChain {
    let mut plugins = Vec::new();

    // Add gain if specified
    if let Some(gain) = gain_db
        && gain.abs() > 0.01
    {
        // Only add if gain is non-zero
        plugins.push(create_gain_plugin(gain));
    }

    // Add crossover filters
    plugins.extend(crossovers);

    // Add EQ
    if !eq_filters.is_empty() {
        plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins,
        drivers: None,
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

/// Create a delay plugin configuration
pub fn create_delay_plugin(delay_ms: f64) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "delay".to_string(),
        parameters: json!({
            "delay_ms": delay_ms
        }),
    }
}

/// Create a convolution plugin configuration
pub fn create_convolution_plugin(wav_path: &str) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "convolution".to_string(),
        parameters: json!({
            "ir_file": wav_path
        }),
    }
}

/// Build a DSP chain for a multi-driver speaker with active crossover
///
/// # Arguments
/// * `channel_name` - Channel name (e.g., "left")
/// * `gains` - Per-driver gains in dB (one per driver)
/// * `delays` - Per-driver delays in ms (one per driver)
/// * `inverts` - Per-driver polarity inversion (optional, one per driver)
/// * `crossover_freqs` - Crossover frequencies in Hz (n_drivers - 1 values)
/// * `crossover_type` - Crossover type string (e.g., "LR24", "Butterworth12")
/// * `eq_filters` - EQ filters for the combined response
/// * `driver_eqs` - Optional per-driver EQ filters (linearization)
///
/// # Returns
/// * ChannelDspChain with per-driver chains and combined EQ
#[allow(clippy::too_many_arguments)]
pub fn build_multidriver_dsp_chain(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    inverts: Option<&[bool]>,
    crossover_freqs: &[f64],
    crossover_type: &str,
    eq_filters: &[Biquad],
    driver_eqs: Option<&[Vec<Biquad>]>,
) -> ChannelDspChain {
    build_multidriver_dsp_chain_with_curves(
        channel_name,
        gains,
        delays,
        inverts,
        crossover_freqs,
        crossover_type,
        eq_filters,
        driver_eqs,
        None,
        None,
        None,
    )
}

/// Build a DSP chain for a multi-driver speaker with curves
///
/// # Arguments
/// * `driver_initial_curves` - Optional per-driver initial curves (extended to full range).
///   When provided, each `DriverDspChain` gets its `initial_curve` populated.
#[allow(clippy::too_many_arguments)]
pub fn build_multidriver_dsp_chain_with_curves(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    inverts: Option<&[bool]>,
    crossover_freqs: &[f64],
    crossover_type: &str,
    eq_filters: &[Biquad],
    driver_eqs: Option<&[Vec<Biquad>]>,
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
    driver_initial_curves: Option<&[crate::Curve]>,
) -> ChannelDspChain {
    let n_drivers = gains.len();

    // Build per-driver chains
    let mut driver_chains = Vec::new();

    for i in 0..n_drivers {
        let mut driver_plugins = Vec::new();

        let invert = inverts.and_then(|inv| inv.get(i)).copied().unwrap_or(false);

        // Add gain plugin if non-zero OR if inverted
        if invert || gains[i].abs() > 0.01 {
            if invert {
                driver_plugins.push(create_gain_plugin_with_invert(gains[i], true));
            } else {
                driver_plugins.push(create_gain_plugin(gains[i]));
            }
        }

        // Add delay plugin if non-zero
        if i < delays.len() && delays[i].abs() > 0.001 {
            driver_plugins.push(create_delay_plugin(delays[i]));
        }

        // Add per-driver EQ (linearization) if provided
        if let Some(eqs) = driver_eqs
            && let Some(filters) = eqs.get(i)
            && !filters.is_empty()
        {
            driver_plugins.push(create_eq_plugin(filters));
        }

        // Add highpass crossover from previous driver (if not first driver)
        if i > 0 {
            let crossover_freq = crossover_freqs[i - 1];
            driver_plugins.push(create_crossover_plugin(
                crossover_type,
                crossover_freq,
                "high",
            ));
        }

        // Add lowpass crossover to next driver (if not last driver)
        if i < n_drivers - 1 {
            let crossover_freq = crossover_freqs[i];
            driver_plugins.push(create_crossover_plugin(
                crossover_type,
                crossover_freq,
                "low",
            ));
        }

        let driver_curve = driver_initial_curves
            .and_then(|curves| curves.get(i))
            .map(|c| c.into());

        driver_chains.push(DriverDspChain {
            name: get_driver_name(i, n_drivers),
            index: i,
            plugins: driver_plugins,
            initial_curve: driver_curve,
        });
    }

    // Build combined EQ (applied to summed output)
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

/// Build a DSP chain for a multi-subwoofer system
///
/// # Arguments
/// * `channel_name` - Channel name (e.g., "lfe")
/// * `group_name` - Name of the sub group
/// * `n_subs` - Number of subwoofers
/// * `gains` - Per-sub gains in dB
/// * `delays` - Per-sub delays in ms
/// * `eq_filters` - Global EQ filters
pub fn build_multisub_dsp_chain(
    channel_name: &str,
    group_name: &str,
    n_subs: usize,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    build_multisub_dsp_chain_with_curves(
        channel_name,
        group_name,
        n_subs,
        gains,
        delays,
        eq_filters,
        None,
        None,
        None,
    )
}

/// Build a DSP chain for a multi-subwoofer system with curves
///
/// # Arguments
/// * `driver_initial_curves` - Optional per-sub initial curves (extended to full range).
pub fn build_multisub_dsp_chain_with_curves(
    channel_name: &str,
    group_name: &str,
    n_subs: usize,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
    driver_initial_curves: Option<&[crate::Curve]>,
) -> ChannelDspChain {
    build_multisub_dsp_chain_with_allpass(
        channel_name,
        group_name,
        n_subs,
        gains,
        delays,
        eq_filters,
        initial_curve,
        final_curve,
        driver_initial_curves,
        None,
        48000.0, // unused when allpass_filters is None
    )
}

/// Build a DSP chain for multi-sub optimization with optional per-sub all-pass filters.
pub fn build_multisub_dsp_chain_with_allpass(
    channel_name: &str,
    group_name: &str,
    n_subs: usize,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
    driver_initial_curves: Option<&[crate::Curve]>,
    allpass_filters: Option<&[(f64, f64)]>,
    sample_rate: f64,
) -> ChannelDspChain {
    // Build per-sub chains
    let mut driver_chains = Vec::new();

    for i in 0..n_subs {
        let mut sub_plugins = Vec::new();

        // Add gain plugin if non-zero
        if i < gains.len() && gains[i].abs() > 0.01 {
            sub_plugins.push(create_gain_plugin(gains[i]));
        }

        // Add delay plugin if non-zero
        if i < delays.len() && delays[i].abs() > 0.001 {
            sub_plugins.push(create_delay_plugin(delays[i]));
        }

        // Add all-pass filter if configured
        if let Some(ap_filters) = allpass_filters
            && let Some(&(freq, q)) = ap_filters.get(i)
        {
            let ap_biquad = Biquad::new(
                math_audio_iir_fir::BiquadFilterType::AllPass,
                freq,
                sample_rate,
                q,
                0.0,
            );
            sub_plugins.push(create_eq_plugin(&[ap_biquad]));
        }

        let driver_curve = driver_initial_curves
            .and_then(|curves| curves.get(i))
            .map(|c| c.into());

        driver_chains.push(DriverDspChain {
            name: format!("{}_{}", group_name, i + 1),
            index: i,
            plugins: sub_plugins,
            initial_curve: driver_curve,
        });
    }

    // Build combined EQ
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

/// Build a DSP chain for a DBA system
pub fn build_dba_dsp_chain(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    build_dba_dsp_chain_with_curves(channel_name, gains, delays, eq_filters, None, None, None)
}

/// Build a DSP chain for a DBA system with curves
///
/// # Arguments
/// * `driver_initial_curves` - Optional per-array initial curves (extended to full range).
///   Index 0 = Front Array, Index 1 = Rear Array.
pub fn build_dba_dsp_chain_with_curves(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
    driver_initial_curves: Option<&[crate::Curve]>,
) -> ChannelDspChain {
    // 2 "drivers": Front and Rear
    let mut driver_chains = Vec::new();

    // Front (Index 0)
    let mut front_plugins = Vec::new();
    if gains[0].abs() > 0.01 {
        front_plugins.push(create_gain_plugin(gains[0]));
    }
    if delays[0].abs() > 0.001 {
        front_plugins.push(create_delay_plugin(delays[0]));
    }
    let front_curve = driver_initial_curves
        .and_then(|curves| curves.first())
        .map(|c| c.into());
    driver_chains.push(DriverDspChain {
        name: "Front Array".to_string(),
        index: 0,
        plugins: front_plugins,
        initial_curve: front_curve,
    });

    // Rear (Index 1) - Inverted
    let mut rear_plugins = Vec::new();
    // Always add gain plugin to handle inversion even if gain is 0
    rear_plugins.push(create_gain_plugin_with_invert(gains[1], true));

    if delays[1].abs() > 0.001 {
        rear_plugins.push(create_delay_plugin(delays[1]));
    }
    let rear_curve = driver_initial_curves
        .and_then(|curves| curves.get(1))
        .map(|c| c.into());
    driver_chains.push(DriverDspChain {
        name: "Rear Array".to_string(),
        index: 1,
        plugins: rear_plugins,
        initial_curve: rear_curve,
    });

    // Combined EQ
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

/// Build a DSP chain for a Gradient Cardioid subwoofer system with curves
///
/// # Arguments
/// * `driver_initial_curves` - Optional per-sub initial curves (extended to full range).
///   Index 0 = Front, Index 1 = Rear.
pub fn build_cardioid_dsp_chain_with_curves(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
    driver_initial_curves: Option<&[crate::Curve]>,
) -> ChannelDspChain {
    // 2 "drivers": Front and Rear
    let mut driver_chains = Vec::new();

    // Front (Index 0) - Primary
    let mut front_plugins = Vec::new();
    if gains[0].abs() > 0.01 {
        front_plugins.push(create_gain_plugin(gains[0]));
    }
    if delays[0].abs() > 0.001 {
        front_plugins.push(create_delay_plugin(delays[0]));
    }
    let front_curve = driver_initial_curves
        .and_then(|curves| curves.first())
        .map(|c| c.into());
    driver_chains.push(DriverDspChain {
        name: "Front Sub".to_string(),
        index: 0,
        plugins: front_plugins,
        initial_curve: front_curve,
    });

    // Rear (Index 1) - Cancellation (Inverted + Delayed)
    let mut rear_plugins = Vec::new();

    // Always add gain plugin to handle inversion
    rear_plugins.push(create_gain_plugin_with_invert(gains[1], true));

    if delays[1].abs() > 0.001 {
        rear_plugins.push(create_delay_plugin(delays[1]));
    }
    let rear_curve = driver_initial_curves
        .and_then(|curves| curves.get(1))
        .map(|c| c.into());
    driver_chains.push(DriverDspChain {
        name: "Rear Sub".to_string(),
        index: 1,
        plugins: rear_plugins,
        initial_curve: rear_curve,
    });

    // Combined EQ
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

/// Create complete DSP chain output
pub fn create_dsp_chain_output(
    channels: HashMap<String, ChannelDspChain>,
    metadata: Option<OptimizationMetadata>,
) -> DspChainOutput {
    DspChainOutput {
        version: super::types::default_config_version(),
        channels,
        metadata,
    }
}

/// Compute the EQ filter response curve from initial and final curves.
///
/// Returns a `CurveData` whose SPL values are `final - initial` (the correction in dB).
pub fn compute_eq_response(
    initial: &super::types::CurveData,
    final_curve: &super::types::CurveData,
) -> super::types::CurveData {
    let spl: Vec<f64> = final_curve
        .spl
        .iter()
        .zip(initial.spl.iter())
        .map(|(&f, &i)| f - i)
        .collect();
    super::types::CurveData {
        freq: initial.freq.clone(),
        spl,
        phase: None,
        norm_range: None,
    }
}

/// Save DSP chain to JSON file
pub fn save_dsp_chain(
    output: &DspChainOutput,
    path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(output)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Add a delay plugin to an existing chain
pub fn add_delay_plugin(chain: &mut ChannelDspChain, delay_ms: f64) {
    let plugin = create_delay_plugin(delay_ms);
    // Insert at the beginning to ensure it applies before other processing (though usually commutative with linear filters)
    chain.plugins.insert(0, plugin);
}

/// Create a band split plugin configuration
pub fn create_band_split_plugin(frequency: f64, crossover_type: &str) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "band_split".to_string(),
        parameters: json!({
            "frequency": frequency,
            "type": crossover_type
        }),
    }
}

/// Create a band merge plugin configuration
pub fn create_band_merge_plugin(bands: usize) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "band_merge".to_string(),
        parameters: json!({
            "bands": bands
        }),
    }
}

/// Build a DSP chain for frequency-based mixed mode crossover
///
/// This creates a chain that:
/// 1. Splits the signal into low and high frequency bands
/// 2. Applies FIR (convolution) to one band
/// 3. Applies IIR (EQ) to the other band
/// 4. Merges the bands back together
///
/// # Arguments
/// * `channel_name` - Channel name (e.g., "left")
/// * `mixed_config` - Mixed mode configuration with crossover settings
/// * `eq_filters` - IIR EQ filters for the IIR band
/// * `fir_wav_path` - Path to the FIR impulse response WAV file
/// * `fir_uses_low` - If true, FIR is applied to low band, IIR to high band
/// * `initial_curve` - Optional initial frequency response curve
pub fn build_mixed_mode_crossover_chain(
    channel_name: &str,
    mixed_config: &MixedModeConfig,
    eq_filters: &[Biquad],
    fir_wav_path: &str,
    fir_uses_low: bool,
    initial_curve: Option<&crate::Curve>,
) -> ChannelDspChain {
    let mut plugins = Vec::new();

    // 1. Split into low and high bands
    plugins.push(create_band_split_plugin(
        mixed_config.crossover_freq,
        &mixed_config.crossover_type,
    ));

    // 2. Apply FIR to designated band (via convolution)
    // After band_split, channels are: [low_L, low_R, high_L, high_R]
    // We need to specify which channels the convolution should process
    let fir_plugin = PluginConfigWrapper {
        plugin_type: "convolution".to_string(),
        parameters: json!({
            "ir_file": fir_wav_path,
            "channels": if fir_uses_low { [0, 1] } else { [2, 3] }
        }),
    };
    plugins.push(fir_plugin);

    // 3. Apply IIR EQ to the other band
    if !eq_filters.is_empty() {
        let filter_configs: Vec<serde_json::Value> = eq_filters
            .iter()
            .map(|biquad| {
                json!({
                    "filter_type": biquad.filter_type.long_name().to_lowercase(),
                    "freq": biquad.freq,
                    "q": biquad.q,
                    "db_gain": biquad.db_gain,
                })
            })
            .collect();

        let eq_plugin = PluginConfigWrapper {
            plugin_type: "eq".to_string(),
            parameters: json!({
                "filters": filter_configs,
                "channels": if fir_uses_low { [2, 3] } else { [0, 1] }
            }),
        };
        plugins.push(eq_plugin);
    }

    // 4. Merge bands back together
    plugins.push(create_band_merge_plugin(2));

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins,
        drivers: None,
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: None, // Will be set by caller after computing response
        eq_response: None,
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;

    #[test]
    fn test_create_gain_plugin() {
        let plugin = create_gain_plugin(-3.5);
        assert_eq!(plugin.plugin_type, "gain");
        assert_eq!(
            plugin.parameters.get("gain_db").unwrap().as_f64().unwrap(),
            -3.5
        );
    }

    #[test]
    fn test_create_gain_plugin_with_invert() {
        let plugin = create_gain_plugin_with_invert(-2.0, true);
        assert_eq!(plugin.plugin_type, "gain");
        assert_eq!(
            plugin.parameters.get("gain_db").unwrap().as_f64().unwrap(),
            -2.0
        );
        assert!(plugin.parameters.get("invert").unwrap().as_bool().unwrap());

        let plugin_no_invert = create_gain_plugin_with_invert(1.5, false);
        assert!(
            !plugin_no_invert
                .parameters
                .get("invert")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_create_eq_plugin() {
        let sample_rate = 48000.0;
        let filters = vec![
            Biquad::new(BiquadFilterType::Peak, 1000.0, sample_rate, 2.0, -3.0),
            Biquad::new(BiquadFilterType::Peak, 4000.0, sample_rate, 1.5, 2.0),
        ];

        let plugin = create_eq_plugin(&filters);
        assert_eq!(plugin.plugin_type, "eq");

        let filters_arr = plugin
            .parameters
            .get("filters")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(filters_arr.len(), 2);

        let first_filter = &filters_arr[0];
        assert_eq!(first_filter.get("freq").unwrap().as_f64().unwrap(), 1000.0);
        assert_eq!(first_filter.get("q").unwrap().as_f64().unwrap(), 2.0);
        assert_eq!(first_filter.get("db_gain").unwrap().as_f64().unwrap(), -3.0);
    }

    #[test]
    fn test_create_crossover_plugin() {
        let plugin = create_crossover_plugin("LR24", 2500.0, "low");
        assert_eq!(plugin.plugin_type, "crossover");
        assert_eq!(
            plugin.parameters.get("type").unwrap().as_str().unwrap(),
            "LR24"
        );
        assert_eq!(
            plugin
                .parameters
                .get("frequency")
                .unwrap()
                .as_f64()
                .unwrap(),
            2500.0
        );
        assert_eq!(
            plugin.parameters.get("output").unwrap().as_str().unwrap(),
            "low"
        );
    }

    #[test]
    fn test_create_delay_plugin() {
        let plugin = create_delay_plugin(15.5);
        assert_eq!(plugin.plugin_type, "delay");
        assert_eq!(
            plugin.parameters.get("delay_ms").unwrap().as_f64().unwrap(),
            15.5
        );
    }

    #[test]
    fn test_create_convolution_plugin() {
        let plugin = create_convolution_plugin("left_fir.wav");
        assert_eq!(plugin.plugin_type, "convolution");
        assert_eq!(
            plugin.parameters.get("ir_file").unwrap().as_str().unwrap(),
            "left_fir.wav"
        );
    }

    #[test]
    fn test_build_channel_dsp_chain_with_gain_and_eq() {
        let sample_rate = 48000.0;
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            sample_rate,
            2.0,
            -3.0,
        )];

        let chain = build_channel_dsp_chain("left", Some(-2.5), Vec::new(), &filters);

        assert_eq!(chain.channel, "left");
        assert_eq!(chain.plugins.len(), 2); // gain + eq
        assert_eq!(chain.plugins[0].plugin_type, "gain");
        assert_eq!(chain.plugins[1].plugin_type, "eq");
        assert!(chain.drivers.is_none());
    }

    #[test]
    fn test_build_channel_dsp_chain_zero_gain_not_added() {
        // Gain of 0.0 should not add gain plugin
        let chain = build_channel_dsp_chain("test", Some(0.0), Vec::new(), &[]);
        assert!(!chain.plugins.iter().any(|p| p.plugin_type == "gain"));
    }

    #[test]
    fn test_build_channel_dsp_chain_small_gain_not_added() {
        // Gain < 0.01 should not be added
        let chain = build_channel_dsp_chain("test", Some(0.005), Vec::new(), &[]);
        assert!(!chain.plugins.iter().any(|p| p.plugin_type == "gain"));
    }

    #[test]
    fn test_build_multidriver_dsp_chain_2way() {
        let gains = vec![-3.0, 0.0];
        let delays = vec![2.5, 0.0];
        let crossover_freqs = vec![2500.0];

        let chain = build_multidriver_dsp_chain(
            "left",
            &gains,
            &delays,
            None,
            &crossover_freqs,
            "LR24",
            &[],
            None,
        );

        assert_eq!(chain.channel, "left");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 2);

        // Verify woofer (index 0)
        let woofer = &drivers[0];
        assert_eq!(woofer.name, "woofer");
        assert_eq!(woofer.index, 0);
        // Woofer should have: gain, delay, lowpass crossover
        assert!(woofer.plugins.iter().any(|p| p.plugin_type == "gain"));
        assert!(woofer.plugins.iter().any(|p| p.plugin_type == "delay"));
        assert!(woofer.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "low"
        }));

        // Verify tweeter (index 1)
        let tweeter = &drivers[1];
        assert_eq!(tweeter.name, "tweeter");
        assert_eq!(tweeter.index, 1);
        // Tweeter should have highpass crossover (no gain since it's 0)
        assert!(tweeter.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "high"
        }));
    }

    #[test]
    fn test_build_multidriver_dsp_chain_3way() {
        let gains = vec![0.0, -2.0, 1.0];
        let delays = vec![0.0, 1.0, 2.0];
        let crossover_freqs = vec![500.0, 3000.0];

        let chain = build_multidriver_dsp_chain(
            "center",
            &gains,
            &delays,
            None,
            &crossover_freqs,
            "LR24",
            &[],
            None,
        );

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 3);

        assert_eq!(drivers[0].name, "woofer");
        assert_eq!(drivers[1].name, "midrange");
        assert_eq!(drivers[2].name, "tweeter");

        // Midrange should have both highpass (from woofer) and lowpass (to tweeter)
        let midrange = &drivers[1];
        let has_highpass = midrange.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "high"
        });
        let has_lowpass = midrange.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "low"
        });
        assert!(has_highpass, "Midrange should have highpass crossover");
        assert!(has_lowpass, "Midrange should have lowpass crossover");
    }

    #[test]
    fn test_build_multisub_dsp_chain() {
        let gains = vec![-2.0, 0.0, 1.0];
        let delays = vec![0.0, 5.0, 10.0];

        let chain = build_multisub_dsp_chain("lfe", "subs", 3, &gains, &delays, &[]);

        assert_eq!(chain.channel, "lfe");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 3);

        assert_eq!(drivers[0].name, "subs_1");
        assert_eq!(drivers[1].name, "subs_2");
        assert_eq!(drivers[2].name, "subs_3");

        // Sub 1 should have delay (5ms)
        assert!(drivers[1].plugins.iter().any(|p| p.plugin_type == "delay"));
    }

    #[test]
    fn test_build_dba_dsp_chain() {
        let gains = vec![0.0, -3.0];
        let delays = vec![0.0, 5.0];

        let chain = build_dba_dsp_chain("dba", &gains, &delays, &[]);

        assert_eq!(chain.channel, "dba");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 2);

        // Front array
        let front = &drivers[0];
        assert_eq!(front.name, "Front Array");
        assert_eq!(front.index, 0);

        // Rear array should have invert flag
        let rear = &drivers[1];
        assert_eq!(rear.name, "Rear Array");
        assert_eq!(rear.index, 1);

        let rear_gain = rear
            .plugins
            .iter()
            .find(|p| p.plugin_type == "gain")
            .expect("Rear should have gain plugin");
        assert!(
            rear_gain
                .parameters
                .get("invert")
                .unwrap()
                .as_bool()
                .unwrap(),
            "Rear should be inverted"
        );

        // Rear should have delay
        assert!(rear.plugins.iter().any(|p| p.plugin_type == "delay"));
    }

    #[test]
    fn test_add_delay_plugin() {
        let mut chain = ChannelDspChain {
            channel: "test".to_string(),
            plugins: vec![create_gain_plugin(-3.0)],
            drivers: None,
            initial_curve: None,
            final_curve: None,
            eq_response: None,
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        };

        add_delay_plugin(&mut chain, 10.0);

        // Delay should be inserted at the beginning
        assert_eq!(chain.plugins.len(), 2);
        assert_eq!(chain.plugins[0].plugin_type, "delay");
        assert_eq!(chain.plugins[1].plugin_type, "gain");
    }

    #[test]
    fn test_create_dsp_chain_output() {
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            build_channel_dsp_chain("left", Some(-2.0), Vec::new(), &[]),
        );

        let metadata = OptimizationMetadata {
            pre_score: 5.0,
            post_score: 2.0,
            algorithm: "cobyla".to_string(),
            iterations: 1000,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let output = create_dsp_chain_output(channels, Some(metadata));

        assert!(output.channels.contains_key("left"));
        assert!(output.metadata.is_some());

        let meta = output.metadata.unwrap();
        assert_eq!(meta.pre_score, 5.0);
        assert_eq!(meta.post_score, 2.0);
    }

    #[test]
    fn test_get_driver_name() {
        // 2-way
        assert_eq!(get_driver_name(0, 2), "woofer");
        assert_eq!(get_driver_name(1, 2), "tweeter");

        // 3-way
        assert_eq!(get_driver_name(0, 3), "woofer");
        assert_eq!(get_driver_name(1, 3), "midrange");
        assert_eq!(get_driver_name(2, 3), "tweeter");

        // 4-way
        assert_eq!(get_driver_name(0, 4), "woofer");
        assert_eq!(get_driver_name(1, 4), "lower_midrange");
        assert_eq!(get_driver_name(2, 4), "upper_midrange");
        assert_eq!(get_driver_name(3, 4), "tweeter");

        // Fallback
        assert_eq!(get_driver_name(5, 8), "driver_5");
    }

    #[test]
    fn test_extend_curve_to_full_range_already_full() {
        // Curve already covers 20 Hz – 20 kHz → returned as-is
        let curve = crate::Curve {
            freq: Array1::from(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]),
            spl: Array1::from(vec![0.0, 1.0, 2.0, 1.0, 0.0]),
            phase: None,
        };
        let extended = extend_curve_to_full_range(&curve);
        assert_eq!(extended.freq.len(), curve.freq.len());
    }

    #[test]
    fn test_extend_curve_to_full_range_narrow() {
        // Curve only covers 100 Hz – 500 Hz → extended to 20 Hz – 20 kHz
        let curve = crate::Curve {
            freq: Array1::from(vec![100.0, 200.0, 300.0, 400.0, 500.0]),
            spl: Array1::from(vec![-5.0, -3.0, 0.0, -2.0, -4.0]),
            phase: None,
        };
        let extended = extend_curve_to_full_range(&curve);

        // Should have more points than the original
        assert!(extended.freq.len() > curve.freq.len());

        // First frequency should be ~20 Hz
        assert!(extended.freq[0] < 25.0);
        assert!(extended.freq[0] >= 20.0);

        // Last frequency should be ~20 kHz
        let last = *extended.freq.last().unwrap();
        assert!(last > 19000.0);
        assert!(last <= 20000.0);

        // SPL at extended low end should equal first measurement value
        assert_eq!(extended.spl[0], -5.0);

        // SPL at extended high end should equal last measurement value
        assert_eq!(*extended.spl.last().unwrap(), -4.0);

        // Original data points should be preserved in the middle
        let orig_start = extended.freq.iter().position(|&f| f == 100.0).unwrap();
        assert_eq!(extended.spl[orig_start], -5.0);
    }

    #[test]
    fn test_extend_curve_to_full_range_empty() {
        let curve = crate::Curve {
            freq: Array1::from(vec![]),
            spl: Array1::from(vec![]),
            phase: None,
        };
        let extended = extend_curve_to_full_range(&curve);
        assert!(extended.freq.is_empty());
    }

    #[test]
    fn test_multisub_allpass_chain_has_eq_plugin_per_sub() {
        let chain = build_multisub_dsp_chain_with_allpass(
            "LFE",
            "subs",
            2,
            &[0.0, -3.0],
            &[0.0, 2.0],
            &[],
            None,
            None,
            None,
            Some(&[(60.0, 1.5), (80.0, 2.0)]),
            96000.0,
        );

        // Each sub should have an EQ plugin (the all-pass filter)
        let drivers = chain.drivers.unwrap();
        assert_eq!(drivers.len(), 2);

        // Sub 0: gain=0 (skipped), delay=0 (skipped), allpass → 1 plugin
        assert_eq!(
            drivers[0].plugins.len(),
            1,
            "Sub 0 should have 1 plugin (allpass), got {}",
            drivers[0].plugins.len()
        );
        assert_eq!(drivers[0].plugins[0].plugin_type, "eq");

        // Sub 1: gain=-3 (added), delay=2 (added), allpass → 3 plugins
        assert_eq!(
            drivers[1].plugins.len(),
            3,
            "Sub 1 should have 3 plugins (gain+delay+allpass), got {}",
            drivers[1].plugins.len()
        );
    }
}
