//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use anyhow::{Context, Result, anyhow};
use autoeq::plot;
use autoeq_env::DATA_GENERATED;
use clap::Parser;
use log::{error, info, warn};
use std::path::PathBuf;

// Include split modules
mod load;
mod postscore;
mod prescore;
mod progress;
mod qa;
mod runopt;
mod save;
mod spacing;

#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod postscore_tests;
#[cfg(test)]
mod prescore_tests;
#[cfg(test)]
mod qa_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod spacing_tests;

/// A command-line tool to find optimal IIR filters to match a frequency curve.
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = autoeq::cli::Args::parse();

    // Check if user wants to see algorithm list
    if args.algo_list {
        autoeq::cli::display_algorithm_list();
    }

    // Check if user wants to see strategy list
    if args.strategy_list {
        autoeq::cli::display_strategy_list();
    }

    // Check if user wants to see PEQ model list
    if args.peq_model_list {
        autoeq::cli::display_peq_model_list();
    }

    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args);

    // Run the main logic
    if let Err(e) = run(args).await {
        error!("Application error: {:#}", e);
        std::process::exit(1);
    }

    Ok(())
}

async fn run(args: autoeq::cli::Args) -> Result<()> {
    // Check if this is multi-driver mode
    if args.loss == autoeq::LossType::DriversFlat {
        return run_multi_driver_optimization(&args).await;
    }

    // Load and prepare all input data
    let (standard_freq, input_curve, target_curve, deviation_curve, spin_data) =
        load::load_and_prepare(&args)
            .await
            .map_err(|e| anyhow!("{}", e))
            .context("Failed to load and prepare input data")?;

    // Objective data
    let (objective_data, use_cea) = autoeq::workflow::setup_objective_data(
        &args,
        &input_curve,
        &target_curve,
        &deviation_curve,
        &spin_data,
    )
    .map_err(|e| anyhow!("{}", e))
    .context("Failed to setup objective data")?;

    // Compute pre-optimization metrics
    let pre_metrics = prescore::compute_pre_optimization_metrics(
        &args,
        &objective_data,
        use_cea,
        &deviation_curve,
        &spin_data,
    )
    .await
    .map_err(|e| anyhow!("{}", e))
    .context("Failed to compute pre-optimization metrics")?;

    // Optimize
    info!("🚀 Starting optimization...");
    let opt_result = runopt::perform_optimization(&args, &objective_data)
        .map_err(|e| anyhow!("{}", e))
        .context("Optimization failed")?;

    // Compute post-optimization metrics
    let post_metrics = postscore::compute_post_optimization_metrics(
        &args,
        &objective_data,
        use_cea,
        &opt_result.params,
        &standard_freq,
        &target_curve,
        &input_curve,
        &spin_data,
        pre_metrics.cea2034_metrics,
        pre_metrics.headphone_loss,
    )
    .await
    .map_err(|e| anyhow!("{}", e))
    .context("Failed to compute post-optimization metrics")?;

    // Print pre and post optimization scores
    postscore::print_optimization_scores(&args, &post_metrics);

    // Extract scores for QA summary
    let (pre_score, post_score) = match objective_data.loss_type {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            (post_metrics.pre_headphone_loss, post_metrics.headphone_loss)
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => (
            post_metrics.pre_cea2034.as_ref().map(|m| m.pref_score),
            post_metrics.cea2034_metrics.as_ref().map(|m| m.pref_score),
        ),
        autoeq::LossType::DriversFlat | autoeq::LossType::MultiSubFlat => {
            // Unreachable: DriversFlat mode uses a separate code path
            unreachable!("DriversFlat mode should not reach this point");
        }
    };

    // Check spacing constraints
    let spacing_ok = spacing::check_spacing_constraints(&opt_result.params, &args);

    // Output QA summary if in QA mode
    if let Some(qa_threshold) = args.qa {
        let converge_str = if opt_result.converged {
            "true"
        } else {
            "false"
        };
        let spacing_str = if spacing_ok { "ok" } else { "ko" };

        // Use scores if available, otherwise use objective function values
        let (pre_str, post_str) = if let (Some(pre), Some(post)) = (pre_score, post_score) {
            (format!("{:.3}", pre), format!("{:.3}", post))
        } else {
            // Fall back to objective function values
            let pre_obj = opt_result.pre_objective.unwrap_or(f64::NAN);
            let post_obj = opt_result.post_objective.unwrap_or(f64::NAN);
            (format!("{:.6}", pre_obj), format!("{:.6}", post_obj))
        };

        // Always output the standard QA summary line for backward compatibility
        // This uses println! because it is the "result" output for scripts
        println!(
            "Converge: {} | Spacing: {} | Pre: {} | Post: {}",
            converge_str, spacing_str, pre_str, post_str
        );

        // Perform additional QA analysis if threshold was provided
        let qa_result = qa::perform_qa_analysis(
            opt_result.converged,
            spacing_ok,
            pre_score,
            post_score,
            qa_threshold,
        );
        qa::display_qa_analysis(&qa_result);

        return Ok(());
    }

    // Normal mode: plot and report
    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = PathBuf::from(DATA_GENERATED);
        path.push("autoeq");
        if let Some(speaker) = &args.speaker {
            // Use speaker name for default filename
            let safe_name = speaker.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            path.push(format!("autoeq_{}", safe_name));
        } else {
            path.push("autoeq_results");
        }
        path
    });

    info!("📊 Generating plots: {}", output_path.display());
    if let Err(e) = plot::plot_results(
        &args,
        &opt_result.params,
        &input_curve,
        &target_curve,
        &deviation_curve,
        &spin_data,
        &output_path,
    )
    .await
    {
        warn!("Failed to generate plots: {}", e);
    } else {
        info!("✅ Plots generated successfully");
    }

    // Save PEQ settings to APO format file
    save::save_peq_to_file(
        &args,
        &opt_result.params,
        &output_path,
        &objective_data.loss_type,
    )
    .await
    .map_err(|e| anyhow!("{}", e))
    .context("Failed to save PEQ file")?;

    Ok(())
}

/// Run multi-driver crossover optimization
async fn run_multi_driver_optimization(args: &autoeq::cli::Args) -> Result<()> {
    info!("🎵 Multi-driver crossover optimization mode");

    // Collect driver file paths
    let driver_paths: Vec<_> = [&args.driver1, &args.driver2, &args.driver3, &args.driver4]
        .iter()
        .filter_map(|p| p.as_ref())
        .cloned()
        .collect();

    if driver_paths.len() < 2 {
        return Err(anyhow!(
            "At least 2 driver files are required for multi-driver optimization"
        ));
    }

    // Load driver measurements
    let measurements = autoeq::workflow::load_driver_measurements_from_files(&driver_paths)
        .map_err(|e| anyhow!("{}", e))
        .context("Failed to load driver measurements")?;

    // Parse crossover type
    let crossover_type = match args.crossover_type.as_str() {
        "butterworth2" => autoeq::loss::CrossoverType::Butterworth2,
        "linkwitzriley2" => autoeq::loss::CrossoverType::LinkwitzRiley2,
        "linkwitzriley4" => autoeq::loss::CrossoverType::LinkwitzRiley4,
        other => {
            return Err(anyhow!(
                "Unknown crossover type '{}'. Valid types: butterworth2, linkwitzriley2, linkwitzriley4",
                other
            ));
        }
    };

    // Create DriversLossData
    let drivers_data = autoeq::loss::DriversLossData::new(measurements, crossover_type);

    info!(
        "✓ Initialized {} drivers with {:?} crossover",
        drivers_data.drivers.len(),
        drivers_data.crossover_type
    );

    info!("📊 Drivers sorted by frequency (lowest to highest):");
    for (i, driver) in drivers_data.drivers.iter().enumerate() {
        let (min_f, max_f) = driver.freq_range();
        info!(
            "   Driver {}: {:.0} Hz - {:.0} Hz (mean: {:.0} Hz)",
            i + 1,
            min_f,
            max_f,
            driver.mean_freq()
        );
    }

    info!("🎯 Optimization parameters:");
    info!(
        "   {} driver gains + {} crossover frequencies = {} parameters",
        drivers_data.drivers.len(),
        drivers_data.drivers.len() - 1,
        drivers_data.drivers.len() + (drivers_data.drivers.len() - 1)
    );
    info!(
        "   Gain bounds: [{:.1}, {:.1}] dB",
        -args.max_db, args.max_db
    );

    // Optimize using shared function
    info!("🚀 Starting optimization...");
    let result = autoeq::workflow::optimize_drivers_crossover(
        drivers_data.clone(),
        args.min_freq,
        args.max_freq,
        args.sample_rate,
        &args.algo,
        args.maxeval,
        args.min_db,
        args.max_db,
        None, // No fixed crossover frequencies - optimize them
    )
    .map_err(|e| anyhow!("{}", e))
    .context("Driver optimization failed")?;

    // Extract results
    let gains = &result.gains;
    let delays = &result.delays;
    let xover_freqs = &result.crossover_freqs;

    // Display results
    info!("✅ Optimization complete!");
    info!("📊 Results:");
    info!("Driver Gains:");
    for (i, gain) in gains.iter().enumerate() {
        info!("   Driver {}: {:+.2} dB", i + 1, gain);
    }
    info!("Driver Delays:");
    for (i, delay) in delays.iter().enumerate() {
        info!("   Driver {}: {:.2} ms", i + 1, delay);
    }
    info!("Crossover Frequencies:");
    for (i, freq) in xover_freqs.iter().enumerate() {
        info!("   Between Driver {} and {}: {:.0} Hz", i + 1, i + 2, freq);
    }
    info!("Crossover Type: {:?}", drivers_data.crossover_type);

    // Display pre and post objective values
    info!("Loss (RMS deviation from flat):");
    info!("   Before optimization: {:.6} dB", result.pre_objective);
    info!("   After optimization:  {:.6} dB", result.post_objective);
    info!(
        "   Improvement: {:.2}%",
        (result.pre_objective - result.post_objective) / result.pre_objective * 100.0
    );

    // Generate plot
    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = std::path::PathBuf::from(autoeq_env::DATA_GENERATED);
        path.push("autoeq");
        path.push("drivers_crossover_results");
        path
    });

    info!("📊 Generating plots: {}", output_path.display());
    if let Err(e) = autoeq::plot::plot_drivers_results(
        &drivers_data,
        gains,
        xover_freqs,
        Some(delays),
        args.sample_rate,
        &output_path,
    ) {
        warn!("Failed to generate plots: {}", e);
    } else {
        info!("✅ Plots generated successfully");
    }

    // QA mode output
    if let Some(_qa_threshold) = args.qa {
        let converge_str = if result.converged { "true" } else { "false" };

        println!(
            "Converge: {} | Pre: {:.6} | Post: {:.6}",
            converge_str, result.pre_objective, result.post_objective
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use autoeq::cli::Args;
    use clap::Parser;
    use ndarray::Array1;

    #[test]
    fn setup_bounds_hp_pk_mode_overrides_first_triplet() {
        use autoeq::cli::PeqModel;
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 2;
        args.min_freq = 30.0;
        args.max_freq = 2000.0;
        args.min_q = 0.3;
        args.max_q = 8.0;
        args.max_db = 12.0;
        args.peq_model = PeqModel::HpPk;

        let (lb, ub) = autoeq::workflow::setup_bounds(&args);
        assert_eq!(lb.len(), args.num_filters * 3);
        assert_eq!(ub.len(), args.num_filters * 3);

        // First triplet should be overridden for HP
        assert!((lb[0] - 20.0_f64.max(args.min_freq).log10()).abs() < 1e-12);
        assert!((ub[0] - 120.0_f64.min(args.min_freq + 20.0).log10()).abs() < 1e-12);
        assert!((lb[1] - 1.0).abs() < 1e-12);
        assert!((ub[1] - 1.5).abs() < 1e-12);
        assert!((lb[2] - 0.0).abs() < 1e-12);
        assert!((ub[2] - 0.0).abs() < 1e-12);

        // Second filter should follow the general pattern
        let gain_lower = -6.0 * args.max_db;
        let q_lower = args.min_q.max(1.0e-6);
        assert!((lb[3] - args.min_freq.log10()).abs() < 1e-12);
        assert!((lb[4] - q_lower).abs() < 1e-12);
        assert!((lb[5] - gain_lower).abs() < 1e-12);
        assert!((ub[3] - args.max_freq.log10()).abs() < 1e-12);
        assert!((ub[4] - args.max_q).abs() < 1e-12);
        assert!((ub[5] - args.max_db).abs() < 1e-12);
    }

    #[test]
    fn listening_window_target_profile() {
        use autoeq::cli::PeqModel;
        let mut args = Args::parse_from(["autoeq-test"]);
        // Ensure we hit the custom target branch and avoid clamping negatives
        args.curve_name = "Listening Window".to_string();
        args.peq_model = PeqModel::HpPk;

        let freqs = Array1::from_vec(vec![500.0_f64, 1000.0_f64, 20000.0_f64]);
        let spl = Array1::<f64>::zeros(freqs.len());
        let curve = autoeq::Curve {
            freq: freqs.clone(),
            spl,
            phase: None,
        };

        let target_curve = autoeq::workflow::build_target_curve(&args, &freqs, &curve)
            .expect("build_target_curve should succeed");
        // Since SPL is zero, target_curve.spl == base_target
        assert!((target_curve.spl[0] - 0.0).abs() < 1e-12);
        assert!((target_curve.spl[1] - 0.0).abs() < 1e-12);
        assert!((target_curve.spl[2] - (-0.5)).abs() < 1e-12);
    }
}
