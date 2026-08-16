//! Validate AAE external validation artifacts and emit a reproducible report.
//!
//! Example:
//!
//! ```text
//! cargo run -p sotf-plugin-aae --bin qa-aae-validation -- \
//!   --manifest validation-manifest.json --run validation-run.json \
//!   --report validation-report.json --fixture-root /licensed/corpus
//! ```

use sotf_plugin_aae::quality_validation::{
    AcceptanceThresholds, evaluate, load_manifest, load_run,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("qa-aae-validation: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut manifest_path = None;
    let mut run_path = None;
    let mut report_path = None;
    let mut fixture_root = None;
    let mut deterministic_evidence = true;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--manifest" => manifest_path = Some(PathBuf::from(next_arg(&mut args, &argument)?)),
            "--run" => run_path = Some(PathBuf::from(next_arg(&mut args, &argument)?)),
            "--report" => report_path = Some(PathBuf::from(next_arg(&mut args, &argument)?)),
            "--fixture-root" => fixture_root = Some(PathBuf::from(next_arg(&mut args, &argument)?)),
            "--no-deterministic-evidence" => deterministic_evidence = false,
            "--help" | "-h" => {
                println!("--manifest PATH [--run PATH] [--report PATH] [--fixture-root PATH]");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}; use --help")),
        }
    }
    let manifest_path = manifest_path.ok_or("--manifest is required")?;
    let manifest = load_manifest(&manifest_path)?;
    let run = run_path.as_deref().map(load_run).transpose()?;
    let report = evaluate(
        &manifest,
        run.as_ref(),
        AcceptanceThresholds::default(),
        deterministic_evidence,
        fixture_root.as_deref(),
    );
    let encoded = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("cannot encode report: {error}"))?;
    if let Some(path) = report_path {
        fs::write(&path, format!("{encoded}\n"))
            .map_err(|error| format!("cannot write report {}: {error}", path.display()))?;
    }
    println!("{encoded}");
    if report.accepted {
        Ok(())
    } else {
        Err("external validation did not pass acceptance thresholds".into())
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))
}
