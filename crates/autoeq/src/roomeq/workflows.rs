//! Specific optimization workflows for different system topologies.

use crate::error::{AutoeqError, Result};
use crate::read::load_source;
use crate::response;
use crate::Curve;
use log::info;
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
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
    SpeakerConfig, PluginConfigWrapper,
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
    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();

    for (role, curve) in &curves {
        let gain = *gains.get(role).unwrap_or(&0.0);
        
        // Apply gain to curve for optimization context
        let mut aligned_curve = curve.clone();
        for s in aligned_curve.spl.iter_mut() {
            *s += gain;
        }

        info!("  Optimizing '{}' with alignment gain {:.2} dB", role, gain);

        let (filters, score) = eq::optimize_channel_eq(
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

        let chain = ChannelDspChain {
            channel: role.clone(),
            plugins,
            drivers: None,
            initial_curve: Some((&aligned_curve).into()), 
            final_curve: None, // Simplified
        };

        channel_chains.insert(role.clone(), chain);
        
        channel_results.insert(role.clone(), ChannelOptimizationResult {
            name: role.clone(),
            pre_score: 0.0, // TODO
            post_score: score,
            initial_curve: curve.clone(),
            final_curve: aligned_curve, // Simplified
            biquads: filters,
            fir_coeffs: None,
        });
    }

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: 0.0,
        combined_post_score: 0.0,
        metadata: OptimizationMetadata {
            pre_score: 0.0,
            post_score: 0.0,
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

    let bass_mgr = config.bass_management.as_ref().ok_or(
        AutoeqError::InvalidConfiguration { message: "Bass management config required".to_string() }
    )?;
    let xover_freq = bass_mgr.crossover_freq;

    // 1. Level Measurement & Alignment
    let mut ranges = HashMap::new();
    ranges.insert("L".to_string(), (xover_freq, 2000.0));
    ranges.insert("R".to_string(), (xover_freq, 2000.0));
    ranges.insert(sub_role.to_string(), (20.0, xover_freq));

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
    // Min freq = xover_freq
    let mut pre_eq_filters = HashMap::new();
    let mut linearized_curves = aligned_curves.clone();

    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = xover_freq;
        
        info!("  Pre-EQ Linearization for '{}'", role);
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
    // And it expects `CrossoverConfig`. We need to synthesize one.
    use super::types::CrossoverConfig;
    let xover_config = CrossoverConfig {
        crossover_type: "LR24".to_string(), // Default or from config?
        frequency: Some(xover_freq),
        frequencies: None,
        frequency_range: None, 
    };
    
    // We need to parse crossover type for the optimizer
    let crossover_type_enum = crossover::parse_crossover_type(&xover_config.crossover_type)
        .map_err(|e| AutoeqError::InvalidConfiguration { message: e.to_string() })?;

    // Optimize
    let (xo_gains, xo_delays, _, _, inversions) = crossover::optimize_crossover(
        vec![virtual_main.clone(), sub_curve.clone()],
        crossover_type_enum,
        sample_rate,
        &config.optimizer,
        Some(vec![xover_freq]), // Fixed frequency
        None,
    ).map_err(|e| AutoeqError::OptimizationFailed { message: e.to_string() })?;

    // Results: index 0 = Mains, index 1 = Sub
    let main_gain_post = xo_gains[0];
    let main_delay_post = xo_delays[0];
    let sub_gain_post = xo_gains[1];
    let sub_delay_post = xo_delays[1];
    let sub_inverted = inversions[1];

    info!("  Crossover Optimized: Main Gain={:.2}, Sub Gain={:.2}, Sub Delay={:.2}", 
          main_gain_post, sub_gain_post, sub_delay_post);

    // 6. Apply Crossover (Filters + Gain/Delay)
    // We calculate the post-crossover curves for Post-EQ
    // Main HighPass, Sub LowPass
    // LR24 HP/LP
    
    // We need helper to apply LR24 filter response to curve
    // We can use Biquad filters.
    // LR24 = 2x Butterworth 2nd Order.
    let hp_filters = math_audio_iir_fir::peq_linkwitzriley_highpass(4, xover_freq, sample_rate);
    let lp_filters = math_audio_iir_fir::peq_linkwitzriley_lowpass(4, xover_freq, sample_rate);
    
    // Unwrap Peq (gain, biquad) -> biquads
    let hp_biquads: Vec<Biquad> = hp_filters.into_iter().map(|(_, b)| b).collect();
    let lp_biquads: Vec<Biquad> = lp_filters.into_iter().map(|(_, b)| b).collect();

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

    let l_post = apply_chain(&linearized_curves["L"], &hp_biquads, main_gain_post, main_delay_post, false);
    let r_post = apply_chain(&linearized_curves["R"], &hp_biquads, main_gain_post, main_delay_post, false);
    let sub_post = apply_chain(&linearized_curves[sub_role], &lp_biquads, sub_gain_post, sub_delay_post, sub_inverted);

    // 7. Post-EQ (Global)
    // L/R: min_freq = xover + 20
    // Sub: max_freq = xover - 20
    let mut post_eq_filters = HashMap::new();
    
    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = xover_freq + 20.0;
        
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
        opt_config.max_freq = xover_freq - 20.0;
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
    
    // L Chain: AlignGain -> PreEQ -> Crossover(HP) -> MainGain/Delay -> PostEQ
    let build_main_chain = |role: &str| -> ChannelDspChain {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 { plugins.push(output::create_gain_plugin(align_gain)); }
        
        if let Some(eqs) = pre_eq_filters.get(role) {
            plugins.push(output::create_eq_plugin(eqs));
        }
        
        // Crossover HP
        plugins.push(output::create_crossover_plugin("LR24", xover_freq, "high"));
        
        // Main Post Gain/Delay
        if main_gain_post.abs() > 0.01 { plugins.push(output::create_gain_plugin(main_gain_post)); }
        if main_delay_post.abs() > 0.001 { plugins.push(output::create_delay_plugin(main_delay_post)); }
        
        if let Some(eqs) = post_eq_filters.get(role) {
            plugins.push(output::create_eq_plugin(eqs));
        }
        
        ChannelDspChain {
            channel: role.to_string(),
            plugins,
            drivers: None,
            initial_curve: None,
            final_curve: None,
        }
    };

    channel_chains.insert("L".to_string(), build_main_chain("L"));
    channel_chains.insert("R".to_string(), build_main_chain("R"));

    // Sub Chain: AlignGain -> Crossover(LP) -> SubGain/Delay/Invert -> PostEQ
    // Note: Sub had no Pre-EQ in this workflow
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 { sub_plugins.push(output::create_gain_plugin(sub_align_gain)); }
    
    sub_plugins.push(output::create_crossover_plugin("LR24", xover_freq, "low"));
    
    if sub_inverted || sub_gain_post.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin_with_invert(sub_gain_post, sub_inverted));
    }
    if sub_delay_post.abs() > 0.001 { sub_plugins.push(output::create_delay_plugin(sub_delay_post)); }
    
    if let Some(eqs) = post_eq_filters.get(sub_role) {
        sub_plugins.push(output::create_eq_plugin(eqs));
    }
    
    let sub_chain = ChannelDspChain {
        channel: sub_role.to_string(),
        plugins: sub_plugins,
        drivers: None,
        initial_curve: None,
        final_curve: None,
    };
    channel_chains.insert(sub_role.to_string(), sub_chain);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results: HashMap::new(), // TODO: Populate
        combined_pre_score: 0.0,
        combined_post_score: 0.0,
        metadata: OptimizationMetadata {
            pre_score: 0.0,
            post_score: 0.0,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    })
}
