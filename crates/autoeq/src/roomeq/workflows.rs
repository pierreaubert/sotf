//! Specific optimization workflows for different system topologies.

use crate::error::{AutoeqError, Result};
use crate::read::load_source;
use crate::response;
use crate::Curve;
use log::info;
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use std::collections::HashMap;
use std::path::Path;

use super::eq;
use super::crossover;
use super::output;
use super::optimize::{
    ChannelOptimizationResult, RoomOptimizationResult,
};
use super::types::{
    ChannelDspChain, OptimizationMetadata, RoomConfig, SystemConfig,
    SpeakerConfig,
};

/// Align channel levels by normalizing down to the lowest level.
pub fn align_channels_to_lowest(
    channels: &HashMap<String, Curve>,
    ranges: &HashMap<String, (f64, f64)>,
) -> HashMap<String, f64> {
    let mut means = HashMap::new();
    let mut min_mean = f64::INFINITY;

    for (name, curve) in channels {
        let (min_f, max_f) = ranges.get(name).cloned().unwrap_or((100.0, 2000.0));
        
        let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
        let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
        
        let mean = compute_average_response(
            &freqs_f32, 
            &spl_f32, 
            Some((min_f as f32, max_f as f32))
        ) as f64;
        
        means.insert(name.clone(), mean);
        if mean < min_mean {
            min_mean = mean;
        }
    }

    let mut gains = HashMap::new();
    for (name, mean) in means {
        let diff = min_mean - mean;
        gains.insert(name.clone(), diff);
        info!("  Level alignment for '{}': {:.2} dB (mean {:.2} -> {:.2})", 
              name, diff, mean, min_mean);
    }
    gains
}

/// Compute flat_loss score for a curve within a frequency range.
///
/// Normalizes SPL by subtracting the mean in the given range, then computes
/// the weighted MSE — same metric used in the main optimization path.
fn compute_flat_score(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    let mean = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;
    let normalized_spl = &curve.spl - mean;
    crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq)
}

/// Helper to load curves for all logical channels
fn load_logical_channels(
    config: &RoomConfig, 
    sys: &SystemConfig
) -> Result<HashMap<String, Curve>> {
    let mut curves = HashMap::new();
    for (role, meas_key) in &sys.speakers {
        if let Some(cfg) = config.speakers.get(meas_key) {
            let source = match cfg {
                SpeakerConfig::Single(s) => s,
                _ => return Err(AutoeqError::InvalidConfiguration { 
                    message: format!("Workflow requires Single speaker config for '{}'", role)
                }),
            };
            let curve = load_source(source)
                .map_err(|e| AutoeqError::InvalidMeasurement { message: e.to_string() })?;
            curves.insert(role.clone(), curve);
        }
    }
    Ok(curves)
}

/// Workflow for Stereo 2.0 (No Subwoofer)
pub fn optimize_stereo_2_0(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    info!("Running Stereo 2.0 Optimization Workflow");

    // 1. Load measurements
    let curves = load_logical_channels(config, sys)?;

    // 2. Alignment
    let mut ranges = HashMap::new();
    for role in curves.keys() {
        ranges.insert(role.clone(), (100.0, 2000.0));
    }
    let gains = align_channels_to_lowest(&curves, &ranges);

    // 3. Optimization
    let min_freq = config.optimizer.min_freq;
    let max_freq = config.optimizer.max_freq;
    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for (role, curve) in &curves {
        let gain = *gains.get(role).unwrap_or(&0.0);

        // Apply gain to curve for optimization context
        let mut aligned_curve = curve.clone();
        for s in aligned_curve.spl.iter_mut() {
            *s += gain;
        }

        // Pre-optimization score
        let pre_score = compute_flat_score(&aligned_curve, min_freq, max_freq);

        info!("  Optimizing '{}' with alignment gain {:.2} dB (pre_score={:.4})", role, gain, pre_score);

        let (filters, _loss) = eq::optimize_channel_eq(
            &aligned_curve,
            &config.optimizer,
            config.target_curve.as_ref(),
            sample_rate,
        ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;

        // Build Chain
        let mut plugins = Vec::new();
        if gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(gain));
        }
        if !filters.is_empty() {
            plugins.push(output::create_eq_plugin(&filters));
        }

        // Compute final response
        let resp = response::compute_peq_complex_response(&filters, &aligned_curve.freq, sample_rate);
        let final_curve_obj = response::apply_complex_response(&aligned_curve, &resp);

        // Post-optimization score
        let post_score = compute_flat_score(&final_curve_obj, min_freq, max_freq);

        info!("  '{}' post_score={:.4}", role, post_score);

        let chain = ChannelDspChain {
            channel: role.clone(),
            plugins,
            drivers: None,
            initial_curve: Some((&aligned_curve).into()),
            final_curve: Some((&final_curve_obj).into()),
        };

        channel_chains.insert(role.clone(), chain);
        pre_scores.push(pre_score);
        post_scores.push(post_score);

        channel_results.insert(role.clone(), ChannelOptimizationResult {
            name: role.clone(),
            pre_score,
            post_score,
            initial_curve: curve.clone(),
            final_curve: final_curve_obj,
            biquads: filters,
            fir_coeffs: None,
        });
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!("Average pre-score: {:.4}, post-score: {:.4}", avg_pre, avg_post);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    })
}

/// Workflow for Stereo 2.1 (With Subwoofer)
pub fn optimize_stereo_2_1(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    info!("Running Stereo 2.1 Optimization Workflow");

    let curves = load_logical_channels(config, sys)?;
    
    // Identify channels
    // Assuming keys "L", "R", "LFE" from spec example
    // Or use sys.subwoofers to identify Sub
    let sub_role = "LFE"; // Hardcoded for now based on typical 2.1
    // Verify existence
    if !curves.contains_key("L") || !curves.contains_key("R") || !curves.contains_key(sub_role) {
        return Err(AutoeqError::InvalidConfiguration { 
            message: "Stereo 2.1 workflow requires 'L', 'R', and 'LFE' channels".to_string() 
        });
    }

    // Resolve Crossover from System Config
    let sub_sys = sys.subwoofers.as_ref().ok_or(
        AutoeqError::InvalidConfiguration { message: "Missing subwoofers configuration".to_string() }
    )?;
    
    let xover_key = sub_sys.crossover.as_deref().ok_or(
        AutoeqError::InvalidConfiguration { message: "Subwoofer config requires 'crossover' reference".to_string() }
    )?;
    
    let xover_config = config.crossovers.as_ref()
        .and_then(|m| m.get(xover_key))
        .ok_or(AutoeqError::InvalidConfiguration { 
            message: format!("Crossover '{}' not found in crossovers section", xover_key) 
        })?;
        
    let xover_type_str = &xover_config.crossover_type;

    // Handle fixed frequency vs range
    let (min_xo, max_xo, est_xo) = if let Some(f) = xover_config.frequency {
        (f, f, f)
    } else if let Some((min, max)) = xover_config.frequency_range {
        (min, max, (min * max).sqrt())
    } else {
        return Err(AutoeqError::InvalidConfiguration { 
            message: "Subwoofer crossover requires 'frequency' or 'frequency_range'".to_string() 
        });
    };

    // 1. Level Measurement & Alignment
    // Use max_xo for boundary to ensure we measure Sub fully and Mains safely.
    // For Sub, restrict to octave below crossover to avoid deep bass peaks skewing level.
    let mut ranges = HashMap::new();
    ranges.insert("L".to_string(), (max_xo, 2000.0));
    ranges.insert("R".to_string(), (max_xo, 2000.0));
    let sub_min_align = (max_xo * 0.5).max(20.0);
    ranges.insert(sub_role.to_string(), (sub_min_align, max_xo));

    let gains = align_channels_to_lowest(&curves, &ranges);

    // Apply gains
    let mut aligned_curves = HashMap::new();
    for (role, curve) in &curves {
        let mut c = curve.clone();
        let g = *gains.get(role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() { *s += g; }
        aligned_curves.insert(role.clone(), c);
    }

    // 3. Pre-EQ (Linearization) for L and R
    // Min freq = min_xo to ensure coverage even if crossover drops to min
    let mut pre_eq_filters = HashMap::new();
    let mut linearized_curves = aligned_curves.clone();

    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = min_xo;
        
        info!("  Pre-EQ Linearization for '{}' (min {:.1} Hz)", role, min_xo);
        let (filters, _) = eq::optimize_channel_eq(
            &aligned_curves[role],
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;

        // Apply filters
        let resp = response::compute_peq_complex_response(&filters, &aligned_curves[role].freq, sample_rate);
        let linear = response::apply_complex_response(&aligned_curves[role], &resp);
        
        pre_eq_filters.insert(role.to_string(), filters);
        linearized_curves.insert(role.to_string(), linear);
    }

    // 4. Crossover Optimization
    // Virtual Main = Avg(L, R)
    // We average the LINEARIZED curves
    let l_curve = &linearized_curves["L"];
    let r_curve = &linearized_curves["R"];
    let sub_curve = &linearized_curves[sub_role]; // Sub is not linearized in step 3? Spec says "Optimal EQ for L and R".

    // Average L and R
    // Average magnitude (SPL)
    // Note: geometric average of magnitude? Or average of dB?
    // compute_average_response does average of SPL values (dB).
    // Let's simple average dB for Virtual Main
    let mut virtual_main = l_curve.clone();
    for i in 0..virtual_main.spl.len() {
        virtual_main.spl[i] = (l_curve.spl[i] + r_curve.spl[i]) / 2.0;
        // Phase averaging is tricky. Use L phase? 
        // For crossover optimization, we need phase.
        // Assuming L and R are phase-coherent (level aligned).
        // Let's use L phase.
    }

    // Optimize Crossover between Virtual Main and Sub
    // We reuse crossover::optimize_crossover. It expects a list of drivers.
    // [VirtualMain, Sub]
    
    // We need to parse crossover type for the optimizer
    let crossover_type_enum = crossover::parse_crossover_type(xover_type_str)
        .map_err(|e| AutoeqError::InvalidConfiguration { message: e.to_string() })?;

    // Determine fixed freqs vs range for optimizer
    let (fixed_freqs, range_opt) = if xover_config.frequency.is_some() {
        (Some(vec![est_xo]), None)
    } else {
        (None, Some((min_xo, max_xo)))
    };

    // Optimize
    let (xo_gains, xo_delays, xo_freqs, _, inversions) = crossover::optimize_crossover(
        vec![virtual_main.clone(), sub_curve.clone()],
        crossover_type_enum,
        sample_rate,
        &config.optimizer,
        fixed_freqs,
        range_opt,
    ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;

    // Results: index 0 = Mains, index 1 = Sub
    let main_gain_post = xo_gains[0];
    let _main_delay_post = xo_delays[0];
    let sub_gain_post = xo_gains[1];
    let _sub_delay_post = xo_delays[1];
    let sub_inverted = inversions[1];
    let final_xo_freq = xo_freqs[0];

    info!("  Crossover Optimized: Freq={:.1} Hz, Main Gain={:.2}, Sub Gain={:.2}, Sub Delay={:.2}", 
          final_xo_freq, main_gain_post, sub_gain_post, _sub_delay_post);

    // 6. Apply Crossover (Filters + Gain/Delay)
    // We calculate the post-crossover curves for Post-EQ using FINAL frequency
    
    let hp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, true);

    let apply_chain = |curve: &Curve, filters: &[Biquad], gain: f64, delay: f64, invert: bool| -> Curve {
        let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
        let mut c = response::apply_complex_response(curve, &resp);
        // Apply gain
        for s in c.spl.iter_mut() { *s += gain; }
        // Apply delay/invert (affects phase)
        // ... phase update logic ...
        // For Post-EQ magnitude, phase doesn't matter much unless we do more summing.
        c
    };

    // Note: Applying to ALIGNED curves (not linearized), and ignoring optimized delay per user request.
    let l_post = apply_chain(&aligned_curves["L"], &hp_biquads, main_gain_post, 0.0, false);
    let r_post = apply_chain(&aligned_curves["R"], &hp_biquads, main_gain_post, 0.0, false);
    let sub_post_initial = apply_chain(&aligned_curves[sub_role], &lp_biquads, sub_gain_post, 0.0, sub_inverted);

    // Re-align Subwoofer level after crossover application
    // Calculate mean SPL of filtered curves to ensure levels match at crossover
    let freqs_f32: Vec<f32> = l_post.freq.iter().map(|&f| f as f32).collect();
    let main_spl_f32: Vec<f32> = l_post.spl.iter().map(|&s| s as f32).collect();
    let sub_spl_f32: Vec<f32> = sub_post_initial.spl.iter().map(|&s| s as f32).collect();

    // Mains: measure above crossover
    let main_mean = compute_average_response(&freqs_f32, &main_spl_f32, Some((final_xo_freq as f32, 2000.0))) as f64;
    
    // Sub: measure below crossover (full passband)
    let sub_mean = compute_average_response(&freqs_f32, &sub_spl_f32, Some((20.0, final_xo_freq as f32))) as f64;
    
    let sub_correction = main_mean - sub_mean;
    info!("  Re-aligning Subwoofer: Main={:.2} dB, Sub={:.2} dB, Correction={:+.2} dB", 
          main_mean, sub_mean, sub_correction);
    
    // Apply correction
    let mut sub_post = sub_post_initial.clone();
    for s in sub_post.spl.iter_mut() { *s += sub_correction; }
    
    let sub_gain_post = sub_gain_post + sub_correction;

    // 7. Post-EQ (Global)
    // L/R: min_freq = xover + 20
    // Sub: max_freq = xover - 20
    let mut post_eq_filters = HashMap::new();
    
    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = final_xo_freq + 20.0;
        
        let (filters, _) = eq::optimize_channel_eq(
            &l_post, // Using l_post for L
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;
        post_eq_filters.insert(role.to_string(), filters);
    }
    
    // Sub Post-EQ
    {
        let mut opt_config = config.optimizer.clone();
        opt_config.max_freq = final_xo_freq - 20.0;
        let (filters, _) = eq::optimize_channel_eq(
            &sub_post,
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;
        post_eq_filters.insert(sub_role.to_string(), filters);
    }

    // 8. Construct Output Chains
    let mut channel_chains = HashMap::new();
    
    // L/R Chain: AlignGain -> Crossover(HP) -> MainGain -> PostEQ (No PreEQ, No Delay)
    for role in ["L", "R"] {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 { plugins.push(output::create_gain_plugin(align_gain)); }
        
        // Pre-EQ removed per user request (optimization relies on Post-EQ)
        
        // Crossover HP
        plugins.push(output::create_crossover_plugin(xover_type_str, final_xo_freq, "high"));
        
        // Main Post Gain (Delay removed)
        if main_gain_post.abs() > 0.01 { plugins.push(output::create_gain_plugin(main_gain_post)); }
        
        let eqs = post_eq_filters.get(role);
        if let Some(e) = eqs {
            plugins.push(output::create_eq_plugin(e));
        }
        
        // Compute final curve
        let intermediate = if role == "L" { &l_post } else { &r_post };
        let final_curve_obj = if let Some(e) = eqs {
            let resp = response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
            response::apply_complex_response(intermediate, &resp)
        } else {
            intermediate.clone()
        };

        let chain = ChannelDspChain {
            channel: role.to_string(),
            plugins,
            drivers: None,
            initial_curve: Some((&aligned_curves[role]).into()),
            final_curve: Some((&final_curve_obj).into()),
        };
        channel_chains.insert(role.to_string(), chain);
    }

    // Sub Chain: AlignGain -> Crossover(LP) -> SubGain(Invert) -> PostEQ (No Delay)
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 { sub_plugins.push(output::create_gain_plugin(sub_align_gain)); }
    
    sub_plugins.push(output::create_crossover_plugin(xover_type_str, final_xo_freq, "low"));
    
    // Sub Gain + Invert (Delay removed)
    if sub_inverted || sub_gain_post.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin_with_invert(sub_gain_post, sub_inverted));
    }
    
    let sub_eqs = post_eq_filters.get(sub_role);
    if let Some(e) = sub_eqs {
        sub_plugins.push(output::create_eq_plugin(e));
    }
    
    // Compute final curve
    let final_sub_curve = if let Some(e) = sub_eqs {
        let resp = response::compute_peq_complex_response(e, &sub_post.freq, sample_rate);
        response::apply_complex_response(&sub_post, &resp)
    } else {
        sub_post.clone()
    };
    
    let sub_chain = ChannelDspChain {
        channel: sub_role.to_string(),
        plugins: sub_plugins,
        drivers: None,
        initial_curve: Some((&aligned_curves[sub_role]).into()),
        final_curve: Some((&final_sub_curve).into()),
    };
    channel_chains.insert(sub_role.to_string(), sub_chain);

    // Compute scores per channel
    let min_freq = config.optimizer.min_freq;
    let max_freq = config.optimizer.max_freq;
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in ["L", "R"] {
        let pre_score = compute_flat_score(&aligned_curves[role], min_freq, max_freq);
        let final_curve_obj = if let Some(e) = post_eq_filters.get(role) {
            let intermediate = if role == "L" { &l_post } else { &r_post };
            let resp = response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
            response::apply_complex_response(intermediate, &resp)
        } else if role == "L" {
            l_post.clone()
        } else {
            r_post.clone()
        };
        let post_score = compute_flat_score(&final_curve_obj, min_freq, max_freq);

        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(role.to_string(), ChannelOptimizationResult {
            name: role.to_string(),
            pre_score,
            post_score,
            initial_curve: aligned_curves[role].clone(),
            final_curve: final_curve_obj,
            biquads: post_eq_filters.get(role).cloned().unwrap_or_default(),
            fir_coeffs: None,
        });
    }

    // Sub channel
    {
        let pre_score = compute_flat_score(&aligned_curves[sub_role], min_freq.max(20.0), final_xo_freq);
        let post_score = compute_flat_score(&final_sub_curve, min_freq.max(20.0), final_xo_freq);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(sub_role.to_string(), ChannelOptimizationResult {
            name: sub_role.to_string(),
            pre_score,
            post_score,
            initial_curve: aligned_curves[sub_role].clone(),
            final_curve: final_sub_curve.clone(),
            biquads: post_eq_filters.get(sub_role).cloned().unwrap_or_default(),
            fir_coeffs: None,
        });
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!("Average pre-score: {:.4}, post-score: {:.4}", avg_pre, avg_post);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    })
}

fn create_crossover_filters(type_str: &str, freq: f64, sample_rate: f64, is_lowpass: bool) -> Vec<Biquad> {
    use math_audio_iir_fir::*;
    let type_lower = type_str.to_lowercase();
    let peq = match type_lower.as_str() {
        "lr24" | "lr4" => if is_lowpass { peq_linkwitzriley_lowpass(4, freq, sample_rate) } else { peq_linkwitzriley_highpass(4, freq, sample_rate) },
        "lr48" | "lr8" => if is_lowpass { peq_linkwitzriley_lowpass(8, freq, sample_rate) } else { peq_linkwitzriley_highpass(8, freq, sample_rate) },
        "bw12" | "butterworth12" => if is_lowpass { peq_butterworth_lowpass(2, freq, sample_rate) } else { peq_butterworth_highpass(2, freq, sample_rate) },
        "bw24" | "butterworth24" => if is_lowpass { peq_butterworth_lowpass(4, freq, sample_rate) } else { peq_butterworth_highpass(4, freq, sample_rate) },
        _ => {
            log::warn!("Unknown crossover type '{}', defaulting to LR24", type_str);
            if is_lowpass { peq_linkwitzriley_lowpass(4, freq, sample_rate) } else { peq_linkwitzriley_highpass(4, freq, sample_rate) }
        }
    };
    peq.into_iter().map(|(_, b)| b).collect()
}
