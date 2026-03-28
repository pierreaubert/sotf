//! CLI tool for computing CEA2034 preference scores from spinorama.org API data
//!
//! This tool fetches speaker measurement data from the spinorama.org API and computes
//! the CEA2034 preference score along with individual score components.
//!
//! The CEA2034 preference score is based on research into listener preferences for
//! loudspeakers and uses metrics including:
//! - NBD ON: Narrow Band Deviation of on-axis response
//! - NBD PIR: Narrow Band Deviation of predicted in-room response
//! - LFX: Low Frequency Extension
//! - SM PIR: Smoothness Metric of predicted in-room response
//!
//! Usage:
//!   cargo run --example cea2034_score --release -- --speaker "KEF R3"
//!   cargo run --example cea2034_score --release -- --speaker "KEF R3" --version "asr"

use autoeq::cea2034;
use autoeq::read;
use clap::Parser;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(
    name = "cea2034_score",
    about = "Compute CEA2034 preference score from spinorama.org API",
    long_about = "Fetches speaker measurement data from spinorama.org and computes the CEA2034 preference score along with individual metric components."
)]
struct Cli {
    /// Speaker name to fetch from spinorama.org
    #[arg(long)]
    speaker: String,

    /// Measurement version (e.g., "asr", "vendor", "Princeton", etc.)
    #[arg(long, default_value = "asr")]
    version: String,

    /// Measurement type to fetch
    #[arg(long, default_value = "CEA2034")]
    measurement: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    println!("Fetching data for speaker: {}", args.speaker);
    println!("Version: {}", args.version);
    println!("Measurement: {}", args.measurement);
    println!();

    // Fetch measurement data from the API
    let plot_data =
        match read::fetch_measurement_plot_data(&args.speaker, &args.version, &args.measurement)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Error: Failed to fetch measurement data");
                eprintln!("  Speaker: {}", args.speaker);
                eprintln!("  Version: {}", args.version);
                eprintln!("  Measurement: {}", args.measurement);
                eprintln!("  Details: {}", e);
                eprintln!();
                eprintln!("Please verify:");
                eprintln!("  - Speaker name is correct (check spinorama.org)");
                eprintln!("  - Version exists for this speaker");
                eprintln!("  - Measurement type is valid (typically CEA2034)");
                return Err(e);
            }
        };

    // Extract all CEA2034 curves using the original frequency grid from the API data
    // This matches the Python implementation and avoids interpolation artifacts
    let cea2034_data = match read::extract_cea2034_curves_original(&plot_data, &args.measurement) {
        Ok(curves) => curves,
        Err(e) => {
            eprintln!("Error: Failed to extract CEA2034 curves from measurement data");
            eprintln!("  Details: {}", e);
            eprintln!();
            eprintln!(
                "This usually means the measurement data is incomplete or in an unexpected format."
            );
            return Err(e);
        }
    };

    // Get the frequency grid from the extracted data
    let freq = &cea2034_data.get("On Axis").unwrap().freq;

    // Compute CEA2034 metrics (no PEQ applied)
    let metrics = match cea2034::compute_cea2034_metrics(freq, &cea2034_data, None).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Failed to compute CEA2034 metrics");
            eprintln!("  Details: {}", e);
            return Err(e);
        }
    };

    // Print results in a formatted manner
    println!("{}", "=".repeat(60));
    println!("CEA2034 Preference Score Report");
    println!("{}", "=".repeat(60));
    println!();
    println!("Speaker:    {}", args.speaker);
    println!("Version:    {}", args.version);
    println!("Origin:     {}", args.version);
    println!();
    println!("{}", "-".repeat(60));
    println!("Overall Preference Score: {:.2}", metrics.pref_score);
    println!("{}", "-".repeat(60));
    println!();
    println!("Score Components:");
    println!(
        "  NBD ON  (Narrow Band Deviation - On Axis):       {:.3}",
        metrics.nbd_on
    );
    println!(
        "  NBD PIR (Narrow Band Deviation - In-Room):       {:.3}",
        metrics.nbd_pir
    );
    println!(
        "  LFX     (Low Frequency Extension):               {:.0} Hz",
        10_f64.powf(metrics.lfx)
    );
    println!(
        "  SM PIR  (Smoothness Metric - In-Room):           {:.3}",
        metrics.sm_pir
    );
    println!();
    println!("{}", "=".repeat(60));
    println!();
    println!("Score interpretation:");
    println!("  Higher preference scores indicate better predicted listener preference");
    println!("  Typical range: 2.0 (poor) to 6.0 (excellent)");
    println!();

    Ok(())
}
