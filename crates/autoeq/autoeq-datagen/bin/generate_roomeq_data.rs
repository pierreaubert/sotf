//! CLI binary to generate room measurement data for RoomEQ testing
//!
//! Usage:
//!   cargo run --bin generate-roomeq-data --release -- --solver bem --output-dir data_tests/roomeq/generated

use anyhow::Result;
use autoeq_datagen::{
    bem_runner, csv_export, fem_runner, hf_extension, roomeq_config_gen, scenarios,
};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SolverChoice {
    Bem,
    Fem,
    Both,
}

#[derive(Parser, Debug)]
#[command(name = "generate-roomeq-data")]
#[command(about = "Generate room measurement data for RoomEQ testing using BEM/FEM solvers")]
struct Args {
    /// Which solver(s) to use
    #[arg(short, long, default_value = "bem")]
    solver: SolverChoice,

    /// Output directory for generated data
    #[arg(short, long, default_value = "data_tests/roomeq/generated")]
    output_dir: PathBuf,

    /// Run only a specific scenario (by name)
    #[arg(long)]
    scenario: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn run_solver(
    solver_name: &str,
    scenario: &scenarios::Scenario,
    output_base: &std::path::Path,
) -> Result<()> {
    let scenario_dir = output_base.join(solver_name).join(&scenario.name);
    std::fs::create_dir_all(&scenario_dir)?;

    log::info!(
        "[{}] Running {} for scenario: {} ({})",
        solver_name,
        solver_name,
        scenario.name,
        scenario.description
    );

    let sim_output = match solver_name {
        "bem" => bem_runner::run_bem(&scenario.simulation)?,
        "fem" => fem_runner::run_fem(&scenario.simulation)?,
        other => anyhow::bail!("Unknown solver: {other}"),
    };

    let output = hf_extension::extend_to_full_range_with_room(&sim_output, &scenario.simulation);

    // Export CSVs
    let csv_files = csv_export::export_csvs(&output, &scenario_dir)?;
    log::info!(
        "[{}] Exported {} CSV files to {}",
        solver_name,
        csv_files.len(),
        scenario_dir.display()
    );

    // Generate roomeq config
    let config = roomeq_config_gen::generate_config(scenario, &scenario_dir)?;
    let config_path = scenario_dir.join("config.json");
    roomeq_config_gen::write_config(&config, &config_path)?;
    log::info!(
        "[{}] Wrote config to {}",
        solver_name,
        config_path.display()
    );

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(if args.verbose { "debug" } else { "info" }),
    )
    .init();

    let all_scenarios = if let Some(ref name) = args.scenario {
        match scenarios::scenario_by_name(name) {
            Some(s) => vec![s],
            None => {
                let available: Vec<String> = scenarios::all_scenarios()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                anyhow::bail!(
                    "Unknown scenario '{}'. Available: {}",
                    name,
                    available.join(", ")
                );
            }
        }
    } else {
        scenarios::all_scenarios()
    };

    let solvers: Vec<&str> = match args.solver {
        SolverChoice::Bem => vec!["bem"],
        SolverChoice::Fem => vec!["fem"],
        SolverChoice::Both => vec!["bem", "fem"],
    };

    log::info!(
        "Generating data for {} scenarios with solver(s): {:?}",
        all_scenarios.len(),
        solvers
    );

    for solver_name in &solvers {
        for scenario in &all_scenarios {
            if let Err(e) = run_solver(solver_name, scenario, &args.output_dir) {
                log::error!(
                    "Failed solver={} scenario={}: {}",
                    solver_name,
                    scenario.name,
                    e
                );
            }
        }
    }

    log::info!(
        "Data generation complete. Output: {}",
        args.output_dir.display()
    );
    Ok(())
}
