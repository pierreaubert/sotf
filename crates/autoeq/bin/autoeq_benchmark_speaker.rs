//! AutoEQ Benchmark CLI: runs optimization scenarios across cached speakers and writes CSV results
//!
//! Scenarios per speaker:
//! 1) --loss speaker-flat --measurement CEA2034 --curve-name "Listening Window"
//! 2) --loss speaker-flat --measurement "Estimated In-Room Response" --curve-name "Estimated In-Room Response"
//! 3) --loss speaker-score --measurement CEA2034 --algo nlopt:isres
//! 4) --loss speaker-score --measurement CEA2034 --algo autoeq:de
//!
//! Input data is expected under data_cached/speakers/org.spinorama/{speaker}/{measurement}.json (Plotly JSON),
//! optionally data_cached/speakers/org.spinorama/{speaker}/metadata.json for metadata preference score.

use autoeq::cea2034 as score;
use autoeq::optim::ObjectiveData;
use autoeq::read;
use clap::Parser;
use ndarray::Array1;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::select;
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinSet;

use autoeq_env::{DATA_CACHED, DATA_GENERATED};

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    about = "Benchmark AutoEQ optimizations across cached speakers"
)]
pub struct BenchArgs {
    #[command(flatten)]
    pub base: autoeq::cli::Args,

    /// Limit to first 5 speakers for quick smoke run
    #[arg(long, default_value_t = false)]
    pub smoke_test: bool,

    /// Number of parallel jobs (0 = use all logical cores)
    #[arg(long, default_value_t = 0)]
    pub jobs: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = BenchArgs::parse();

    // Check if user wants to see algorithm list
    if args.base.algo_list {
        autoeq::cli::display_algorithm_list();
    }

    // Set up signal handling for graceful shutdown
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    // Spawn a dedicated signal handler task
    tokio::spawn(async move {
        let mut sigint_count = 0;
        loop {
            if let Err(e) = tokio::signal::ctrl_c().await {
                eprintln!("⚠️ Error setting up signal handler: {}", e);
                break;
            }

            sigint_count += 1;

            if sigint_count == 1 {
                eprintln!("\n🛑 Received interrupt signal (1/2). Stopping benchmark gracefully...");
                eprintln!("📝 Press Ctrl+C again within 5 seconds to force immediate termination.");
                shutdown_clone.store(true, Ordering::Relaxed);

                // Wait 5 seconds for graceful shutdown (longer than autoeq since benchmarks can take time)
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(5) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            } else {
                eprintln!("\n‼️ Received second interrupt signal. Forcing immediate termination!");
                std::process::exit(130); // Standard exit code for SIGINT
            }
        }
    });

    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args.base);

    // Enumerate speakers as subdirectories of ./data_cached/speakers/org.spinorama/
    let speakers_dir = PathBuf::from(DATA_CACHED)
        .join("speakers")
        .join("org.spinorama");
    let speakers = list_speakers(speakers_dir)?;
    let speakers: Vec<String> = if args.smoke_test {
        speakers.into_iter().take(5).collect()
    } else {
        speakers
    };
    if speakers.is_empty() {
        eprintln!("No speakers found under ./data. Exiting.");
        return Ok(());
    }

    // Determine parallelism
    let jobs = if args.jobs > 0 {
        args.jobs
    } else {
        num_cpus::get()
    };
    eprintln!("Running benchmark with {} parallel jobs", jobs);
    eprintln!("Press Ctrl+C to gracefully stop the benchmark and save partial results...");

    // Channel for rows; writer runs on main task
    let (tx, mut rx) = mpsc::channel::<(
        String,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    )>(jobs * 2);
    let sem = std::sync::Arc::new(Semaphore::new(jobs));
    let mut set = JoinSet::new();

    for speaker in speakers.clone() {
        let tx = tx.clone();
        let sem = sem.clone();
        let base_args = args.base.clone();
        let shutdown_clone = Arc::clone(&shutdown);
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");

            // Check for shutdown signal before starting work
            if shutdown_clone.load(Ordering::Relaxed) {
                let _ = tx.send((speaker, None, None, None, None, None)).await;
                return;
            }

            // For local cache usage, version value is irrelevant provided cache exists.
            let version = "latest".to_string();

            // Scenario 1
            let mut a1 = base_args.clone();
            a1.speaker = Some(speaker.clone());
            a1.version = Some(version.clone());
            a1.measurement = Some("CEA2034".to_string());
            a1.curve_name = "Listening Window".to_string();
            a1.loss = autoeq::LossType::SpeakerFlat;
            let s1 = if shutdown_clone.load(Ordering::Relaxed) {
                None
            } else {
                run_one(&a1, Arc::clone(&shutdown_clone))
                    .await
                    .ok()
                    .map(|m| m.pref_score)
            };

            // Scenario 2
            let mut a2 = base_args.clone();
            a2.speaker = Some(speaker.clone());
            a2.version = Some(version.clone());
            a2.measurement = Some("Estimated In-Room Response".to_string());
            a2.curve_name = "Estimated In-Room Response".to_string();
            a2.loss = autoeq::LossType::SpeakerFlat;
            let s2 = if shutdown_clone.load(Ordering::Relaxed) {
                None
            } else {
                run_one(&a2, Arc::clone(&shutdown_clone))
                    .await
                    .ok()
                    .map(|m| m.pref_score)
            };

            // Scenario 3: Score loss with nlopt:isres
            let mut a3 = base_args.clone();
            a3.speaker = Some(speaker.clone());
            a3.version = Some(version.clone());
            a3.measurement = Some("CEA2034".to_string());
            a3.loss = autoeq::LossType::SpeakerScore;
            a3.algo = "mh:rga".to_string();
            let s3 = if shutdown_clone.load(Ordering::Relaxed) {
                None
            } else {
                run_one(&a3, Arc::clone(&shutdown_clone))
                    .await
                    .ok()
                    .map(|m| m.pref_score)
            };

            // Scenario 4: Score loss with autoeq:de
            let mut a4 = base_args.clone();
            a4.speaker = Some(speaker.clone());
            a4.version = Some(version.clone());
            a4.measurement = Some("CEA2034".to_string());
            a4.loss = autoeq::LossType::SpeakerScore;
            a4.algo = "autoeq:de".to_string();
            let s4 = if shutdown_clone.load(Ordering::Relaxed) {
                None
            } else {
                run_one(&a4, Arc::clone(&shutdown_clone))
                    .await
                    .ok()
                    .map(|m| m.pref_score)
            };

            // Metadata preference
            let meta_pref = read_metadata_pref_score(&speaker).ok().flatten();

            let _ = tx.send((speaker, s1, s2, s3, s4, meta_pref)).await;
        });
    }
    drop(tx); // close sender when tasks finish

    // CSV writer: header then rows as they arrive (unordered)
    let mut wtr =
        csv::Writer::from_path(std::path::Path::new(DATA_GENERATED).join("benchmark.csv"))?;
    wtr.write_record([
        "speaker",
        "flat_cea2034_lw",
        "flat_eir",
        "score_cea2034_isres",
        "score_cea2034_autoeq_de",
        "metadata_pref",
    ])?;

    // Collect deltas (scenario - metadata) for end-of-run statistics
    let mut deltas_s1: Vec<f64> = Vec::new();
    let mut deltas_s2: Vec<f64> = Vec::new();
    let mut deltas_s3: Vec<f64> = Vec::new();
    let mut deltas_s4: Vec<f64> = Vec::new();

    let mut completed_speakers = 0;
    let total_speakers = speakers.len();

    // Main result collection loop with signal handling
    loop {
        select! {
            result = rx.recv() => {
                match result {
                    Some((speaker, s1, s2, s3, s4, meta_pref)) => {
                        completed_speakers += 1;
                        eprintln!("Completed {}/{} speakers: {}", completed_speakers, total_speakers, speaker);

                        wtr.write_record([
                            speaker.as_str(),
                            fmt_opt_f64(s1).as_str(),
                            fmt_opt_f64(s2).as_str(),
                            fmt_opt_f64(s3).as_str(),
                            fmt_opt_f64(s4).as_str(),
                            fmt_opt_f64(meta_pref).as_str(),
                        ])?;

                        // Accumulate deltas vs metadata when both values are present and finite
                        if let (Some(v), Some(m)) = (s1, meta_pref)
                            && v.is_finite() && m.is_finite() {
                                deltas_s1.push(v - m);
                            }
                        if let (Some(v), Some(m)) = (s2, meta_pref)
                            && v.is_finite() && m.is_finite() {
                                deltas_s2.push(v - m);
                            }
                        if let (Some(v), Some(m)) = (s3, meta_pref)
                            && v.is_finite() && m.is_finite() {
                                deltas_s3.push(v - m);
                            }
                        if let (Some(v), Some(m)) = (s4, meta_pref)
                            && v.is_finite() && m.is_finite() {
                                deltas_s4.push(v - m);
                            }
                    }
                    None => {
                        // Channel closed, all tasks are done
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Check for shutdown signal periodically
                if shutdown.load(Ordering::Relaxed) {
                    eprintln!("\n🛑 Shutdown signal detected. Stopping benchmark gracefully...");

                    // Abort all pending tasks
                    set.abort_all();

                    eprintln!("⏹️  Aborted {} pending tasks. Saving partial results...",
                             total_speakers - completed_speakers);
                    break;
                }
            }
        }
    }
    wtr.flush()?;

    // Ensure all remaining tasks are cleaned up
    while let Some(_res) = set.join_next().await {
        // ignore task result; errors are reflected as empty row fields
    }

    if completed_speakers < total_speakers {
        eprintln!(
            "⚠️  Benchmark incomplete: {}/{} speakers processed due to early termination.",
            completed_speakers, total_speakers
        );
    } else {
        eprintln!(
            "✅ Benchmark completed successfully: {}/{} speakers processed.",
            completed_speakers, total_speakers
        );
    }

    // Print end-of-run statistics comparing scenarios to metadata
    eprintln!("\n=== Benchmark statistics (scenario - metadata) ===");
    print_stats("flat_cea2034_lw", &deltas_s1);
    print_stats("flat_eir", &deltas_s2);
    print_stats("score_isres", &deltas_s3);
    print_stats("score_de", &deltas_s4);

    Ok(())
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.6}", x),
        _ => String::from(""),
    }
}

/// Compute mean and sample standard deviation of a slice.
/// Returns (mean, std). For n == 0 returns None. For n == 1, std = 0.0.
fn mean_std(data: &[f64]) -> Option<(f64, f64)> {
    let n = data.len();
    if n == 0 {
        return None;
    }
    let mean = data.iter().sum::<f64>() / (n as f64);
    if n == 1 {
        return Some((mean, 0.0));
    }
    let var_num: f64 = data
        .iter()
        .map(|&x| {
            let dx = x - mean;
            dx * dx
        })
        .sum();
    let std = (var_num / ((n - 1) as f64)).sqrt();
    Some((mean, std))
}

fn print_stats(name: &str, data: &[f64]) {
    match mean_std(data) {
        Some((m, s)) => {
            eprintln!(
                "{:>20}: N={:>4}, mean={:+.4}, std={:.4}",
                name,
                data.len(),
                m,
                s
            );
        }
        None => {
            eprintln!("{:>20}: N=   0", name);
        }
    }
}

fn list_speakers<P: AsRef<Path>>(data_dir: P) -> Result<Vec<String>, Box<dyn Error>> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(data_dir) {
        Ok(e) => e,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(out);
            } else {
                return Err(e.into());
            }
        }
    };
    for ent in entries {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir()
            && let Some(name) = p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

async fn run_one(
    args: &autoeq::cli::Args,
    shutdown: Arc<AtomicBool>,
) -> Result<score::ScoreMetrics, String> {
    // Check for shutdown before starting
    if shutdown.load(Ordering::Relaxed) {
        return Err("Task cancelled due to shutdown".into());
    }

    let (input_curve, spin_data_raw) = load_input_curve(args).await.map_err(|e| e.to_string())?;

    // Check for shutdown after data loading
    if shutdown.load(Ordering::Relaxed) {
        return Err("Task cancelled during data loading".into());
    }

    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);
    let input_curve_normalized =
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &input_curve);
    let target_curve =
        build_target_curve(args, &standard_freq, &input_curve).map_err(|e| e.to_string())?;
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: &target_curve.spl - &input_curve_normalized.spl,
        phase: None,
    };
    let spin_data = spin_data_raw.map(|spin_data| {
        spin_data
            .into_iter()
            .map(|(name, curve)| {
                let interpolated = read::interpolate_log_space(&standard_freq, &curve);
                (name, interpolated)
            })
            .collect()
    });
    let (objective_data, use_cea) = setup_objective_data(
        args,
        &input_curve_normalized,
        &target_curve,
        &deviation_curve,
        &spin_data,
    )
    .map_err(|e| e.to_string())?;

    // Check for shutdown before optimization
    if shutdown.load(Ordering::Relaxed) {
        return Err("Task cancelled before optimization".into());
    }

    let x = perform_optimization(args, &objective_data, Arc::clone(&shutdown))
        .await
        .map_err(|e| e.to_string())?;

    if use_cea {
        let freq = &standard_freq;
        let peq_after = autoeq::x2peq::compute_peq_response_from_x(
            freq,
            &x,
            args.sample_rate,
            args.effective_peq_model(),
        );
        let metrics =
            score::compute_cea2034_metrics(freq, spin_data.as_ref().unwrap(), Some(&peq_after))
                .await
                .map_err(|e| e.to_string())?;
        Ok(metrics)
    } else {
        Err("CEA2034 data required to compute preference score".to_string())
    }
}

async fn load_input_curve(
    args: &autoeq::cli::Args,
) -> Result<(autoeq::Curve, Option<HashMap<String, autoeq::Curve>>), String> {
    autoeq::workflow::load_input_curve(args)
        .await
        .map_err(|e| e.to_string())
}

fn build_target_curve(
    args: &autoeq::cli::Args,
    standard_freq: &Array1<f64>,
    input_curve: &autoeq::Curve,
) -> Result<autoeq::Curve, autoeq::AutoeqError> {
    autoeq::workflow::build_target_curve(args, standard_freq, input_curve)
}

fn setup_objective_data(
    args: &autoeq::cli::Args,
    input_curve: &autoeq::Curve,
    target_curve: &autoeq::Curve,
    deviation_curve: &autoeq::Curve,
    spin_data: &Option<HashMap<String, autoeq::Curve>>,
) -> Result<(ObjectiveData, bool), autoeq::AutoeqError> {
    autoeq::workflow::setup_objective_data(
        args,
        input_curve,
        target_curve,
        deviation_curve,
        spin_data,
    )
}

async fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
    shutdown: Arc<AtomicBool>,
) -> Result<Vec<f64>, String> {
    if shutdown.load(Ordering::Relaxed) {
        return Err("Optimization cancelled by shutdown".to_string());
    }

    let args_clone = args.clone();
    let objective_data_clone = objective_data.clone();

    let mut optimization_task = tokio::task::spawn_blocking(move || {
        autoeq::workflow::perform_optimization(&args_clone, &objective_data_clone)
            .map_err(|e| e.to_string())
    });

    // Wait for optimization with periodic shutdown checks
    loop {
        select! {
            result = &mut optimization_task => {
                match result {
                    Ok(opt_result) => return opt_result.map_err(|e| format!("Optimization error: {}", e)),
                    Err(e) => return Err(format!("Optimization task failed: {}", e)),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if shutdown.load(Ordering::Relaxed) {
                    optimization_task.abort();
                    return Err("Optimization cancelled by shutdown".to_string());
                }
            }
        }
    }
}

fn read_metadata_pref_score(speaker: &str) -> Result<Option<f64>, Box<dyn Error>> {
    let p = read::data_dir_for(speaker).join("metadata.json");
    let content = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            } else {
                return Err(e.into());
            }
        }
    };
    let v: Value = serde_json::from_str(&content)?;
    Ok(extract_pref_from_metadata_value(&v))
}

fn extract_pref_from_metadata_value(v: &Value) -> Option<f64> {
    // Path: measurements[default_measurement][pref_rating_eq].pref_score
    let default_measurement = v.get("default_measurement").and_then(|x| x.as_str())?;
    let measurements = v.get("measurements")?;
    let m = measurements.get(default_measurement)?;
    let pref = m.get("pref_rating_eq")?;
    pref.get("pref_score").and_then(|x| x.as_f64())
}

#[cfg(test)]
mod tests {
    use super::extract_pref_from_metadata_value;
    use serde_json::json;

    #[test]
    fn metadata_pref_path_extracts() {
        let v = json!({
            "default_measurement": "CEA2034",
            "measurements": {
                "CEA2034": {
                    "pref_rating_eq": {"pref_score": 6.789},
                    "pref_rating": {"pref_score": 5.0}
                }
            }
        });
        let got = extract_pref_from_metadata_value(&v);
        assert!(got.is_some());
        assert!((got.unwrap() - 6.789).abs() < 1e-12);
    }

    #[test]
    fn mean_std_basic() {
        let d = vec![1.0, 2.0, 3.0, 4.0];
        let (m, s) = super::mean_std(&d).unwrap();
        assert!((m - 2.5).abs() < 1e-12, "mean got {}", m);
        let expected_std = (5.0_f64 / 3.0).sqrt(); // sample std
        assert!((s - expected_std).abs() < 1e-12, "std got {}", s);
    }
}
